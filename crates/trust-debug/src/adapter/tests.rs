//! Adapter unit tests.
//! - stdio framing roundtrips
//! - request dispatch smoke tests

use super::protocol_io::{read_message, write_message};
use super::*;
use crate::protocol::{
    Event, InitializeArguments, InitializeResponseBody, IoReleaseArguments, IoWriteArguments,
    MessageType, Response, SetExpressionArguments, SetExpressionResponseBody, ThreadsResponseBody,
};
use crate::DebugSession;
use std::collections::BTreeMap;
use std::io::BufReader;

#[path = "attach_breakpoints_tests.rs"]
mod attach_breakpoints_tests;
#[path = "lifecycle_contract_tests.rs"]
mod lifecycle_contract_tests;
#[path = "tests_part_01.rs"]
mod tests_part_01;
#[path = "tests_part_02.rs"]
mod tests_part_02;
#[path = "tests_part_03.rs"]
mod tests_part_03;
#[path = "variable_paging_tests.rs"]
mod variable_paging_tests;
