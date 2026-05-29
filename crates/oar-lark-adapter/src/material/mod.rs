mod blob;
mod provider;
mod types;

pub use blob::{
    compose_encrypted_grant_blob, read_access_token_from_encrypted_grant, GrantAccessTokenReadError,
};
pub use provider::{
    AesGcmRefreshMaterialProvider, AesGcmRefreshMaterialProviderError,
    FeishuStoredRefreshMaterialProvider, FeishuStoredRefreshMaterialProviderError,
};
pub use types::{
    AesGcmKeyResolver, AsyncAesGcmKeyResolver, AsyncFeishuGrantMaterialStore,
    DecryptedFeishuGrantMaterial, FeishuGrantMaterialStore, StoredFeishuGrantMaterial,
};

#[cfg(test)]
mod tests;
