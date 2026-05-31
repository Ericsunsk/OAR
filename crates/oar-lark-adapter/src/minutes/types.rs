use std::fmt;

use serde::{Deserialize, Serialize};

use crate::redaction::SecretString;

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuMinuteReadRequest {
    pub user_access_token: SecretString,
    pub source_ref: String,
}

impl fmt::Debug for FeishuMinuteReadRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuMinuteReadRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("source_ref", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinuteReadSummary {
    pub title: Option<String>,
    pub create_time_ms: Option<String>,
    pub duration_ms: Option<String>,
}

impl MinuteReadSummary {
    pub(super) fn from_feishu_minute(minute: super::feishu_types::FeishuMinute) -> Self {
        Self {
            title: non_empty(minute.title),
            create_time_ms: digits_only(minute.create_time),
            duration_ms: digits_only(minute.duration),
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

fn digits_only(value: Option<String>) -> Option<String> {
    non_empty(value).filter(|value| value.chars().all(|ch| ch.is_ascii_digit()))
}
