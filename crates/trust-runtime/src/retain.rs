//! Retain storage support.

#![allow(missing_docs)]

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::runtime::RetainSnapshot;
use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, StructValue, TimeOfDayValue, Value,
};
use crate::Runtime;

const RETAIN_MAGIC: &[u8; 4] = b"STRN";
const RETAIN_V1_VERSION: u16 = 1;
const RETAIN_VERSION: u16 = 2;
const RETAIN_TRAILER: &[u8; 4] = b"ENDR";

include!("retain/manager.rs");
include!("retain/store.rs");
include!("retain/codec.rs");
include!("retain/reader.rs");
