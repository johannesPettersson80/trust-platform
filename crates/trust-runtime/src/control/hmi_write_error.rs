#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HmiWriteFailure {
    TypeMismatch,
    StringCapacityExceeded,
    SubrangeViolation,
    NonFiniteValue,
}

impl HmiWriteFailure {
    const fn stable_code(self) -> crate::error::StableErrorCode {
        match self {
            Self::TypeMismatch => crate::error::StableErrorCode::RuntimeTypeMismatch,
            Self::StringCapacityExceeded => {
                crate::error::StableErrorCode::RuntimeStringCapacityExceeded
            }
            Self::SubrangeViolation => crate::error::StableErrorCode::RuntimeSubrangeViolation,
            Self::NonFiniteValue => crate::error::StableErrorCode::RuntimeNonFiniteValue,
        }
    }
}

#[derive(Debug)]
pub(super) struct HmiWriteError {
    message: String,
    code: Option<crate::error::StableErrorCode>,
}

impl HmiWriteError {
    fn plain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
        }
    }

    fn admission(message: impl Into<String>, failure: HmiWriteFailure) -> Self {
        Self {
            message: message.into(),
            code: Some(failure.stable_code()),
        }
    }
}
