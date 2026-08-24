//! Adapter unit tests.
//! - stdio framing roundtrips
//! - request dispatch smoke tests

use super::protocol_io::{read_message, write_message};
use super::*;
use crate::protocol::{
    BreakpointLocationsArguments, BreakpointLocationsResponseBody, ContinueArguments,
    EvaluateArguments, EvaluateResponseBody, Event, InitializeArguments, InitializeResponseBody,
    IoReleaseArguments, IoStateEventBody, IoWriteArguments, MessageType, NextArguments,
    PauseArguments, Request, Response, ScopesArguments, ScopesResponseBody,
    SetBreakpointsArguments, SetBreakpointsResponseBody, SetExpressionArguments,
    SetExpressionResponseBody, Source, SourceBreakpoint, StackTraceArguments,
    StackTraceResponseBody, StepInArguments, StepOutArguments, ThreadsResponseBody,
    VariablesArguments, VariablesResponseBody,
};
use crate::DebugSession;
use indexmap::IndexMap;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::io::BufReader;
use trust_hir::{Type, TypeId};
use trust_runtime::debug::{DebugControl, DebugHook, DebugStopReason, SourceLocation};
use trust_runtime::harness::TestHarness;
use trust_runtime::io::IoAddress;
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::Value as RuntimeValue;
use trust_runtime::value::{ArrayValue, Duration, StructValue};
use trust_runtime::Runtime;

#[path = "lifecycle_contract_tests.rs"]
mod lifecycle_contract_tests;
#[path = "tests_part_01.rs"]
mod tests_part_01;
#[path = "tests_part_02.rs"]
mod tests_part_02;
#[path = "tests_part_03.rs"]
mod tests_part_03;
#[path = "tests_part_04.rs"]
mod tests_part_04;
#[path = "tests_part_05.rs"]
mod tests_part_05;
#[path = "tests_part_06.rs"]
mod tests_part_06;
