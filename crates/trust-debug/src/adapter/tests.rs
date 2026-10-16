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

// These I/O-only presentation fixtures used to invent bindings on a live
// Runtime. Declare their scalar destinations before consuming the builder.
fn install_io_fixture(mut builder: trust_runtime::RuntimeBuilder) -> Runtime {
    let bindings = builder.io().bindings().to_vec();
    for binding in bindings {
        let trust_runtime::io::IoTarget::Name(name) = binding.target else {
            panic!("I/O presentation fixture requires a named declaration");
        };
        let type_id = if let trust_runtime::io::IoSize::Bytes(length) = binding.address.size {
            builder.registry_mut().register_string_with_length(length)
        } else {
            binding
                .value_type
                .unwrap_or_else(|| super::io::io_type_id(&binding.address))
        };
        builder
            .register_global(trust_runtime::program_model::VarDef {
                name,
                type_id,
                initializer: None,
                retain: trust_runtime::RetainPolicy::Unspecified,
                static_storage: false,
                external: false,
                in_out: false,
                constant: false,
                address: None,
            })
            .unwrap();
    }
    builder.build().unwrap()
}

#[path = "hosted_observation_frames_tests.rs"]
mod hosted_observation_frames_tests;
#[path = "installed_context_tests.rs"]
mod installed_context_tests;
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
