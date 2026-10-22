//! The kind of an engine answer (specification 35, §0.1a): an answer of a method that answers more than one shape names its kind in
//! `kind`, so that the page shows each answer by the word the engine writes and never by the fields it finds. The engine writes the
//! kind and never reads it, so it is serialize-only, and the answer's type fixes the word: an answer cannot name another kind. The
//! review of an identification is the one answer of `controlStudio/identify` without it: only `review` answers it, and its `kind`
//! names the experiment the campaign runs.

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use super::{
    IdentifyResult, ImportedRecording, KeepResult, KeepReviewAnswer, ManualIdentificationAnswer,
    OperationStarted, ProjectTestResult, ResetResult, TuneResult,
};
use crate::commissioning::application_records::{ApplicationReview, ApplyOutcome, RestoreOutcome};
use crate::tuning::search::SearchAnswer;

/// An engine answer that names its kind (§0.1a).
pub trait NamedAnswer {
    /// The word the answer's `kind` field holds.
    const KIND: &'static str;
}

/// The `kind` field of an answer of the type `T`: it holds nothing and writes `T`'s word ([`NamedAnswer::KIND`]).
pub struct AnswerKind<T>(PhantomData<fn() -> T>);

impl<T> AnswerKind<T> {
    /// The kind of an answer of the type `T`.
    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Default for AnswerKind<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for AnswerKind<T> {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl<T> PartialEq for AnswerKind<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> Eq for AnswerKind<T> {}

impl<T: NamedAnswer> fmt::Debug for AnswerKind<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(T::KIND)
    }
}

impl<T: NamedAnswer> Serialize for AnswerKind<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(T::KIND)
    }
}

impl<T: NamedAnswer> JsonSchema for AnswerKind<T> {
    /// The word is written in place, so the page's declaration names it as the field's one value.
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Owned(format!("AnswerKind_{}", T::KIND))
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "const": T::KIND })
    }
}

/// An identification's answer: the model the study now holds (B3.8), as Identify, an imported model and a fitted pair answer it.
impl NamedAnswer for IdentifyResult {
    const KIND: &'static str = "identification";
}

/// A project test that recorded no model the fit supports (C12, B3.5).
impl NamedAnswer for ProjectTestResult {
    const KIND: &'static str = "projectTest";
}

/// The Recorded-data fit's state (B3.3).
impl NamedAnswer for ManualIdentificationAnswer {
    const KIND: &'static str = "manualIdentification";
}

/// The new working study of a reset (A8.4).
impl NamedAnswer for ResetResult {
    const KIND: &'static str = "reset";
}

/// A recording file the study keeps (B3.6).
impl NamedAnswer for ImportedRecording {
    const KIND: &'static str = "importedRecording";
}

/// A long operation that started, whose updates follow.
impl NamedAnswer for OperationStarted {
    const KIND: &'static str = "operationStarted";
}

/// The review of an Apply or a Restore (B6.1, B6.4).
impl NamedAnswer for ApplicationReview {
    const KIND: &'static str = "applicationReview";
}

/// The review of a Keep in project (B6.5).
impl NamedAnswer for KeepReviewAnswer {
    const KIND: &'static str = "keepReview";
}

/// The end of an Apply (B6.3).
impl NamedAnswer for ApplyOutcome {
    const KIND: &'static str = "applyOutcome";
}

/// The end of a Restore (B6.4).
impl NamedAnswer for RestoreOutcome {
    const KIND: &'static str = "restoreOutcome";
}

/// The end of a Keep in project (B6.5).
impl NamedAnswer for KeepResult {
    const KIND: &'static str = "keepResult";
}

/// A Tune's candidate, or a manual preview of one (B4.3 to B4.6).
impl NamedAnswer for TuneResult {
    const KIND: &'static str = "tuneResult";
}

/// A finite search's trials (C5).
impl NamedAnswer for SearchAnswer {
    const KIND: &'static str = "searchAnswer";
}

#[cfg(test)]
#[path = "answer_kind_tests.rs"]
mod tests;
