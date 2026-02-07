pub use super::core::{
    document_highlight, goto_declaration, goto_definition, goto_implementation,
    goto_type_definition, prepare_rename, references_with_progress, rename, selection_range,
};

#[cfg(test)]
pub use super::core::references;
