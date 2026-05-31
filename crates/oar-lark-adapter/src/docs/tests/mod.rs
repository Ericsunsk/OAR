use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::docs::{
    build_docx_metadata_request, build_docx_raw_content_request, build_wiki_node_request,
    AsyncFeishuDocRead, DocReadSummary, FeishuDocReadClient, FeishuDocReadError,
    FeishuDocReadRequest,
};
use crate::oauth::HttpResponse;
use crate::redaction::SecretString;
use crate::test_support::http::FakeHttpClient;

#[test]
fn request_builders_redact_tokens_and_use_expected_paths() {
    let access_token = SecretString::new("u-very-secret-doc-token");
    let metadata = build_docx_metadata_request(
        &FeishuOpenApiConfig::default(),
        &access_token,
        "doxcni6mOy7jLRWbEylaKKabcef",
    )
    .expect("metadata request");
    assert_eq!(metadata.method, "GET");
    assert!(metadata
        .url
        .ends_with("/open-apis/docx/v1/documents/doxcni6mOy7jLRWbEylaKKabcef"));

    let raw = build_docx_raw_content_request(
        &FeishuOpenApiConfig::default(),
        &access_token,
        "doxcni6mOy7jLRWbEylaKKabcef",
    )
    .expect("raw request");
    assert!(raw
        .url
        .ends_with("/open-apis/docx/v1/documents/doxcni6mOy7jLRWbEylaKKabcef/raw_content?lang=0"));
    assert_eq!(raw.max_response_bytes, 24 * 1024);

    let wiki = build_wiki_node_request(
        &FeishuOpenApiConfig::default(),
        &access_token,
        "wikcnKQ1k3p8Vabcef",
    )
    .expect("wiki request");
    assert!(wiki.url.contains("/open-apis/wiki/v2/spaces/get_node?"));
    assert!(wiki.url.contains("token=wikcnKQ1k3p8Vabcef"));
    assert!(wiki.url.contains("obj_type=wiki"));

    for request in [metadata, raw, wiki] {
        assert!(request.headers.iter().any(|(name, value)| {
            name == "Authorization" && value == "Bearer u-very-secret-doc-token"
        }));
        let debug = format!("{request:?}");
        assert!(!debug.contains("u-very-secret-doc-token"));
        assert!(!debug.contains("doxcni6mOy7jLRWbEylaKKabcef"));
        assert!(!debug.contains("wikcnKQ1k3p8Vabcef"));
        assert!(debug.contains("[REDACTED]"));
    }
}

#[test]
fn raw_content_request_preserves_lower_runtime_response_cap() {
    let access_token = SecretString::new("u-token");
    let config = FeishuOpenApiConfig {
        max_response_bytes: 4096,
        ..FeishuOpenApiConfig::default()
    };

    let raw = build_docx_raw_content_request(&config, &access_token, "doxcni6mOy7jLRWbEylaKKabcef")
        .expect("raw request");

    assert_eq!(raw.max_response_bytes, 4096);
}

#[test]
fn doc_request_debug_redacts_source_ref() {
    let request = FeishuDocReadRequest {
        user_access_token: SecretString::new("u-very-secret-doc-token"),
        source_ref: "docx://doxcni6mOy7jLRWbEylaKKabcef".to_string(),
    };

    let debug = format!("{request:?}");
    assert!(!debug.contains("u-very-secret-doc-token"));
    assert!(!debug.contains("doxcni6mOy7jLRWbEylaKKabcef"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn sync_client_reads_docx_metadata_and_plain_text() {
    let mut client = FeishuDocReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_responses(vec![
            HttpResponse::new(
                200,
                json!({"code":0,"data":{"document":{"revision_id":3,"title":"Launch Notes"}}})
                    .to_string(),
            ),
            HttpResponse::new(
                200,
                json!({"code":0,"data":{"content":"Launch notes\nwith details"}}).to_string(),
            ),
        ]),
    );

    let summary = client
        .get_doc_summary(FeishuDocReadRequest {
            user_access_token: SecretString::new("u-token"),
            source_ref: "docx://doxcni6mOy7jLRWbEylaKKabcef".to_string(),
        })
        .expect("summary");

    assert_eq!(
        summary,
        DocReadSummary {
            title: Some("Launch Notes".to_string()),
            doc_type: "docx".to_string(),
            revision_id: Some("3".to_string()),
            content_preview: "Launch notes\nwith details".to_string(),
            content_truncated: false,
            content_char_count: 25,
        }
    );
}

#[test]
fn client_resolves_wiki_node_to_docx() {
    let mut client = FeishuDocReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_responses(vec![
            HttpResponse::new(
                200,
                json!({"code":0,"data":{"node":{"obj_token":"doxcni6mOy7jLRWbEylaKKabcef","obj_type":"docx","title":"Wiki Fallback"}}})
                    .to_string(),
            ),
            HttpResponse::new(
                200,
                json!({"code":0,"data":{"document":{"revision_id":9,"title":""}}}).to_string(),
            ),
            HttpResponse::new(200, json!({"code":0,"data":{"content":"Wiki content"}}).to_string()),
        ]),
    );

    let summary = client
        .get_doc_summary(FeishuDocReadRequest {
            user_access_token: SecretString::new("u-token"),
            source_ref: "wiki://wikcnKQ1k3p8Vabcef".to_string(),
        })
        .expect("summary");

    assert_eq!(summary.title.as_deref(), Some("Wiki Fallback"));
    assert_eq!(summary.revision_id.as_deref(), Some("9"));
    assert_eq!(summary.content_preview, "Wiki content");

    let requests = &client.http_client().requests;
    assert_eq!(requests.len(), 3);
    assert!(requests[0]
        .url
        .contains("/open-apis/wiki/v2/spaces/get_node?"));
    assert!(requests[0].url.contains("token=wikcnKQ1k3p8Vabcef"));
    assert!(requests[1]
        .url
        .ends_with("/open-apis/docx/v1/documents/doxcni6mOy7jLRWbEylaKKabcef"));
    assert!(requests[2]
        .url
        .ends_with("/open-apis/docx/v1/documents/doxcni6mOy7jLRWbEylaKKabcef/raw_content?lang=0"));
}

#[test]
fn wiki_node_rejects_non_docx_payload() {
    let mut client = FeishuDocReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"node":{"obj_token":"shtcnToken","obj_type":"sheet","title":"Sheet"}}})
                .to_string(),
        )),
    );

    let error = client
        .get_doc_summary(FeishuDocReadRequest {
            user_access_token: SecretString::new("u-token"),
            source_ref: "wiki://wikcnKQ1k3p8Vabcef".to_string(),
        })
        .expect_err("non-docx");

    assert_eq!(error, FeishuDocReadError::UnsupportedDocumentType);
}

#[test]
fn async_client_reads_docx_metadata_and_plain_text() {
    let mut client = FeishuDocReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_responses(vec![
            HttpResponse::new(
                200,
                json!({"code":0,"data":{"document":{"revision_id":4,"title":"Async Launch"}}})
                    .to_string(),
            ),
            HttpResponse::new(
                200,
                json!({"code":0,"data":{"content":"Async launch notes"}}).to_string(),
            ),
        ]),
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let summary = runtime
        .block_on(AsyncFeishuDocRead::get_doc_summary(
            &mut client,
            FeishuDocReadRequest {
                user_access_token: SecretString::new("u-token"),
                source_ref: "docx://doxcni6mOy7jLRWbEylaKKabcef".to_string(),
            },
        ))
        .expect("summary");

    assert_eq!(summary.title.as_deref(), Some("Async Launch"));
    assert_eq!(summary.revision_id.as_deref(), Some("4"));
    assert_eq!(summary.content_preview, "Async launch notes");
    assert_eq!(client.http_client().requests.len(), 2);
}
