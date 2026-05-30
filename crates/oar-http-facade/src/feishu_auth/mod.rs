use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use oar_core::action::capability::default_agent_feishu_oauth_scope_strings;
use oar_lark_adapter::{FeishuOAuthLoginConfig, FeishuOpenApiConfig, ReqwestAsyncHttpClient};
use sqlx::PgPool;

mod events;
mod handlers;
mod persistence;
mod routes;
mod session;
mod util;

pub(crate) use events::{feishu_login_session_event, feishu_login_session_event_stream_response};
#[cfg(test)]
pub(crate) use handlers::authorize_test_session;
pub(crate) use handlers::{
    complete_feishu_login_callback, create_feishu_login_session, feishu_login_session_status,
};
#[cfg(test)]
pub(crate) use persistence::{build_feishu_login_persistence_plan, FeishuLoginPersistenceError};
pub(crate) use routes::{
    auth_session_events_id, auth_session_status_id, is_auth_session_events_route,
    is_auth_session_status_route,
};
use session::FeishuLoginSession;
pub(crate) use util::iso8601_utc;

use crate::util::non_empty_env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FeishuLoginRuntimeConfigError {
    PartialAuthConfig,
    InvalidOpenApiConfig,
    InvalidLoginConfig,
    HttpClientBuildFailed,
}

impl FeishuLoginRuntime {
    pub(crate) fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
        grant_persistence: Option<FeishuGrantPersistenceRuntime>,
    ) -> Result<Option<Self>, FeishuLoginRuntimeConfigError> {
        let app_id = non_empty_env(env, "OAR_FEISHU_APP_ID");
        let app_secret = non_empty_env(env, "OAR_FEISHU_APP_SECRET");
        let redirect_uri = non_empty_env(env, "OAR_FEISHU_REDIRECT_URI");
        let has_any_auth_config =
            app_id.is_some() || app_secret.is_some() || redirect_uri.is_some();
        if !has_any_auth_config {
            return Ok(None);
        }

        let (Some(app_id), Some(app_secret), Some(redirect_uri)) =
            (app_id, app_secret, redirect_uri)
        else {
            return Err(FeishuLoginRuntimeConfigError::PartialAuthConfig);
        };

        let open_api = FeishuOpenApiConfig::from_env_map(env)
            .map_err(|_| FeishuLoginRuntimeConfigError::InvalidOpenApiConfig)?;
        let authorize_base_url = non_empty_env(env, "OAR_FEISHU_AUTHORIZE_BASE_URL")
            .unwrap_or_else(|| "https://open.feishu.cn".to_string());
        let scope = non_empty_env(env, "OAR_FEISHU_AUTH_SCOPE")
            .or_else(|| Some(default_feishu_auth_scope()));
        let config = FeishuOAuthLoginConfig::new(
            open_api.clone(),
            authorize_base_url,
            app_id,
            app_secret,
            redirect_uri,
            scope,
        )
        .map_err(|_| FeishuLoginRuntimeConfigError::InvalidLoginConfig)?;
        let http_client = ReqwestAsyncHttpClient::with_config(&open_api)
            .map_err(|_| FeishuLoginRuntimeConfigError::HttpClientBuildFailed)?;
        Ok(Some(Self {
            config,
            http_client,
            grant_persistence,
            sessions: Mutex::new(HashMap::new()),
        }))
    }

    pub(crate) fn grant_persistence(&self) -> Option<&FeishuGrantPersistenceRuntime> {
        self.grant_persistence.as_ref()
    }

    pub(crate) fn open_api_config(&self) -> FeishuOpenApiConfig {
        self.config.open_api.clone()
    }

    pub(crate) fn client_id(&self) -> &str {
        &self.config.client_id
    }

    pub(crate) fn client_secret(&self) -> oar_lark_adapter::SecretString {
        self.config.client_secret.clone()
    }
}

fn default_feishu_auth_scope() -> String {
    default_agent_feishu_oauth_scope_strings().join(" ")
}

impl FeishuGrantPersistenceRuntime {
    pub(crate) fn new(pool: PgPool, grant_key_id: String, grant_key_material: [u8; 32]) -> Self {
        Self {
            pool,
            grant_key_id,
            grant_key_material,
        }
    }

    pub(crate) fn pool(&self) -> PgPool {
        self.pool.clone()
    }

    pub(crate) fn grant_key_id(&self) -> &str {
        &self.grant_key_id
    }

    pub(crate) fn grant_key_material(&self) -> [u8; 32] {
        self.grant_key_material
    }
}

pub(crate) struct FeishuLoginRuntime {
    config: FeishuOAuthLoginConfig,
    http_client: ReqwestAsyncHttpClient,
    grant_persistence: Option<FeishuGrantPersistenceRuntime>,
    sessions: Mutex<HashMap<String, FeishuLoginSession>>,
}

impl fmt::Debug for FeishuLoginRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuLoginRuntime")
            .field("config", &self.config)
            .field("http_client", &"[REDACTED]")
            .field("grant_persistence", &self.grant_persistence.is_some())
            .field("sessions", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct FeishuGrantPersistenceRuntime {
    pool: PgPool,
    grant_key_id: String,
    grant_key_material: [u8; 32],
}

impl fmt::Debug for FeishuGrantPersistenceRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuGrantPersistenceRuntime")
            .field("pool", &"[REDACTED]")
            .field("grant_key_id", &"[REDACTED]")
            .field("grant_key_material", &"[REDACTED]")
            .finish()
    }
}
