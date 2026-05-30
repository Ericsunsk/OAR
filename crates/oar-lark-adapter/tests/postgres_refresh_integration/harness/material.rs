use oar_lark_adapter::{
    AesGcmGrantEncryptor, AesGcmKeyResolver, FeishuGrantEncryptionInput, FeishuGrantEncryptor,
    GrantTimeSource, PostgresFeishuGrantMaterialStore, SecretString,
    StaticFeishuAppCredentialProvider,
};
use sqlx::PgPool;

use super::constants::{CLIENT_SECRET, GRANT_ID, KEY_ID, TENANT_ID};

#[derive(Clone)]
pub(crate) struct FixedKeyResolver {
    pub(crate) key: [u8; 32],
}

impl AesGcmKeyResolver for FixedKeyResolver {
    type Error = std::convert::Infallible;

    fn key_for(&mut self, key_id: &str) -> Result<[u8; 32], Self::Error> {
        assert_eq!(key_id, KEY_ID);
        Ok(self.key)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FixedClock {
    pub(crate) now_ms: u64,
}

impl GrantTimeSource for FixedClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

pub(crate) fn encrypted_blob_from_plaintext(
    key: [u8; 32],
    now_ms: u64,
    access_token: &str,
    refresh_token: &str,
) -> Vec<u8> {
    let mut encryptor = AesGcmGrantEncryptor::with_clock(KEY_ID, key, FixedClock { now_ms });
    let envelope = FeishuGrantEncryptor::encrypt(
        &mut encryptor,
        FeishuGrantEncryptionInput {
            grant_id: GRANT_ID.to_string(),
            tenant_id: TENANT_ID.to_string(),
            expected_fingerprint: "seed-fingerprint".to_string(),
            access_token: SecretString::new(access_token),
            refresh_token: SecretString::new(refresh_token),
            expires_in_seconds: 60,
            refresh_token_expires_in_seconds: Some(120),
            token_type: Some("Bearer".to_string()),
            scope: Some("offline_access auth:user.id:read okr.progress.write".to_string()),
        },
    )
    .expect("seed grant encryption should succeed");

    oar_lark_adapter::material::compose_encrypted_grant_blob(
        envelope.encrypted_primary,
        envelope.encrypted_renewal,
    )
}

pub(crate) fn make_material_provider(
    pool: PgPool,
    key: [u8; 32],
) -> oar_lark_adapter::FeishuStoredRefreshMaterialProvider<
    PostgresFeishuGrantMaterialStore,
    FixedKeyResolver,
    StaticFeishuAppCredentialProvider,
> {
    oar_lark_adapter::FeishuStoredRefreshMaterialProvider::new(
        PostgresFeishuGrantMaterialStore::new(pool),
        FixedKeyResolver { key },
        StaticFeishuAppCredentialProvider::new("cli_test", SecretString::new(CLIENT_SECRET)),
    )
}
