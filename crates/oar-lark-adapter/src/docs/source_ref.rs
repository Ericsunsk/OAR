use std::fmt;

use reqwest::Url;

use super::error::FeishuDocReadError;

#[derive(Clone, PartialEq, Eq)]
pub struct DocSourceRef {
    pub kind: DocSourceRefKind,
}

#[derive(Clone, PartialEq, Eq)]
pub enum DocSourceRefKind {
    Docx { document_id: String },
    Wiki { node_token: String },
}

impl fmt::Debug for DocSourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocSourceRef")
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Debug for DocSourceRefKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Docx { .. } => f
                .debug_struct("Docx")
                .field("document_id", &"[REDACTED]")
                .finish(),
            Self::Wiki { .. } => f
                .debug_struct("Wiki")
                .field("node_token", &"[REDACTED]")
                .finish(),
        }
    }
}

impl DocSourceRef {
    pub fn source_ref(&self) -> String {
        match &self.kind {
            DocSourceRefKind::Docx { document_id } => format!("docx://{document_id}"),
            DocSourceRefKind::Wiki { node_token } => format!("wiki://{node_token}"),
        }
    }
}

pub fn parse_doc_source_ref(source_ref: &str) -> Result<DocSourceRef, FeishuDocReadError> {
    let trimmed = source_ref.trim();
    if let Some(document_id) = trimmed.strip_prefix("docx://") {
        return docx_ref(document_id);
    }
    if let Some(document_id) = trimmed.strip_prefix("doc://") {
        return docx_ref(document_id);
    }
    if let Some(document_id) = trimmed.strip_prefix("feishu://docx/") {
        return docx_ref(document_id);
    }
    if let Some(document_id) = trimmed.strip_prefix("feishu://doc/") {
        return docx_ref(document_id);
    }
    if let Some(document_id) = trimmed.strip_prefix("feishu://docs/docx/") {
        return docx_ref(document_id);
    }
    if let Some(node_token) = trimmed.strip_prefix("wiki://") {
        return wiki_ref(node_token);
    }
    if let Some(node_token) = trimmed.strip_prefix("feishu://wiki/") {
        return wiki_ref(node_token);
    }
    parse_doc_url(trimmed).ok_or(FeishuDocReadError::InvalidSourceRef)
}

fn parse_doc_url(value: &str) -> Option<DocSourceRef> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    if !is_supported_doc_host(url.host_str()?) {
        return None;
    }
    let mut segments = url.path_segments()?.filter(|segment| !segment.is_empty());
    while let Some(segment) = segments.next() {
        match segment {
            "docx" => return docx_ref(segments.next()?).ok(),
            "wiki" => return wiki_ref(segments.next()?).ok(),
            _ => {}
        }
    }
    None
}

fn is_supported_doc_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "feishu.cn"
        || host.ends_with(".feishu.cn")
        || host == "larksuite.com"
        || host.ends_with(".larksuite.com")
}

fn docx_ref(document_id: &str) -> Result<DocSourceRef, FeishuDocReadError> {
    let document_id = document_id.trim();
    if !valid_doc_token(document_id) {
        return Err(FeishuDocReadError::InvalidSourceRef);
    }
    Ok(DocSourceRef {
        kind: DocSourceRefKind::Docx {
            document_id: document_id.to_string(),
        },
    })
}

fn wiki_ref(node_token: &str) -> Result<DocSourceRef, FeishuDocReadError> {
    let node_token = node_token.trim();
    if !valid_doc_token(node_token) {
        return Err(FeishuDocReadError::InvalidSourceRef);
    }
    Ok(DocSourceRef {
        kind: DocSourceRefKind::Wiki {
            node_token: node_token.to_string(),
        },
    })
}

pub(super) fn valid_doc_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docx_and_wiki_refs_without_legacy_cross_type_guessing() {
        let docx = parse_doc_source_ref(" docx://doxcni6mOy7jLRWbEylaKKabcef ").expect("docx");
        assert_eq!(docx.source_ref(), "docx://doxcni6mOy7jLRWbEylaKKabcef");

        let doc = parse_doc_source_ref("doc://doxcni6mOy7jLRWbEylaKKabcef").expect("doc");
        assert_eq!(doc.source_ref(), "docx://doxcni6mOy7jLRWbEylaKKabcef");

        let feishu = parse_doc_source_ref("feishu://docs/docx/doxcni6mOy7jLRWbEylaKKabcef")
            .expect("feishu docx");
        assert_eq!(feishu.source_ref(), "docx://doxcni6mOy7jLRWbEylaKKabcef");

        let feishu_doc =
            parse_doc_source_ref("feishu://doc/doxcni6mOy7jLRWbEylaKKabcef").expect("feishu doc");
        assert_eq!(
            feishu_doc.source_ref(),
            "docx://doxcni6mOy7jLRWbEylaKKabcef"
        );

        let wiki = parse_doc_source_ref("wiki://wikcnKQ1k3p8Vabcef").expect("wiki");
        assert_eq!(wiki.source_ref(), "wiki://wikcnKQ1k3p8Vabcef");
    }

    #[test]
    fn parses_feishu_urls_for_docx_and_wiki_tokens() {
        let docx = parse_doc_source_ref(
            "https://example.feishu.cn/docx/doxcni6mOy7jLRWbEylaKKabcef?from=copy",
        )
        .expect("docx url");
        assert_eq!(docx.source_ref(), "docx://doxcni6mOy7jLRWbEylaKKabcef");

        let wiki = parse_doc_source_ref("https://example.larksuite.com/wiki/wikcnKQ1k3p8Vabcef")
            .expect("wiki url");
        assert_eq!(wiki.source_ref(), "wiki://wikcnKQ1k3p8Vabcef");
    }

    #[test]
    fn rejects_unsafe_or_unsupported_refs() {
        assert_eq!(
            parse_doc_source_ref("task://task_123"),
            Err(FeishuDocReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_doc_source_ref("docx://doc/child"),
            Err(FeishuDocReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_doc_source_ref("wiki://wik token"),
            Err(FeishuDocReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_doc_source_ref("docx://doc?debug=true"),
            Err(FeishuDocReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_doc_source_ref("docx://.."),
            Err(FeishuDocReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_doc_source_ref("docx://doc.token"),
            Err(FeishuDocReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_doc_source_ref("https://example.com/docx/doxcni6mOy7jLRWbEylaKKabcef"),
            Err(FeishuDocReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_doc_source_ref("http://example.feishu.cn/docx/doxcni6mOy7jLRWbEylaKKabcef"),
            Err(FeishuDocReadError::InvalidSourceRef)
        );
    }

    #[test]
    fn debug_redacts_doc_tokens() {
        let doc = parse_doc_source_ref("docx://doxcni6mOy7jLRWbEylaKKabcef").expect("doc");
        let wiki = parse_doc_source_ref("wiki://wikcnKQ1k3p8Vabcef").expect("wiki");

        let doc_debug = format!("{doc:?}");
        let wiki_debug = format!("{wiki:?}");

        assert!(doc_debug.contains("[REDACTED]"));
        assert!(wiki_debug.contains("[REDACTED]"));
        assert!(!doc_debug.contains("doxcni6m"));
        assert!(!wiki_debug.contains("wikcnKQ"));
    }
}
