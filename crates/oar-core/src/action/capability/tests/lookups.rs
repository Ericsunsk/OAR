use super::super::*;

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

    let calendar =
        find_by_action_type(CapabilityActionType::CalendarRead).expect("calendar read lookup");
    assert_eq!(calendar.capability, AgentCapability::CalendarRead);
    assert_eq!(calendar.required_scope.as_str(), "calendar.read");
    assert_eq!(calendar.feishu_scopes[0].as_str(), "calendar:calendar:read");
    assert_eq!(calendar.execution_mode, CapabilityExecutionMode::AutoRead);
    assert_eq!(find_by_action_type_str("calendar.read"), Some(calendar));
    assert!(
        !calendar.enters_execution_allowlist(),
        "calendar read must stay outside write execution allowlist"
    );

    let event = find_by_action_type(CapabilityActionType::CalendarEventRead)
        .expect("calendar event read lookup");
    assert_eq!(event.capability, AgentCapability::CalendarEventRead);
    assert_eq!(event.required_scope.as_str(), "calendar.event.read");
    assert_eq!(
        event.feishu_scopes[0].as_str(),
        "calendar:calendar.event:read"
    );
    assert_eq!(event.execution_mode, CapabilityExecutionMode::AutoRead);
    assert_eq!(find_by_action_type_str("calendar.event.read"), Some(event));
    assert!(
        !event.enters_execution_allowlist(),
        "calendar event read must stay outside write execution allowlist"
    );

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

    let docx_document = find_by_action_type(CapabilityActionType::DocxDocumentRead)
        .expect("docx document read lookup");
    assert_eq!(docx_document.capability, AgentCapability::DocxDocumentRead);
    assert_eq!(docx_document.required_scope.as_str(), "docx.document.read");
    assert_eq!(
        docx_document.feishu_scopes[0].as_str(),
        "docx:document:readonly"
    );
    assert_eq!(
        docx_document.execution_mode,
        CapabilityExecutionMode::AutoRead
    );
    assert_eq!(
        find_by_action_type_str("docx.document.read"),
        Some(docx_document)
    );
    assert!(
        !docx_document.enters_execution_allowlist(),
        "docx document read must stay outside write execution allowlist"
    );
    assert_eq!(
        "docx.document.read"
            .parse::<CapabilityActionType>()
            .expect("docx document action type parse"),
        CapabilityActionType::DocxDocumentRead
    );

    let wiki_node =
        find_by_action_type(CapabilityActionType::WikiNodeRead).expect("wiki node read lookup");
    assert_eq!(wiki_node.capability, AgentCapability::WikiNodeRead);
    assert_eq!(wiki_node.required_scope.as_str(), "wiki.node.read");
    assert_eq!(wiki_node.feishu_scopes[0].as_str(), "wiki:node:read");
    assert_eq!(wiki_node.execution_mode, CapabilityExecutionMode::AutoRead);
    assert_eq!(find_by_action_type_str("wiki.node.read"), Some(wiki_node));
    assert!(
        !wiki_node.enters_execution_allowlist(),
        "wiki node read must stay outside write execution allowlist"
    );
    assert_eq!(
        "wiki.node.read"
            .parse::<CapabilityActionType>()
            .expect("wiki node action type parse"),
        CapabilityActionType::WikiNodeRead
    );

    let minutes_basic = find_by_action_type(CapabilityActionType::MinutesBasicRead)
        .expect("minutes basic read lookup");
    assert_eq!(minutes_basic.capability, AgentCapability::MinutesBasicRead);
    assert_eq!(minutes_basic.required_scope.as_str(), "minutes.basic.read");
    assert_eq!(
        minutes_basic.feishu_scopes[0].as_str(),
        "minutes:minutes.basic:read"
    );
    assert_eq!(
        minutes_basic.execution_mode,
        CapabilityExecutionMode::AutoRead
    );
    assert_eq!(
        find_by_action_type_str("minutes.basic.read"),
        Some(minutes_basic)
    );
    assert!(
        !minutes_basic.enters_execution_allowlist(),
        "minutes basic read must stay outside write execution allowlist"
    );
    assert_eq!(
        "minutes.basic.read"
            .parse::<CapabilityActionType>()
            .expect("minutes basic action type parse"),
        CapabilityActionType::MinutesBasicRead
    );

    let minutes_search = find_by_action_type(CapabilityActionType::MinutesSearchRead)
        .expect("minutes search read lookup");
    assert_eq!(
        minutes_search.capability,
        AgentCapability::MinutesSearchRead
    );
    assert_eq!(
        minutes_search.required_scope.as_str(),
        "minutes.search.read"
    );
    assert_eq!(
        minutes_search.feishu_scopes[0].as_str(),
        "minutes:minutes.search:read"
    );
    assert_eq!(
        minutes_search.execution_mode,
        CapabilityExecutionMode::AutoRead
    );
    assert_eq!(
        find_by_action_type_str("minutes.search.read"),
        Some(minutes_search)
    );
    assert!(
        !minutes_search.enters_execution_allowlist(),
        "minutes search read must stay outside write execution allowlist"
    );
    assert_eq!(
        "minutes.search.read"
            .parse::<CapabilityActionType>()
            .expect("minutes search action type parse"),
        CapabilityActionType::MinutesSearchRead
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
