#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeishuOkrProgressListTarget {
    Objective(String),
    KeyResult(String),
}

impl FeishuOkrProgressListTarget {
    pub fn id(&self) -> &str {
        match self {
            FeishuOkrProgressListTarget::Objective(id)
            | FeishuOkrProgressListTarget::KeyResult(id) => id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OkrUserIdType {
    OpenId,
    UserId,
    UnionId,
}

impl OkrUserIdType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OkrUserIdType::OpenId => "open_id",
            OkrUserIdType::UserId => "user_id",
            OkrUserIdType::UnionId => "union_id",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OkrDepartmentIdType {
    OpenDepartmentId,
    DepartmentId,
}

impl OkrDepartmentIdType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OkrDepartmentIdType::OpenDepartmentId => "open_department_id",
            OkrDepartmentIdType::DepartmentId => "department_id",
        }
    }
}
