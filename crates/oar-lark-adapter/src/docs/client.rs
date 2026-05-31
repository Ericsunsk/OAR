use async_trait::async_trait;
use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::http_headers::bearer_accept_headers;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpRequest};
use crate::redaction::SecretString;
use crate::url_encoding::{encode_query, percent_encode};

use super::error::FeishuDocReadError;
use super::feishu_types::FeishuWikiNode;
use super::response_parser::{
    map_status_or_parse_docx_metadata, map_status_or_parse_docx_raw_content,
    map_status_or_parse_wiki_node, DocxMetadata,
};
use super::source_ref::{parse_doc_source_ref, valid_doc_token, DocSourceRef, DocSourceRefKind};
use super::types::{non_empty, DocReadSummary, FeishuDocReadRequest};

const DOCX_METADATA_PATH_PREFIX: &str = "/open-apis/docx/v1/documents";
const DOCX_RAW_CONTENT_SUFFIX: &str = "raw_content";
const WIKI_GET_NODE_PATH: &str = "/open-apis/wiki/v2/spaces/get_node";
const DOC_PREVIEW_CHAR_LIMIT: usize = 96;
const DOC_RAW_CONTENT_MAX_RESPONSE_BYTES: usize = 24 * 1024;

#[derive(Debug, Clone)]
pub struct FeishuDocReadClient<H> {
    config: FeishuOpenApiConfig,
    http_client: H,
}

impl<H> FeishuDocReadClient<H> {
    pub fn new(config: FeishuOpenApiConfig, http_client: H) -> Self {
        Self {
            config,
            http_client,
        }
    }

    pub fn http_client(&self) -> &H {
        &self.http_client
    }
}

impl<H> FeishuDocReadClient<H>
where
    H: HttpClient,
{
    pub fn get_doc_summary(
        &mut self,
        request: FeishuDocReadRequest,
    ) -> Result<DocReadSummary, FeishuDocReadError> {
        let source_ref = parse_doc_source_ref(&request.source_ref)?;
        self.get_doc_summary_for_ref(&request.user_access_token, source_ref)
    }

    fn get_doc_summary_for_ref(
        &mut self,
        access_token: &SecretString,
        source_ref: DocSourceRef,
    ) -> Result<DocReadSummary, FeishuDocReadError> {
        match source_ref.kind {
            DocSourceRefKind::Docx { document_id } => {
                self.get_docx_summary(access_token, &document_id, None)
            }
            DocSourceRefKind::Wiki { node_token } => {
                let target =
                    docx_target_from_wiki_node(self.read_wiki_node(access_token, &node_token)?)?;
                self.get_docx_summary(access_token, &target.document_id, target.fallback_title)
            }
        }
    }

    fn get_docx_summary(
        &mut self,
        access_token: &SecretString,
        document_id: &str,
        fallback_title: Option<String>,
    ) -> Result<DocReadSummary, FeishuDocReadError> {
        let metadata_raw = self
            .http_client
            .send_json(build_docx_metadata_request(
                &self.config,
                access_token,
                document_id,
            )?)
            .map_err(FeishuDocReadError::from)?;
        let metadata = map_status_or_parse_docx_metadata(metadata_raw.status, &metadata_raw.body)?;
        let content_raw = self
            .http_client
            .send_json(build_docx_raw_content_request(
                &self.config,
                access_token,
                document_id,
            )?)
            .map_err(FeishuDocReadError::from)?;
        let content = map_status_or_parse_docx_raw_content(content_raw.status, &content_raw.body)?;

        Ok(docx_summary(metadata, content, fallback_title))
    }

    fn read_wiki_node(
        &mut self,
        access_token: &SecretString,
        node_token: &str,
    ) -> Result<super::feishu_types::FeishuWikiNode, FeishuDocReadError> {
        let raw = self
            .http_client
            .send_json(build_wiki_node_request(
                &self.config,
                access_token,
                node_token,
            )?)
            .map_err(FeishuDocReadError::from)?;
        map_status_or_parse_wiki_node(raw.status, &raw.body)
    }
}

#[async_trait]
pub trait AsyncFeishuDocRead {
    async fn get_doc_summary(
        &mut self,
        request: FeishuDocReadRequest,
    ) -> Result<DocReadSummary, FeishuDocReadError>;
}

#[async_trait]
impl<H> AsyncFeishuDocRead for FeishuDocReadClient<H>
where
    H: AsyncHttpClient + Send,
{
    async fn get_doc_summary(
        &mut self,
        request: FeishuDocReadRequest,
    ) -> Result<DocReadSummary, FeishuDocReadError> {
        let source_ref = parse_doc_source_ref(&request.source_ref)?;
        match source_ref.kind {
            DocSourceRefKind::Docx { document_id } => {
                self.get_docx_summary_async(&request.user_access_token, &document_id, None)
                    .await
            }
            DocSourceRefKind::Wiki { node_token } => {
                let target = docx_target_from_wiki_node(
                    self.read_wiki_node_async(&request.user_access_token, &node_token)
                        .await?,
                )?;
                self.get_docx_summary_async(
                    &request.user_access_token,
                    &target.document_id,
                    target.fallback_title,
                )
                .await
            }
        }
    }
}

impl<H> FeishuDocReadClient<H>
where
    H: AsyncHttpClient + Send,
{
    async fn get_docx_summary_async(
        &mut self,
        access_token: &SecretString,
        document_id: &str,
        fallback_title: Option<String>,
    ) -> Result<DocReadSummary, FeishuDocReadError> {
        let metadata_raw = self
            .http_client
            .send_json(build_docx_metadata_request(
                &self.config,
                access_token,
                document_id,
            )?)
            .await
            .map_err(FeishuDocReadError::from)?;
        let metadata = map_status_or_parse_docx_metadata(metadata_raw.status, &metadata_raw.body)?;
        let content_raw = self
            .http_client
            .send_json(build_docx_raw_content_request(
                &self.config,
                access_token,
                document_id,
            )?)
            .await
            .map_err(FeishuDocReadError::from)?;
        let content = map_status_or_parse_docx_raw_content(content_raw.status, &content_raw.body)?;

        Ok(docx_summary(metadata, content, fallback_title))
    }

    async fn read_wiki_node_async(
        &mut self,
        access_token: &SecretString,
        node_token: &str,
    ) -> Result<super::feishu_types::FeishuWikiNode, FeishuDocReadError> {
        let raw = self
            .http_client
            .send_json(build_wiki_node_request(
                &self.config,
                access_token,
                node_token,
            )?)
            .await
            .map_err(FeishuDocReadError::from)?;
        map_status_or_parse_wiki_node(raw.status, &raw.body)
    }
}

pub fn build_docx_metadata_request(
    config: &FeishuOpenApiConfig,
    user_access_token: &SecretString,
    document_id: &str,
) -> Result<HttpRequest, FeishuDocReadError> {
    validate_token(document_id)?;
    Ok(HttpRequest {
        method: "GET".to_string(),
        url: format!(
            "{}/{}/{}",
            config.base_url.trim_end_matches('/'),
            DOCX_METADATA_PATH_PREFIX.trim_start_matches('/'),
            percent_encode(document_id.trim())
        ),
        headers: bearer_accept_headers(user_access_token),
        body: json!({}),
        max_response_bytes: config.max_response_bytes,
    })
}

pub fn build_docx_raw_content_request(
    config: &FeishuOpenApiConfig,
    user_access_token: &SecretString,
    document_id: &str,
) -> Result<HttpRequest, FeishuDocReadError> {
    validate_token(document_id)?;
    Ok(HttpRequest {
        method: "GET".to_string(),
        url: format!(
            "{}/{}/{}/{}?{}",
            config.base_url.trim_end_matches('/'),
            DOCX_METADATA_PATH_PREFIX.trim_start_matches('/'),
            percent_encode(document_id.trim()),
            DOCX_RAW_CONTENT_SUFFIX,
            encode_query([("lang", "0")])
        ),
        headers: bearer_accept_headers(user_access_token),
        body: json!({}),
        max_response_bytes: config
            .max_response_bytes
            .min(DOC_RAW_CONTENT_MAX_RESPONSE_BYTES),
    })
}

pub fn build_wiki_node_request(
    config: &FeishuOpenApiConfig,
    user_access_token: &SecretString,
    node_token: &str,
) -> Result<HttpRequest, FeishuDocReadError> {
    validate_token(node_token)?;
    Ok(HttpRequest {
        method: "GET".to_string(),
        url: format!(
            "{}/{}?{}",
            config.base_url.trim_end_matches('/'),
            WIKI_GET_NODE_PATH.trim_start_matches('/'),
            encode_query([("token", node_token.trim()), ("obj_type", "wiki")])
        ),
        headers: bearer_accept_headers(user_access_token),
        body: json!({}),
        max_response_bytes: config.max_response_bytes,
    })
}

fn validate_token(token: &str) -> Result<(), FeishuDocReadError> {
    if valid_doc_token(token.trim()) {
        Ok(())
    } else {
        Err(FeishuDocReadError::InvalidRequest)
    }
}

struct DocxTarget {
    document_id: String,
    fallback_title: Option<String>,
}

fn docx_target_from_wiki_node(node: FeishuWikiNode) -> Result<DocxTarget, FeishuDocReadError> {
    let obj_type = node.obj_type.as_deref().unwrap_or_default();
    if obj_type != "docx" {
        return Err(FeishuDocReadError::UnsupportedDocumentType);
    }
    let document_id = node.obj_token.ok_or(FeishuDocReadError::InvalidJson)?;
    if !valid_doc_token(&document_id) {
        return Err(FeishuDocReadError::InvalidJson);
    }
    Ok(DocxTarget {
        document_id,
        fallback_title: node.title,
    })
}

fn docx_summary(
    metadata: DocxMetadata,
    content: String,
    fallback_title: Option<String>,
) -> DocReadSummary {
    DocReadSummary::docx(
        non_empty(metadata.title).or_else(|| non_empty(fallback_title)),
        metadata.revision_id,
        content,
        DOC_PREVIEW_CHAR_LIMIT,
    )
}
