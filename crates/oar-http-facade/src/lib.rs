#![forbid(unsafe_code)]

mod agent;
mod agent_routes;
mod config;
mod feishu_auth;
mod health;
mod persistence;
mod response;
mod review_inbox_routes;
mod routing;
mod runtime;
mod server;
mod session_auth;
mod tenant_maintenance;
mod tenant_maintenance_daemon;
mod tenant_maintenance_daemon_failure;
mod tenant_maintenance_daemon_stage_status;
mod tenant_maintenance_daemon_status;
mod util;

pub(crate) use routing::{accepts_event_stream, event_stream_required};
pub(crate) use session_auth::{
    authenticate_oar_session, oar_session_auth_error_response,
    protected_route_requires_session_store, AuthenticatedContext,
};

pub use config::{OarHttpFacadeConfig, OarHttpFacadeConfigError};
pub use response::FacadeResponse;
pub use routing::{dispatch_request, dispatch_request_with_runtime};
pub use runtime::{OarHttpFacadeRuntime, OarHttpFacadeRuntimeError};
pub use server::{
    handle_hyper_request, handle_hyper_request_with_runtime, run, run_with_runtime,
    OarHttpFacadeError,
};

#[cfg(test)]
mod tests;
