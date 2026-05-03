//! PLCopen XML interchange (ST-focused subset profile).

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use trust_syntax::lexer::{lex, TokenKind};
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

include!("plcopen/contracts.rs");
include!("plcopen/profile.rs");
include!("plcopen/export.rs");
include!("plcopen/import.rs");
include!("plcopen/source_parse.rs");
include!("plcopen/export_helpers.rs");
include!("plcopen/xml_common.rs");
include!("plcopen/codesys_structure.rs");
include!("plcopen/pou_interface.rs");
include!("plcopen/pou_externals.rs");
include!("plcopen/codesys_export_meta.rs");
include!("plcopen/st_extract.rs");
include!("plcopen/import_data_globals.rs");
include!("plcopen/import_project_model.rs");
include!("plcopen/type_parser.rs");
include!("plcopen/shims_metrics.rs");

#[cfg(test)]
#[path = "plcopen/tests.rs"]
mod tests;
