use hyper::http::StatusCode;
use serde_json::json;

use crate::response::{json_facade_response, FacadeResponse};

use super::super::dto::ReviewDecisionRequestDto;

pub(in crate::review_inbox_routes) fn decode_review_decision_request(
    body: &[u8],
) -> Result<ReviewDecisionRequestDto, FacadeResponse> {
    let request: ReviewDecisionRequestDto = serde_json::from_slice(body).map_err(|_| {
        json_facade_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "review_decision_invalid_json",
                "safe_message": "Review decision request body must be valid JSON."
            }),
        )
    })?;
    if request.action_id.trim().is_empty()
        || request.action_version == 0
        || request.note.chars().count() > 320
    {
        return Err(json_facade_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "review_decision_invalid_request",
                "safe_message": "Review decision request is invalid."
            }),
        ));
    }
    Ok(request)
}
