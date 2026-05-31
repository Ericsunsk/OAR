use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct FeishuDocxMetadataResponse {
    pub(super) code: i64,
    pub(super) data: Option<FeishuDocxMetadataData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuDocxMetadataData {
    pub(super) document: Option<FeishuDocxDocument>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuDocxDocument {
    pub(super) revision_id: Option<i64>,
    pub(super) title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuDocxRawContentResponse {
    pub(super) code: i64,
    pub(super) data: Option<FeishuDocxRawContentData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuDocxRawContentData {
    pub(super) content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuWikiNodeResponse {
    pub(super) code: i64,
    pub(super) data: Option<FeishuWikiNodeData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuWikiNodeData {
    pub(super) node: Option<FeishuWikiNode>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct FeishuWikiNode {
    pub(super) obj_token: Option<String>,
    pub(super) obj_type: Option<String>,
    pub(super) title: Option<String>,
}
