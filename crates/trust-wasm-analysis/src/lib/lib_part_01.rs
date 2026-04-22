use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use text_size::{TextRange, TextSize};
use trust_hir::db::FileId;
use trust_hir::project::{Project, SourceKey};
use trust_hir::DiagnosticSeverity;
use trust_ide::StdlibFilter;

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
use wasm_bindgen::prelude::wasm_bindgen;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    message: String,
}

impl EngineError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EngineError {}

type EngineResult<T> = Result<T, EngineError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentInput {
    pub uri: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoadedDocument {
    pub uri: String,
    pub file_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplyDocumentsResult {
    pub documents: Vec<LoadedDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineStatus {
    pub document_count: usize,
    pub uris: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ByteSpan {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelatedInfoItem {
    pub range: Range,
    pub span: ByteSpan,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticItem {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub range: Range,
    pub span: ByteSpan,
    pub related: Vec<RelatedInfoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AstSimilarityRequest {
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalAstSummary {
    pub algorithm: String,
    pub gram_size: usize,
    pub parse_error_count: usize,
    pub stream: Vec<String>,
    pub five_grams: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AstSimilarityResult {
    pub algorithm: String,
    pub gram_size: usize,
    pub score: f32,
    pub shared_grams: usize,
    pub left_grams: usize,
    pub right_grams: usize,
    pub threshold_070: bool,
    pub threshold_095: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HoverRequest {
    pub uri: String,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HoverItem {
    pub contents: String,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletionRequest {
    pub uri: String,
    pub position: Position,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletionTextEditItem {
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: String,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub text_edit: Option<CompletionTextEditItem>,
    pub sort_priority: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferencesRequest {
    pub uri: String,
    pub position: Position,
    pub include_declaration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferenceItem {
    pub uri: String,
    pub range: Range,
    pub is_write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefinitionRequest {
    pub uri: String,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefinitionItem {
    pub uri: String,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentHighlightRequest {
    pub uri: String,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentHighlightItem {
    pub range: Range,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenameRequest {
    pub uri: String,
    pub position: Position,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenameEdit {
    pub uri: String,
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug, Default)]
pub struct BrowserAnalysisEngine {
    project: Project,
    documents: BTreeMap<String, String>,
}
