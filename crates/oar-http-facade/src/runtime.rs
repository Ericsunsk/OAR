use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use oar_runtime::TenantMaintenanceRuntimeConfig;
use sqlx::postgres::PgPoolOptions;

use crate::agent::{AgentModelSettingsRuntime, AgentRuntime, AgentRuntimeConfigError};
use crate::feishu_auth::{FeishuLoginRuntime, FeishuLoginRuntimeConfigError};
use crate::persistence::{FacadePersistenceConfig, FacadePersistenceRuntime};
use crate::util::non_empty_env;

#[derive(Clone, Default)]
pub struct OarHttpFacadeRuntime {
    pub(crate) persistence: Option<FacadePersistenceRuntime>,
    pub(crate) feishu_login: Option<Arc<FeishuLoginRuntime>>,
    pub(crate) agent: Option<Arc<AgentRuntime>>,
    pub(crate) agent_settings: Option<Arc<AgentModelSettingsRuntime>>,
    pub(crate) tenant_maintenance: Option<TenantMaintenanceRuntimeSettings>,
}

impl fmt::Debug for OarHttpFacadeRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OarHttpFacadeRuntime")
            .field("persistence", &self.persistence.is_some())
            .field("feishu_login", &self.feishu_login.is_some())
            .field("agent", &self.agent.is_some())
            .field("agent_settings", &self.agent_settings.is_some())
            .field("tenant_maintenance", &self.tenant_maintenance.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceRuntimeSettings {
    pub(crate) runtime: TenantMaintenanceRuntimeConfig,
    pub(crate) instance_id: String,
    pub(crate) due_lookahead_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OarHttpFacadeRuntimeError {
    PartialFeishuAuthConfig,
    InvalidFeishuOpenApiConfig,
    InvalidFeishuLoginConfig,
    InvalidPersistenceConfig,
    PartialAgentConfig,
    InvalidAgentConfig,
    TenantMaintenanceRequiresDatabase,
    TenantMaintenanceRequiresFeishuAuth,
    TenantMaintenanceMissingInstanceId,
    InvalidTenantMaintenanceConfig,
    DatabaseConnectFailed,
    HttpClientBuildFailed,
}

impl fmt::Display for OarHttpFacadeRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PartialFeishuAuthConfig => {
                write!(f, "oar_feishu_auth_config_partial")
            }
            Self::InvalidFeishuOpenApiConfig => {
                write!(f, "oar_feishu_open_api_config_invalid")
            }
            Self::InvalidFeishuLoginConfig => {
                write!(f, "oar_feishu_login_config_invalid")
            }
            Self::InvalidPersistenceConfig => {
                write!(f, "oar_persistence_config_invalid")
            }
            Self::PartialAgentConfig => write!(f, "oar_agent_config_partial"),
            Self::InvalidAgentConfig => write!(f, "oar_agent_config_invalid"),
            Self::TenantMaintenanceRequiresDatabase => {
                write!(f, "oar_tenant_maintenance_database_required")
            }
            Self::TenantMaintenanceRequiresFeishuAuth => {
                write!(f, "oar_tenant_maintenance_feishu_auth_required")
            }
            Self::TenantMaintenanceMissingInstanceId => {
                write!(f, "oar_tenant_maintenance_instance_id_required")
            }
            Self::InvalidTenantMaintenanceConfig => {
                write!(f, "oar_tenant_maintenance_config_invalid")
            }
            Self::DatabaseConnectFailed => write!(f, "oar_database_connect_failed"),
            Self::HttpClientBuildFailed => write!(f, "oar_feishu_http_client_build_failed"),
        }
    }
}

impl Error for OarHttpFacadeRuntimeError {}

impl OarHttpFacadeRuntime {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub(crate) fn persistence(&self) -> Option<&FacadePersistenceRuntime> {
        self.persistence.as_ref()
    }

    pub fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        Self::from_env_map_with_persistence(env, None)
    }

    pub async fn from_env_map_async(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        let Some(database_url) = non_empty_env(env, "DATABASE_URL") else {
            return Self::from_env_map_with_persistence(env, None);
        };
        let persistence_config = FacadePersistenceConfig::from_env_map(env)
            .map_err(|_| OarHttpFacadeRuntimeError::InvalidPersistenceConfig)?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .map_err(|_| OarHttpFacadeRuntimeError::DatabaseConnectFailed)?;
        Self::from_env_map_with_persistence(
            env,
            Some(FacadePersistenceRuntime::new(pool, persistence_config)),
        )
    }

    pub(crate) fn from_env_map_with_persistence(
        env: &impl Fn(&str) -> Option<String>,
        persistence: Option<FacadePersistenceRuntime>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        let agent = AgentRuntime::from_env_map(env)
            .map_err(agent_runtime_config_error)?
            .map(Arc::new);
        let agent_settings = match persistence.as_ref() {
            Some(persistence) => Some(Arc::new(
                AgentModelSettingsRuntime::new(
                    persistence.pool(),
                    persistence.grant_key_id().to_string(),
                    persistence.grant_key_material(),
                )
                .map_err(agent_runtime_config_error)?,
            )),
            None => None,
        };
        let feishu_login = FeishuLoginRuntime::from_env_map(env)
            .map_err(feishu_runtime_config_error)?
            .map(Arc::new);
        let tenant_maintenance = tenant_maintenance_runtime_settings_from_env_map(
            env,
            persistence.as_ref(),
            feishu_login.as_ref(),
        )?;
        Ok(Self {
            persistence,
            feishu_login,
            agent,
            agent_settings,
            tenant_maintenance,
        })
    }
}

const TENANT_MAINTENANCE_ENABLED_ENV: &str = "OAR_TENANT_MAINTENANCE_ENABLED";
const TENANT_MAINTENANCE_INSTANCE_ID_ENV: &str = "OAR_TENANT_MAINTENANCE_INSTANCE_ID";
const TENANT_MAINTENANCE_INTERVAL_MS_ENV: &str = "OAR_TENANT_MAINTENANCE_INTERVAL_MS";
const TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS_ENV: &str = "OAR_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS";
const ALLOW_EPHEMERAL_GRANT_KEY_ENV: &str = "OAR_ALLOW_EPHEMERAL_GRANT_KEY";
const DEFAULT_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 60_000;
const DEFAULT_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 300_000;
const MIN_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 5_000;
const MAX_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 3_600_000;
const MIN_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 1_000;
const MAX_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 86_400_000;
const TENANT_MAINTENANCE_INSTANCE_ID_MAX_LEN: usize = 128;

fn tenant_maintenance_runtime_settings_from_env_map(
    env: &impl Fn(&str) -> Option<String>,
    persistence: Option<&FacadePersistenceRuntime>,
    feishu_login: Option<&Arc<FeishuLoginRuntime>>,
) -> Result<Option<TenantMaintenanceRuntimeSettings>, OarHttpFacadeRuntimeError> {
    if !enabled_env_flag(env, TENANT_MAINTENANCE_ENABLED_ENV)? {
        return Ok(None);
    }
    if enabled_env_flag(env, ALLOW_EPHEMERAL_GRANT_KEY_ENV)? {
        return Err(OarHttpFacadeRuntimeError::InvalidTenantMaintenanceConfig);
    }
    if persistence.is_none() {
        return Err(OarHttpFacadeRuntimeError::TenantMaintenanceRequiresDatabase);
    }
    if feishu_login.is_none() {
        return Err(OarHttpFacadeRuntimeError::TenantMaintenanceRequiresFeishuAuth);
    }

    let instance_id = non_empty_env(env, TENANT_MAINTENANCE_INSTANCE_ID_ENV)
        .ok_or(OarHttpFacadeRuntimeError::TenantMaintenanceMissingInstanceId)?;
    validate_tenant_maintenance_instance_id(&instance_id)?;
    let interval_ms = bounded_u64_env(
        env,
        TENANT_MAINTENANCE_INTERVAL_MS_ENV,
        DEFAULT_TENANT_MAINTENANCE_INTERVAL_MS,
        MIN_TENANT_MAINTENANCE_INTERVAL_MS,
        MAX_TENANT_MAINTENANCE_INTERVAL_MS,
    )?;
    let due_lookahead_ms = bounded_u64_env(
        env,
        TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS_ENV,
        DEFAULT_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS,
        MIN_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS,
        MAX_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS,
    )?;
    let runtime = TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::from_millis(interval_ms),
    };
    runtime
        .validate()
        .map_err(|_| OarHttpFacadeRuntimeError::InvalidTenantMaintenanceConfig)?;

    Ok(Some(TenantMaintenanceRuntimeSettings {
        runtime,
        instance_id,
        due_lookahead_ms,
    }))
}

fn validate_tenant_maintenance_instance_id(value: &str) -> Result<(), OarHttpFacadeRuntimeError> {
    let valid = !value.is_empty()
        && value.len() <= TENANT_MAINTENANCE_INSTANCE_ID_MAX_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'));
    if !valid {
        return Err(OarHttpFacadeRuntimeError::InvalidTenantMaintenanceConfig);
    }
    Ok(())
}

fn enabled_env_flag(
    env: &impl Fn(&str) -> Option<String>,
    key: &str,
) -> Result<bool, OarHttpFacadeRuntimeError> {
    let Some(value) = non_empty_env(env, key) else {
        return Ok(false);
    };
    match value.as_str() {
        "1" | "true" | "TRUE" | "yes" | "YES" => Ok(true),
        "0" | "false" | "FALSE" | "no" | "NO" => Ok(false),
        _ => Err(OarHttpFacadeRuntimeError::InvalidTenantMaintenanceConfig),
    }
}

fn bounded_u64_env(
    env: &impl Fn(&str) -> Option<String>,
    key: &str,
    default_value: u64,
    min_value: u64,
    max_value: u64,
) -> Result<u64, OarHttpFacadeRuntimeError> {
    let Some(value) = non_empty_env(env, key) else {
        return Ok(default_value);
    };
    let value = value
        .parse::<u64>()
        .map_err(|_| OarHttpFacadeRuntimeError::InvalidTenantMaintenanceConfig)?;
    if !(min_value..=max_value).contains(&value) {
        return Err(OarHttpFacadeRuntimeError::InvalidTenantMaintenanceConfig);
    }
    Ok(value)
}

fn agent_runtime_config_error(error: AgentRuntimeConfigError) -> OarHttpFacadeRuntimeError {
    match error {
        AgentRuntimeConfigError::PartialOpenAICompatibleConfig => {
            OarHttpFacadeRuntimeError::PartialAgentConfig
        }
        AgentRuntimeConfigError::PartialAnthropicConfig => {
            OarHttpFacadeRuntimeError::PartialAgentConfig
        }
        AgentRuntimeConfigError::InvalidOpenAICompatibleBaseURL
        | AgentRuntimeConfigError::InvalidAnthropicBaseURL
        | AgentRuntimeConfigError::InvalidAgentProvider
        | AgentRuntimeConfigError::AmbiguousAgentProviderConfig
        | AgentRuntimeConfigError::HttpClientBuildFailed => {
            OarHttpFacadeRuntimeError::InvalidAgentConfig
        }
    }
}

fn feishu_runtime_config_error(error: FeishuLoginRuntimeConfigError) -> OarHttpFacadeRuntimeError {
    match error {
        FeishuLoginRuntimeConfigError::PartialAuthConfig => {
            OarHttpFacadeRuntimeError::PartialFeishuAuthConfig
        }
        FeishuLoginRuntimeConfigError::InvalidOpenApiConfig => {
            OarHttpFacadeRuntimeError::InvalidFeishuOpenApiConfig
        }
        FeishuLoginRuntimeConfigError::InvalidLoginConfig => {
            OarHttpFacadeRuntimeError::InvalidFeishuLoginConfig
        }
        FeishuLoginRuntimeConfigError::HttpClientBuildFailed => {
            OarHttpFacadeRuntimeError::HttpClientBuildFailed
        }
    }
}
