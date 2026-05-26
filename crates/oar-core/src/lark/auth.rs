use std::fmt;
use std::time::{Duration, SystemTime};

use crate::domain::token_refresh::{
    AuthRefreshAdapter, EncryptedGrantMaterial, RefreshOutcome, TokenRefreshGrantSnapshot,
};

const SAFE_TRANSIENT_ERROR: &str = "lark auth refresh temporarily unavailable";
const SAFE_REAUTH_ERROR: &str = "reauthentication required";
const SAFE_PARSE_ERROR: &str = "invalid lark auth refresh envelope";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LarkAuthGrantState {
    Valid,
    Expired,
    NeedsRefresh,
    Revoked,
    ReauthRequired,
}

#[derive(Clone, PartialEq, Eq)]
pub struct LarkAuthRefreshRequest {
    pub grant_id: String,
    pub tenant_id: String,
    pub expected_fingerprint: String,
    pub grant_state: LarkAuthGrantState,
    pub has_refresh_material: bool,
    pub is_revoked: bool,
    pub reauth_marked: bool,
}

impl fmt::Debug for LarkAuthRefreshRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LarkAuthRefreshRequest")
            .field("grant_id", &self.grant_id)
            .field("tenant_id", &self.tenant_id)
            .field("expected_fingerprint", &"[REDACTED]")
            .field("grant_state", &self.grant_state)
            .field("has_refresh_material", &self.has_refresh_material)
            .field("is_revoked", &self.is_revoked)
            .field("reauth_marked", &self.reauth_marked)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LarkAuthRefreshSuccess {
    pub encrypted_primary: Vec<u8>,
    pub encrypted_renewal: Vec<u8>,
    pub key_id: String,
    pub new_fingerprint: String,
    pub refreshed_at_ms: u64,
    pub expires_at_ms: Option<u64>,
}

impl fmt::Debug for LarkAuthRefreshSuccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LarkAuthRefreshSuccess")
            .field("encrypted_primary", &"[REDACTED]")
            .field("encrypted_renewal", &"[REDACTED]")
            .field("key_id", &self.key_id)
            .field("new_fingerprint", &"[REDACTED]")
            .field("refreshed_at_ms", &self.refreshed_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum LarkAuthRefreshFailure {
    Transient { safe_error: String },
    ReauthRequired { safe_error: String },
}

impl fmt::Debug for LarkAuthRefreshFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LarkAuthRefreshFailure::Transient { safe_error } => f
                .debug_struct("Transient")
                .field(
                    "safe_error",
                    &sanitize_safe_error(safe_error, SAFE_TRANSIENT_ERROR),
                )
                .finish(),
            LarkAuthRefreshFailure::ReauthRequired { safe_error } => f
                .debug_struct("ReauthRequired")
                .field(
                    "safe_error",
                    &sanitize_safe_error(safe_error, SAFE_REAUTH_ERROR),
                )
                .finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum LarkAuthRefreshResponse {
    Success(LarkAuthRefreshSuccess),
    Failure(LarkAuthRefreshFailure),
}

impl fmt::Debug for LarkAuthRefreshResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LarkAuthRefreshResponse::Success(success) => {
                f.debug_tuple("Success").field(success).finish()
            }
            LarkAuthRefreshResponse::Failure(failure) => {
                f.debug_tuple("Failure").field(failure).finish()
            }
        }
    }
}

pub trait LarkAuthRefreshClient {
    type Error;

    fn refresh(
        &mut self,
        request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshResponse, Self::Error>;
}

#[derive(Clone, PartialEq, Eq)]
pub struct LarkAuthRefreshAdapter<C> {
    client: C,
}

impl<C> fmt::Debug for LarkAuthRefreshAdapter<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LarkAuthRefreshAdapter")
            .field("client", &"[REDACTED]")
            .finish()
    }
}

impl<C> LarkAuthRefreshAdapter<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    fn map_snapshot(snapshot: &TokenRefreshGrantSnapshot) -> LarkAuthRefreshRequest {
        LarkAuthRefreshRequest {
            grant_id: snapshot.grant_id.0.clone(),
            tenant_id: snapshot.tenant_id.0.clone(),
            expected_fingerprint: snapshot.expected_fingerprint.clone(),
            grant_state: map_grant_state(snapshot.state),
            has_refresh_material: snapshot.has_refresh_material,
            is_revoked: snapshot.revoked_at.is_some(),
            reauth_marked: snapshot.reauth_required_at.is_some(),
        }
    }
}

impl<C> AuthRefreshAdapter for LarkAuthRefreshAdapter<C>
where
    C: LarkAuthRefreshClient,
{
    fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        let request = Self::map_snapshot(snapshot);
        match self.client.refresh(&request) {
            Ok(LarkAuthRefreshResponse::Success(success)) => RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: success.encrypted_primary,
                    encrypted_renewal: success.encrypted_renewal,
                },
                key_id: success.key_id,
                new_fingerprint: success.new_fingerprint,
                refreshed_at: ms_to_system_time(success.refreshed_at_ms),
                expires_at: success.expires_at_ms.map(ms_to_system_time),
            },
            Ok(LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::Transient {
                safe_error,
            })) => RefreshOutcome::TransientFailure {
                safe_error: sanitize_safe_error(&safe_error, SAFE_TRANSIENT_ERROR),
            },
            Ok(LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::ReauthRequired {
                safe_error,
            })) => RefreshOutcome::ReauthFailure {
                safe_error: sanitize_safe_error(&safe_error, SAFE_REAUTH_ERROR),
            },
            Err(_) => RefreshOutcome::TransientFailure {
                safe_error: SAFE_TRANSIENT_ERROR.to_string(),
            },
        }
    }
}

fn map_grant_state(state: crate::domain::identity::TokenGrantState) -> LarkAuthGrantState {
    match state {
        crate::domain::identity::TokenGrantState::Valid => LarkAuthGrantState::Valid,
        crate::domain::identity::TokenGrantState::Expired => LarkAuthGrantState::Expired,
        crate::domain::identity::TokenGrantState::NeedsRefresh => LarkAuthGrantState::NeedsRefresh,
        crate::domain::identity::TokenGrantState::Revoked => LarkAuthGrantState::Revoked,
        crate::domain::identity::TokenGrantState::ReauthRequired => {
            LarkAuthGrantState::ReauthRequired
        }
    }
}

fn ms_to_system_time(ms: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
}

fn sanitize_safe_error(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || contains_sensitive_marker(trimmed) {
        return fallback.to_string();
    }

    match trimmed {
        "invalid_grant" | "temporarily unavailable" | SAFE_TRANSIENT_ERROR | SAFE_REAUTH_ERROR => {
            trimmed.to_string()
        }
        _ => fallback.to_string(),
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum LarkAuthRefreshParseError {
    InvalidEnvelope,
    SensitiveContentDetected,
}

impl fmt::Debug for LarkAuthRefreshParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LarkAuthRefreshParseError::InvalidEnvelope => {
                write!(f, "LarkAuthRefreshParseError(InvalidEnvelope)")
            }
            LarkAuthRefreshParseError::SensitiveContentDetected => {
                write!(f, "LarkAuthRefreshParseError(SensitiveContentDetected)")
            }
        }
    }
}

impl fmt::Display for LarkAuthRefreshParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LarkAuthRefreshParseError::InvalidEnvelope => write!(f, "{}", SAFE_PARSE_ERROR),
            LarkAuthRefreshParseError::SensitiveContentDetected => {
                write!(f, "{}", SAFE_PARSE_ERROR)
            }
        }
    }
}

impl std::error::Error for LarkAuthRefreshParseError {}

pub fn parse_lark_auth_refresh_response(
    raw: &str,
) -> Result<LarkAuthRefreshResponse, LarkAuthRefreshParseError> {
    if contains_sensitive_marker(raw) {
        return Err(LarkAuthRefreshParseError::SensitiveContentDetected);
    }

    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| LarkAuthRefreshParseError::InvalidEnvelope)?;
    reject_sensitive_json(&value)?;

    parse_safe_envelope(value)
}

fn parse_safe_envelope(
    value: serde_json::Value,
) -> Result<LarkAuthRefreshResponse, LarkAuthRefreshParseError> {
    let obj = value
        .as_object()
        .ok_or(LarkAuthRefreshParseError::InvalidEnvelope)?;
    let outcome = obj
        .get("outcome")
        .and_then(serde_json::Value::as_str)
        .ok_or(LarkAuthRefreshParseError::InvalidEnvelope)?;

    match outcome {
        "success" => {
            let encrypted_primary = parse_byte_vec(obj.get("encrypted_primary"))?;
            let encrypted_renewal = parse_byte_vec(obj.get("encrypted_renewal"))?;
            let key_id = parse_string(obj.get("key_id"))?;
            let new_fingerprint = parse_string(obj.get("new_fingerprint"))?;
            let refreshed_at_ms = parse_u64(obj.get("refreshed_at_ms"))?;
            let expires_at_ms = match obj.get("expires_at_ms") {
                Some(serde_json::Value::Null) | None => None,
                Some(value) => Some(parse_u64(Some(value))?),
            };
            Ok(LarkAuthRefreshResponse::Success(LarkAuthRefreshSuccess {
                encrypted_primary,
                encrypted_renewal,
                key_id,
                new_fingerprint,
                refreshed_at_ms,
                expires_at_ms,
            }))
        }
        "transient_failure" => Ok(LarkAuthRefreshResponse::Failure(
            LarkAuthRefreshFailure::Transient {
                safe_error: parse_string(obj.get("safe_error"))?,
            },
        )),
        "reauth_required" => Ok(LarkAuthRefreshResponse::Failure(
            LarkAuthRefreshFailure::ReauthRequired {
                safe_error: parse_string(obj.get("safe_error"))?,
            },
        )),
        _ => Err(LarkAuthRefreshParseError::InvalidEnvelope),
    }
}

fn parse_string(value: Option<&serde_json::Value>) -> Result<String, LarkAuthRefreshParseError> {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or(LarkAuthRefreshParseError::InvalidEnvelope)
}

fn parse_u64(value: Option<&serde_json::Value>) -> Result<u64, LarkAuthRefreshParseError> {
    value
        .and_then(serde_json::Value::as_u64)
        .ok_or(LarkAuthRefreshParseError::InvalidEnvelope)
}

fn parse_byte_vec(value: Option<&serde_json::Value>) -> Result<Vec<u8>, LarkAuthRefreshParseError> {
    let arr = value
        .and_then(serde_json::Value::as_array)
        .ok_or(LarkAuthRefreshParseError::InvalidEnvelope)?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let n = item
            .as_u64()
            .ok_or(LarkAuthRefreshParseError::InvalidEnvelope)?;
        let byte = u8::try_from(n).map_err(|_| LarkAuthRefreshParseError::InvalidEnvelope)?;
        out.push(byte);
    }
    Ok(out)
}

fn reject_sensitive_json(value: &serde_json::Value) -> Result<(), LarkAuthRefreshParseError> {
    match value {
        serde_json::Value::Object(map) => {
            for (key, nested) in map {
                if contains_sensitive_marker(key) {
                    return Err(LarkAuthRefreshParseError::SensitiveContentDetected);
                }
                reject_sensitive_json(nested)?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                reject_sensitive_json(item)?;
            }
            Ok(())
        }
        serde_json::Value::String(text) => {
            if contains_sensitive_marker(text) {
                Err(LarkAuthRefreshParseError::SensitiveContentDetected)
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn contains_sensitive_marker(input: &str) -> bool {
    let normalized = input.to_ascii_lowercase();
    normalized.contains("access_token")
        || normalized.contains("refresh_token")
        || normalized.contains("authorization_code")
        || normalized.contains("auth_code")
        || normalized.contains("authorization")
        || normalized.contains("bearer")
        || normalized.contains("refresh_token=")
}
