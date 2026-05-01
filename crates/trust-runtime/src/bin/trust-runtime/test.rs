//! ST test runner command.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration as StdDuration, Instant};

use anyhow::Context;
use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{collect_project_source_files, resolve_sources_root};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::CompileSession;
use trust_runtime::Runtime;
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::cli::TestOutput;
use crate::style;

include!("test_cmd/models.rs");
include!("test_cmd/command.rs");
include!("test_cmd/output.rs");
include!("test_cmd/execute.rs");
include!("test_cmd/discovery.rs");

#[cfg(test)]
#[path = "test_cmd/tests.rs"]
mod tests;
