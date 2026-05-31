use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use hyper::body::Incoming;
use hyper::header::{ACCEPT, AUTHORIZATION};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

use crate::agent_routes;
use crate::config::OarHttpFacadeConfig;
use crate::feishu_auth::{
    auth_session_events_id, feishu_login_session_event_stream_response,
    is_auth_session_events_route,
};
use crate::response::{not_found, ResponseBody};
use crate::review_inbox_routes;
use crate::routing::{accepts_event_stream, dispatch_request_with_runtime, event_stream_required};
use crate::runtime::OarHttpFacadeRuntime;
use crate::tenant_maintenance_daemon::{
    spawn_tenant_maintenance_daemon, TenantMaintenanceDaemonHandle,
};
use crate::tenant_maintenance_daemon_failure::TenantMaintenanceDaemonFailureCode;

#[derive(Debug)]
pub enum OarHttpFacadeError {
    Bind(std::io::Error),
    Accept(std::io::Error),
    TenantMaintenanceStart,
    TenantMaintenanceStopped,
}

impl fmt::Display for OarHttpFacadeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind(_) => write!(f, "oar_http_facade_bind_failed"),
            Self::Accept(_) => write!(f, "oar_http_facade_accept_failed"),
            Self::TenantMaintenanceStart => write!(f, "oar_tenant_maintenance_daemon_start_failed"),
            Self::TenantMaintenanceStopped => {
                write!(f, "oar_tenant_maintenance_daemon_stopped")
            }
        }
    }
}

impl Error for OarHttpFacadeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bind(error) | Self::Accept(error) => Some(error),
            Self::TenantMaintenanceStart | Self::TenantMaintenanceStopped => None,
        }
    }
}

pub async fn run(config: OarHttpFacadeConfig) -> Result<(), OarHttpFacadeError> {
    run_with_runtime(config, OarHttpFacadeRuntime::disabled()).await
}

pub async fn run_with_runtime(
    config: OarHttpFacadeConfig,
    runtime: OarHttpFacadeRuntime,
) -> Result<(), OarHttpFacadeError> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(OarHttpFacadeError::Bind)?;
    info!(bind_addr = %config.bind_addr, "oar http facade listening");
    let tenant_maintenance_daemon = spawn_tenant_maintenance_daemon(&runtime).map_err(|error| {
        error!(
            safe_error = %error,
            "tenant maintenance daemon failed to start"
        );
        OarHttpFacadeError::TenantMaintenanceStart
    })?;
    let runtime = Arc::new(runtime);

    accept_loop(listener, runtime, tenant_maintenance_daemon).await
}

async fn accept_loop(
    listener: TcpListener,
    runtime: Arc<OarHttpFacadeRuntime>,
    mut tenant_maintenance_daemon: Option<TenantMaintenanceDaemonHandle>,
) -> Result<(), OarHttpFacadeError> {
    loop {
        if let Some(daemon) = tenant_maintenance_daemon.as_mut() {
            tokio::select! {
                accepted = listener.accept() => {
                    let (stream, remote_addr) = match accepted {
                        Ok(accepted) => accepted,
                        Err(error) => {
                            if let Some(daemon) = tenant_maintenance_daemon.take() {
                                daemon.shutdown().await;
                            }
                            return Err(OarHttpFacadeError::Accept(error));
                        }
                    };
                    spawn_connection_task(stream, remote_addr, Arc::clone(&runtime));
                }
                result = daemon.wait_finished() => {
                    match result {
                        Ok(()) => {
                            runtime
                                .tenant_maintenance_daemon_status()
                                .mark_daemon_failed(
                                    TenantMaintenanceDaemonFailureCode::DaemonStoppedUnexpectedly,
                                );
                            error!("tenant maintenance daemon stopped unexpectedly");
                        }
                        Err(error) => {
                            runtime
                                .tenant_maintenance_daemon_status()
                                .mark_daemon_failed(
                                    TenantMaintenanceDaemonFailureCode::DaemonTaskFailed,
                                );
                            error!(
                                panic = error.is_panic(),
                                cancelled = error.is_cancelled(),
                                "tenant maintenance daemon task failed"
                            );
                        }
                    }
                    return Err(OarHttpFacadeError::TenantMaintenanceStopped);
                }
            }
        } else {
            let (stream, remote_addr) = listener
                .accept()
                .await
                .map_err(OarHttpFacadeError::Accept)?;
            spawn_connection_task(stream, remote_addr, Arc::clone(&runtime));
        }
    }
}

fn spawn_connection_task(
    stream: TcpStream,
    remote_addr: std::net::SocketAddr,
    runtime: Arc<OarHttpFacadeRuntime>,
) {
    tokio::spawn(async move {
        let io = TokioIo::new(stream);
        if let Err(error) = http1::Builder::new()
            .serve_connection(
                io,
                service_fn(move |request| {
                    handle_hyper_request_with_runtime(Arc::clone(&runtime), request)
                }),
            )
            .await
        {
            error!(?error, %remote_addr, "oar http facade connection failed");
        }
    });
}

pub async fn handle_hyper_request(
    request: Request<Incoming>,
) -> Result<Response<ResponseBody>, Infallible> {
    handle_hyper_request_with_runtime(Arc::new(OarHttpFacadeRuntime::disabled()), request).await
}

pub async fn handle_hyper_request_with_runtime(
    runtime: Arc<OarHttpFacadeRuntime>,
    request: Request<Incoming>,
) -> Result<Response<ResponseBody>, Infallible> {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(str::to_string);
    let authorization = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let accept = request
        .headers()
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if agent_routes::is_body_route(&method, &path) {
        return Ok(agent_routes::body_route_response(
            runtime,
            &method,
            &path,
            authorization.as_deref(),
            accept.as_deref(),
            request.into_body(),
        )
        .await);
    }
    if review_inbox_routes::is_body_route(&method, &path) {
        return Ok(review_inbox_routes::body_route_response(
            runtime,
            &method,
            &path,
            authorization.as_deref(),
            request.into_body(),
        )
        .await
        .into_hyper_response());
    }

    if is_auth_session_events_route(&method, &path) {
        if !accepts_event_stream(accept.as_deref()) {
            return Ok(event_stream_required(
                "Auth session events require Accept: text/event-stream.",
            )
            .into_hyper_response());
        }
        let Some(session_id) = auth_session_events_id(&path) else {
            return Ok(not_found().into_hyper_response());
        };
        return Ok(feishu_login_session_event_stream_response(
            runtime.feishu_login.clone(),
            session_id.to_string(),
        ));
    }

    let facade_response = dispatch_request_with_runtime(
        runtime,
        &method,
        &path,
        query.as_deref(),
        authorization.as_deref(),
        accept.as_deref(),
    )
    .await;
    Ok(facade_response.into_hyper_response())
}
