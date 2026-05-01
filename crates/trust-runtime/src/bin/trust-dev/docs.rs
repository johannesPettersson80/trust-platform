//! API documentation generation from tagged ST comments.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::resolve_sources_root;
use trust_syntax::lexer::{self, Token, TokenKind};
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::cli::DocsFormat;
use crate::style;

include!("docs/models.rs");
include!("docs/command.rs");
include!("docs/source_collect.rs");
include!("docs/syntax_helpers.rs");
include!("docs/tag_parser.rs");
include!("docs/render.rs");

#[cfg(test)]
#[path = "docs/tests.rs"]
mod tests;
