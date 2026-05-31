use std::error::Error;
use std::fmt;
use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;

use crate::agent::{AgentModelSettingsRuntime, AgentRuntime, AgentRuntimeConfigError};
use crate::feishu_auth::{FeishuLoginRuntime, FeishuLoginRuntimeConfigError};
use crate::persistence::{FacadePersistenceConfig, FacadePersistenceRuntime};
use crate::tenant_maintenance::{
    tenant_maintenance_runtime_settings_from_env_map, TenantMaintenanceRuntimeSettings,
    TenantMaintenanceSettingsError,
};
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
pub enum OarHttpFacadeRuntimeError {
    PartialFeishuAuthConfig,
    InvalidFeishuOpenApiConfig,
    InvalidFeishuLoginConfig,
    InvalidPersistenceConfig,
    PartialAgentConfig,
    InvalidAgentConfig,
    TenantMaintenanceRequiresDatabase,
    TenantMaintenanceRequiresFeishuAuth,
    TenantMaintenanceRequiresAuditOutboxSink,
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
            Self::TenantMaintenanceRequiresAuditOutboxSink => {
                write!(f, "oar_tenant_maintenance_audit_outbox_sink_required")
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
            persistence.is_some(),
            feishu_login.is_some(),
        )
        .map_err(tenant_maintenance_settings_error)?;
        Ok(Self {
            persistence,
            feishu_login,
            agent,
            agent_settings,
            tenant_maintenance,
        })
    }
}

fn tenant_maintenance_settings_error(
    error: TenantMaintenanceSettingsError,
) -> OarHttpFacadeRuntimeError {
    match error {
        TenantMaintenanceSettingsError::RequiresDatabase => {
            OarHttpFacadeRuntimeError::TenantMaintenanceRequiresDatabase
        }
        TenantMaintenanceSettingsError::RequiresFeishuAuth => {
            OarHttpFacadeRuntimeError::TenantMaintenanceRequiresFeishuAuth
        }
        TenantMaintenanceSettingsError::RequiresAuditOutboxSink => {
            OarHttpFacadeRuntimeError::TenantMaintenanceRequiresAuditOutboxSink
        }
        TenantMaintenanceSettingsError::MissingInstanceId => {
            OarHttpFacadeRuntimeError::TenantMaintenanceMissingInstanceId
        }
        TenantMaintenanceSettingsError::InvalidConfig => {
            OarHttpFacadeRuntimeError::InvalidTenantMaintenanceConfig
        }
    }
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
