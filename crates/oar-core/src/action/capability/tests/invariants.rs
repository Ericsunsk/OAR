use std::collections::HashSet;

use super::super::*;

#[test]
fn capability_matrix_action_types_capabilities_and_lookups_are_stable() {
    let mut action_types = HashSet::new();
    let mut capabilities = HashSet::new();

    assert!(
        !all_capabilities().is_empty(),
        "capability matrix should not be empty"
    );

    for spec in all_capabilities() {
        assert!(
            action_types.insert(spec.action_type),
            "duplicate action_type {}",
            spec.action_type_str()
        );
        assert!(
            capabilities.insert(spec.capability),
            "duplicate capability {}",
            spec.capability.as_str()
        );
        assert_eq!(
            find_by_action_type(spec.action_type),
            Some(spec),
            "{} must be lookupable by action_type",
            spec.action_type_str()
        );
        assert_eq!(
            find_by_capability(spec.capability),
            Some(spec),
            "{} must be lookupable by capability",
            spec.action_type_str()
        );
        assert_eq!(
            find_by_action_type_str(spec.action_type.as_str()),
            Some(spec),
            "{} must be lookupable by action_type string",
            spec.action_type_str()
        );
        assert_eq!(
            spec.action_type.as_str().parse::<CapabilityActionType>(),
            Ok(spec.action_type),
            "{} action_type string must round-trip through FromStr",
            spec.action_type_str()
        );
    }
}

#[test]
fn confirmed_write_capabilities_are_guarded_high_risk_writes() {
    let confirmed_write_capabilities: Vec<_> = all_capabilities()
        .iter()
        .filter(|capability| capability.execution_mode == CapabilityExecutionMode::ConfirmedWrite)
        .collect();

    assert!(
        !confirmed_write_capabilities.is_empty(),
        "matrix should include confirmed write capabilities"
    );

    for capability in confirmed_write_capabilities {
        assert_eq!(
            capability.effect,
            CapabilityEffect::Write,
            "{} must be a write capability",
            capability.action_type_str()
        );
        assert_eq!(
            capability.safety,
            CapabilitySafety::WRITE_GUARDED,
            "{} must use WRITE_GUARDED safety",
            capability.action_type_str()
        );
        assert_eq!(
            capability.risk,
            RiskLevel::High,
            "{} must be high risk",
            capability.action_type_str()
        );
        assert!(
            capability.enters_execution_allowlist(),
            "{} must enter the confirmed write execution allowlist",
            capability.action_type_str()
        );
    }
}

#[test]
fn draft_only_capabilities_are_write_scoped_but_not_execution_allowlisted() {
    let draft_capabilities: Vec<_> = all_capabilities()
        .iter()
        .filter(|capability| capability.execution_mode == CapabilityExecutionMode::DraftOnly)
        .collect();

    assert!(
        !draft_capabilities.is_empty(),
        "matrix should include draft-only capabilities"
    );

    for capability in draft_capabilities {
        assert!(
            capability.is_write(),
            "{} should represent a write-capable external scope",
            capability.action_type_str()
        );
        assert!(
            !capability.enters_execution_allowlist(),
            "{} must not enter the production execution allowlist",
            capability.action_type_str()
        );
        assert!(
            capability.safety.requires_audit,
            "{} should still leave a draft/audit trace",
            capability.action_type_str()
        );
    }
}

#[test]
fn read_capabilities_do_not_require_confirmation_or_dry_run() {
    let read_capabilities: Vec<_> = all_capabilities()
        .iter()
        .filter(|capability| capability.execution_mode == CapabilityExecutionMode::AutoRead)
        .collect();

    assert!(
        !read_capabilities.is_empty(),
        "matrix should include read capabilities"
    );

    for capability in read_capabilities {
        assert_eq!(
            capability.effect,
            CapabilityEffect::Read,
            "{} should be read-only",
            capability.action_type_str()
        );
        assert!(
            !capability.enters_execution_allowlist(),
            "{} must not enter the write execution allowlist",
            capability.action_type_str()
        );
        assert!(
            !capability.safety.requires_human_confirmation,
            "{} should not require human confirmation",
            capability.action_type_str()
        );
        assert!(
            !capability.safety.requires_dry_run,
            "{} should not require dry-run",
            capability.action_type_str()
        );
        assert!(
            capability.safety.requires_audit,
            "{} should still leave an audit or sync trace",
            capability.action_type_str()
        );
    }
}
