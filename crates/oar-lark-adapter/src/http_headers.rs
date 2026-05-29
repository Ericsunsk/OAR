use crate::redaction::SecretString;

const OAR_USER_AGENT: &str = concat!("oar-lark-adapter/", env!("CARGO_PKG_VERSION"));

pub(crate) fn json_headers() -> Vec<(String, String)> {
    vec![content_type_json_header(), user_agent_header()]
}

pub(crate) fn json_accept_headers() -> Vec<(String, String)> {
    vec![
        content_type_json_header(),
        accept_json_header(),
        user_agent_header(),
    ]
}

pub(crate) fn bearer_accept_headers(user_access_token: &SecretString) -> Vec<(String, String)> {
    vec![
        bearer_secret_header(user_access_token),
        accept_json_header(),
        user_agent_header(),
    ]
}

pub(crate) fn bearer_json_headers(user_access_token: &SecretString) -> Vec<(String, String)> {
    vec![
        bearer_secret_header(user_access_token),
        accept_json_header(),
        content_type_json_header(),
        user_agent_header(),
    ]
}

pub(crate) fn bearer_json_headers_from_raw_token(access_token: &str) -> Vec<(String, String)> {
    vec![
        bearer_header(format!("Bearer {access_token}")),
        content_type_json_header(),
        user_agent_header(),
    ]
}

fn bearer_secret_header(user_access_token: &SecretString) -> (String, String) {
    bearer_header(format!("Bearer {}", user_access_token.expose_secret()))
}

fn bearer_header(value: String) -> (String, String) {
    ("Authorization".to_string(), value)
}

fn accept_json_header() -> (String, String) {
    ("Accept".to_string(), "application/json".to_string())
}

fn content_type_json_header() -> (String, String) {
    (
        "Content-Type".to_string(),
        "application/json; charset=utf-8".to_string(),
    )
}

fn user_agent_header() -> (String, String) {
    ("User-Agent".to_string(), OAR_USER_AGENT.to_string())
}
