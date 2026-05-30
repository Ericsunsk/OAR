use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FacadePersistenceConfig {
    grant_key_id: String,
    grant_key_material: [u8; 32],
}

impl FacadePersistenceConfig {
    pub(crate) fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, FacadePersistenceConfigError> {
        let grant_key_id = optional_env(env, "OAR_GRANT_KEY_ID");
        let grant_key_hex = optional_env(env, "OAR_GRANT_KEY_HEX");

        let (grant_key_id, grant_key_material) = match (grant_key_id, grant_key_hex) {
            (Some(grant_key_id), Some(grant_key_hex)) => {
                (grant_key_id, decode_grant_key_hex(&grant_key_hex)?)
            }
            (Some(_), None) => return Err(FacadePersistenceConfigError::MissingGrantKeyHex),
            (None, Some(_)) => return Err(FacadePersistenceConfigError::MissingGrantKeyId),
            (None, None) if env_flag(env, "OAR_ALLOW_EPHEMERAL_GRANT_KEY") => {
                generate_ephemeral_dev_grant_key()?
            }
            (None, None) => return Err(FacadePersistenceConfigError::MissingGrantKeyId),
        };

        Ok(Self {
            grant_key_id,
            grant_key_material,
        })
    }

    pub(super) fn grant_key_id(&self) -> &str {
        &self.grant_key_id
    }

    pub(super) fn grant_key_material(&self) -> [u8; 32] {
        self.grant_key_material
    }
}

impl fmt::Debug for FacadePersistenceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FacadePersistenceConfig")
            .field("grant_key_id", &"[REDACTED]")
            .field("grant_key_material", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FacadePersistenceConfigError {
    MissingGrantKeyId,
    MissingGrantKeyHex,
    InvalidGrantKeyHex,
    EphemeralGrantKeyUnavailable,
}

impl fmt::Debug for FacadePersistenceConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGrantKeyId => {
                write!(f, "FacadePersistenceConfigError(missing_oar_grant_key_id)")
            }
            Self::MissingGrantKeyHex => {
                write!(f, "FacadePersistenceConfigError(missing_oar_grant_key_hex)")
            }
            Self::InvalidGrantKeyHex => {
                write!(f, "FacadePersistenceConfigError(invalid_oar_grant_key_hex)")
            }
            Self::EphemeralGrantKeyUnavailable => write!(
                f,
                "FacadePersistenceConfigError(ephemeral_grant_key_unavailable)"
            ),
        }
    }
}

impl fmt::Display for FacadePersistenceConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGrantKeyId => write!(f, "oar grant key id is required"),
            Self::MissingGrantKeyHex => write!(f, "oar grant key hex is required"),
            Self::InvalidGrantKeyHex => {
                write!(f, "oar grant key hex must decode to exactly 32 bytes")
            }
            Self::EphemeralGrantKeyUnavailable => {
                write!(f, "oar ephemeral grant key could not be generated")
            }
        }
    }
}

impl Error for FacadePersistenceConfigError {}

fn decode_grant_key_hex(grant_key_hex: &str) -> Result<[u8; 32], FacadePersistenceConfigError> {
    let decoded =
        hex::decode(grant_key_hex).map_err(|_| FacadePersistenceConfigError::InvalidGrantKeyHex)?;
    decoded
        .try_into()
        .map_err(|_| FacadePersistenceConfigError::InvalidGrantKeyHex)
}

fn generate_ephemeral_dev_grant_key() -> Result<(String, [u8; 32]), FacadePersistenceConfigError> {
    let mut bytes = [0_u8; 40];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| FacadePersistenceConfigError::EphemeralGrantKeyUnavailable)?;

    let key_id = format!("dev-ephemeral-{}", hex::encode(&bytes[..8]));
    let mut material = [0_u8; 32];
    material.copy_from_slice(&bytes[8..]);
    Ok((key_id, material))
}

fn optional_env(env: &impl Fn(&str) -> Option<String>, key: &str) -> Option<String> {
    env(key).and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn env_flag(env: &impl Fn(&str) -> Option<String>, key: &str) -> bool {
    optional_env(env, key)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistence_config_parses_stable_grant_key_without_leaking_material() {
        let config = FacadePersistenceConfig::from_env_map(&|key| match key {
            "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
            "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
            _ => None,
        })
        .expect("config");

        assert_eq!(config.grant_key_id, "key-prod-v1");
        assert_eq!(config.grant_key_material, [0x11; 32]);
        assert!(!format!("{config:?}").contains("key-prod-v1"));
    }

    #[test]
    fn persistence_config_rejects_partial_or_invalid_grant_key_without_leaking_input() {
        let partial = FacadePersistenceConfig::from_env_map(&|key| match key {
            "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
            _ => None,
        })
        .expect_err("partial key should fail");
        assert_eq!(partial, FacadePersistenceConfigError::MissingGrantKeyHex);

        let bad_value = "not-hex-sensitive-key";
        let invalid = FacadePersistenceConfig::from_env_map(&|key| match key {
            "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
            "OAR_GRANT_KEY_HEX" => Some(bad_value.to_string()),
            _ => None,
        })
        .expect_err("invalid key should fail");
        assert_eq!(invalid, FacadePersistenceConfigError::InvalidGrantKeyHex);
        assert!(!format!("{invalid:?} {invalid}").contains(bad_value));
    }

    #[test]
    fn persistence_config_can_generate_ephemeral_dev_key() {
        let config = FacadePersistenceConfig::from_env_map(&|key| {
            (key == "OAR_ALLOW_EPHEMERAL_GRANT_KEY").then(|| "true".to_string())
        })
        .expect("ephemeral key");

        assert!(config.grant_key_id.starts_with("dev-ephemeral-"));
        assert_ne!(config.grant_key_material, [0; 32]);
        assert!(!format!("{config:?}").contains(&config.grant_key_id));
    }
}
