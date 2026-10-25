//! Bundle build helpers: a project's sources built into its program image (specification 44 §4.1).

use anyhow::Context;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::SourceFile;

include!("contracts.rs");
include!("build.rs");
include!("deps.rs");

#[cfg(test)]
mod tests {
    //! The tests that build a project need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

    include!("tests.rs");
}
