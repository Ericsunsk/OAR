pub trait GrantTimeSource: Send + Sync + 'static {
    fn now_ms(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemGrantClock;

impl GrantTimeSource for SystemGrantClock {
    fn now_ms(&self) -> u64 {
        let now = std::time::SystemTime::now();
        let Ok(duration) = now.duration_since(std::time::UNIX_EPOCH) else {
            return 0;
        };
        duration.as_millis().min(u128::from(u64::MAX)) as u64
    }
}
