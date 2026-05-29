use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentCapability {
    OkrContentRead,
    OkrProgressRead,
    OkrProgressCreate,
    OkrProgressUpdate,
}

impl AgentCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OkrContentRead => "okr_content_read",
            Self::OkrProgressRead => "okr_progress_read",
            Self::OkrProgressCreate => "okr_progress_create",
            Self::OkrProgressUpdate => "okr_progress_update",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityActionType {
    OkrContentRead,
    OkrProgressRead,
    OkrProgressCreate,
    OkrProgressUpdate,
}

impl CapabilityActionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OkrContentRead => "okr.content.read",
            Self::OkrProgressRead => "okr.progress.read",
            Self::OkrProgressCreate => "okr.progress.create",
            Self::OkrProgressUpdate => "okr.progress.update",
        }
    }
}

impl FromStr for CapabilityActionType {
    type Err = UnknownCapabilityActionType;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "okr.content.read" => Ok(Self::OkrContentRead),
            "okr.progress.read" => Ok(Self::OkrProgressRead),
            "okr.progress.create" => Ok(Self::OkrProgressCreate),
            "okr.progress.update" => Ok(Self::OkrProgressUpdate),
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
    OkrContentRead,
    OkrProgressRead,
    OkrProgressWrite,
}

impl FeishuScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OkrContentRead => "okr:okr.content:readonly",
            Self::OkrProgressRead => "okr:okr.progress:readonly",
            Self::OkrProgressWrite => "okr:okr.progress:writeonly",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OarRequiredScope {
    OkrContentRead,
    OkrProgressRead,
    OkrProgressWrite,
}

impl OarRequiredScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OkrContentRead => "okr.content.read",
            Self::OkrProgressRead => "okr.progress.read",
            Self::OkrProgressWrite => "okr.progress.write",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityEffect {
    Read,
    Write,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySpec {
    pub capability: AgentCapability,
    pub action_type: CapabilityActionType,
    pub adapter: PlatformAdapter,
    pub required_scope: OarRequiredScope,
    pub feishu_scopes: &'static [FeishuScope],
    pub effect: CapabilityEffect,
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
}

pub const OKR_CONTENT_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrContentRead];
pub const OKR_PROGRESS_READ_SCOPES: &[FeishuScope] = &[FeishuScope::OkrProgressRead];
pub const OKR_PROGRESS_WRITE_SCOPES: &[FeishuScope] = &[FeishuScope::OkrProgressWrite];

pub const CAPABILITY_MATRIX: &[CapabilitySpec] = &[
    CapabilitySpec {
        capability: AgentCapability::OkrContentRead,
        action_type: CapabilityActionType::OkrContentRead,
        adapter: PlatformAdapter::Lark,
        required_scope: OarRequiredScope::OkrContentRead,
        feishu_scopes: OKR_CONTENT_READ_SCOPES,
        effect: CapabilityEffect::Read,
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
        risk: RiskLevel::High,
        safety: CapabilitySafety::WRITE_GUARDED,
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
    fn all_write_capabilities_require_confirmation_audit_and_dry_run() {
        let write_capabilities: Vec<_> = all_capabilities()
            .iter()
            .filter(|capability| capability.is_write())
            .collect();

        assert!(
            !write_capabilities.is_empty(),
            "matrix should include write capabilities"
        );

        for capability in write_capabilities {
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
    fn read_capabilities_do_not_require_confirmation_or_dry_run() {
        let read_capabilities: Vec<_> = all_capabilities()
            .iter()
            .filter(|capability| capability.effect == CapabilityEffect::Read)
            .collect();

        assert!(
            !read_capabilities.is_empty(),
            "matrix should include read capabilities"
        );

        for capability in read_capabilities {
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
    fn okr_progress_update_and_create_action_types_are_lookupable() {
        let update =
            find_by_action_type(CapabilityActionType::OkrProgressUpdate).expect("update lookup");
        assert_eq!(update.capability, AgentCapability::OkrProgressUpdate);
        assert_eq!(update.required_scope.as_str(), "okr.progress.write");
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
            create.feishu_scopes[0].as_str(),
            "okr:okr.progress:writeonly"
        );
        assert_eq!(find_by_action_type_str("okr.progress.create"), Some(create));
    }
}
