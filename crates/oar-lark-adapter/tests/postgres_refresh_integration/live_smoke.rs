use std::env;
use std::time::{Duration, UNIX_EPOCH};

use oar_core::action::audit_event::{AuditEventType, ExecutionStatus};
use oar_core::domain::identity::TokenGrantState;
use oar_core::domain::token_refresh::types::{TokenRefreshCommandKind, TokenRefreshReportStatus};
use oar_core::storage::postgres::{
    PostgresAuditEventRepository, PostgresTokenGrantRepository, PostgresTokenRefreshOrchestrator,
};
use oar_lark_adapter::{
    AesGcmGrantEncryptor, FeishuGrantEncryptionInput, FeishuGrantEncryptor, FeishuOpenApiConfig,
    PostgresFeishuAuthRefreshEnvConfig, PostgresFeishuAuthRefreshEnvConfigError, SecretString,
};

use super::harness::{
    assert_no_byte_secret, assert_no_sensitive_text, audit_context, run_live_postgres_test,
    seed_identity_graph, seed_refresh_candidate_grant_with_key_id_and_scopes, ACTOR_ID, GRANT_ID,
    OLD_FP, SEED_ACCESS_TOKEN, TENANT_ID, TRACE_ID,
};

const LIVE_SMOKE_FLAG: &str = "OAR_TEST_FEISHU_REFRESH_SMOKE_ENABLED";
const LIVE_REFRESH_TOKEN_ENV: &str = "OAR_TEST_FEISHU_REFRESH_TOKEN";
const LIVE_SCOPE_ENV: &str = "OAR_TEST_FEISHU_REFRESH_SCOPE";

#[test]
fn postgres_live_feishu_adapter_refresh_smoke_rotates_real_grant_and_audits() {
    let live_env = match LiveRefreshSmokeEnv::from_process_env() {
        Ok(Some(live_env)) => live_env,
        Ok(None) => return,
        Err(error) => panic!("live Feishu refresh smoke env is incomplete: {error}"),
    };

    run_live_postgres_test(
        "adapter_live_feishu_refresh_smoke",
        move |pool| async move {
            seed_identity_graph(&pool).await?;
            let refresh_started_at_ms = current_time_ms();

            let initial_blob = encrypted_live_blob(&live_env);
            seed_refresh_candidate_grant_with_key_id_and_scopes(
                &pool,
                GRANT_ID,
                live_env.refresh_config.grant_key_id.as_str(),
                live_env.scopes.clone(),
                initial_blob.clone(),
            )
            .await?;

            let adapter = oar_lark_adapter::build_postgres_async_feishu_auth_refresh_adapter(
                pool.clone(),
                live_env.open_api_config.clone(),
                live_env.refresh_config.app_id.clone(),
                live_env.refresh_config.app_secret.clone(),
                live_env.refresh_config.grant_key_id.clone(),
                live_env.refresh_config.grant_key_material,
            )?;
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter);
            let snapshot = PostgresTokenGrantRepository::new(pool.clone())
                .list_refresh_candidate_snapshots(
                    TENANT_ID,
                    UNIX_EPOCH + Duration::from_millis(1_779_466_500_000),
                    1,
                )
                .await?
                .pop()
                .expect("seeded live grant should be due");

            let report = orchestrator
                .refresh_grant_with_audit(
                    snapshot,
                    UNIX_EPOCH + Duration::from_millis(1_779_466_000_000),
                    audit_context(TRACE_ID, 23),
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert_eq!(
                report.service_report.command,
                Some(TokenRefreshCommandKind::RotateGrantCas)
            );
            assert_eq!(report.service_report.safe_error, None);
            assert!(report.service_report.adapter_called);
            assert!(report.service_report.sink_called);
            assert_eq!(report.event.event_type, AuditEventType::ExecutionSucceeded);
            assert_eq!(report.event.target.resource_type, "token_grant");
            assert_eq!(report.event.target.resource_id, GRANT_ID);
            assert_eq!(report.event.target.action_type, "token_refresh.rotate");
            assert_eq!(
                report
                    .event
                    .execution
                    .as_ref()
                    .map(|execution| &execution.status),
                Some(&ExecutionStatus::Succeeded)
            );
            assert_eq!(report.event.actor.actor_id, ACTOR_ID);

            let rotated = PostgresTokenGrantRepository::new(pool.clone())
                .get_by_id(TENANT_ID, GRANT_ID)
                .await?
                .expect("live grant should still exist");
            assert_eq!(rotated.state, TokenGrantState::Valid);
            assert_eq!(
                rotated.oauth_grant_key_id,
                live_env.refresh_config.grant_key_id
            );
            assert_ne!(rotated.oauth_grant_fingerprint, OLD_FP);
            assert_ne!(rotated.encrypted_oauth_grant, initial_blob);
            assert_eq!(rotated.last_refresh_error, None);
            let refreshed_at_ms = rotated
                .refreshed_at_ms
                .expect("live refresh should record refreshed_at_ms");
            assert!(refreshed_at_ms >= refresh_started_at_ms);
            assert!(refreshed_at_ms <= current_time_ms());
            assert!(rotated.expires_at_ms.unwrap_or_default() > refreshed_at_ms);
            assert_no_byte_secret(&rotated.encrypted_oauth_grant);
            assert_no_live_secret_bytes(&rotated.encrypted_oauth_grant, &live_env);

            let audit_events = PostgresAuditEventRepository::new(pool.clone())
                .find_by_tenant_and_trace_id(TENANT_ID, TRACE_ID)
                .await?;
            assert_eq!(audit_events, vec![report.event.clone()]);
            let audit_text = serde_json::to_string(&audit_events)?;
            assert_no_sensitive_text(&audit_text);
            assert_no_live_secret_text(&audit_text, &live_env);
            assert_no_sensitive_text(&format!("{report:?}"));
            assert_no_live_secret_text(&format!("{report:?}"), &live_env);

            Ok(())
        },
    );
}

#[derive(Clone)]
struct LiveRefreshSmokeEnv {
    refresh_token: String,
    scopes: Vec<String>,
    open_api_config: FeishuOpenApiConfig,
    refresh_config: PostgresFeishuAuthRefreshEnvConfig,
}

impl LiveRefreshSmokeEnv {
    fn from_process_env() -> Result<Option<Self>, LiveRefreshSmokeEnvError> {
        if !env_flag(LIVE_SMOKE_FLAG) {
            eprintln!("skip adapter_live_feishu_refresh_smoke: {LIVE_SMOKE_FLAG} is not enabled");
            return Ok(None);
        }

        let missing = required_env_keys()
            .iter()
            .copied()
            .filter(|key| optional_env(key).is_none())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(LiveRefreshSmokeEnvError::MissingRequired(missing));
        }

        let open_api_config = FeishuOpenApiConfig::from_env_map(&|key| optional_env(key))
            .map_err(|_| LiveRefreshSmokeEnvError::InvalidOpenApiConfig)?;
        let refresh_config =
            PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| optional_env(key))
                .map_err(LiveRefreshSmokeEnvError::InvalidRefreshAdapterConfig)?;
        Ok(Some(Self {
            refresh_token: optional_env(LIVE_REFRESH_TOKEN_ENV).expect("presence checked"),
            scopes: optional_env(LIVE_SCOPE_ENV)
                .map(|scope| {
                    scope
                        .split_whitespace()
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            open_api_config,
            refresh_config,
        }))
    }
}

#[derive(Debug)]
enum LiveRefreshSmokeEnvError {
    MissingRequired(Vec<&'static str>),
    InvalidRefreshAdapterConfig(PostgresFeishuAuthRefreshEnvConfigError),
    InvalidOpenApiConfig,
}

impl std::fmt::Display for LiveRefreshSmokeEnvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequired(keys) => write!(f, "missing {}", keys.join(", ")),
            Self::InvalidRefreshAdapterConfig(error) => {
                write!(f, "invalid refresh adapter env config: {error:?}")
            }
            Self::InvalidOpenApiConfig => write!(f, "invalid Feishu OpenAPI config"),
        }
    }
}

impl std::error::Error for LiveRefreshSmokeEnvError {}

fn required_env_keys() -> [&'static str; 6] {
    [
        "DATABASE_URL",
        "OAR_FEISHU_APP_ID",
        "OAR_FEISHU_APP_SECRET",
        "OAR_GRANT_KEY_ID",
        "OAR_GRANT_KEY_HEX",
        LIVE_REFRESH_TOKEN_ENV,
    ]
}

fn optional_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn env_flag(key: &str) -> bool {
    optional_env(key)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn encrypted_live_blob(live_env: &LiveRefreshSmokeEnv) -> Vec<u8> {
    let mut encryptor = AesGcmGrantEncryptor::new(
        live_env.refresh_config.grant_key_id.clone(),
        live_env.refresh_config.grant_key_material,
    );
    let envelope = FeishuGrantEncryptor::encrypt(
        &mut encryptor,
        FeishuGrantEncryptionInput {
            grant_id: GRANT_ID.to_string(),
            tenant_id: TENANT_ID.to_string(),
            expected_fingerprint: "seed-live-fingerprint".to_string(),
            access_token: SecretString::new(SEED_ACCESS_TOKEN),
            refresh_token: SecretString::new(live_env.refresh_token.clone()),
            expires_in_seconds: 60,
            refresh_token_expires_in_seconds: None,
            token_type: Some("Bearer".to_string()),
            scope: (!live_env.scopes.is_empty()).then(|| live_env.scopes.join(" ")),
        },
    )
    .expect("live seed grant encryption should succeed");

    oar_lark_adapter::material::compose_encrypted_grant_blob(
        envelope.encrypted_primary,
        envelope.encrypted_renewal,
    )
}

fn assert_no_live_secret_text(text: &str, live_env: &LiveRefreshSmokeEnv) {
    for needle in [
        live_env.refresh_config.app_secret.expose_secret(),
        live_env.refresh_token.as_str(),
    ] {
        assert!(
            !text.contains(needle),
            "live sensitive value leaked into text"
        );
    }
}

fn assert_no_live_secret_bytes(bytes: &[u8], live_env: &LiveRefreshSmokeEnv) {
    assert!(
        !contains_subslice(bytes, live_env.refresh_token.as_bytes()),
        "live sensitive value leaked into encrypted blob"
    );
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && needle.len() <= haystack.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis() as u64
}
