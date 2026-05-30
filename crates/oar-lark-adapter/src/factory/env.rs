use std::fmt;
use std::fs::File;
use std::io::Read;

use crate::redaction::SecretString;

#[derive(Clone, PartialEq, Eq)]
pub struct PostgresFeishuAuthRefreshEnvConfig {
    pub app_id: String,
    pub app_secret: SecretString,
    pub grant_key_id: String,
    pub grant_key_material: [u8; 32],
}

impl fmt::Debug for PostgresFeishuAuthRefreshEnvConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresFeishuAuthRefreshEnvConfig")
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("grant_key_id", &"[REDACTED]")
            .field("grant_key_material", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PostgresFeishuAuthRefreshEnvConfigError {
    MissingAppId,
    MissingAppSecret,
    MissingGrantKeyId,
    MissingGrantKeyHex,
    InvalidGrantKeyHex,
    EphemeralGrantKeyUnavailable,
}

impl fmt::Debug for PostgresFeishuAuthRefreshEnvConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAppId => write!(
                f,
                "PostgresFeishuAuthRefreshEnvConfigError(missing_oar_feishu_app_id)"
            ),
            Self::MissingAppSecret => write!(
                f,
                "PostgresFeishuAuthRefreshEnvConfigError(missing_oar_feishu_app_secret)"
            ),
            Self::MissingGrantKeyId => write!(
                f,
                "PostgresFeishuAuthRefreshEnvConfigError(missing_oar_grant_key_id)"
            ),
            Self::MissingGrantKeyHex => write!(
                f,
                "PostgresFeishuAuthRefreshEnvConfigError(missing_oar_grant_key_hex)"
            ),
            Self::InvalidGrantKeyHex => write!(
                f,
                "PostgresFeishuAuthRefreshEnvConfigError(invalid_oar_grant_key_hex)"
            ),
            Self::EphemeralGrantKeyUnavailable => write!(
                f,
                "PostgresFeishuAuthRefreshEnvConfigError(ephemeral_grant_key_unavailable)"
            ),
        }
    }
}

impl fmt::Display for PostgresFeishuAuthRefreshEnvConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAppId => write!(f, "oar feishu app id is required"),
            Self::MissingAppSecret => write!(f, "oar feishu app secret is required"),
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

impl std::error::Error for PostgresFeishuAuthRefreshEnvConfigError {}

impl PostgresFeishuAuthRefreshEnvConfig {
    pub fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, PostgresFeishuAuthRefreshEnvConfigError> {
        let app_id = required_env(
            env,
            "OAR_FEISHU_APP_ID",
            PostgresFeishuAuthRefreshEnvConfigError::MissingAppId,
        )?;
        let app_secret = required_env(
            env,
            "OAR_FEISHU_APP_SECRET",
            PostgresFeishuAuthRefreshEnvConfigError::MissingAppSecret,
        )?;
        let grant_key_id = optional_env(env, "OAR_GRANT_KEY_ID");
        let grant_key_hex = optional_env(env, "OAR_GRANT_KEY_HEX");

        let (grant_key_id, grant_key_material) = match (grant_key_id, grant_key_hex) {
            (Some(grant_key_id), Some(grant_key_hex)) => {
                (grant_key_id, decode_grant_key_hex(&grant_key_hex)?)
            }
            (Some(_), None) => {
                return Err(PostgresFeishuAuthRefreshEnvConfigError::MissingGrantKeyHex);
            }
            (None, Some(_)) => {
                return Err(PostgresFeishuAuthRefreshEnvConfigError::MissingGrantKeyId);
            }
            (None, None) if env_flag(env, "OAR_ALLOW_EPHEMERAL_GRANT_KEY") => {
                generate_ephemeral_dev_grant_key()?
            }
            (None, None) => {
                return Err(PostgresFeishuAuthRefreshEnvConfigError::MissingGrantKeyId);
            }
        };

        Ok(Self {
            app_id,
            app_secret: SecretString::new(app_secret),
            grant_key_id,
            grant_key_material,
        })
    }
}

fn decode_grant_key_hex(
    grant_key_hex: &str,
) -> Result<[u8; 32], PostgresFeishuAuthRefreshEnvConfigError> {
    let decoded = hex::decode(grant_key_hex)
        .map_err(|_| PostgresFeishuAuthRefreshEnvConfigError::InvalidGrantKeyHex)?;
    decoded
        .try_into()
        .map_err(|_| PostgresFeishuAuthRefreshEnvConfigError::InvalidGrantKeyHex)
}

fn generate_ephemeral_dev_grant_key(
) -> Result<(String, [u8; 32]), PostgresFeishuAuthRefreshEnvConfigError> {
    let mut bytes = [0_u8; 40];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| PostgresFeishuAuthRefreshEnvConfigError::EphemeralGrantKeyUnavailable)?;

    let key_id = format!("dev-ephemeral-{}", hex::encode(&bytes[..8]));
    let mut material = [0_u8; 32];
    material.copy_from_slice(&bytes[8..]);
    Ok((key_id, material))
}

fn required_env(
    env: &impl Fn(&str) -> Option<String>,
    key: &str,
    error: PostgresFeishuAuthRefreshEnvConfigError,
) -> Result<String, PostgresFeishuAuthRefreshEnvConfigError> {
    optional_env(env, key).ok_or(error)
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
