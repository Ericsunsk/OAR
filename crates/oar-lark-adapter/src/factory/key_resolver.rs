use std::fmt;

use crate::material::AesGcmKeyResolver;

#[derive(Clone)]
pub struct StaticAesGcmKeyResolver {
    key_id: String,
    key_material: [u8; 32],
}

impl StaticAesGcmKeyResolver {
    pub fn new(key_id: impl Into<String>, key_material: [u8; 32]) -> Self {
        Self {
            key_id: key_id.into(),
            key_material,
        }
    }
}

impl fmt::Debug for StaticAesGcmKeyResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StaticAesGcmKeyResolver")
            .field("key_id", &"[REDACTED]")
            .field("key_material", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Display for StaticAesGcmKeyResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StaticAesGcmKeyResolver([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("grant key is unavailable")]
pub struct StaticAesGcmKeyResolverError;

impl AesGcmKeyResolver for StaticAesGcmKeyResolver {
    type Error = StaticAesGcmKeyResolverError;

    fn key_for(&mut self, key_id: &str) -> Result<[u8; 32], Self::Error> {
        if key_id == self.key_id {
            Ok(self.key_material)
        } else {
            Err(StaticAesGcmKeyResolverError)
        }
    }
}
