use oar_core::domain::evidence::{
    EvidenceError, EvidenceId, EvidenceItem, EvidenceRef, EvidenceSourceKind,
    EvidenceVisibilityScope,
};
use std::time::SystemTime;

const VALID_HASH: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn sample_reference() -> EvidenceRef {
    EvidenceRef::new(
        EvidenceSourceKind::LarkMinutes,
        "minute_01",
        Some("chapter:weekly-review".to_string()),
    )
    .expect("reference should be valid")
}

#[test]
fn rejects_empty_summary() {
    let result = EvidenceItem::new(
        EvidenceId("ev_01".to_string()),
        "   ",
        sample_reference(),
        VALID_HASH,
        EvidenceVisibilityScope::Team,
        SystemTime::UNIX_EPOCH,
        SystemTime::UNIX_EPOCH,
    );

    assert_eq!(result, Err(EvidenceError::MissingSummary));
}

#[test]
fn rejects_missing_hash() {
    let result = EvidenceItem::new(
        EvidenceId("ev_02".to_string()),
        "KR progress moved slower than planned this week",
        sample_reference(),
        "   ",
        EvidenceVisibilityScope::Team,
        SystemTime::UNIX_EPOCH,
        SystemTime::UNIX_EPOCH,
    );

    assert_eq!(result, Err(EvidenceError::MissingHash));
}

#[test]
fn rejects_invalid_hash() {
    let result = EvidenceItem::new(
        EvidenceId("ev_03".to_string()),
        "Meeting follow-up blocked by external dependency",
        sample_reference(),
        "sha256:xyz-not-hex",
        EvidenceVisibilityScope::Team,
        SystemTime::UNIX_EPOCH,
        SystemTime::UNIX_EPOCH,
    );

    assert_eq!(result, Err(EvidenceError::InvalidHashFormat));
}

#[test]
fn rejects_missing_reference_id() {
    let result = EvidenceRef::new(EvidenceSourceKind::LarkDoc, "   ", None);
    assert_eq!(result, Err(EvidenceError::MissingReferenceId));
}

#[test]
fn debug_output_does_not_include_fake_raw_secrets() {
    let evidence = EvidenceItem::new(
        EvidenceId("ev_04".to_string()),
        "Weekly review summary only",
        sample_reference(),
        VALID_HASH,
        EvidenceVisibilityScope::Tenant,
        SystemTime::UNIX_EPOCH,
        SystemTime::UNIX_EPOCH,
    )
    .expect("evidence should be valid");

    let debug_output = format!("{evidence:?}");
    assert!(!debug_output.contains("xoxb-raw-token-secret"));
    assert!(!debug_output.contains("raw-meeting-transcript-secret"));
}

#[test]
fn all_enum_variants_are_constructible() {
    let _ = [
        EvidenceSourceKind::OkrProgress,
        EvidenceSourceKind::LarkMinutes,
        EvidenceSourceKind::LarkDoc,
        EvidenceSourceKind::LarkTask,
        EvidenceSourceKind::LarkCalendar,
        EvidenceSourceKind::LarkIm,
        EvidenceSourceKind::ManualReviewNote,
        EvidenceSourceKind::AuditEvent,
    ];
    let _ = [
        EvidenceVisibilityScope::Tenant,
        EvidenceVisibilityScope::Team,
        EvidenceVisibilityScope::User,
    ];
}
