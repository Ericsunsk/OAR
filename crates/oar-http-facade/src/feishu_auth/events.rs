use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, StatusCode};
use hyper::Response;
use serde_json::json;
use tokio::sync::{mpsc, Notify};
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;

use super::session::{
    auth_event_is_terminal, auth_event_json, auth_event_name, expire_session_if_needed,
    session_status_json,
};
use super::FeishuLoginRuntime;
use crate::response::{
    json_facade_response, service_unavailable, sse_facade_response, FacadeResponse, ResponseBody,
};

const FEISHU_LOGIN_SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

pub(crate) fn feishu_login_session_event(
    runtime: Option<&FeishuLoginRuntime>,
    session_id: &str,
) -> FacadeResponse {
    let Some(runtime) = runtime else {
        return service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        );
    };
    match feishu_login_event_snapshot(runtime, session_id) {
        Ok(snapshot) => sse_facade_response(snapshot.frame),
        Err(response) => response,
    }
}

pub(crate) fn feishu_login_session_event_stream_response(
    runtime: Option<Arc<FeishuLoginRuntime>>,
    session_id: String,
) -> Response<ResponseBody> {
    let Some(runtime) = runtime else {
        return service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        )
        .into_hyper_response();
    };
    let initial_snapshot = match feishu_login_event_snapshot(&runtime, &session_id) {
        Ok(snapshot) => snapshot,
        Err(response) => return response.into_hyper_response(),
    };

    let (sender, receiver) = mpsc::channel::<Result<Frame<Bytes>, Infallible>>(8);
    tokio::spawn(async move {
        if send_sse_frame(&sender, initial_snapshot.frame)
            .await
            .is_err()
            || initial_snapshot.is_terminal
        {
            return;
        }

        let notify = initial_snapshot.notify;
        let mut last_version = initial_snapshot.version;
        let mut keepalive = time::interval(FEISHU_LOGIN_SSE_KEEPALIVE_INTERVAL);
        keepalive.tick().await;

        loop {
            let notified = notify.notified();
            tokio::pin!(notified);

            match feishu_login_event_snapshot(&runtime, &session_id) {
                Ok(snapshot) if snapshot.version != last_version => {
                    last_version = snapshot.version;
                    let is_terminal = snapshot.is_terminal;
                    if send_sse_frame(&sender, snapshot.frame).await.is_err() || is_terminal {
                        break;
                    }
                    continue;
                }
                Ok(_) => {}
                Err(_) => break,
            }

            tokio::select! {
                _ = &mut notified => {
                    match feishu_login_event_snapshot(&runtime, &session_id) {
                        Ok(snapshot) => {
                            last_version = snapshot.version;
                            let is_terminal = snapshot.is_terminal;
                            if send_sse_frame(&sender, snapshot.frame).await.is_err() || is_terminal {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                _ = keepalive.tick() => {
                    match feishu_login_event_snapshot(&runtime, &session_id) {
                        Ok(snapshot) if snapshot.is_terminal => {
                            let _ = send_sse_frame(&sender, snapshot.frame).await;
                            break;
                        }
                        Ok(_) => {
                            if send_sse_frame(&sender, ": keepalive\n\n".to_string()).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    let body = StreamBody::new(ReceiverStream::new(receiver)).boxed();
    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn send_sse_frame(
    sender: &mpsc::Sender<Result<Frame<Bytes>, Infallible>>,
    frame: String,
) -> Result<(), mpsc::error::SendError<Result<Frame<Bytes>, Infallible>>> {
    sender.send(Ok(Frame::data(Bytes::from(frame)))).await
}

struct FeishuLoginEventSnapshot {
    frame: String,
    is_terminal: bool,
    version: u64,
    notify: Arc<Notify>,
}

fn feishu_login_event_snapshot(
    runtime: &FeishuLoginRuntime,
    session_id: &str,
) -> Result<FeishuLoginEventSnapshot, FacadeResponse> {
    let mut sessions = runtime.sessions.lock().expect("feishu login session mutex");
    let Some(session) = sessions.get_mut(session_id) else {
        return Err(json_facade_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": "feishu_auth_session_not_found",
                "safe_message": "Feishu login session was not found."
            }),
        ));
    };
    expire_session_if_needed(session);
    let status = session_status_json(session);
    let event = auth_event_name(&status);
    let is_terminal = auth_event_is_terminal(event);
    Ok(FeishuLoginEventSnapshot {
        frame: format!(
            "event: {event}\ndata: {}\n\n",
            auth_event_json(session_id, event, &status)
        ),
        is_terminal,
        version: session.event_version,
        notify: Arc::clone(&session.notify),
    })
}
