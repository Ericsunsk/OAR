use crate::okr::source_ref::OkrKrSourceRef;
use crate::okr::types::{OkrReadKeyResult, OkrReadObjective, OkrReadOkr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OkrKrReviewTarget<'a> {
    pub(super) identity: OkrKrIdentity,
    pub(super) okr: &'a OkrReadOkr,
    pub(super) objective: &'a OkrReadObjective,
    pub(super) kr: &'a OkrReadKeyResult,
}

impl<'a> OkrKrReviewTarget<'a> {
    pub(super) fn from_parts(
        okr: &'a OkrReadOkr,
        objective: &'a OkrReadObjective,
        kr: &'a OkrReadKeyResult,
    ) -> Option<Self> {
        Some(Self {
            identity: OkrKrIdentity::from_parts(okr, objective, kr)?,
            okr,
            objective,
            kr,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OkrKrIdentity {
    source_ref: OkrKrSourceRef,
}

impl OkrKrIdentity {
    fn from_parts(
        okr: &OkrReadOkr,
        objective: &OkrReadObjective,
        kr: &OkrReadKeyResult,
    ) -> Option<Self> {
        Some(Self {
            source_ref: OkrKrSourceRef::new(
                non_empty(okr.okr_id.as_deref())?,
                non_empty(objective.objective_id.as_deref())?,
                non_empty(kr.kr_id.as_deref())?,
            )?,
        })
    }

    pub(super) fn source_id(&self) -> String {
        self.source_ref.source_id()
    }

    pub(super) fn locator(&self) -> String {
        self.source_ref.source_ref()
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
