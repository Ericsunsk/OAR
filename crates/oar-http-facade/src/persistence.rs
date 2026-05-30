use std::fmt;

use sqlx::PgPool;

mod config;

pub(crate) use config::FacadePersistenceConfig;

#[derive(Clone)]
pub(crate) struct FacadePersistenceRuntime {
    pool: PgPool,
    grant_key_id: String,
    grant_key_material: [u8; 32],
}

impl FacadePersistenceRuntime {
    pub(crate) fn new(pool: PgPool, config: FacadePersistenceConfig) -> Self {
        Self {
            pool,
            grant_key_id: config.grant_key_id().to_string(),
            grant_key_material: config.grant_key_material(),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        pool: PgPool,
        grant_key_id: String,
        grant_key_material: [u8; 32],
    ) -> Self {
        Self {
            pool,
            grant_key_id,
            grant_key_material,
        }
    }

    pub(crate) fn pool(&self) -> PgPool {
        self.pool.clone()
    }

    pub(crate) fn grant_key_id(&self) -> &str {
        &self.grant_key_id
    }

    pub(crate) fn grant_key_material(&self) -> [u8; 32] {
        self.grant_key_material
    }
}

impl fmt::Debug for FacadePersistenceRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FacadePersistenceRuntime")
            .field("pool", &"[REDACTED]")
            .field("grant_key_id", &"[REDACTED]")
            .field("grant_key_material", &"[REDACTED]")
            .finish()
    }
}
