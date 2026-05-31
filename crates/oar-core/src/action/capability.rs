use std::str::FromStr;

mod feishu;
mod matrix;

pub use feishu::{
    default_agent_feishu_oauth_scope_bundle, default_agent_feishu_oauth_scope_strings,
    feishu_oauth_scope_strings_for_action_types, feishu_scopes_for_action_types,
    try_feishu_scopes_for_action_types, CapabilityScopeDerivationError, FeishuScope,
    FeishuScopeBundle, CALENDAR_EVENT_READ_SCOPES, CALENDAR_FREE_BUSY_READ_SCOPES,
    CALENDAR_READ_SCOPES, DEFAULT_AGENT_FEISHU_OAUTH_ACTION_TYPES,
    DEFAULT_AGENT_FEISHU_OAUTH_SCOPE_BUNDLE, DOCX_DOCUMENT_READ_SCOPES,
    FEISHU_OFFLINE_ACCESS_SCOPE, IM_MESSAGE_SEND_AS_BOT_SCOPES, OKR_CONTENT_READ_SCOPES,
    OKR_PERIOD_READ_SCOPES, OKR_PROGRESS_READ_SCOPES, OKR_PROGRESS_WRITE_SCOPES,
    OKR_REVIEW_READ_SCOPES, OKR_SETTING_READ_SCOPES, TASK_READ_SCOPES, TASK_WRITE_SCOPES,
    WIKI_NODE_READ_SCOPES,
};
pub use matrix::{
    all_capabilities, find_by_action_type, find_by_action_type_str, find_by_capability,
    CAPABILITY_MATRIX,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentCapability {
    OkrPeriodRead,
    OkrContentRead,
    OkrProgressRead,
    OkrProgressCreate,
    OkrProgressUpdate,
    OkrReviewRead,
    OkrSettingRead,
    CalendarRead,
    CalendarEventRead,
    CalendarFreeBusyRead,
    TaskRead,
    TaskCreate,
    DocxDocumentRead,
    WikiNodeRead,
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
            Self::CalendarRead => "calendar_read",
            Self::CalendarEventRead => "calendar_event_read",
            Self::CalendarFreeBusyRead => "calendar_free_busy_read",
            Self::TaskRead => "task_read",
            Self::TaskCreate => "task_create",
            Self::DocxDocumentRead => "docx_document_read",
            Self::WikiNodeRead => "wiki_node_read",
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
    CalendarRead,
    CalendarEventRead,
    CalendarFreeBusyRead,
    TaskRead,
    TaskCreate,
    DocxDocumentRead,
    WikiNodeRead,
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
            Self::CalendarRead => "calendar.read",
            Self::CalendarEventRead => "calendar.event.read",
            Self::CalendarFreeBusyRead => "calendar.free_busy.read",
            Self::TaskRead => "task.read",
            Self::TaskCreate => "task.create",
            Self::DocxDocumentRead => "docx.document.read",
            Self::WikiNodeRead => "wiki.node.read",
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
            "calendar.read" => Ok(Self::CalendarRead),
            "calendar.event.read" => Ok(Self::CalendarEventRead),
            "calendar.free_busy.read" => Ok(Self::CalendarFreeBusyRead),
            "task.read" => Ok(Self::TaskRead),
            "task.create" => Ok(Self::TaskCreate),
            "docx.document.read" => Ok(Self::DocxDocumentRead),
            "wiki.node.read" => Ok(Self::WikiNodeRead),
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
pub enum OarRequiredScope {
    OkrPeriodRead,
    OkrContentRead,
    OkrProgressRead,
    OkrProgressWrite,
    OkrReviewRead,
    OkrSettingRead,
    CalendarRead,
    CalendarEventRead,
    CalendarFreeBusyRead,
    TaskRead,
    TaskWrite,
    DocxDocumentRead,
    WikiNodeRead,
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
            Self::CalendarRead => "calendar.read",
            Self::CalendarEventRead => "calendar.event.read",
            Self::CalendarFreeBusyRead => "calendar.free_busy.read",
            Self::TaskRead => "task.read",
            Self::TaskWrite => "task.write",
            Self::DocxDocumentRead => "docx.document.read",
            Self::WikiNodeRead => "wiki.node.read",
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

#[cfg(test)]
mod tests;
