use std::fmt;

use serde::{Deserialize, Serialize};

use crate::redaction::SecretString;

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuDocReadRequest {
    pub user_access_token: SecretString,
    pub source_ref: String,
}

impl fmt::Debug for FeishuDocReadRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuDocReadRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("source_ref", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocReadSummary {
    pub title: Option<String>,
    pub doc_type: String,
    pub revision_id: Option<String>,
    pub content_preview: String,
    pub content_truncated: bool,
    pub content_char_count: usize,
}

impl DocReadSummary {
    pub(super) fn docx(
        title: Option<String>,
        revision_id: Option<String>,
        content: String,
        max_preview_chars: usize,
    ) -> Self {
        let content_char_count = content.chars().count();
        let content_preview = content.chars().take(max_preview_chars).collect::<String>();
        Self {
            title: non_empty(title),
            doc_type: "docx".to_string(),
            revision_id: non_empty(revision_id),
            content_truncated: content_char_count > max_preview_chars,
            content_char_count,
            content_preview,
        }
    }
}

pub(super) fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
