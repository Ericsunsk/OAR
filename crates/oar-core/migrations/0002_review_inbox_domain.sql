-- Review inbox domain persistence for Evidence -> ProposedAction -> Decision.
-- Stores summaries, references, hashes, and projections; raw evidence bodies stay out.

CREATE TABLE evidence_items (
    id TEXT NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    summary TEXT NOT NULL CHECK (length(btrim(summary)) > 0),
    source_kind TEXT NOT NULL CHECK (
        source_kind IN (
            'okr_progress',
            'lark_minutes',
            'lark_doc',
            'manual_review_note',
            'audit_event'
        )
    ),
    source_id TEXT NOT NULL CHECK (length(btrim(source_id)) > 0),
    locator TEXT,
    content_hash TEXT NOT NULL CHECK (content_hash ~ '^sha256:[0-9A-Fa-f]{64}$'),
    visibility_scope TEXT NOT NULL CHECK (visibility_scope IN ('tenant', 'team', 'user')),
    observed_at TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id)
);

CREATE INDEX idx_evidence_items_tenant_source ON evidence_items (tenant_id, source_kind, source_id);
CREATE INDEX idx_evidence_items_tenant_visibility ON evidence_items (tenant_id, visibility_scope);

CREATE TABLE proposed_actions (
    id TEXT NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    actor_user_id TEXT NOT NULL,
    target_user_id TEXT,
    owner_user_id TEXT,
    version BIGINT NOT NULL CHECK (version > 0),
    status TEXT NOT NULL CHECK (status IN ('draft', 'published', 'superseded', 'withdrawn')),
    kind TEXT NOT NULL CHECK (
        kind IN (
            'create_kr_progress',
            'update_kr_progress',
            'delete_kr_progress_dry_run',
            'custom'
        )
    ),
    custom_kind TEXT,
    risk_severity TEXT NOT NULL CHECK (risk_severity IN ('low', 'medium', 'high', 'critical')),
    suggested_payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id, version),
    FOREIGN KEY (tenant_id, actor_user_id)
        REFERENCES oar_users(tenant_id, id),
    FOREIGN KEY (tenant_id, target_user_id)
        REFERENCES oar_users(tenant_id, id),
    FOREIGN KEY (tenant_id, owner_user_id)
        REFERENCES oar_users(tenant_id, id),
    CHECK ((kind = 'custom') = (custom_kind IS NOT NULL))
);

CREATE INDEX idx_proposed_actions_inbox ON proposed_actions (tenant_id, actor_user_id, status, updated_at);
CREATE INDEX idx_proposed_actions_owner ON proposed_actions (tenant_id, owner_user_id, status);

CREATE TABLE proposed_action_evidence_refs (
    proposed_action_id TEXT NOT NULL,
    evidence_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    proposed_action_version BIGINT NOT NULL CHECK (proposed_action_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, proposed_action_id, proposed_action_version, evidence_id),
    FOREIGN KEY (tenant_id, proposed_action_id, proposed_action_version)
        REFERENCES proposed_actions(tenant_id, id, version) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, evidence_id)
        REFERENCES evidence_items(tenant_id, id)
);

CREATE INDEX idx_proposed_action_evidence_refs_tenant ON proposed_action_evidence_refs (tenant_id, evidence_id);

CREATE TABLE proposed_action_decisions (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    proposed_action_id TEXT NOT NULL,
    proposed_action_version BIGINT NOT NULL CHECK (proposed_action_version > 0),
    actor_user_id TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('confirm', 'edit_then_confirm', 'reject')),
    edited_payload JSONB,
    confirmed_action_id TEXT,
    decided_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, proposed_action_id, proposed_action_version),
    FOREIGN KEY (tenant_id, actor_user_id)
        REFERENCES oar_users(tenant_id, id),
    FOREIGN KEY (tenant_id, proposed_action_id, proposed_action_version)
        REFERENCES proposed_actions(tenant_id, id, version),
    FOREIGN KEY (tenant_id, confirmed_action_id)
        REFERENCES confirmed_actions(tenant_id, action_id)
        DEFERRABLE INITIALLY DEFERRED,
    CHECK (
        (decision = 'reject' AND confirmed_action_id IS NULL)
        OR (decision IN ('confirm', 'edit_then_confirm') AND confirmed_action_id IS NOT NULL)
    ),
    CHECK (
        (decision = 'edit_then_confirm' AND edited_payload IS NOT NULL)
        OR (decision IN ('confirm', 'reject') AND edited_payload IS NULL)
    )
);

CREATE INDEX idx_proposed_action_decisions_actor ON proposed_action_decisions (tenant_id, actor_user_id, decided_at);

CREATE SEQUENCE review_inbox_sync_cursor_seq AS BIGINT MINVALUE 1;

CREATE TABLE review_inbox_items (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    user_id TEXT NOT NULL,
    proposed_action_id TEXT NOT NULL,
    proposed_action_version BIGINT NOT NULL CHECK (proposed_action_version > 0),
    risk_score INTEGER NOT NULL CHECK (risk_score >= 0 AND risk_score <= 100),
    priority INTEGER NOT NULL CHECK (priority >= 0),
    status TEXT NOT NULL CHECK (
        status IN (
            'open',
            'confirmed',
            'rejected',
            'executing',
            'succeeded',
            'failed',
            'withdrawn'
        )
    ),
    sort_key BIGINT NOT NULL,
    source_cursor_value BIGINT NOT NULL DEFAULT 0 CHECK (source_cursor_value >= 0),
    sync_cursor_value BIGINT NOT NULL DEFAULT nextval('review_inbox_sync_cursor_seq') CHECK (sync_cursor_value >= 0),
    updated_at TIMESTAMPTZ NOT NULL,
    ledger_status TEXT CHECK (
        ledger_status IS NULL
        OR ledger_status IN ('confirmed', 'executing', 'succeeded', 'failed', 'cancelled')
    ),
    operation_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, user_id, proposed_action_id),
    UNIQUE (tenant_id, operation_id),
    FOREIGN KEY (tenant_id, user_id)
        REFERENCES oar_users(tenant_id, id),
    FOREIGN KEY (tenant_id, proposed_action_id, proposed_action_version)
        REFERENCES proposed_actions(tenant_id, id, version),
    FOREIGN KEY (tenant_id, operation_id)
        REFERENCES operation_ledger(tenant_id, operation_id)
        DEFERRABLE INITIALLY DEFERRED
);

CREATE INDEX idx_review_inbox_items_user_sort ON review_inbox_items (tenant_id, user_id, status, sort_key DESC);
CREATE INDEX idx_review_inbox_items_sync ON review_inbox_items (tenant_id, user_id, sync_cursor_value);

CREATE TABLE scheduler_jobs (
    id TEXT NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    job_kind TEXT NOT NULL CHECK (job_kind IN ('token_refresh_sweep')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'running')),
    next_run_at TIMESTAMPTZ NOT NULL,
    lease_id TEXT,
    lease_until TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_started_at TIMESTAMPTZ,
    last_finished_at TIMESTAMPTZ,
    last_safe_error_code TEXT CHECK (
        last_safe_error_code IS NULL
        OR (
            last_safe_error_code ~ '^[a-z0-9_:.-]{1,64}$'
            AND last_safe_error_code !~* '(access[_ -]?token|refresh[_ -]?token|authorization|bearer|client_secret|authorization_code|oauth_grant|fingerprint|encrypted)'
        )
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, job_kind),
    UNIQUE (tenant_id, id),
    CHECK (
        (status = 'running' AND lease_id IS NOT NULL AND lease_until IS NOT NULL)
        OR (status <> 'running' AND lease_id IS NULL AND lease_until IS NULL)
    )
);

CREATE INDEX idx_scheduler_jobs_due ON scheduler_jobs (tenant_id, job_kind, status, next_run_at, lease_until);
