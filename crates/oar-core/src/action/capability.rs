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

#[cfg(test)]
mod tests {
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

        let progress = find_by_action_type(CapabilityActionType::OkrProgressRead)
            .expect("progress read lookup");
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

        let task_read =
            find_by_action_type(CapabilityActionType::TaskRead).expect("task read lookup");
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
}
