use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentCapability {
    OkrPeriodRead,
    OkrContentRead,
    OkrProgressRead,
    OkrProgressCreate,
    OkrProgressUpdate,
    OkrReviewRead,
    OkrSettingRead,
    CalendarFreeBusyRead,
    TaskRead,
    TaskCreate,
    ImMessageSend,
}

impl AgentCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OkrPeriodRead => "okr_period_read",
            Self::OkrContentRead => "okr_content_read",
            Self::OkrProgressRead => "okr_progress_read",
            Self::OkrProgressCreate => "okr_progress_create",
            Self::OkrProgressUpdate => "okr_progress_update",
            Self::OkrReviewRead => "okr_review_read",
            Self::OkrSettingRead => "okr_setting_read",
            Self::CalendarFreeBusyRead => "calendar_free_busy_read",
            Self::TaskRead => "task_read",
            Self::TaskCreate => "task_create",
            Self::ImMessageSend => "im_message_send",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityActionType {
    OkrPeriodRead,
    OkrContentRead,
    OkrProgressRead,
    OkrProgressCreate,
    OkrProgressUpdate,
    OkrReviewRead,
    OkrSettingRead,
    CalendarFreeBusyRead,
    TaskRead,
    TaskCreate,
    ImMessageSend,
}

impl CapabilityActionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OkrPeriodRead => "okr.period.read",
            Self::OkrContentRead => "okr.content.read",
            Self::OkrProgressRead => "okr.progress.read",
            Self::OkrProgressCreate => "okr.progress.create",
            Self::OkrProgressUpdate => "okr.progress.update",
            Self::OkrReviewRead => "okr.review.read",
            Self::OkrSettingRead => "okr.setting.read",
            Self::CalendarFreeBusyRead => "calendar.free_busy.read",
            Self::TaskRead => "task.read",
            Self::TaskCreate => "task.create",
            Self::ImMessageSend => "im.message.send",
        }
    }
}

impl FromStr for CapabilityActionType {
    type Err = UnknownCapabilityActionType;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "okr.period.read" => Ok(Self::OkrPeriodRead),
            "okr.content.read" => Ok(Self::OkrContentRead),
            "okr.progress.read" => Ok(Self::OkrProgressRead),
            "okr.progress.create" => Ok(Self::OkrProgressCreate),
            "okr.progress.update" => Ok(Self::OkrProgressUpdate),
            "okr.review.read" => Ok(Self::OkrReviewRead),
            "okr.setting.read" => Ok(Self::OkrSettingRead),
            "calendar.free_busy.read" => Ok(Self::CalendarFreeBusyRead),
            "task.read" => Ok(Self::TaskRead),
            "task.create" => Ok(Self::TaskCreate),
            "im.message.send" => Ok(Self::ImMessageSend),
            other => Err(UnknownCapabilityActionType {
                value: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCapabilityActionType {
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformAdapter {
    Lark,
}

impl PlatformAdapter {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lark => "lark",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeishuScope {
    OkrPeriodRead,
    OkrContentRead,
    OkrProgressRead,
    OkrProgressWrite,
    OkrReviewRead,
    OkrSettingRead,
    CalendarFreeBusyRead,
    TaskRead,
    TaskWrite,
    ImMessageSendAsBot,
}

impl FeishuScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OkrPeriodRead => "okr:okr.period:readonly",
            Self::OkrContentRead => "okr:okr.content:readonly",
            Self::OkrProgressRead => "okr:okr.progress:readonly",
            Self::OkrProgressWrite => "okr:okr.progress:writeonly",
            Self::OkrReviewRead => "okr:okr.review:readonly",
            Self::OkrSettingRead => "okr:okr.setting:read",
            Self::CalendarFreeBusyRead => "calendar:calendar.free_busy:read",
            Self::TaskRead => "task:task:read",
            Self::TaskWrite => "task:task:writeonly",
            Self::ImMessageSendAsBot => "im:message:send_as_bot",
        }
    }
}

pub const FEISHU_OFFLINE_ACCESS_SCOPE: &str = "offline_access";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OarRequiredScope {
    OkrPeriodRead,
    OkrContentRead,
    OkrProgressRead,
    OkrProgressWrite,
    OkrReviewRead,
    OkrSettingRead,
    CalendarFreeBusyRead,
    TaskRead,
    TaskWrite,
    ImMessageSendAsBot,
}

impl OarRequiredScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OkrPeriodRead => "okr.period.read",
            Self::OkrContentRead => "okr.content.read",
            Self::OkrProgressRead => "okr.progress.read",
            Self::OkrProgressWrite => "okr.progress.write",
            Self::OkrReviewRead => "okr.review.read",
            Self::OkrSettingRead => "okr.setting.read",
            Self::CalendarFreeBusyRead => "calendar.free_busy.read",
            Self::TaskRead => "task.read",
            Self::TaskWrite => "task.write",
            Self::ImMessageSendAsBot => "im.message.send_as_bot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityEffect {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityExecutionMode {
    AutoRead,
    DraftOnly,
    ConfirmedWrite,
}

impl CapabilityExecutionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AutoRead => "auto_read",
            Self::DraftOnly => "draft_only",
            Self::ConfirmedWrite => "confirmed_write",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySafety {
    pub requires_dry_run: bool,
    pub requires_human_confirmation: bool,
    pub requires_audit: bool,
}

impl CapabilitySafety {
    pub const READ_ONLY: Self = Self {
        requires_dry_run: false,
        requires_human_confirmation: false,
        requires_audit: true,
    };

    pub const WRITE_GUARDED: Self = Self {
        requires_dry_run: true,
        requires_human_confirmation: true,
        requires_audit: true,
    };

    pub const DRAFT_ONLY: Self = Self {
        requires_dry_run: false,
        requires_human_confirmation: false,
        requires_audit: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySpec {
    pub capability: AgentCapability,
    pub action_type: CapabilityActionType,
    pub adapter: PlatformAdapter,
    pub required_scope: OarRequiredScope,
    pub feishu_scopes: &'static [FeishuScope],
    pub effect: CapabilityEffect,
    pub execution_mode: CapabilityExecutionMode,
    pub risk: RiskLevel,
    pub safety: CapabilitySafety,
}

impl CapabilitySpec {
    pub const fn action_type_str(self) -> &'static str {
        self.action_type.as_str()
    }

    pub const fn is_write(self) -> bool {
        matches!(self.effect, CapabilityEffect::Write)
    }

    pub const fn enters_execution_allowlist(self) -> bool {
        matches!(
            (self.effect, self.execution_mode),
            (
                CapabilityEffect::Write,
                CapabilityExecutionMode::ConfirmedWrite
            )
        )
    }
}

pub const OKR_PERIOD_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrPeriodRead];
pub const OKR_CONTENT_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrContentRead];
pub const OKR_PROGRESS_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrProgressRead];
pub const OKR_PROGRESS_WRITE_SCOPES: &[FeishuScope] = &[FeishuScope::OkrProgressWrite];
pub const OKR_REVIEW_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrReviewRead];
pub const OKR_SETTING_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrSettingRead];
pub const CALENDAR_FREE_BUSY_READ_SCOPES: &[FeishuScope] = &[FeishuScope::CalendarFreeBusyRead];
pub const TASK_READ_SCOPES: &[FeishuScope] = &[FeishuScope::TaskRead];
pub const TASK_WRITE_SCOPES: &[FeishuScope] = &[FeishuScope::TaskWrite];
pub const IM_MESSAGE_SEND_AS_BOT_SCOPES: &[FeishuScope] = &[FeishuScope::ImMessageSendAsBot];

pub const DEFAULT_AGENT_FEISHU_OAUTH_ACTION_TYPES: &[CapabilityActionType] = &[
    CapabilityActionType::OkrPeriodRead,
    CapabilityActionType::OkrContentRead,
    CapabilityActionType::OkrProgressRead,
    CapabilityActionType::OkrProgressCreate,
    CapabilityActionType::OkrProgressUpdate,
    CapabilityActionType::OkrReviewRead,
    CapabilityActionType::OkrSettingRead,
    CapabilityActionType::CalendarFreeBusyRead,
    CapabilityActionType::TaskRead,
    CapabilityActionType::TaskCreate,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeishuScopeBundle {
    key: &'static str,
    action_types: &'static [CapabilityActionType],
}

impl FeishuScopeBundle {
    pub const fn key(self) -> &'static str {
        self.key
    }

    pub const fn action_types(self) -> &'static [CapabilityActionType] {
        self.action_types
    }

    pub fn feishu_scopes(self) -> Vec<FeishuScope> {
        feishu_scopes_for_action_types(self.action_types)
    }

    pub fn oauth_scope_strings(self) -> Vec<&'static str> {
        feishu_oauth_scope_strings_for_action_types(self.action_types)
    }
}

pub const DEFAULT_AGENT_FEISHU_OAUTH_SCOPE_BUNDLE: FeishuScopeBundle = FeishuScopeBundle {
    key: "default_agent_user_authorization",
    action_types: DEFAULT_AGENT_FEISHU_OAUTH_ACTION_TYPES,
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

pub const fn default_agent_feishu_oauth_scope_bundle() -> FeishuScopeBundle {
    DEFAULT_AGENT_FEISHU_OAUTH_SCOPE_BUNDLE
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityScopeDerivationError {
    CapabilityNotRegistered(CapabilityActionType),
}

pub fn try_feishu_scopes_for_action_types(
    action_types: &[CapabilityActionType],
) -> Result<Vec<FeishuScope>, CapabilityScopeDerivationError> {
    let mut scopes = Vec::new();

    for action_type in action_types {
        let Some(spec) = find_by_action_type(*action_type) else {
            return Err(CapabilityScopeDerivationError::CapabilityNotRegistered(
                *action_type,
            ));
        };

        for scope in spec.feishu_scopes {
            if !scopes.contains(scope) {
                scopes.push(*scope);
            }
        }
    }

    Ok(scopes)
}

pub fn feishu_scopes_for_action_types(action_types: &[CapabilityActionType]) -> Vec<FeishuScope> {
    try_feishu_scopes_for_action_types(action_types)
        .expect("all capability action types must be registered in CAPABILITY_MATRIX")
}

pub fn feishu_oauth_scope_strings_for_action_types(
    action_types: &[CapabilityActionType],
) -> Vec<&'static str> {
    let mut scopes = vec![FEISHU_OFFLINE_ACCESS_SCOPE];

    for scope in feishu_scopes_for_action_types(action_types) {
        let scope = scope.as_str();
        if !scopes.contains(&scope) {
            scopes.push(scope);
        }
    }

    scopes
}

pub fn default_agent_feishu_oauth_scope_strings() -> Vec<&'static str> {
    default_agent_feishu_oauth_scope_bundle().oauth_scope_strings()
}

#[cfg(test)]
mod tests;
