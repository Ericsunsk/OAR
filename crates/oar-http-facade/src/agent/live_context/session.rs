use std::time::SystemTime;

use oar_core::storage::postgres::{EncryptedTokenGrantRecord, PostgresTokenGrantRepository};
use oar_lark_adapter::{
    material::read_access_token_from_encrypted_grant, FeishuCalendarReadClient,
    FeishuOkrReadClient, FeishuOpenApiConfig, FeishuTaskReadClient, ReqwestAsyncHttpClient,
    SecretString,
};

use super::grant::{
    grant_requires_refresh_before_read, live_read_grant_denial_reason,
    refresh_grant_before_live_read, resolve_grant_id_for_user, resolve_lark_open_id_for_grant,
    system_time_to_ms,
};
use crate::{AuthenticatedContext, OarHttpFacadeRuntime};

pub(super) struct LiveFeishuReadSession {
    pool: sqlx::PgPool,
    token_grant: EncryptedTokenGrantRecord,
    access_token: SecretString,
    open_api_config: FeishuOpenApiConfig,
    http_client: ReqwestAsyncHttpClient,
    now: SystemTime,
}

impl LiveFeishuReadSession {
    pub(super) async fn open<F>(
        runtime: &OarHttpFacadeRuntime,
        auth_context: &AuthenticatedContext,
        mut gate_scopes: F,
    ) -> Result<Self, LiveFeishuReadSessionError>
    where
        F: FnMut(&[String]) -> bool,
    {
        let Some(persistence) = runtime.session_persistence() else {
            return Err(LiveFeishuReadSessionError::evidence_degraded(
                "后端未配置 Feishu 授权存储",
            ));
        };

        let pool = persistence.pool();
        let grant_id = resolve_grant_id_for_user(&pool, auth_context)
            .await
            .map_err(LiveFeishuReadSessionError::evidence_degraded)?;

        let token_grant = PostgresTokenGrantRepository::new(pool.clone())
            .get_by_id(&auth_context.tenant_id, &grant_id)
            .await
            .map_err(|_| LiveFeishuReadSessionError::evidence_degraded("读取授权 grant 失败"))?
            .ok_or_else(|| LiveFeishuReadSessionError::evidence_degraded("未找到用户授权 grant"))?;

        validate_grant_for_live_read(&token_grant, persistence.grant_key_id())?;
        if !gate_scopes(&token_grant.scopes) {
            return Err(LiveFeishuReadSessionError::DemandRejected);
        }

        let mut token_grant = token_grant;
        let now = SystemTime::now();
        let now_ms = system_time_to_ms(now);
        if grant_requires_refresh_before_read(&token_grant, now_ms) {
            let Some(login) = runtime.feishu_login.as_ref() else {
                return Err(LiveFeishuReadSessionError::evidence_degraded(
                    "后端未配置 Feishu 授权刷新",
                ));
            };
            token_grant = refresh_grant_before_live_read(
                pool.clone(),
                login,
                persistence,
                auth_context,
                &token_grant,
                now,
                now_ms,
            )
            .await
            .map_err(|error| LiveFeishuReadSessionError::evidence_degraded(error.safe_reason()))?;
        }

        validate_grant_for_live_read(&token_grant, persistence.grant_key_id())?;
        if !gate_scopes(&token_grant.scopes) {
            return Err(LiveFeishuReadSessionError::DemandRejected);
        }

        let access_token = read_access_token_from_encrypted_grant(
            &token_grant.encrypted_oauth_grant,
            persistence.grant_key_material(),
        )
        .map_err(|_| LiveFeishuReadSessionError::evidence_degraded("授权令牌解密失败"))?;

        let open_api_config = runtime
            .feishu_login
            .as_ref()
            .map(|login| login.open_api_config())
            .unwrap_or_default();
        let http_client = ReqwestAsyncHttpClient::with_config(&open_api_config).map_err(|_| {
            LiveFeishuReadSessionError::evidence_degraded("Feishu HTTP 客户端初始化失败")
        })?;

        Ok(Self {
            pool,
            token_grant,
            access_token,
            open_api_config,
            http_client,
            now,
        })
    }

    pub(super) fn access_token(&self) -> SecretString {
        self.access_token.clone()
    }

    pub(super) fn now(&self) -> SystemTime {
        self.now
    }

    pub(super) fn okr_client(&self) -> FeishuOkrReadClient<ReqwestAsyncHttpClient> {
        FeishuOkrReadClient::new(self.open_api_config.clone(), self.http_client.clone())
    }

    pub(super) fn task_client(&self) -> FeishuTaskReadClient<ReqwestAsyncHttpClient> {
        FeishuTaskReadClient::new(self.open_api_config.clone(), self.http_client.clone())
    }

    pub(super) fn calendar_client(&self) -> FeishuCalendarReadClient<ReqwestAsyncHttpClient> {
        FeishuCalendarReadClient::new(self.open_api_config.clone(), self.http_client.clone())
    }

    pub(super) async fn resolve_lark_open_id(
        &self,
        auth_context: &AuthenticatedContext,
    ) -> Result<String, &'static str> {
        resolve_lark_open_id_for_grant(&self.pool, auth_context, &self.token_grant).await
    }
}

pub(super) enum LiveFeishuReadSessionError {
    DemandRejected,
    Degraded(String),
}

impl LiveFeishuReadSessionError {
    fn evidence_degraded(reason: impl Into<String>) -> Self {
        Self::Degraded(format!("未读取到实时 Feishu 证据：{}。", reason.into()))
    }

    pub(super) fn push_degraded(self, degraded: &mut Vec<String>) {
        if let Self::Degraded(summary) = self {
            degraded.push(summary);
        }
    }
}

fn validate_grant_for_live_read(
    token_grant: &EncryptedTokenGrantRecord,
    expected_grant_key_id: &str,
) -> Result<(), LiveFeishuReadSessionError> {
    if let Some(reason) = live_read_grant_denial_reason(token_grant) {
        return Err(LiveFeishuReadSessionError::evidence_degraded(reason));
    }

    if token_grant.oauth_grant_key_id != expected_grant_key_id {
        return Err(LiveFeishuReadSessionError::evidence_degraded(
            "授权密钥版本不匹配",
        ));
    }

    Ok(())
}
