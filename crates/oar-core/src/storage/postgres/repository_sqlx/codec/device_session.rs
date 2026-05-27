use super::super::{PgRepositoryResult, PostgresRepositoryError};
use crate::domain::device_sync::{DeviceEntryPoint, SessionState};

pub(in crate::storage::postgres::repository_sqlx) fn device_entry_point_to_db(
    value: &DeviceEntryPoint,
) -> &'static str {
    match value {
        DeviceEntryPoint::MacOs => "macos",
        DeviceEntryPoint::Ios => "ios",
        DeviceEntryPoint::Web => "web",
        DeviceEntryPoint::Lark => "lark",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn device_entry_point_from_db(
    value: &str,
) -> PgRepositoryResult<DeviceEntryPoint> {
    match value {
        "macos" => Ok(DeviceEntryPoint::MacOs),
        "ios" => Ok(DeviceEntryPoint::Ios),
        "web" => Ok(DeviceEntryPoint::Web),
        "lark" => Ok(DeviceEntryPoint::Lark),
        other => Err(PostgresRepositoryError::UnknownDeviceEntryPoint(
            other.to_string(),
        )),
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn device_session_state_to_db(
    value: &SessionState,
) -> &'static str {
    match value {
        SessionState::Active => "active",
        SessionState::Revoked => "revoked",
        SessionState::Expired => "expired",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn device_session_state_from_db(
    value: &str,
) -> PgRepositoryResult<SessionState> {
    match value {
        "active" => Ok(SessionState::Active),
        "revoked" => Ok(SessionState::Revoked),
        "expired" => Ok(SessionState::Expired),
        other => Err(PostgresRepositoryError::UnknownDeviceSessionState(
            other.to_string(),
        )),
    }
}
