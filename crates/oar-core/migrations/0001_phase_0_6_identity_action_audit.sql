-- Phase 0.6 persistence draft for identity, device sync, execution ledger, and audit.
-- This migration is intentionally Postgres-first and driver-agnostic.

CREATE TABLE tenants (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE oar_users (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    display_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'disabled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id)
);

CREATE TABLE lark_identities (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'bot', 'app', 'service')),
    actor_external_id TEXT NOT NULL,
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, actor_kind, actor_external_id)
);

CREATE TABLE token_grants (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    identity_id TEXT NOT NULL,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'bot', 'app', 'service')),
    scope_boundary TEXT NOT NULL CHECK (scope_boundary IN ('tenant', 'user', 'admin', 'bot', 'service')),
    scopes TEXT[] NOT NULL DEFAULT '{}',
    state TEXT NOT NULL CHECK (state IN ('valid', 'needs_refresh', 'expired', 'revoked', 'reauth_required')),
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ,
    refreshed_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    reauth_required_at TIMESTAMPTZ,
    last_refresh_error TEXT,
    encrypted_oauth_grant BYTEA NOT NULL,
    oauth_grant_key_id TEXT NOT NULL,
    oauth_grant_fingerprint TEXT NOT NULL,
    revocation_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, identity_id) REFERENCES lark_identities(tenant_id, id)
);

CREATE INDEX idx_token_grants_identity_state ON token_grants (identity_id, state);
CREATE INDEX idx_token_grants_tenant_state ON token_grants (tenant_id, state);

CREATE TABLE device_sessions (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    user_id TEXT NOT NULL,
    entry_point TEXT NOT NULL CHECK (entry_point IN ('macos', 'ios', 'web', 'lark')),
    state TEXT NOT NULL CHECK (state IN ('active', 'revoked', 'expired')),
    sync_stream TEXT NOT NULL,
    sync_cursor_value BIGINT NOT NULL DEFAULT 0 CHECK (sync_cursor_value >= 0),
    sync_cursor_updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    session_identity_hash TEXT NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    expired_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, user_id) REFERENCES oar_users(tenant_id, id),
    UNIQUE (tenant_id, session_identity_hash)
);

CREATE INDEX idx_device_sessions_user_state ON device_sessions (tenant_id, user_id, state);
CREATE INDEX idx_device_sessions_sync_cursor ON device_sessions (tenant_id, sync_stream, sync_cursor_value);

CREATE TABLE confirmed_actions (
    action_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    actor_user_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('proposed', 'confirmed', 'executing', 'succeeded', 'failed', 'cancelled')),
    confirmed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, actor_user_id) REFERENCES oar_users(tenant_id, id),
    UNIQUE (tenant_id, action_id),
    UNIQUE (tenant_id, idempotency_key)
);

CREATE INDEX idx_confirmed_actions_actor_status ON confirmed_actions (tenant_id, actor_user_id, status);

CREATE TABLE operation_ledger (
    operation_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    action_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('proposed', 'confirmed', 'executing', 'succeeded', 'failed', 'cancelled')),
    last_error TEXT,
    executing_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, action_id) REFERENCES confirmed_actions(tenant_id, action_id),
    UNIQUE (tenant_id, operation_id),
    UNIQUE (tenant_id, idempotency_key)
);

CREATE INDEX idx_operation_ledger_action ON operation_ledger (tenant_id, action_id);
CREATE INDEX idx_operation_ledger_status ON operation_ledger (tenant_id, status);

CREATE TABLE audit_events (
    event_id TEXT PRIMARY KEY,
    trace_id TEXT NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence > 0),
    occurred_at_ms BIGINT NOT NULL CHECK (occurred_at_ms >= 0),
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'bot', 'app', 'system', 'service')),
    actor_id TEXT NOT NULL,
    actor_display_name TEXT,
    target_resource_type TEXT NOT NULL,
    target_resource_id TEXT NOT NULL,
    target_action_type TEXT NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'proposed_action_decision_recorded',
            'confirmed_action_recorded',
            'dry_run_executed',
            'execution_denied',
            'execution_succeeded',
            'execution_failed'
        )
    ),
    before_summary JSONB,
    after_summary JSONB,
    execution_result JSONB,
    operation_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, operation_id) REFERENCES operation_ledger(tenant_id, operation_id),
    UNIQUE (tenant_id, trace_id, sequence)
);

CREATE INDEX idx_audit_events_trace_sequence ON audit_events (tenant_id, trace_id, sequence);
CREATE INDEX idx_audit_events_tenant_time ON audit_events (tenant_id, occurred_at_ms);
CREATE INDEX idx_audit_events_target ON audit_events (tenant_id, target_resource_type, target_resource_id);

CREATE OR REPLACE FUNCTION prevent_audit_event_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'audit_events are append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER audit_events_no_update
BEFORE UPDATE ON audit_events
FOR EACH ROW EXECUTE FUNCTION prevent_audit_event_mutation();

CREATE TRIGGER audit_events_no_delete
BEFORE DELETE ON audit_events
FOR EACH ROW EXECUTE FUNCTION prevent_audit_event_mutation();

CREATE TABLE audit_outbox (
    id BIGSERIAL PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    stream TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'sent', 'failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at TIMESTAMPTZ
);

CREATE INDEX idx_audit_outbox_pending ON audit_outbox (status, next_attempt_at, created_at);
CREATE INDEX idx_audit_outbox_tenant_stream_pending
ON audit_outbox (tenant_id, stream, status, next_attempt_at, created_at, id);
