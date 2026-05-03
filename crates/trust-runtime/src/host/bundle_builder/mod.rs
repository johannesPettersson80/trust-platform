//! Bundle build helpers (compile sources to program.stbc).

use anyhow::Context;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{CompileSession, SourceFile};

include!("contracts.rs");
include!("build.rs");
include!("deps.rs");

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
