use super::harness::*;

#[path = "tenant_maintenance/outbox_drain.rs"]
mod outbox_drain;
#[path = "tenant_maintenance/run_once.rs"]
mod run_once;
#[path = "tenant_maintenance/scheduled_sweep.rs"]
mod scheduled_sweep;

fn assert_safe_stage_error(value: &str) {
    let lowered = value.to_ascii_lowercase();
    for marker in [
        "access_token",
        "refresh_token",
        "authorization_code",
        "authorization:",
        "bearer ",
        "stdout",
        "stderr",
        "encrypted",
        "fingerprint",
        "oauth_grant",
        "tok_",
        "rt_fake",
        "at_fake",
    ] {
        assert!(
            !lowered.contains(marker),
            "tenant maintenance stage error leaked sensitive marker: {marker}"
        );
    }
}
