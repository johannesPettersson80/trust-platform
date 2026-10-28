//! `trust-ide` - IDE features for IEC 61131-3 Structured Text.
//!
//! This crate provides IDE functionality built on top of `trust-hir`:
//!
//! - **Completion**: Context-aware autocomplete suggestions
//! - **Go to Definition**: Navigate to symbol declarations
//! - **Hover**: Display type information and documentation
//! - **Find References**: Find all usages of a symbol
//! - **Rename**: Safe symbol renaming
//! - **Diagnostics**: Error and warning collection
//! - **Semantic Tokens**: Rich syntax highlighting
//!
//! # Architecture
//!
//! All IDE features are implemented as pure functions that take a database
//! and position/range parameters, making them easy to test and compose.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

pub mod call_hierarchy;
pub mod completion;
pub mod diagnostics;
pub mod goto_def;
pub mod hover;
pub mod implementation;
pub mod inlay_hints;
/// Inline value hints for constant/enum references.
pub mod inline_values;
pub mod linked_editing;
pub mod refactor;
pub mod references;
pub mod rename;
pub mod rename_identity;
pub mod selection_range;
pub mod semantic_tokens;
pub mod signature_help;
pub mod stdlib_docs;
/// Shared source text-range helpers.
pub mod text_range;
pub mod type_hierarchy;
pub mod util;
/// Shared helpers for VAR/CONSTANT declaration inspection.
pub mod var_decl;

pub use call_hierarchy::{
    incoming_calls, incoming_calls_in_files, outgoing_calls, outgoing_calls_in_files,
    prepare_call_hierarchy, prepare_call_hierarchy_in_files, CallHierarchyIncomingCall,
    CallHierarchyItem, CallHierarchyOutgoingCall,
};
pub use completion::{complete, complete_with_filter, CompletionItem, CompletionKind};
pub use goto_def::{goto_declaration, goto_definition, goto_type_definition, DefinitionResult};
pub use hover::{hover, hover_with_filter, HoverResult};
pub use implementation::{goto_implementation, ImplementationResult};
pub use inlay_hints::{inlay_hints, InlayHint, InlayHintKind};
pub use inline_values::{
    inline_value_data, inline_value_hints, InlineValueData, InlineValueHint, InlineValueScope,
    InlineValueTarget,
};
pub use linked_editing::linked_editing_ranges;
pub use refactor::{
    convert_function_block_to_function, convert_function_to_function_block, extract_method,
    extract_pou, extract_property, generate_interface_stubs, inline_symbol, move_namespace_path,
    ExtractResult, ExtractTargetKind, InlineResult, InlineTargetKind,
};
pub use references::{find_references, FindReferencesOptions, Reference};
pub use rename::rename;
pub use selection_range::{selection_ranges, SelectionRange};
pub use semantic_tokens::{semantic_tokens, SemanticToken, SemanticTokenType};
pub use signature_help::{
    call_signature_info, signature_help, CallSignatureInfo, CallSignatureParam, Signature,
    SignatureHelpResult, SignatureParameter,
};
pub use stdlib_docs::StdlibFilter;
pub use type_hierarchy::{prepare_type_hierarchy, subtypes, supertypes, TypeHierarchyItem};
pub use util::symbol_name_at_position;
