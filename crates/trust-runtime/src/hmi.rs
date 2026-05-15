//! HMI schema/value contract helpers.

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use trust_hir::types::Type;

use crate::debug::dap::value_type_name;
use crate::debug::DebugSnapshot;
use crate::error::RuntimeError;
use crate::runtime::RuntimeMetadata;
use crate::value::Value;

mod descriptor;
mod scaffold;

use descriptor::{apply_hmi_dir_descriptor, load_hmi_toml};
pub use descriptor::{load_hmi_dir, load_hmi_scene_view, write_hmi_dir_descriptor};
use descriptor::{load_hmi_dir_impl, map_hmi_dir_page, render_hmi_dir_page_toml};
#[cfg(test)]
use scaffold::parse_hmi_annotation_payload;
use scaffold::{
    collect_scaffold_points, collect_source_symbol_index, escape_toml_string, format_toml_number,
    is_hex_color, parse_annotations, title_case, write_scaffold_file,
};
pub use scaffold::{
    scaffold_hmi_dir, scaffold_hmi_dir_with_sources, scaffold_hmi_dir_with_sources_mode,
};

const HMI_SCHEMA_VERSION: u32 = 1;
const HMI_DESCRIPTOR_VERSION: u32 = 1;
const DEFAULT_PAGE_ID: &str = "overview";
const DEFAULT_TREND_PAGE_ID: &str = "trends";
const DEFAULT_ALARM_PAGE_ID: &str = "alarms";
const DEFAULT_GROUP_NAME: &str = "General";
const DEFAULT_RESPONSIVE_MODE: &str = "auto";
const TREND_HISTORY_LIMIT: usize = 4096;
const ALARM_HISTORY_LIMIT: usize = 1024;
const HMI_DIAG_UNKNOWN_BIND: &str = "HMI_BIND_UNKNOWN_PATH";
const HMI_DIAG_TYPE_MISMATCH: &str = "HMI_BIND_TYPE_MISMATCH";
const HMI_DIAG_UNKNOWN_WIDGET: &str = "HMI_UNKNOWN_WIDGET_KIND";

include!("hmi/contracts.rs");
include!("hmi/persistence.rs");
include!("hmi/customization.rs");
include!("hmi/runtime_views.rs");
include!("hmi/points.rs");
include!("hmi/trust_twin_scene.rs");
include!("hmi/layout.rs");
include!("hmi/catalog.rs");
#[cfg(test)]
mod tests;
