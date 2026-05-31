use super::error::FeishuDocReadError;
use super::feishu_types::{
    FeishuDocxMetadataResponse, FeishuDocxRawContentResponse, FeishuWikiNode,
    FeishuWikiNodeResponse,
};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DocxMetadata {
    pub(super) title: Option<String>,
    pub(super) revision_id: Option<String>,
}

pub(super) fn map_status_or_parse_docx_metadata(
    status: u16,
    body: &str,
) -> Result<DocxMetadata, FeishuDocReadError> {
    map_status_or_parse_docs_response(status, body, |body| {
        let parsed: FeishuDocxMetadataResponse =
            serde_json::from_str(body).map_err(|_| FeishuDocReadError::InvalidJson)?;
        if parsed.code != 0 {
            return Err(map_api_code(parsed.code));
        }
        let document = parsed
            .data
            .and_then(|data| data.document)
            .ok_or(FeishuDocReadError::InvalidJson)?;
        Ok(DocxMetadata {
            title: document.title,
            revision_id: document.revision_id.map(|value| value.to_string()),
        })
    })
}

pub(super) fn map_status_or_parse_docx_raw_content(
    status: u16,
    body: &str,
) -> Result<String, FeishuDocReadError> {
    map_status_or_parse_docs_response(status, body, |body| {
        let parsed: FeishuDocxRawContentResponse =
            serde_json::from_str(body).map_err(|_| FeishuDocReadError::InvalidJson)?;
        if parsed.code != 0 {
            return Err(map_api_code(parsed.code));
        }
        parsed
            .data
            .and_then(|data| data.content)
            .ok_or(FeishuDocReadError::InvalidJson)
    })
}

pub(super) fn map_status_or_parse_wiki_node(
    status: u16,
    body: &str,
) -> Result<FeishuWikiNode, FeishuDocReadError> {
    map_status_or_parse_docs_response(status, body, |body| {
        let parsed: FeishuWikiNodeResponse =
            serde_json::from_str(body).map_err(|_| FeishuDocReadError::InvalidJson)?;
        if parsed.code != 0 {
            return Err(map_api_code(parsed.code));
        }
        parsed
            .data
            .and_then(|data| data.node)
            .ok_or(FeishuDocReadError::InvalidJson)
    })
}

fn map_status_or_parse_docs_response<T>(
    status: u16,
    body: &str,
    parse_success: impl FnOnce(&str) -> Result<T, FeishuDocReadError>,
) -> Result<T, FeishuDocReadError> {
    match status {
        200..=299 => parse_success(body),
        401 => Err(FeishuDocReadError::Unauthorized),
        403 => Err(FeishuDocReadError::Forbidden),
        404 => Err(FeishuDocReadError::NotFound),
        429 => Err(FeishuDocReadError::UpstreamTransient),
        400..=499 => Err(FeishuDocReadError::UpstreamClient),
        _ => Err(FeishuDocReadError::UpstreamTransient),
    }
}

fn map_api_code(code: i64) -> FeishuDocReadError {
    match code {
        401 | 99991663 | 99991664 => FeishuDocReadError::Unauthorized,
        403 | 1770032 | 131006 => FeishuDocReadError::Forbidden,
        404 | 1770002 | 1770003 | 131005 => FeishuDocReadError::NotFound,
        1770033 => FeishuDocReadError::OversizedResponse,
        99991400 => FeishuDocReadError::UpstreamTransient,
        1770001 | 131002 => FeishuDocReadError::UpstreamClient,
        131001 | 131007 => FeishuDocReadError::UpstreamTransient,
        _ => FeishuDocReadError::ApiFailure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_docx_permission_and_not_found_codes() {
        assert_eq!(
            map_status_or_parse_docx_metadata(200, r#"{"code":1770032,"msg":"forbidden"}"#)
                .expect_err("forbidden"),
            FeishuDocReadError::Forbidden
        );
        assert_eq!(
            map_status_or_parse_docx_raw_content(200, r#"{"code":1770002,"msg":"not found"}"#)
                .expect_err("not found"),
            FeishuDocReadError::NotFound
        );
    }

    #[test]
    fn parses_docx_metadata_and_raw_content() {
        let metadata = map_status_or_parse_docx_metadata(
            200,
            r#"{"code":0,"data":{"document":{"revision_id":7,"title":"Project Plan"}}}"#,
        )
        .expect("metadata");
        assert_eq!(metadata.title.as_deref(), Some("Project Plan"));
        assert_eq!(metadata.revision_id.as_deref(), Some("7"));

        let content = map_status_or_parse_docx_raw_content(
            200,
            r#"{"code":0,"data":{"content":"hello\nworld"}}"#,
        )
        .expect("content");
        assert_eq!(content, "hello\nworld");
    }

    #[test]
    fn parses_wiki_node() {
        let node = map_status_or_parse_wiki_node(
            200,
            r#"{"code":0,"data":{"node":{"obj_token":"doxcni6mOy7jLRWbEylaKKabcef","obj_type":"docx","title":"Wiki Plan"}}}"#,
        )
        .expect("node");
        assert_eq!(node.obj_type.as_deref(), Some("docx"));
        assert_eq!(node.title.as_deref(), Some("Wiki Plan"));
    }
}
