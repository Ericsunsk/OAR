use super::{
    PgRepositoryResult, PostgresRepositoryError, MAX_REFRESH_ERROR_CHARS, REDACTED_REFRESH_ERROR,
    REDACTED_TENANT_ACTUAL,
};
use serde_json::Value;
use sqlx::PgPool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) fn option_u64_to_i64(value: Option<u64>) -> Option<i64> {
    value.map(|value| value as i64)
}

pub(super) fn json_option<T: serde::Serialize>(
    value: &Option<T>,
) -> PgRepositoryResult<Option<Value>> {
    value
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(PostgresRepositoryError::from)
}

pub(super) fn json_value_option<T>(value: Option<Value>) -> PgRepositoryResult<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    value
        .map(serde_json::from_value)
        .transpose()
        .map_err(PostgresRepositoryError::from)
}

pub(super) fn non_negative_i64_to_u64(value: i64, field: &'static str) -> PgRepositoryResult<u64> {
    if value < 0 {
        return Err(PostgresRepositoryError::NegativeInteger { field, value });
    }
    Ok(value as u64)
}

pub(super) fn optional_non_negative_i64_to_u64(
    value: Option<i64>,
    field: &'static str,
) -> PgRepositoryResult<Option<u64>> {
    value
        .map(|value| non_negative_i64_to_u64(value, field))
        .transpose()
}

pub(super) fn system_time_to_ms(value: SystemTime) -> PgRepositoryResult<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|_| PostgresRepositoryError::NegativeInteger {
            field: "system_time",
            value: -1,
        })
}

pub(super) fn option_system_time_to_i64_ms(
    value: Option<SystemTime>,
) -> PgRepositoryResult<Option<i64>> {
    value
        .map(system_time_to_ms)
        .transpose()
        .map(|maybe| maybe.map(|ms| ms as i64))
}

pub(super) fn ms_to_system_time(value: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(value)
}

pub(super) fn redacted_tenant_actual() -> String {
    REDACTED_TENANT_ACTUAL.to_string()
}

pub(super) async fn tenant_mismatch_or_row_not_found<T>(
    pool: &PgPool,
    exists_by_id_sql: &'static str,
    id: &str,
    expected_tenant_id: &str,
) -> PgRepositoryResult<T> {
    let conflicting_tenant = sqlx::query(exists_by_id_sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;

    if conflicting_tenant.is_some() {
        return Err(PostgresRepositoryError::TenantMismatch {
            field: "tenant_id",
            expected: expected_tenant_id.to_string(),
            actual: redacted_tenant_actual(),
        });
    }

    Err(sqlx::Error::RowNotFound.into())
}

pub(super) fn sanitize_refresh_error_for_storage(reason: &str) -> String {
    if crate::security::contains_sensitive_marker(reason) {
        return REDACTED_REFRESH_ERROR.to_string();
    }

    let trimmed = reason.trim();
    trimmed
        .chars()
        .map(|char| if char.is_control() { ' ' } else { char })
        .take(MAX_REFRESH_ERROR_CHARS)
        .collect()
}
