use std::fmt;

#[derive(Clone)]
pub enum FeishuStoredRefreshMaterialProviderError<G, C> {
    Grant(G),
    Credential(C),
}

impl<G, C> fmt::Debug for FeishuStoredRefreshMaterialProviderError<G, C>
where
    G: fmt::Debug,
    C: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grant(_) => write!(f, "FeishuStoredRefreshMaterialProviderError(grant)"),
            Self::Credential(_) => {
                write!(f, "FeishuStoredRefreshMaterialProviderError(credential)")
            }
        }
    }
}

impl<G, C> fmt::Display for FeishuStoredRefreshMaterialProviderError<G, C>
where
    G: fmt::Display,
    C: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grant(_) => write!(f, "grant material is unavailable"),
            Self::Credential(_) => write!(f, "feishu app credential is unavailable"),
        }
    }
}

impl<G, C> std::error::Error for FeishuStoredRefreshMaterialProviderError<G, C>
where
    G: std::error::Error + 'static,
    C: std::error::Error + 'static,
{
}

#[derive(Clone)]
pub enum AesGcmRefreshMaterialProviderError<S, K> {
    Store(S),
    KeyResolver(K),
    GrantMismatch,
    FingerprintMismatch,
    MalformedGrantMaterial,
    DecryptFailed,
}

impl<S, K> fmt::Debug for AesGcmRefreshMaterialProviderError<S, K>
where
    S: fmt::Debug,
    K: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(_) => write!(f, "AesGcmRefreshMaterialProviderError(store)"),
            Self::KeyResolver(_) => write!(f, "AesGcmRefreshMaterialProviderError(key_resolver)"),
            Self::GrantMismatch => write!(f, "AesGcmRefreshMaterialProviderError(grant_mismatch)"),
            Self::FingerprintMismatch => {
                write!(
                    f,
                    "AesGcmRefreshMaterialProviderError(fingerprint_mismatch)"
                )
            }
            Self::MalformedGrantMaterial => {
                write!(
                    f,
                    "AesGcmRefreshMaterialProviderError(malformed_grant_material)"
                )
            }
            Self::DecryptFailed => write!(f, "AesGcmRefreshMaterialProviderError(decrypt_failed)"),
        }
    }
}

impl<S, K> fmt::Display for AesGcmRefreshMaterialProviderError<S, K>
where
    S: fmt::Display,
    K: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(_) => write!(f, "grant material is unavailable"),
            Self::KeyResolver(_) => write!(f, "grant key is unavailable"),
            Self::GrantMismatch => write!(f, "grant material does not match request"),
            Self::FingerprintMismatch => write!(f, "grant material fingerprint mismatch"),
            Self::MalformedGrantMaterial => write!(f, "grant material payload is invalid"),
            Self::DecryptFailed => write!(f, "grant material decryption failed"),
        }
    }
}

impl<S, K> std::error::Error for AesGcmRefreshMaterialProviderError<S, K>
where
    S: std::error::Error + 'static,
    K: std::error::Error + 'static,
{
}
