use std::collections::HashSet;

use super::super::*;

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
            FeishuScope::CalendarRead.as_str(),
            FeishuScope::CalendarEventRead.as_str(),
            FeishuScope::CalendarFreeBusyRead.as_str(),
            FeishuScope::TaskRead.as_str(),
            FeishuScope::TaskWrite.as_str(),
            FeishuScope::DocxDocumentRead.as_str(),
            FeishuScope::WikiNodeRead.as_str(),
            FeishuScope::MinutesBasicRead.as_str(),
        ]
    );
    assert!(!scopes.contains(&FeishuScope::ImMessageSendAsBot.as_str()));
    assert!(!scopes.contains(&"docx:document"));
    assert!(!scopes.contains(&"wiki:wiki"));
    assert!(!scopes.contains(&"wiki:wiki:readonly"));
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
        .collect::<HashSet<_>>()
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
    assert!(
        bundle
            .action_types()
            .contains(&CapabilityActionType::DocxDocumentRead),
        "default authorization should request docx document read scope"
    );
    assert!(
        bundle
            .action_types()
            .contains(&CapabilityActionType::WikiNodeRead),
        "default authorization should request wiki node read scope"
    );
    assert!(
        bundle
            .action_types()
            .contains(&CapabilityActionType::MinutesBasicRead),
        "default authorization should request minutes basic read scope"
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
        "docx:document",
        "wiki:wiki",
        "wiki:wiki:readonly",
        "minutes:minutes",
        "minutes:minutes:readonly",
        "minutes:minutes.artifacts:read",
        "minutes:minutes.transcript:export",
        "minutes:minutes.media:export",
        "minutes:minutes.upload:write",
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
