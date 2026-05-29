use super::*;

#[test]
fn confirmed_write_capabilities_require_confirmation_audit_and_dry_run() {
    let confirmed_write_capabilities: Vec<_> = all_capabilities()
        .iter()
        .filter(|capability| capability.enters_execution_allowlist())
        .collect();

    assert!(
        !confirmed_write_capabilities.is_empty(),
        "matrix should include confirmed write capabilities"
    );

    for capability in confirmed_write_capabilities {
        assert!(
            capability.safety.requires_human_confirmation,
            "{} must require human confirmation",
            capability.action_type_str()
        );
        assert!(
            capability.safety.requires_audit,
            "{} must require audit",
            capability.action_type_str()
        );
        assert!(
            capability.safety.requires_dry_run,
            "{} must require dry-run",
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

#[test]
fn okr_read_capabilities_are_explicitly_mapped_to_minimal_feishu_read_scopes() {
    let period =
        find_by_action_type(CapabilityActionType::OkrPeriodRead).expect("period read lookup");
    assert_eq!(period.capability, AgentCapability::OkrPeriodRead);
    assert_eq!(period.required_scope.as_str(), "okr.period.read");
    assert_eq!(period.feishu_scopes[0].as_str(), "okr:okr.period:readonly");
    assert_eq!(period.execution_mode, CapabilityExecutionMode::AutoRead);
    assert_eq!(find_by_action_type_str("okr.period.read"), Some(period));
    assert_eq!(
        "okr.period.read"
            .parse::<CapabilityActionType>()
            .expect("period action type parse"),
        CapabilityActionType::OkrPeriodRead
    );

    let content =
        find_by_action_type(CapabilityActionType::OkrContentRead).expect("content read lookup");
    assert_eq!(content.capability, AgentCapability::OkrContentRead);
    assert_eq!(content.required_scope.as_str(), "okr.content.read");
    assert_eq!(
        content.feishu_scopes[0].as_str(),
        "okr:okr.content:readonly"
    );
    assert_eq!(content.execution_mode, CapabilityExecutionMode::AutoRead);
    assert_eq!(find_by_action_type_str("okr.content.read"), Some(content));

    let progress =
        find_by_action_type(CapabilityActionType::OkrProgressRead).expect("progress read lookup");
    assert_eq!(progress.capability, AgentCapability::OkrProgressRead);
    assert_eq!(progress.required_scope.as_str(), "okr.progress.read");
    assert_eq!(
        progress.feishu_scopes[0].as_str(),
        "okr:okr.progress:readonly"
    );
    assert_eq!(progress.execution_mode, CapabilityExecutionMode::AutoRead);
    assert_eq!(find_by_action_type_str("okr.progress.read"), Some(progress));
}

#[test]
fn okr_progress_update_and_create_action_types_are_lookupable() {
    let update =
        find_by_action_type(CapabilityActionType::OkrProgressUpdate).expect("update lookup");
    assert_eq!(update.capability, AgentCapability::OkrProgressUpdate);
    assert_eq!(update.required_scope.as_str(), "okr.progress.write");
    assert_eq!(
        update.execution_mode,
        CapabilityExecutionMode::ConfirmedWrite
    );
    assert_eq!(
        update.feishu_scopes[0].as_str(),
        "okr:okr.progress:writeonly"
    );
    assert_eq!(find_by_action_type_str("okr.progress.update"), Some(update));

    let create =
        find_by_action_type(CapabilityActionType::OkrProgressCreate).expect("create lookup");
    assert_eq!(create.capability, AgentCapability::OkrProgressCreate);
    assert_eq!(create.required_scope.as_str(), "okr.progress.write");
    assert_eq!(
        create.execution_mode,
        CapabilityExecutionMode::ConfirmedWrite
    );
    assert_eq!(
        create.feishu_scopes[0].as_str(),
        "okr:okr.progress:writeonly"
    );
    assert_eq!(find_by_action_type_str("okr.progress.create"), Some(create));
}

#[test]
fn next_batch_capabilities_are_lookupable_with_non_executing_posture() {
    let review =
        find_by_action_type(CapabilityActionType::OkrReviewRead).expect("review read lookup");
    assert_eq!(review.capability, AgentCapability::OkrReviewRead);
    assert_eq!(review.required_scope.as_str(), "okr.review.read");
    assert_eq!(review.feishu_scopes[0].as_str(), "okr:okr.review:readonly");
    assert_eq!(review.execution_mode, CapabilityExecutionMode::AutoRead);

    let setting =
        find_by_action_type(CapabilityActionType::OkrSettingRead).expect("setting read lookup");
    assert_eq!(setting.capability, AgentCapability::OkrSettingRead);
    assert_eq!(setting.required_scope.as_str(), "okr.setting.read");
    assert_eq!(setting.feishu_scopes[0].as_str(), "okr:okr.setting:read");
    assert_eq!(setting.execution_mode, CapabilityExecutionMode::AutoRead);

    let free_busy = find_by_action_type(CapabilityActionType::CalendarFreeBusyRead)
        .expect("free-busy read lookup");
    assert_eq!(free_busy.capability, AgentCapability::CalendarFreeBusyRead);
    assert_eq!(free_busy.required_scope.as_str(), "calendar.free_busy.read");
    assert_eq!(
        free_busy.feishu_scopes[0].as_str(),
        "calendar:calendar.free_busy:read"
    );
    assert_eq!(free_busy.execution_mode, CapabilityExecutionMode::AutoRead);

    let task_read = find_by_action_type(CapabilityActionType::TaskRead).expect("task read lookup");
    assert_eq!(task_read.capability, AgentCapability::TaskRead);
    assert_eq!(task_read.required_scope.as_str(), "task.read");
    assert_eq!(task_read.feishu_scopes[0].as_str(), "task:task:read");
    assert_eq!(task_read.execution_mode, CapabilityExecutionMode::AutoRead);

    let task_create =
        find_by_action_type(CapabilityActionType::TaskCreate).expect("task create lookup");
    assert_eq!(task_create.capability, AgentCapability::TaskCreate);
    assert_eq!(task_create.required_scope.as_str(), "task.write");
    assert_eq!(task_create.feishu_scopes[0].as_str(), "task:task:writeonly");
    assert_eq!(
        task_create.execution_mode,
        CapabilityExecutionMode::DraftOnly
    );

    let message_send =
        find_by_action_type(CapabilityActionType::ImMessageSend).expect("message send lookup");
    assert_eq!(message_send.capability, AgentCapability::ImMessageSend);
    assert_eq!(
        message_send.required_scope.as_str(),
        "im.message.send_as_bot"
    );
    assert_eq!(
        message_send.feishu_scopes[0].as_str(),
        "im:message:send_as_bot"
    );
    assert_eq!(
        message_send.execution_mode,
        CapabilityExecutionMode::DraftOnly
    );
}

#[test]
fn default_feishu_oauth_bundle_contains_expected_user_authorization_scopes() {
    let scopes = default_agent_feishu_oauth_scope_strings();

    assert_eq!(
        scopes,
        vec![
            FEISHU_OFFLINE_ACCESS_SCOPE,
            FeishuScope::OkrPeriodRead.as_str(),
            FeishuScope::OkrContentRead.as_str(),
            FeishuScope::OkrProgressRead.as_str(),
            FeishuScope::OkrProgressWrite.as_str(),
            FeishuScope::OkrReviewRead.as_str(),
            FeishuScope::OkrSettingRead.as_str(),
            FeishuScope::CalendarFreeBusyRead.as_str(),
            FeishuScope::TaskRead.as_str(),
            FeishuScope::TaskWrite.as_str(),
        ]
    );
    assert!(!scopes.contains(&FeishuScope::ImMessageSendAsBot.as_str()));
}

#[test]
fn feishu_scope_derivation_is_stable_and_deduplicated() {
    let scopes = feishu_scopes_for_action_types(&[
        CapabilityActionType::OkrProgressCreate,
        CapabilityActionType::OkrProgressUpdate,
        CapabilityActionType::TaskCreate,
        CapabilityActionType::TaskCreate,
    ]);

    assert_eq!(
        scopes,
        vec![FeishuScope::OkrProgressWrite, FeishuScope::TaskWrite]
    );

    let unique_len = default_agent_feishu_oauth_scope_strings()
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(unique_len, default_agent_feishu_oauth_scope_strings().len());
}

#[test]
fn feishu_oauth_bundle_keeps_authorization_metadata_separate() {
    let bundle = default_agent_feishu_oauth_scope_bundle();

    assert_eq!(bundle.key(), "default_agent_user_authorization");
    assert!(
        bundle
            .action_types()
            .contains(&CapabilityActionType::OkrProgressCreate),
        "default authorization should still request OKR progress write scope"
    );
    assert!(
        bundle
            .action_types()
            .contains(&CapabilityActionType::TaskCreate),
        "default authorization should still request task write scope"
    );
    assert_eq!(
        bundle.feishu_scopes(),
        feishu_scopes_for_action_types(bundle.action_types())
    );
}

#[test]
fn capability_matrix_contains_no_coarse_or_delete_feishu_scopes() {
    let forbidden = [
        "okr:okr",
        "okr:okr.content:writeonly",
        "okr:okr.period:writeonly",
        "okr:okr.progress:delete",
        "task:task:write",
        "calendar:calendar",
        "im:message",
    ];

    for capability in all_capabilities() {
        for scope in capability.feishu_scopes {
            assert!(
                !forbidden.contains(&scope.as_str()),
                "{} must not use coarse or destructive Feishu scope {}",
                capability.action_type_str(),
                scope.as_str()
            );
        }
    }
}
