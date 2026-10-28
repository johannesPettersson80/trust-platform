//! The typed refusals of the HMI authoring engine (specification 43, §4).
//!
//! A refusal carries one stable code of the tool contract and the sentence that names what to
//! change. Displayed, it reads `CODE: message`, as the TypeScript engine's thrown errors did.

use std::fmt;

/// The request or one of its fields is not valid (specification 43, §4).
pub(super) const INVALID_ARGUMENT: &str = "INVALID_ARGUMENT";
/// The request names a contract version this engine does not serve.
pub(super) const UNSUPPORTED_VERSION: &str = "UNSUPPORTED_VERSION";
/// The project or the document it names is not available.
pub(super) const PROJECT_UNAVAILABLE: &str = "PROJECT_UNAVAILABLE";
/// The request was made against a revision that has since changed.
pub(super) const CONFLICT: &str = "CONFLICT";
/// The request was cancelled before it committed.
pub(super) const CANCELLED: &str = "CANCELLED";
/// The draft or page would not be valid, or a rule of the page refuses the change.
pub(super) const VALIDATION_FAILED: &str = "VALIDATION_FAILED";
/// A product limit (batch size, input size, storage, draft count) is exceeded.
pub(super) const LIMIT_EXCEEDED: &str = "LIMIT_EXCEEDED";
/// The draft was applied or discarded, or its pages are no longer retained.
pub(super) const EXPIRED_DRAFT: &str = "EXPIRED_DRAFT";
/// The PLC program does not compile, so the HMI cannot be checked against it (specification 43, §3, §10.1).
pub(super) const COMPILATION_FAILED: &str = "COMPILATION_FAILED";
/// The request needs a context this connection cannot resolve, such as a remote or virtual workspace.
pub(super) const UNSUPPORTED_CONTEXT: &str = "UNSUPPORTED_CONTEXT";
/// Rendering or reading a project image failed: the image is missing or cannot be read.
pub(super) const RENDER_FAILED: &str = "RENDER_FAILED";

/// A refusal of the authoring engine: its stable code and its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentError {
    /// One of the stable codes above.
    pub(super) code: &'static str,
    /// The sentence that names the values involved and the change to make.
    pub(super) message: String,
}

/// The result of an authoring operation.
pub(super) type AgentResult<T> = Result<T, AgentError>;

impl AgentError {
    /// A refusal with the given code.
    pub(super) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// An `INVALID_ARGUMENT` refusal.
    pub(super) fn invalid(message: impl Into<String>) -> Self {
        Self::new(INVALID_ARGUMENT, message)
    }

    /// A `CONFLICT` refusal.
    pub(super) fn conflict(message: impl Into<String>) -> Self {
        Self::new(CONFLICT, message)
    }

    /// A `VALIDATION_FAILED` refusal.
    pub(super) fn validation(message: impl Into<String>) -> Self {
        Self::new(VALIDATION_FAILED, message)
    }

    /// A `LIMIT_EXCEEDED` refusal.
    pub(super) fn limit(message: impl Into<String>) -> Self {
        Self::new(LIMIT_EXCEEDED, message)
    }
}

impl fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AgentError {}
