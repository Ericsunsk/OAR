use std::sync::Arc;
use std::time::SystemTime;

use http_body_util::{BodyExt, LengthLimitError, Limited};
use hyper::body::{Body, Bytes};
use hyper::http::{Method, StatusCode};
use oar_core::storage::postgres::PostgresReviewInboxRepository;
use serde_json::json;

use crate::response::{json_facade_response, not_found, service_unavailable, FacadeResponse};
use crate::runtime::OarHttpFacadeRuntime;
use crate::{authenticate_oar_session, oar_session_auth_error_response, AuthenticatedContext};

mod decision;
mod dto;
mod projection;

use decision::{decode_review_decision_request, record_decision_for_context};
use projection::snapshot_response_body;

const DEFAULT_SNAPSHOT_LIMIT: u32 = 100;
const REVIEW_DECISIONS_PATH: &str = "/review-inbox/decisions";
const REVIEW_DECISION_BODY_LIMIT_BYTES: usize = 64 * 1024;

pub(crate) fn is_body_route(method: &Method, path: &str) -> bool {
    *method == Method::POST && path == REVIEW_DECISIONS_PATH
}

pub(crate) async fn body_route_response<B>(
    runtime: Arc<OarHttpFacadeRuntime>,
    method: &Method,
    path: &str,
    authorization: Option<&str>,
    body: B,
) -> FacadeResponse
where
    B: Body<Data = Bytes>,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    if !is_body_route(method, path) {
        return not_found();
    }

    let auth_context = match authenticate_oar_session(&runtime, authorization).await {
        Ok(context) => context,
        Err(error) => return oar_session_auth_error_response(error),
    };
    let body = match Limited::new(body, REVIEW_DECISION_BODY_LIMIT_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(error) if error.downcast_ref::<LengthLimitError>().is_some() => {
            return json_facade_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                json!({
                    "error": "review_decision_body_too_large",
                    "safe_message": "Review decision request body is too large."
                }),
            );
        }
        Err(_) => {
            return json_facade_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "review_decision_body_unreadable",
                    "safe_message": "Review decision request body could not be read."
                }),
            );
        }
    };
    let request = match decode_review_decision_request(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };

    record_decision_for_context(&runtime, &auth_context, request).await
}

pub(crate) async fn snapshot_for_context(
    runtime: &OarHttpFacadeRuntime,
    context: &AuthenticatedContext,
) -> FacadeResponse {
    let Some(persistence) = runtime.persistence() else {
        return service_unavailable(
            "review_inbox_snapshot_store_unavailable",
            "Review inbox snapshot storage is temporarily unavailable.",
        );
    };

    let repository = PostgresReviewInboxRepository::new(persistence.pool());
    match repository
        .load_review_inbox_snapshot(
            &context.tenant_id,
            &context.user_id,
            0,
            DEFAULT_SNAPSHOT_LIMIT,
        )
        .await
    {
        Ok(snapshot) => json_facade_response(
            StatusCode::OK,
            snapshot_response_body(&snapshot, SystemTime::now()),
        ),
        Err(_) => service_unavailable(
            "review_inbox_snapshot_unavailable",
            "Review inbox snapshot is temporarily unavailable.",
        ),
    }
}

#[cfg(test)]
mod tests;
