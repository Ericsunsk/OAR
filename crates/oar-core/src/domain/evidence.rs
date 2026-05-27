use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EvidenceId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSourceKind {
    OkrProgress,
    LarkMinutes,
    LarkDoc,
    ManualReviewNote,
    AuditEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceVisibilityScope {
    Tenant,
    Team,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRef {
    pub source_kind: EvidenceSourceKind,
    pub source_id: String,
    pub locator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceItem {
    pub id: EvidenceId,
    pub summary: String,
    pub reference: EvidenceRef,
    pub content_hash: String,
    pub visibility: EvidenceVisibilityScope,
    pub observed_at: SystemTime,
    pub recorded_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    MissingSummary,
    MissingReferenceId,
    MissingHash,
    InvalidHashFormat,
}

impl EvidenceRef {
    pub fn new(
        source_kind: EvidenceSourceKind,
        source_id: impl Into<String>,
        locator: Option<String>,
    ) -> Result<Self, EvidenceError> {
        let source_id = source_id.into().trim().to_string();
        if source_id.is_empty() {
            return Err(EvidenceError::MissingReferenceId);
        }

        Ok(Self {
            source_kind,
            source_id,
            locator,
        })
    }
}

impl EvidenceItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: EvidenceId,
        summary: impl Into<String>,
        reference: EvidenceRef,
        content_hash: impl Into<String>,
        visibility: EvidenceVisibilityScope,
        observed_at: SystemTime,
        recorded_at: SystemTime,
    ) -> Result<Self, EvidenceError> {
        let summary = summary.into().trim().to_string();
        if summary.is_empty() {
            return Err(EvidenceError::MissingSummary);
        }

        let content_hash = content_hash.into().trim().to_string();
        validate_sha256_hash(&content_hash)?;

        Ok(Self {
            id,
            summary,
            reference,
            content_hash,
            visibility,
            observed_at,
            recorded_at,
        })
    }
}

fn validate_sha256_hash(value: &str) -> Result<(), EvidenceError> {
    if value.is_empty() {
        return Err(EvidenceError::MissingHash);
    }

    let Some(digest) = value.strip_prefix("sha256:") else {
        return Err(EvidenceError::InvalidHashFormat);
    };

    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(EvidenceError::InvalidHashFormat);
    }

    Ok(())
}
