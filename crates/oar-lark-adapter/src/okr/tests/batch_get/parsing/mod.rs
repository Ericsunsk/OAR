use serde_json::Value;

use super::{sample_request, FakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{
    FeishuOkrBatchGetData, FeishuOkrBatchGetResponse, FeishuOkrReadClient, OkrReadSnapshot,
};

mod raw_response;
mod read_model;

fn parse_batch_get_body(body: Value) -> FeishuOkrBatchGetResponse {
    let response = HttpResponse::new(200, body.to_string());
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    client.batch_get_okrs(sample_request()).expect("success")
}

fn batch_get_data(body: Value) -> FeishuOkrBatchGetData {
    parse_batch_get_body(body).data.expect("data")
}

fn snapshot_from_batch_get_body(body: Value) -> OkrReadSnapshot {
    OkrReadSnapshot::from_batch_get_data(&batch_get_data(body))
}
