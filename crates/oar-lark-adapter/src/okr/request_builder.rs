use serde_json::Value;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpRequest;
use crate::url_encoding::{encode_query, percent_encode};

use super::types::{
    FeishuOkrBatchGetRequest, FeishuOkrCycleListRequest, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrObjectiveKeyResultsListRequest, FeishuOkrProgressListRequest,
    FeishuOkrProgressListTarget,
};

const OKR_BATCH_GET_PATH: &str = "/open-apis/okr/v1/okrs/batch_get";
const OKR_CYCLES_PATH: &str = "/open-apis/okr/v2/cycles";
const OKR_OBJECTIVES_PATH: &str = "/open-apis/okr/v2/objectives";
const OKR_KEY_RESULTS_PATH: &str = "/open-apis/okr/v2/key_results";
const OAR_USER_AGENT: &str = concat!("oar-lark-adapter/", env!("CARGO_PKG_VERSION"));
pub(super) const DEFAULT_PROGRESS_PAGE_SIZE: u32 = 100;

pub fn build_batch_get_okr_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrBatchGetRequest,
) -> HttpRequest {
    let mut query = vec![("user_id_type", request.user_id_type.as_str().to_string())];
    for okr_id in request.okr_ids {
        query.push(("okr_ids", okr_id));
    }
    if let Some(lang) = request.lang {
        query.push(("lang", lang));
    }
    build_get_request(config, OKR_BATCH_GET_PATH, query, request.user_access_token)
}

pub fn build_list_cycles_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrCycleListRequest,
) -> HttpRequest {
    let mut query = vec![
        ("user_id_type", request.user_id_type.as_str().to_string()),
        ("user_id", request.user_id),
    ];
    query.extend(build_page_query(
        request.page_size,
        request.page_token,
        request.lang,
    ));
    build_get_request(config, OKR_CYCLES_PATH, query, request.user_access_token)
}

pub fn build_list_cycle_objectives_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrCycleObjectivesListRequest,
) -> HttpRequest {
    let path = format!(
        "{}/{}/objectives",
        OKR_CYCLES_PATH,
        percent_encode(&request.cycle_id)
    );
    let mut query = vec![("user_id_type", request.user_id_type.as_str().to_string())];
    query.extend(build_page_query(
        request.page_size,
        request.page_token,
        request.lang,
    ));
    build_get_request(config, &path, query, request.user_access_token)
}

pub fn build_list_objective_key_results_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrObjectiveKeyResultsListRequest,
) -> HttpRequest {
    let path = format!(
        "{}/{}/key_results",
        OKR_OBJECTIVES_PATH,
        percent_encode(&request.objective_id)
    );
    let mut query = vec![("user_id_type", request.user_id_type.as_str().to_string())];
    query.extend(build_page_query(
        request.page_size,
        request.page_token,
        request.lang,
    ));
    build_get_request(config, &path, query, request.user_access_token)
}

pub fn build_progress_list_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrProgressListRequest,
) -> HttpRequest {
    let path = match &request.target {
        FeishuOkrProgressListTarget::Objective(objective_id) => format!(
            "{}/{}/progresses",
            OKR_OBJECTIVES_PATH,
            percent_encode(objective_id)
        ),
        FeishuOkrProgressListTarget::KeyResult(key_result_id) => format!(
            "{}/{}/progresses",
            OKR_KEY_RESULTS_PATH,
            percent_encode(key_result_id)
        ),
    };
    let mut query = vec![
        ("user_id_type", request.user_id_type.as_str().to_string()),
        (
            "department_id_type",
            request.department_id_type.as_str().to_string(),
        ),
        (
            "page_size",
            request
                .page_size
                .unwrap_or(DEFAULT_PROGRESS_PAGE_SIZE)
                .to_string(),
        ),
    ];
    if let Some(page_token) = request.page_token {
        query.push(("page_token", page_token));
    }
    build_get_request(config, &path, query, request.user_access_token)
}

fn build_get_request(
    config: &FeishuOpenApiConfig,
    path: &str,
    query: Vec<(&str, String)>,
    user_access_token: crate::redaction::SecretString,
) -> HttpRequest {
    let query_string = encode_query(query);
    let url = if query_string.is_empty() {
        format!("{}{}", config.base_url.trim_end_matches('/'), path)
    } else {
        format!(
            "{}{}?{}",
            config.base_url.trim_end_matches('/'),
            path,
            query_string
        )
    };

    HttpRequest {
        method: "GET".to_string(),
        url,
        headers: vec![
            (
                "Authorization".to_string(),
                format!("Bearer {}", user_access_token.expose_secret()),
            ),
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), OAR_USER_AGENT.to_string()),
        ],
        body: Value::Object(serde_json::Map::new()),
        max_response_bytes: config.max_response_bytes,
    }
}

fn build_page_query(
    page_size: Option<u32>,
    page_token: Option<String>,
    lang: Option<String>,
) -> Vec<(&'static str, String)> {
    let mut query = Vec::new();
    if let Some(page_size) = page_size {
        query.push(("page_size", page_size.to_string()));
    }
    if let Some(page_token) = page_token {
        query.push(("page_token", page_token));
    }
    if let Some(lang) = lang {
        query.push(("lang", lang));
    }
    query
}
