//! Retain storage support.

#![allow(missing_docs)]

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::RuntimeError;
use crate::runtime::RetainSnapshot;
use crate::value::{Duration, Value};

include!("retain/manager.rs");
include!("retain/store.rs");
#[cfg(test)]
include!("retain/store_tests.rs");
include!("retain/codec.rs");
include!("retain/reader.rs");
