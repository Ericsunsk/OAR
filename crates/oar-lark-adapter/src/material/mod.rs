mod blob;
mod provider;
mod types;

pub use blob::compose_encrypted_grant_blob;
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
