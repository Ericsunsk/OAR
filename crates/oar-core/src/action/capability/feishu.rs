use super::{find_by_action_type, CapabilityActionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeishuScope {
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
    MinutesBasicRead,
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
            Self::CalendarRead => "calendar:calendar:read",
            Self::CalendarEventRead => "calendar:calendar.event:read",
            Self::CalendarFreeBusyRead => "calendar:calendar.free_busy:read",
            Self::TaskRead => "task:task:read",
            Self::TaskWrite => "task:task:writeonly",
            Self::DocxDocumentRead => "docx:document:readonly",
            Self::WikiNodeRead => "wiki:node:read",
            Self::MinutesBasicRead => "minutes:minutes.basic:read",
            Self::ImMessageSendAsBot => "im:message:send_as_bot",
        }
    }
}

pub const FEISHU_OFFLINE_ACCESS_SCOPE: &str = "offline_access";

pub const OKR_PERIOD_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrPeriodRead];
pub const OKR_CONTENT_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrContentRead];
pub const OKR_PROGRESS_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrProgressRead];
pub const OKR_PROGRESS_WRITE_SCOPES: &[FeishuScope] = &[FeishuScope::OkrProgressWrite];
pub const OKR_REVIEW_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrReviewRead];
pub const OKR_SETTING_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrSettingRead];
pub const CALENDAR_READ_SCOPES: &[FeishuScope] = &[FeishuScope::CalendarRead];
pub const CALENDAR_EVENT_READ_SCOPES: &[FeishuScope] = &[FeishuScope::CalendarEventRead];
pub const CALENDAR_FREE_BUSY_READ_SCOPES: &[FeishuScope] = &[FeishuScope::CalendarFreeBusyRead];
pub const TASK_READ_SCOPES: &[FeishuScope] = &[FeishuScope::TaskRead];
pub const TASK_WRITE_SCOPES: &[FeishuScope] = &[FeishuScope::TaskWrite];
pub const DOCX_DOCUMENT_READ_SCOPES: &[FeishuScope] = &[FeishuScope::DocxDocumentRead];
pub const WIKI_NODE_READ_SCOPES: &[FeishuScope] = &[FeishuScope::WikiNodeRead];
pub const MINUTES_BASIC_READ_SCOPES: &[FeishuScope] = &[FeishuScope::MinutesBasicRead];
pub const IM_MESSAGE_SEND_AS_BOT_SCOPES: &[FeishuScope] = &[FeishuScope::ImMessageSendAsBot];

pub const DEFAULT_AGENT_FEISHU_OAUTH_ACTION_TYPES: &[CapabilityActionType] = &[
    CapabilityActionType::OkrPeriodRead,
    CapabilityActionType::OkrContentRead,
    CapabilityActionType::OkrProgressRead,
    CapabilityActionType::OkrProgressCreate,
    CapabilityActionType::OkrProgressUpdate,
    CapabilityActionType::OkrReviewRead,
    CapabilityActionType::OkrSettingRead,
    CapabilityActionType::CalendarRead,
    CapabilityActionType::CalendarEventRead,
    CapabilityActionType::CalendarFreeBusyRead,
    CapabilityActionType::TaskRead,
    CapabilityActionType::TaskCreate,
    CapabilityActionType::DocxDocumentRead,
    CapabilityActionType::WikiNodeRead,
    CapabilityActionType::MinutesBasicRead,
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
