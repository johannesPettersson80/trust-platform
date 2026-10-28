//! The visual programs of truST: Ladder, Blockly, Statechart and SFC (specification 25 §11.4,
//! specification 17).
//!
//! Each visual program is a JSON file that its editor draws. This crate holds every rule about
//! those files: their formats and checks, the edits an editor stores in them, the Structured Text
//! companion and runtime entry that execute them (specification 17 §3), the template of a new
//! program and the rules of an import. The VS Code extension draws the editors and forwards each
//! request here through the `trust-lsp.visual*` commands.

#![allow(clippy::module_name_repetitions)]

pub mod blockly;
pub mod companion;
pub mod direct_io;
pub mod import;
pub mod js;
pub mod json;
pub mod kind;
pub mod ladder;
pub mod naming;
pub mod new_program;
pub mod sfc;
pub mod statechart;

pub use json::Json;
pub use kind::VisualKind;

/// One problem a visual program has, for the editor to show.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Problem {
    /// What is wrong, in a sentence.
    pub message: String,
    /// The id of the element (node, step, transition) the problem is about, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
}

impl Problem {
    /// A problem about the whole program.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            element: None,
        }
    }

    /// A problem about the element `element`.
    #[must_use]
    pub fn at(element: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            element: Some(element.into()),
        }
    }
}

/// The companion function block of a visual program and the globals its runtime entry declares
/// (specification 17 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    /// The `FUNCTION_BLOCK … END_FUNCTION_BLOCK` text, and for an SFC with actions the type of its
    /// action flags after it, without the companion file's banner.
    pub function_block: String,
    /// The globals of the runtime entry, after the function block instance.
    pub globals: Vec<Global>,
    /// Warnings about parts of the program the companion could not express.
    pub warnings: Vec<String>,
}

/// One global of the runtime entry: `name [AT address] : type [:= initial];`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    /// The variable name.
    pub name: String,
    /// The direct address it is declared `AT`, canonical.
    pub address: Option<String>,
    /// The Structured Text type.
    pub ty: String,
    /// The initial value as a Structured Text literal.
    pub initial: Option<String>,
}

impl Global {
    /// The declaration line of the runtime entry's `VAR_GLOBAL`, indented by two spaces.
    #[must_use]
    pub fn declaration(&self) -> String {
        let address = self
            .address
            .as_deref()
            .map(|address| format!(" AT {address}"))
            .unwrap_or_default();
        let initial = self
            .initial
            .as_deref()
            .map(|initial| format!(" := {initial}"))
            .unwrap_or_default();
        format!("  {}{address} : {}{initial};", self.name, self.ty)
    }
}
