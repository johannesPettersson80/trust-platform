//! The front of the build (specification 44 §3, steps 1 and 2): the project's sources checked by
//! trust-hir and lowered to their declarations, with each source's syntax tree and what type
//! checking learned of its expressions, which the generator reads the statements from.

use std::collections::HashMap;
use std::sync::Arc;

use trust_hir::db::{FileAnalysis, SemanticDatabase};
use trust_hir::SymbolId;
use trust_hir::TypeId;
use trust_syntax::syntax::SyntaxNode;

use super::NativeError;
use crate::harness::{lower_application_and_frontend, LoweredApplication, SourceFile};

/// One source file.
pub(super) struct Source {
    /// The file's label: its path, or `file_<n>` without one.
    pub(super) label: String,
    /// The text.
    pub(super) text: String,
    /// The syntax tree.
    pub(super) root: SyntaxNode,
    /// What type checking learned of the file's expressions.
    pub(super) analysis: Arc<FileAnalysis>,
    /// The byte offset where each line starts.
    line_starts: Vec<u32>,
}

impl Source {
    /// The 1-based line and column of byte `offset`.
    pub(super) fn position(&self, offset: u32) -> (u32, u32) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next.saturating_sub(1),
        };
        let start = self.line_starts.get(line).copied().unwrap_or(0);
        let column = self
            .text
            .get(start as usize..offset as usize)
            .map_or(0, |prefix| prefix.chars().count());
        (
            u32::try_from(line + 1).unwrap_or(u32::MAX),
            u32::try_from(column + 1).unwrap_or(u32::MAX),
        )
    }

    /// The type trust-hir gave the expression `node`.
    pub(super) fn type_of(&self, node: &SyntaxNode) -> Option<TypeId> {
        let range = node.text_range();
        self.analysis
            .expression_types
            .get(&(u32::from(range.start()), u32::from(range.end())))
            .copied()
            .filter(|ty| *ty != TypeId::UNKNOWN)
    }

    /// The symbol trust-hir resolved `node` to: a name, a member, a callee, or the POU whose
    /// parameters a call binds.
    pub(super) fn symbol_of(&self, node: &SyntaxNode) -> Option<SymbolId> {
        let range = node.text_range();
        self.analysis
            .resolved_symbols
            .get(&(u32::from(range.start()), u32::from(range.end())))
            .copied()
    }

    /// Where `node` is, for a diagnostic: `label:line`.
    pub(super) fn at(&self, node: &SyntaxNode) -> String {
        let (line, _) = self.position(first_token_start(node));
        format!("{}:{line}", self.label)
    }
}

/// The start of the first token of `node` that is not whitespace or a comment: a node's range
/// begins with the trivia before it.
pub(super) fn first_token_start(node: &SyntaxNode) -> u32 {
    node.descendants_with_tokens()
        .find_map(|element| match element.into_token() {
            Some(token) if !token.kind().is_trivia() => Some(u32::from(token.text_range().start())),
            _ => None,
        })
        .unwrap_or_else(|| u32::from(node.text_range().start()))
}

/// The end of the last token of `node` that is not whitespace or a comment.
pub(super) fn last_token_end(node: &SyntaxNode) -> u32 {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .last()
        .map_or_else(
            || u32::from(node.text_range().end()),
            |token| u32::from(token.text_range().end()),
        )
}

/// A checked project.
pub(super) struct Front {
    /// The lowered declarations.
    pub(super) lowered: LoweredApplication,
    /// The sources, in the project's order.
    pub(super) sources: Vec<Source>,
}

impl Front {
    /// Checks and lowers `sources`.
    ///
    /// # Errors
    ///
    /// [`NativeError::Compile`] with every error trust-hir reports, labelled by file.
    pub(super) fn analyze(sources: &[SourceFile]) -> Result<Self, NativeError> {
        let (lowered, frontend) = lower_application_and_frontend(
            sources,
            true,
            crate::value::StorageAbi::UniformTemporal64,
        )
        .map_err(|error| NativeError::Compile(error.to_string()))?;
        // The configuration as the runtime prepares it for its own build: a project without a
        // CONFIGURATION runs each PROGRAM as one instance of its name, without a task, and the
        // program types and instances are resolved and checked by the same rules (specification
        // 18 §2.2).
        let lowered = lowered
            .prepare_authoring(&[])
            .map_err(|error| NativeError::Compile(error.to_string()))?;
        let database = frontend.project.database();
        let mut checked = Vec::with_capacity(sources.len());
        for (index, ((file_id, root), source)) in frontend
            .file_ids
            .iter()
            .zip(frontend.syntaxes.iter())
            .zip(sources)
            .enumerate()
        {
            let mut line_starts = vec![0u32];
            for (offset, byte) in source.text.bytes().enumerate() {
                if byte == b'\n' {
                    line_starts.push(u32::try_from(offset + 1).unwrap_or(u32::MAX));
                }
            }
            checked.push(Source {
                label: source
                    .path
                    .clone()
                    .unwrap_or_else(|| format!("file_{index}")),
                text: source.text.clone(),
                root: root.clone(),
                analysis: database.analyze(*file_id),
                line_starts,
            });
        }
        Ok(Self {
            lowered,
            sources: checked,
        })
    }

    /// The statement ids of every statement in every POU body, in source order: file by file,
    /// statement by statement (specification 44 §3, step 2).
    pub(super) fn statement_ids(&self) -> StatementIds {
        let mut ids = StatementIds::default();
        for (file, source) in self.sources.iter().enumerate() {
            for node in source.root.descendants() {
                if is_statement(node.kind()) {
                    let id = u32::try_from(ids.order.len()).unwrap_or(u32::MAX);
                    ids.by_node
                        .insert((file, u32::from(node.text_range().start())), id);
                    ids.order.push((file, node));
                }
            }
        }
        ids
    }
}

/// Whether `kind` is a statement that executes: an empty statement does nothing and has no id.
pub(super) fn is_statement(kind: trust_syntax::syntax::SyntaxKind) -> bool {
    use trust_syntax::syntax::SyntaxKind as K;
    matches!(
        kind,
        K::AssignStmt
            | K::ExprStmt
            | K::IfStmt
            | K::CaseStmt
            | K::ForStmt
            | K::WhileStmt
            | K::RepeatStmt
            | K::ReturnStmt
            | K::ExitStmt
            | K::ContinueStmt
            | K::JmpStmt
            | K::LabelStmt
    )
}

/// The id of every statement.
#[derive(Default)]
pub(super) struct StatementIds {
    by_node: HashMap<(usize, u32), u32>,
    /// The statements by id: their file and node.
    pub(super) order: Vec<(usize, SyntaxNode)>,
}

impl StatementIds {
    /// The id of the statement `node` of file `file`.
    pub(super) fn id(&self, file: usize, node: &SyntaxNode) -> Option<u32> {
        self.by_node
            .get(&(file, u32::from(node.text_range().start())))
            .copied()
    }
}
