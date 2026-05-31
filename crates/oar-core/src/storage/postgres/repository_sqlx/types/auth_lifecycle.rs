#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresAuthLogoutRevokeRequest<'a> {
    pub tenant_id: &'a str,
    pub user_id: &'a str,
    pub session_id: &'a str,
    pub grant_id_hint: Option<&'a str>,
    pub occurred_at_ms: u64,
    pub revocation_reason: &'a str,
    pub audit_trace_id: &'a str,
    pub audit_action_type: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresAuthLogoutRevokeReport {
    pub revoked_grant_ids: Vec<String>,
}
