use std::error::Error;
use std::fmt;
use std::sync::Arc;

use oar_lark_adapter::PostgresFeishuAuthRefreshEnvConfig;
use sqlx::postgres::PgPoolOptions;

use crate::agent::{AgentModelSettingsRuntime, AgentRuntime, AgentRuntimeConfigError};
use crate::feishu_auth::{
    FeishuGrantPersistenceRuntime, FeishuLoginRuntime, FeishuLoginRuntimeConfigError,
};
use crate::util::non_empty_env;

#[derive(Clone, Default)]
pub struct OarHttpFacadeRuntime {
    pub(crate) feishu_login: Option<Arc<FeishuLoginRuntime>>,
    pub(crate) agent: Option<Arc<AgentRuntime>>,
    pub(crate) agent_settings: Option<Arc<AgentModelSettingsRuntime>>,
}

impl fmt::Debug for OarHttpFacadeRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OarHttpFacadeRuntime")
            .field("feishu_login", &self.feishu_login.is_some())
            .field("agent", &self.agent.is_some())
            .field("agent_settings", &self.agent_settings.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OarHttpFacadeRuntimeError {
    PartialFeishuAuthConfig,
    InvalidFeishuOpenApiConfig,
    InvalidFeishuLoginConfig,
    InvalidFeishuGrantConfig,
    PartialAgentConfig,
    InvalidAgentConfig,
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
            Self::InvalidFeishuGrantConfig => {
                write!(f, "oar_feishu_grant_config_invalid")
            }
            Self::PartialAgentConfig => write!(f, "oar_agent_config_partial"),
            Self::InvalidAgentConfig => write!(f, "oar_agent_config_invalid"),
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

    pub fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        Self::from_env_map_with_persistence(env, None)
    }

    pub async fn from_env_map_async(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        let runtime = Self::from_env_map(env)?;
        if runtime.feishu_login.is_none() {
            return Ok(runtime);
        }

        let Some(database_url) = non_empty_env(env, "DATABASE_URL") else {
            return Ok(runtime);
        };
        let grant_config = PostgresFeishuAuthRefreshEnvConfig::from_env_map(env)
            .map_err(|_| OarHttpFacadeRuntimeError::InvalidFeishuGrantConfig)?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .map_err(|_| OarHttpFacadeRuntimeError::DatabaseConnectFailed)?;
        Self::from_env_map_with_persistence(
            env,
            Some(FeishuGrantPersistenceRuntime::new(
                pool,
                grant_config.grant_key_id,
                grant_config.grant_key_material,
            )),
        )
    }

    fn from_env_map_with_persistence(
        env: &impl Fn(&str) -> Option<String>,
        grant_persistence: Option<FeishuGrantPersistenceRuntime>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        let agent = AgentRuntime::from_env_map(env)
            .map_err(agent_runtime_config_error)?
            .map(Arc::new);
        let agent_settings = match grant_persistence.as_ref() {
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
        let feishu_login = FeishuLoginRuntime::from_env_map(env, grant_persistence)
            .map_err(feishu_runtime_config_error)?
            .map(Arc::new);
        Ok(Self {
            feishu_login,
            agent,
            agent_settings,
        })
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
