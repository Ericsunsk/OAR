use http_body_util::{BodyExt, LengthLimitError, Limited};
use hyper::body::{Body, Bytes};
use hyper::http::StatusCode;
use serde_json::json;

use crate::response::{json_facade_response, FacadeResponse};

pub(super) async fn collect_limited_body<B>(
    body: B,
    limit_bytes: usize,
    too_large_error: &'static str,
    too_large_safe_message: &'static str,
    unreadable_error: &'static str,
    unreadable_safe_message: &'static str,
) -> Result<Bytes, FacadeResponse>
where
    B: Body<Data = Bytes>,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    match Limited::new(body, limit_bytes).collect().await {
        Ok(collected) => Ok(collected.to_bytes()),
        Err(error) if error.downcast_ref::<LengthLimitError>().is_some() => {
            Err(json_facade_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                json!({
                    "error": too_large_error,
                    "safe_message": too_large_safe_message
                }),
            ))
        }
        Err(_) => Err(json_facade_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": unreadable_error,
                "safe_message": unreadable_safe_message
            }),
        )),
    }
}

#[cfg(test)]
mod tests {
    use http_body_util::Full;
    use hyper::body::Bytes;
    use hyper::http::StatusCode;
    use serde_json::Value;

    use super::collect_limited_body;

    #[tokio::test]
    async fn collect_limited_body_maps_stream_length_limit_to_configured_error() {
        assert_length_limit_error(
            "agent_request_body_too_large",
            "Agent request body is too large.",
            "agent_request_body_unreadable",
            "Agent request body could not be read.",
        )
        .await;
    }

    #[tokio::test]
    async fn collect_limited_body_maps_settings_length_limit_to_configured_error() {
        assert_length_limit_error(
            "agent_settings_body_too_large",
            "Agent settings request body is too large.",
            "agent_settings_body_unreadable",
            "Agent settings request body could not be read.",
        )
        .await;
    }

    async fn assert_length_limit_error(
        too_large_error: &'static str,
        too_large_safe_message: &'static str,
        unreadable_error: &'static str,
        unreadable_safe_message: &'static str,
    ) {
        let response = collect_limited_body(
            Full::new(Bytes::from_static(b"abcdef")),
            3,
            too_large_error,
            too_large_safe_message,
            unreadable_error,
            unreadable_safe_message,
        )
        .await
        .expect_err("too large response");
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["error"], too_large_error);
        assert_eq!(body["safe_message"], too_large_safe_message);
    }
}
