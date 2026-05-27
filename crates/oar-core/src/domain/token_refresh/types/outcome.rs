use std::fmt;
use std::time::SystemTime;

use super::material::EncryptedGrantMaterial;

#[derive(Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    Success {
        rotated_material: EncryptedGrantMaterial,
        key_id: String,
        new_fingerprint: String,
        refreshed_at: SystemTime,
        expires_at: Option<SystemTime>,
    },
    TransientFailure {
        safe_error: String,
    },
    ReauthFailure {
        safe_error: String,
    },
    ConfigRequired {
        safe_error: String,
    },
}

impl fmt::Debug for RefreshOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success {
                rotated_material,
                refreshed_at,
                expires_at,
                ..
            } => f
                .debug_struct("Success")
                .field("rotated_material", rotated_material)
                .field("key_id", &"[REDACTED]")
                .field("new_fingerprint", &"[REDACTED]")
                .field("refreshed_at", refreshed_at)
                .field("expires_at", expires_at)
                .finish(),
            Self::TransientFailure { safe_error } => f
                .debug_struct("TransientFailure")
                .field("safe_error", safe_error)
                .finish(),
            Self::ReauthFailure { safe_error } => f
                .debug_struct("ReauthFailure")
                .field("safe_error", safe_error)
                .finish(),
            Self::ConfigRequired { safe_error } => f
                .debug_struct("ConfigRequired")
                .field("safe_error", safe_error)
                .finish(),
        }
    }
}
