use std::collections::HashSet;

pub(super) enum TenantIdValidationError {
    EmptyRegistry,
    EmptyTenantId,
    DuplicateTenantId(String),
}

pub(super) fn canonicalize_tenant_id(tenant_id: &str) -> String {
    tenant_id.trim().to_string()
}

pub(super) fn validate_tenant_ids(
    tenant_ids: Vec<String>,
) -> Result<Vec<String>, TenantIdValidationError> {
    if tenant_ids.is_empty() {
        return Err(TenantIdValidationError::EmptyRegistry);
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(tenant_ids.len());
    for tenant_id in tenant_ids {
        let tenant_id = canonicalize_tenant_id(&tenant_id);
        if tenant_id.is_empty() {
            return Err(TenantIdValidationError::EmptyTenantId);
        }
        if !seen.insert(tenant_id.clone()) {
            return Err(TenantIdValidationError::DuplicateTenantId(tenant_id));
        }
        normalized.push(tenant_id);
    }

    Ok(normalized)
}

pub(super) fn validate_tenant_ids_allow_empty(
    tenant_ids: Vec<String>,
) -> Result<Vec<String>, TenantIdValidationError> {
    if tenant_ids.is_empty() {
        Ok(Vec::new())
    } else {
        validate_tenant_ids(tenant_ids)
    }
}
