//! The refusals of Identify (specification 35, B3 and B5): each one is the sentence the engineer reads. The `TypeScript`
//! threw an `Error` with the sentence as its message; the same text is carried here, so a refused step reads as it did.

/// A refusal of Identify: the sentence that says why a recording, a setting or a review cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Refusal(pub String);

impl Refusal {
    /// A refusal with the given sentence.
    #[must_use]
    pub fn new(sentence: impl Into<String>) -> Self {
        Self(sentence.into())
    }
}
