mod client;
mod error;
mod feishu_types;
mod response_parser;
mod source_ref;
#[cfg(test)]
mod tests;
mod types;

pub use client::{
    build_docx_metadata_request, build_docx_raw_content_request, build_wiki_node_request,
    AsyncFeishuDocRead, FeishuDocReadClient,
};
pub use error::FeishuDocReadError;
pub use source_ref::{parse_doc_source_ref, DocSourceRef, DocSourceRefKind};
pub use types::{DocReadSummary, FeishuDocReadRequest};
