//! Integration tests for IDE features.

use text_size::TextSize;

use trust_hir::db::{FileId, SourceDatabase};
use trust_hir::Database;
use trust_ide::completion::{complete, CompletionKind};
use trust_ide::diagnostics::collect_diagnostics;
use trust_ide::hover;
use trust_ide::references::{find_references, FindReferencesOptions};
use trust_ide::rename::rename;
use trust_ide::semantic_tokens::{semantic_tokens, SemanticTokenType};
use trust_ide::{goto_definition, goto_implementation, signature_help};

fn setup(source: &str) -> (Database, FileId) {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    (db, file)
}

// =============================================================================
// Completion Context Tests
// =============================================================================

#[path = "ide_features/ide_features_part_01.rs"]
mod ide_features_part_01;
#[path = "ide_features/ide_features_part_02.rs"]
mod ide_features_part_02;
#[path = "ide_features/ide_features_part_03.rs"]
mod ide_features_part_03;
#[path = "ide_features/ide_features_part_04.rs"]
mod ide_features_part_04;
#[path = "ide_features/ide_features_part_05.rs"]
mod ide_features_part_05;
#[path = "ide_features/ide_features_part_06.rs"]
mod ide_features_part_06;
