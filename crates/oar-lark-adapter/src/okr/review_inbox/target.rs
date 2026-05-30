use crate::okr::types::{OkrReadKeyResult, OkrReadObjective, OkrReadOkr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OkrKrReviewTarget<'a> {
    pub(super) identity: OkrKrIdentity<'a>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OkrKrIdentity<'a> {
    okr_id: &'a str,
    objective_id: &'a str,
    kr_id: &'a str,
}

impl<'a> OkrKrIdentity<'a> {
    fn from_parts(
        okr: &'a OkrReadOkr,
        objective: &'a OkrReadObjective,
        kr: &'a OkrReadKeyResult,
    ) -> Option<Self> {
        Some(Self {
            okr_id: non_empty(okr.okr_id.as_deref())?,
            objective_id: non_empty(objective.objective_id.as_deref())?,
            kr_id: non_empty(kr.kr_id.as_deref())?,
        })
    }

    pub(super) fn source_id(self) -> String {
        format!(
            "okr:{}:objective:{}:kr:{}",
            self.okr_id, self.objective_id, self.kr_id
        )
    }

    pub(super) fn locator(self) -> String {
        format!(
            "okr://{}/objectives/{}/krs/{}",
            self.okr_id, self.objective_id, self.kr_id
        )
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
