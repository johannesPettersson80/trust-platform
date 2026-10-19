//! Bundle deployment, versioning, and rollback.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use indicatif::{ProgressBar, ProgressStyle};
use trust_runtime::config::{IoConfig, RuntimeBundle, RuntimeConfig};
use trust_runtime::deployment::program_store::{
    bundle_targets, claim_label, copy_bundle, default_bundle_label, prune_bundles,
    resolve_existing_bundle_link, restore_pointer_pair, rollback, scratch_path,
    update_deployment_pointers, validate_pointer_slot, BUNDLES_DIR, CURRENT_LINK, PREVIOUS_LINK,
};
#[cfg(test)]
use trust_runtime::deployment::program_store::{
    read_link_target, replace_symlink, validate_bundle_label, validate_bundle_relative_path,
};
use trust_runtime::io::IoDriverRegistry;
use trust_runtime::watchdog::WatchdogPolicy;

use crate::style;

include!("deploy/commands.rs");
include!("deploy/bundle_io.rs");
include!("deploy/summary_models.rs");
include!("deploy/diffs.rs");
include!("deploy/helpers.rs");

#[cfg(test)]
mod tests {
    use super::*;

    include!("deploy/tests.rs");
}
