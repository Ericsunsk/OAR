-- User-owned Agent model settings. API keys are encrypted before storage.

CREATE TABLE agent_model_settings (
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    user_id TEXT NOT NULL,
    detected_protocol TEXT NOT NULL CHECK (detected_protocol IN ('openai-compatible', 'anthropic')),
    base_url TEXT NOT NULL CHECK (length(btrim(base_url)) > 0),
    selected_model TEXT NOT NULL CHECK (length(btrim(selected_model)) > 0),
    encrypted_api_key BYTEA NOT NULL CHECK (octet_length(encrypted_api_key) > 0),
    api_key_key_id TEXT NOT NULL CHECK (length(btrim(api_key_key_id)) > 0),
    api_key_fingerprint TEXT NOT NULL CHECK (api_key_fingerprint ~ '^sha256:[0-9A-Fa-f]{64}$'),
    anthropic_version TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id),
    FOREIGN KEY (tenant_id, user_id) REFERENCES workspace_users(tenant_id, id)
);

CREATE INDEX idx_agent_model_settings_protocol
ON agent_model_settings (tenant_id, detected_protocol, updated_at DESC);
