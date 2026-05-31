use super::{
    AgentCapability, CapabilityActionType, CapabilityEffect, CapabilityExecutionMode,
    CapabilitySafety, CapabilitySpec, OarRequiredScope, PlatformAdapter, RiskLevel,
    CALENDAR_EVENT_READ_SCOPES, CALENDAR_FREE_BUSY_READ_SCOPES, CALENDAR_READ_SCOPES,
    DOCX_DOCUMENT_READ_SCOPES, IM_MESSAGE_SEND_AS_BOT_SCOPES, OKR_CONTENT_READ_SCOPES,
    OKR_PERIOD_READ_SCOPES, OKR_PROGRESS_READ_SCOPES, OKR_PROGRESS_WRITE_SCOPES,
    OKR_REVIEW_READ_SCOPES, OKR_SETTING_READ_SCOPES, TASK_READ_SCOPES, TASK_WRITE_SCOPES,
    WIKI_NODE_READ_SCOPES,
};

pub const CAPABILITY_MATRIX: &[CapabilitySpec] = &[
    CapabilitySpec {
        capability: AgentCapability::OkrPeriodRead,
        action_type: CapabilityActionType::OkrPeriodRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::OkrPeriodRead,
        feishu_scopes: OKR_PERIOD_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::OkrContentRead,
        action_type: CapabilityActionType::OkrContentRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::OkrContentRead,
        feishu_scopes: OKR_CONTENT_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::OkrProgressRead,
        action_type: CapabilityActionType::OkrProgressRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::OkrProgressRead,
        feishu_scopes: OKR_PROGRESS_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::OkrProgressCreate,
        action_type: CapabilityActionType::OkrProgressCreate,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::OkrProgressWrite,
        feishu_scopes: OKR_PROGRESS_WRITE_SCOPES,
        effect: CapabilityEffect::Write,
        execution_mode: CapabilityExecutionMode::ConfirmedWrite,
        risk: RiskLevel::High,
        safety: CapabilitySafety::WRITE_GUARDED,
    },
    CapabilitySpec {
        capability: AgentCapability::OkrProgressUpdate,
        action_type: CapabilityActionType::OkrProgressUpdate,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::OkrProgressWrite,
        feishu_scopes: OKR_PROGRESS_WRITE_SCOPES,
        effect: CapabilityEffect::Write,
        execution_mode: CapabilityExecutionMode::ConfirmedWrite,
        risk: RiskLevel::High,
        safety: CapabilitySafety::WRITE_GUARDED,
    },
    CapabilitySpec {
        capability: AgentCapability::OkrReviewRead,
        action_type: CapabilityActionType::OkrReviewRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::OkrReviewRead,
        feishu_scopes: OKR_REVIEW_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::OkrSettingRead,
        action_type: CapabilityActionType::OkrSettingRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::OkrSettingRead,
        feishu_scopes: OKR_SETTING_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::CalendarRead,
        action_type: CapabilityActionType::CalendarRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::CalendarRead,
        feishu_scopes: CALENDAR_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::CalendarEventRead,
        action_type: CapabilityActionType::CalendarEventRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::CalendarEventRead,
        feishu_scopes: CALENDAR_EVENT_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::CalendarFreeBusyRead,
        action_type: CapabilityActionType::CalendarFreeBusyRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::CalendarFreeBusyRead,
        feishu_scopes: CALENDAR_FREE_BUSY_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::TaskRead,
        action_type: CapabilityActionType::TaskRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::TaskRead,
        feishu_scopes: TASK_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::TaskCreate,
        action_type: CapabilityActionType::TaskCreate,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::TaskWrite,
        feishu_scopes: TASK_WRITE_SCOPES,
        effect: CapabilityEffect::Write,
        execution_mode: CapabilityExecutionMode::DraftOnly,
        risk: RiskLevel::High,
        safety: CapabilitySafety::DRAFT_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::DocxDocumentRead,
        action_type: CapabilityActionType::DocxDocumentRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::DocxDocumentRead,
        feishu_scopes: DOCX_DOCUMENT_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::WikiNodeRead,
        action_type: CapabilityActionType::WikiNodeRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::WikiNodeRead,
        feishu_scopes: WIKI_NODE_READ_SCOPES,
        effect: CapabilityEffect::Read,
        execution_mode: CapabilityExecutionMode::AutoRead,
        risk: RiskLevel::Low,
        safety: CapabilitySafety::READ_ONLY,
    },
    CapabilitySpec {
        capability: AgentCapability::ImMessageSend,
        action_type: CapabilityActionType::ImMessageSend,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::ImMessageSendAsBot,
        feishu_scopes: IM_MESSAGE_SEND_AS_BOT_SCOPES,
        effect: CapabilityEffect::Write,
        execution_mode: CapabilityExecutionMode::DraftOnly,
        risk: RiskLevel::High,
        safety: CapabilitySafety::DRAFT_ONLY,
    },
];

pub fn all_capabilities() -> &'static [CapabilitySpec] {
    CAPABILITY_MATRIX
}

pub fn find_by_capability(capability: AgentCapability) -> Option<&'static CapabilitySpec> {
    CAPABILITY_MATRIX
        .iter()
        .find(|spec| spec.capability == capability)
}

pub fn find_by_action_type(action_type: CapabilityActionType) -> Option<&'static CapabilitySpec> {
    CAPABILITY_MATRIX
        .iter()
        .find(|spec| spec.action_type == action_type)
}

pub fn find_by_action_type_str(action_type: &str) -> Option<&'static CapabilitySpec> {
    CAPABILITY_MATRIX
        .iter()
        .find(|spec| spec.action_type.as_str() == action_type)
}
