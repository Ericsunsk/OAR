use super::super::*;

pub(super) fn validate_token_refresh_plan(
    planned: &TokenRefreshPlannedCommand,
) -> PgRepositoryResult<()> {
    let expected_command_kind = planned.command.kind();
    if planned.report.command_kind != expected_command_kind {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "command_kind",
            expected: format!("{expected_command_kind:?}"),
            actual: format!("{:?}", planned.report.command_kind),
        });
    }

    if planned.report.tenant_id != *planned.tenant_id() {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "tenant_id",
            expected: planned.tenant_id().0.clone(),
            actual: planned.report.tenant_id.0.clone(),
        });
    }

    if planned.report.grant_id != *planned.grant_id() {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "grant_id",
            expected: planned.grant_id().0.clone(),
            actual: planned.report.grant_id.0.clone(),
        });
    }

    Ok(())
}

fn token_refresh_apply_result_from_record(
    record: EncryptedTokenGrantRecord,
) -> TokenRefreshApplyResult {
    TokenRefreshApplyResult {
        grant_id: crate::domain::identity::TokenGrantId(record.id),
        tenant_id: crate::domain::identity::TenantId(record.tenant_id),
        state: record.state,
        fingerprint: record.oauth_grant_fingerprint,
    }
}

pub(in crate::storage::postgres::repository_sqlx) async fn apply_refresh_command_with_executor<
    'e,
    E,
>(
    executor: E,
    command: TokenRefreshRepositoryCommand,
) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let row = match command {
        TokenRefreshRepositoryCommand::RotateGrantCas {
            grant_id,
            tenant_id,
            expected_fingerprint,
            expires_at_ms,
            refreshed_at_ms,
            encrypted_grant_blob,
            grant_key_id,
            new_fingerprint,
        } => {
            sqlx::query(ROTATE_TOKEN_GRANT)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(option_u64_to_i64(expires_at_ms))
                .bind(refreshed_at_ms as i64)
                .bind(&encrypted_grant_blob.0)
                .bind(&grant_key_id)
                .bind(&new_fingerprint)
                .fetch_optional(executor)
                .await?
        }
        TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            grant_id,
            tenant_id,
            expected_fingerprint,
            refreshed_at_ms,
            safe_error,
        } => {
            let safe_error = sanitize_refresh_error_for_storage(&safe_error);
            sqlx::query(MARK_TOKEN_GRANT_REFRESH_FAILED)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(refreshed_at_ms as i64)
                .bind(&safe_error)
                .fetch_optional(executor)
                .await?
        }
        TokenRefreshRepositoryCommand::MarkReauthRequired {
            grant_id,
            tenant_id,
            expected_fingerprint,
            reauth_required_at_ms,
            safe_error,
        } => {
            let safe_error = sanitize_refresh_error_for_storage(&safe_error);
            sqlx::query(MARK_TOKEN_GRANT_REAUTH_REQUIRED)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(reauth_required_at_ms as i64)
                .bind(&safe_error)
                .fetch_optional(executor)
                .await?
        }
        TokenRefreshRepositoryCommand::MarkConfigRequired {
            grant_id,
            tenant_id,
            expected_fingerprint,
            refreshed_at_ms,
            safe_error,
        } => {
            let safe_error = sanitize_refresh_error_for_storage(&safe_error);
            sqlx::query(MARK_TOKEN_GRANT_REFRESH_FAILED)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(refreshed_at_ms as i64)
                .bind(&safe_error)
                .fetch_optional(executor)
                .await?
        }
    };

    row.as_ref().map(encrypted_token_grant_from_row).transpose()
}

pub(in crate::storage::postgres::repository_sqlx) async fn apply_refresh_command_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    command: TokenRefreshRepositoryCommand,
) -> PgRepositoryResult<Option<TokenRefreshApplyResult>> {
    apply_refresh_command_with_executor(&mut **tx, command)
        .await
        .map(|value| value.map(token_refresh_apply_result_from_record))
}
