//! Runtime historian and observability helpers.

#![allow(missing_docs)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use glob::Pattern;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use tracing::warn;

use crate::debug::DebugSnapshot;
use crate::error::RuntimeError;
use crate::metrics::RuntimeMetricsSnapshot;
use crate::value::Value;

include!("types.rs");
include!("service.rs");
include!("sampling.rs");
include!("alerts.rs");
include!("metrics.rs");

#[cfg(test)]
mod tests;
