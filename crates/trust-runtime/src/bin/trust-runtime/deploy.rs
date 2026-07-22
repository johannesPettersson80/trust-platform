//! Bundle deployment, versioning, and rollback.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use indicatif::{ProgressBar, ProgressStyle};
use trust_runtime::config::{IoConfig, RuntimeBundle, RuntimeConfig};
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
