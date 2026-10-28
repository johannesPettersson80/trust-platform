//! The `set`, `var.force`, `var.unforce` and `var.forced` operation contract: the same
//! parameters, refusals, error texts and JSON as trust-runtime answers today (specification 11,
//! "Live Values"; specification 34 §4.3.1).
//!
//! Every expected value is the stable answer the hosted handlers give today, moved here with the
//! handlers. The platform's queues are the shared `LiveValueQueues` behind the `VariableQueuePort`,
//! so a moved test keeps its exact expectations.

use alloc::vec::Vec;
use core::cell::RefCell;

use serde_json::{json, Value as JsonValue};
use smol_str::SmolStr;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::value::Value;

use super::*;
use crate::live_values::queues::{ForcedVar, LiveValueQueues, PendingVarWrite};

/// A platform's Live Values variable queues over the shared `LiveValueQueues`, as `DebugControl`
/// holds them. The inherent helpers set up a test's starting state and read the queues back.
struct FakeQueues {
    queues: RefCell<LiveValueQueues>,
    generation: Generation,
    /// The image of the program the requests write, when the platform has one at hand.
    image: Option<Vec<u8>>,
}

impl FakeQueues {
    fn new() -> Self {
        Self::with_generation(Generation::FIRST)
    }

    fn with_generation(generation: Generation) -> Self {
        Self {
            queues: RefCell::new(LiveValueQueues::new()),
            generation,
            image: None,
        }
    }

    /// The queues of a platform that has the typed fixture's program at hand.
    fn with_program() -> Self {
        Self {
            image: Some(crate::live_values::typed_fixture::image()),
            ..Self::new()
        }
    }

    fn enqueue_global_write(&self, name: &str, value: Value) {
        self.queues
            .borrow_mut()
            .try_upsert_var_write(PendingVarTarget::Global(SmolStr::new(name)), value)
            .expect("the test queue is not full");
    }

    fn force_global(&self, name: &str, value: Value) {
        self.queues
            .borrow_mut()
            .try_upsert_forced_var(ForcedVarTarget::Global(SmolStr::new(name)), value)
            .expect("the test queue is not full");
    }

    fn force_retain(&self, name: &str, value: Value) {
        self.queues
            .borrow_mut()
            .try_upsert_forced_var(ForcedVarTarget::Retain(SmolStr::new(name)), value)
            .expect("the test queue is not full");
    }

    fn force_instance(
        &self,
        generation: Generation,
        instance: InstanceId,
        name: &str,
        value: Value,
    ) {
        self.queues
            .borrow_mut()
            .try_upsert_forced_var(
                ForcedVarTarget::Instance(generation, instance, SmolStr::new(name)),
                value,
            )
            .expect("the test queue is not full");
    }

    fn drain_var_writes(&self) -> Vec<PendingVarWrite> {
        self.queues.borrow_mut().drain_var_writes()
    }

    fn forced_vars(&self) -> Vec<ForcedVar> {
        self.queues.borrow().forced_vars.clone()
    }
}

impl VariableQueuePort for FakeQueues {
    fn generation(&self) -> Generation {
        self.generation
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.queues.borrow_mut().try_upsert_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        self.queues
            .borrow_mut()
            .try_upsert_forced_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.queues.borrow_mut().clear_forced_vars(
            |target| matches!(target, ForcedVarTarget::Global(current) if current.as_str() == name),
        );
    }

    fn release_retain(&self, name: &str) {
        self.queues.borrow_mut().clear_forced_vars(
            |target| matches!(target, ForcedVarTarget::Retain(current) if current.as_str() == name),
        );
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.queues.borrow_mut().clear_forced_vars(|target| {
            matches!(target, ForcedVarTarget::Instance(_, current_id, current_name) if *current_id == instance && current_name.as_str() == name)
        });
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.queues.borrow().forced_snapshot()
    }

    fn with_program_image(
        &self,
        read: &mut dyn FnMut(&trust_runtime_core::native::image::Image<'_>),
    ) -> bool {
        let Some(bytes) = self.image.as_deref() else {
            return false;
        };
        read(&trust_runtime_core::native::image::Image::parse(bytes).expect("the image parses"));
        true
    }
}

fn result(response: ControlResponse) -> JsonValue {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result.expect("response result")
}

fn assert_invalid_params(response: &ControlResponse) {
    assert!(!response.ok, "invalid parameters must reject");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("invalid params")),
        "missing invalid-params diagnostic: {:?}",
        response.error
    );
}

fn assert_unsupported_value(response: &ControlResponse) {
    assert!(!response.ok, "unsupported scalar must reject");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unsupported value")),
        "missing unsupported-value diagnostic: {:?}",
        response.error
    );
}

fn set_params(target: &str, value: &str) -> JsonValue {
    json!({"target": target, "value": value})
}

fn target_params(target: &str) -> JsonValue {
    json!({"target": target})
}

#[test]
fn set_requires_params() {
    let response = set(6, None, &FakeQueues::new());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! set_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(&set(200, Some($params), &FakeQueues::new()));
        }
    };
}

set_invalid_params_case!(set_rejects_null_params, JsonValue::Null);
set_invalid_params_case!(set_rejects_array_params, json!([]));
set_invalid_params_case!(set_rejects_string_params, json!("global:x"));
set_invalid_params_case!(set_rejects_missing_target, json!({"value": "1"}));
set_invalid_params_case!(set_rejects_missing_value, json!({"target": "global:x"}));
set_invalid_params_case!(
    set_rejects_unknown_field,
    json!({"target": "global:x", "value": "1", "immediate": true})
);
set_invalid_params_case!(
    set_rejects_null_target,
    json!({"target": null, "value": "1"})
);
set_invalid_params_case!(
    set_rejects_numeric_target,
    json!({"target": 1, "value": "1"})
);
set_invalid_params_case!(
    set_rejects_null_value,
    json!({"target": "global:x", "value": null})
);
set_invalid_params_case!(
    set_rejects_boolean_value,
    json!({"target": "global:x", "value": true})
);

#[test]
fn set_trims_and_queues_global_target() {
    let queues = FakeQueues::new();
    assert_eq!(
        result(set(
            7,
            Some(set_params("global: \tglobal_value\n", "7")),
            &queues
        )),
        json!({"status": "queued"})
    );

    let writes = queues.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::LInt(7));
    match &writes[0].target {
        PendingVarTarget::Global(name) => assert_eq!(name, "global_value"),
        target => panic!("unexpected queued target: {target:?}"),
    }
}

#[test]
fn set_trims_and_queues_retain_target() {
    let queues = FakeQueues::new();
    assert_eq!(
        result(set(
            8,
            Some(set_params("retain: retained_value ", "FALSE")),
            &queues
        )),
        json!({"status": "queued"})
    );

    let writes = queues.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::Bool(false));
    match &writes[0].target {
        PendingVarTarget::Retain(name) => assert_eq!(name, "retained_value"),
        target => panic!("unexpected queued target: {target:?}"),
    }
}

#[test]
fn set_scalar_grammar_accepts_boolean_case_and_whitespace() {
    let queues = FakeQueues::new();
    for (target, text) in [
        ("global:a", "TRUE"),
        ("global:b", "false"),
        ("global:c", " \tTrUe\n"),
    ] {
        let response = set(9, Some(set_params(target, text)), &queues);
        assert!(response.ok, "{text:?} rejected: {:?}", response.error);
    }

    let values = queues
        .drain_var_writes()
        .into_iter()
        .map(|write| write.value)
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [Value::Bool(true), Value::Bool(false), Value::Bool(true)]
    );
}

#[test]
fn set_scalar_grammar_accepts_signed_i64_boundaries() {
    let queues = FakeQueues::new();
    for (target, text) in [
        ("global:a", "0"),
        ("global:b", "+0007"),
        ("global:c", "-0007"),
        ("global:d", "9223372036854775807"),
        ("global:e", "-9223372036854775808"),
    ] {
        let response = set(10, Some(set_params(target, text)), &queues);
        assert!(response.ok, "{text:?} rejected: {:?}", response.error);
    }

    let values = queues
        .drain_var_writes()
        .into_iter()
        .map(|write| write.value)
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [
            Value::LInt(0),
            Value::LInt(7),
            Value::LInt(-7),
            Value::LInt(i64::MAX),
            Value::LInt(i64::MIN),
        ]
    );
}

macro_rules! set_invalid_scalar_case {
    ($name:ident, $text:expr) => {
        #[test]
        fn $name() {
            let queues = FakeQueues::new();
            assert_unsupported_value(&set(300, Some(set_params("global:target", $text)), &queues));
            assert!(queues.drain_var_writes().is_empty());
        }
    };
}

set_invalid_scalar_case!(set_rejects_empty_value, "");
set_invalid_scalar_case!(set_rejects_whitespace_value, " \t\n");
set_invalid_scalar_case!(set_rejects_integer_separator, "1_000");
set_invalid_scalar_case!(set_rejects_hex_prefix, "16#FF");
set_invalid_scalar_case!(set_rejects_decimal_point, "1.0");
set_invalid_scalar_case!(set_rejects_exponent, "1e3");
set_invalid_scalar_case!(set_rejects_quoted_text, "\"TRUE\"");
set_invalid_scalar_case!(set_rejects_positive_i64_overflow, "9223372036854775808");
set_invalid_scalar_case!(set_rejects_negative_i64_overflow, "-9223372036854775809");

#[test]
fn set_rejects_empty_global_name_without_queueing() {
    let queues = FakeQueues::new();
    let response = set(11, Some(set_params("global: \t", "1")), &queues);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing global name"));
    assert!(queues.drain_var_writes().is_empty());
}

#[test]
fn set_rejects_empty_retain_name_without_queueing() {
    let queues = FakeQueues::new();
    let response = set(12, Some(set_params("retain:\n", "1")), &queues);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing retain name"));
    assert!(queues.drain_var_writes().is_empty());
}

#[test]
fn set_rejects_unsupported_target_without_queueing() {
    let queues = FakeQueues::new();
    for target in [
        "instance:1:value",
        "local:value",
        "GLOBAL:value",
        " global:value",
        "value",
    ] {
        let response = set(13, Some(set_params(target, "1")), &queues);
        assert!(!response.ok, "{target:?} unexpectedly accepted");
        assert_eq!(response.error.as_deref(), Some("unsupported target"));
    }
    assert!(queues.drain_var_writes().is_empty());
}

#[test]
fn set_replaces_the_same_pending_target_in_place() {
    let queues = FakeQueues::new();
    assert!(set(14, Some(set_params("global:a", "1")), &queues).ok);
    assert!(set(15, Some(set_params("retain:b", "2")), &queues).ok);
    assert!(set(16, Some(set_params("global:a", "3")), &queues).ok);

    let writes = queues.drain_var_writes();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].value, Value::LInt(3));
    assert_eq!(writes[1].value, Value::LInt(2));
    assert!(matches!(
        &writes[0].target,
        PendingVarTarget::Global(name) if name == "a"
    ));
    assert!(matches!(
        &writes[1].target,
        PendingVarTarget::Retain(name) if name == "b"
    ));
}

#[test]
fn rejected_set_preserves_existing_pending_write() {
    let queues = FakeQueues::new();
    queues.enqueue_global_write("a", Value::LInt(7));

    assert_unsupported_value(&set(
        17,
        Some(set_params("global:a", "not-a-value")),
        &queues,
    ));
    let writes = queues.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::LInt(7));
}

#[derive(Debug, Clone, Copy)]
enum ExpectedTarget<'a> {
    Global(&'a str),
    Retain(&'a str),
    Instance(u32, &'a str),
}

fn assert_parsed_target(text: &str, expected: ExpectedTarget<'_>) {
    let target = parse_var_target(text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
    match (target, expected) {
        (VarTarget::Global(actual), ExpectedTarget::Global(expected))
        | (VarTarget::Retain(actual), ExpectedTarget::Retain(expected)) => {
            assert_eq!(actual, expected);
        }
        (
            VarTarget::Instance(actual_id, actual),
            ExpectedTarget::Instance(expected_id, expected),
        ) => {
            assert_eq!(actual_id, expected_id);
            assert_eq!(actual, expected);
        }
        (_, expected) => panic!("target kind did not match {expected:?}"),
    }
}

#[test]
fn var_target_parser_accepts_and_trims_global_and_retain_names() {
    assert_parsed_target(
        "global: \tglobal_value\n",
        ExpectedTarget::Global("global_value"),
    );
    assert_parsed_target(
        "retain: retained_value ",
        ExpectedTarget::Retain("retained_value"),
    );
}

#[test]
fn var_target_parser_accepts_instance_id_boundaries() {
    assert_parsed_target("instance:0:value", ExpectedTarget::Instance(0, "value"));
    assert_parsed_target(
        "instance:4294967295:value",
        ExpectedTarget::Instance(u32::MAX, "value"),
    );
}

#[test]
fn var_target_parser_normalizes_instance_id_and_trims_name() {
    assert_parsed_target(
        "instance:000007: \tvalue\n",
        ExpectedTarget::Instance(7, "value"),
    );
}

#[test]
fn var_target_parser_preserves_colons_after_instance_name_boundary() {
    assert_parsed_target(
        "instance:7:member:field",
        ExpectedTarget::Instance(7, "member:field"),
    );
}

macro_rules! invalid_var_target_case {
    ($name:ident, $target:expr, $error:expr) => {
        #[test]
        fn $name() {
            match parse_var_target($target) {
                Ok(_) => panic!("expected target parse to fail"),
                Err(error) => assert_eq!(error, $error),
            }
        }
    };
}

invalid_var_target_case!(
    var_target_rejects_empty_text,
    "",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_leading_whitespace,
    " global:value",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_uppercase_global_prefix,
    "GLOBAL:value",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_uppercase_retain_prefix,
    "RETAIN:value",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_uppercase_instance_prefix,
    "INSTANCE:1:value",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_missing_global_name,
    "global:",
    "missing global name"
);
invalid_var_target_case!(
    var_target_rejects_blank_global_name,
    "global: \t",
    "missing global name"
);
invalid_var_target_case!(
    var_target_rejects_missing_retain_name,
    "retain:",
    "missing retain name"
);
invalid_var_target_case!(
    var_target_rejects_blank_retain_name,
    "retain:\n",
    "missing retain name"
);
invalid_var_target_case!(
    var_target_rejects_missing_instance_id,
    "instance::value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_negative_instance_id,
    "instance:-1:value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_positive_sign_instance_id,
    "instance:+1:value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_nondigit_instance_id,
    "instance:one:value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_overflowing_instance_id,
    "instance:4294967296:value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_missing_instance_name,
    "instance:1",
    "missing instance name"
);
invalid_var_target_case!(
    var_target_rejects_empty_instance_name,
    "instance:1:",
    "missing instance name"
);
invalid_var_target_case!(
    var_target_rejects_blank_instance_name,
    "instance:1: \t",
    "missing instance name"
);

#[test]
fn var_force_requires_params() {
    let response = var_force(18, None, &FakeQueues::new());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! force_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(&var_force(400, Some($params), &FakeQueues::new()));
        }
    };
}

force_invalid_params_case!(var_force_rejects_null_params, JsonValue::Null);
force_invalid_params_case!(var_force_rejects_array_params, json!([]));
force_invalid_params_case!(var_force_rejects_string_params, json!("global:x"));
force_invalid_params_case!(var_force_rejects_missing_target, json!({"value": "1"}));
force_invalid_params_case!(
    var_force_rejects_missing_value,
    json!({"target": "global:x"})
);
force_invalid_params_case!(
    var_force_rejects_unknown_field,
    json!({"target": "global:x", "value": "1", "sticky": true})
);
force_invalid_params_case!(
    var_force_rejects_null_target,
    json!({"target": null, "value": "1"})
);
force_invalid_params_case!(
    var_force_rejects_object_target,
    json!({"target": {}, "value": "1"})
);
force_invalid_params_case!(
    var_force_rejects_null_value,
    json!({"target": "global:x", "value": null})
);
force_invalid_params_case!(
    var_force_rejects_numeric_value,
    json!({"target": "global:x", "value": 1})
);

#[test]
fn var_force_accepts_each_target_kind_in_arrival_order() {
    let queues = FakeQueues::new();
    for (target, value) in [
        ("global:a", "TRUE"),
        ("retain:b", "2"),
        ("instance:7:c", "-3"),
    ] {
        assert_eq!(
            result(var_force(19, Some(set_params(target, value)), &queues)),
            json!({"status": "forced"})
        );
    }

    let forced = queues.forced_vars();
    assert_eq!(forced.len(), 3);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Global(name) if name == "a"
    ));
    assert_eq!(forced[0].value, Value::Bool(true));
    assert!(matches!(
        &forced[1].target,
        ForcedVarTarget::Retain(name) if name == "b"
    ));
    assert_eq!(forced[1].value, Value::LInt(2));
    assert!(matches!(
        &forced[2].target,
        ForcedVarTarget::Instance(_, InstanceId(7), name) if name == "c"
    ));
    assert_eq!(forced[2].value, Value::LInt(-3));
}

#[test]
fn var_force_resolves_an_instance_target_in_the_ports_generation() {
    // A request that names a bare instance identity belongs to the generation the port reports;
    // resolving it at `Generation::FIRST` instead would force the wrong installation.
    let queues = FakeQueues::with_generation(Generation(5));
    assert_eq!(
        result(var_force(
            23,
            Some(set_params("instance:7:c", "1")),
            &queues
        )),
        json!({"status": "forced"})
    );

    let forced = queues.forced_vars();
    assert_eq!(forced.len(), 1);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Instance(generation, InstanceId(7), name)
            if *generation == Generation(5) && name == "c"
    ));
    assert_eq!(forced[0].value, Value::LInt(1));
}

#[test]
fn var_force_replaces_same_target_without_duplication() {
    let queues = FakeQueues::new();
    assert!(var_force(20, Some(set_params("global:a", "1")), &queues).ok);
    assert!(var_force(21, Some(set_params("retain:b", "2")), &queues).ok);
    assert!(var_force(22, Some(set_params("global:a", "3")), &queues).ok);

    let forced = queues.forced_vars();
    assert_eq!(forced.len(), 2);
    assert_eq!(forced[0].value, Value::LInt(3));
    assert_eq!(forced[1].value, Value::LInt(2));
}

#[test]
fn rejected_var_force_preserves_existing_force() {
    let queues = FakeQueues::new();
    queues.force_global("a", Value::LInt(7));

    assert_unsupported_value(&var_force(
        24,
        Some(set_params("global:a", "not-a-value")),
        &queues,
    ));
    let forced = queues.forced_vars();
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].value, Value::LInt(7));
}

#[test]
fn invalid_target_never_creates_a_force() {
    let queues = FakeQueues::new();
    for target in ["global:", "retain: ", "instance:no:c", "instance:1:"] {
        let response = var_force(25, Some(set_params(target, "1")), &queues);
        assert!(!response.ok, "{target:?} unexpectedly accepted");
    }
    assert!(queues.forced_vars().is_empty());
}

#[test]
fn var_unforce_requires_params() {
    let response = var_unforce(26, None, &FakeQueues::new());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! unforce_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(&var_unforce(500, Some($params), &FakeQueues::new()));
        }
    };
}

unforce_invalid_params_case!(var_unforce_rejects_null_params, JsonValue::Null);
unforce_invalid_params_case!(var_unforce_rejects_array_params, json!([]));
unforce_invalid_params_case!(var_unforce_rejects_string_params, json!("global:x"));
unforce_invalid_params_case!(var_unforce_rejects_missing_target, json!({}));
unforce_invalid_params_case!(
    var_unforce_rejects_unknown_field,
    json!({"target": "global:x", "all": true})
);
unforce_invalid_params_case!(var_unforce_rejects_null_target, json!({"target": null}));
unforce_invalid_params_case!(var_unforce_rejects_numeric_target, json!({"target": 1}));

#[test]
fn var_unforce_removes_only_the_exact_global_target() {
    let queues = FakeQueues::new();
    queues.force_global("a", Value::LInt(1));
    queues.force_retain("a", Value::LInt(2));

    assert_eq!(
        result(var_unforce(27, Some(target_params("global: a ")), &queues)),
        json!({"status": "released"})
    );
    let forced = queues.forced_vars();
    assert_eq!(forced.len(), 1);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Retain(name) if name == "a"
    ));
}

#[test]
fn var_unforce_removes_only_the_exact_retain_target() {
    let queues = FakeQueues::new();
    queues.force_retain("a", Value::LInt(1));
    queues.force_retain("b", Value::LInt(2));

    assert!(var_unforce(28, Some(target_params("retain:a")), &queues).ok);
    let forced = queues.forced_vars();
    assert_eq!(forced.len(), 1);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Retain(name) if name == "b"
    ));
}

#[test]
fn var_unforce_removes_only_the_exact_instance_target() {
    let queues = FakeQueues::new();
    queues.force_instance(Generation::FIRST, InstanceId(7), "a", Value::LInt(1));
    queues.force_instance(Generation::FIRST, InstanceId(8), "a", Value::LInt(2));

    assert!(var_unforce(29, Some(target_params("instance:7:a")), &queues).ok);
    let forced = queues.forced_vars();
    assert_eq!(forced.len(), 1);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Instance(_, InstanceId(8), name) if name == "a"
    ));
}

#[test]
fn var_unforce_is_idempotent_for_an_absent_force() {
    let queues = FakeQueues::new();
    assert_eq!(
        result(var_unforce(
            30,
            Some(target_params("global:absent")),
            &queues
        )),
        json!({"status": "released"})
    );
    assert!(queues.forced_vars().is_empty());
}

#[test]
fn rejected_var_unforce_preserves_existing_forces() {
    let queues = FakeQueues::new();
    queues.force_global("a", Value::LInt(1));

    let response = var_unforce(31, Some(target_params("instance:no:a")), &queues);
    assert!(!response.ok);
    let forced = queues.forced_vars();
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].value, Value::LInt(1));
}

#[test]
fn var_forced_empty_projection_has_stable_shape() {
    assert_eq!(
        result(var_forced(32, &FakeQueues::new())),
        json!({"vars": []})
    );
}

#[test]
fn var_forced_projection_preserves_order_and_canonicalizes_targets() {
    let queues = FakeQueues::new();
    queues.force_global("a", Value::Bool(true));
    queues.force_retain("b", Value::LInt(-2));
    queues.force_instance(Generation::FIRST, InstanceId(7), "c:d", Value::LInt(3));

    assert_eq!(
        result(var_forced(33, &queues)),
        json!({
            "vars": [
                {"target": "global:a", "value": "TRUE"},
                {"target": "retain:b", "value": "-2"},
                {"target": "instance:7:c:d", "value": "3"}
            ]
        })
    );
}

#[test]
fn var_forced_projection_keeps_replacement_position() {
    let queues = FakeQueues::new();
    queues.force_global("a", Value::LInt(1));
    queues.force_retain("b", Value::LInt(2));
    queues.force_global("a", Value::LInt(3));

    assert_eq!(
        result(var_forced(34, &queues)),
        json!({
            "vars": [
                {"target": "global:a", "value": "3"},
                {"target": "retain:b", "value": "2"}
            ]
        })
    );
}

/// `set` of `target` with `value` on `queues`.
fn set_value(queues: &FakeQueues, target: &str, value: &str) -> ControlResponse {
    set(40, Some(json!({"target": target, "value": value})), queues)
}

#[test]
fn set_takes_a_literal_of_its_target_s_declared_type() {
    // Specification 11 (typed values): where the program resolves the target, the value is an IEC
    // literal of its declared type and is queued as that type's value: the REAL `Setpoint` takes
    // 3.5, the INT `Level` 16#7F (127), the enumeration `Mode` Color#Blue, the STRING[5] `Name`
    // 'hey' and the TIME `Delay` T#250ms (250,000,000 ns). Catches a value queued untyped, so that a
    // REAL, a TIME, a text or an enumeration could not be written at all.
    let queues = FakeQueues::with_program();
    for (target, value) in [
        ("global:Setpoint", "3.5"),
        ("global:Level", "16#7F"),
        ("global:Mode", "Color#Blue"),
        ("global:Name", "'hey'"),
        ("global:Delay", "T#250ms"),
    ] {
        assert_eq!(
            result(set_value(&queues, target, value)),
            json!({"status": "queued"}),
            "{target}"
        );
    }
    let drained: Vec<Value> = queues
        .drain_var_writes()
        .into_iter()
        .map(|write| write.value)
        .collect();
    assert_eq!(
        drained,
        [
            Value::Real(3.5),
            Value::Int(127),
            Value::Enum(alloc::boxed::Box::new(
                trust_runtime_core::value::EnumValue::from_canonical_parts(
                    "Color".into(),
                    "Blue".into(),
                    2,
                )
            )),
            Value::String("hey".into()),
            Value::Time(trust_runtime_core::value::Duration::from_nanos(250_000_000)),
        ]
    );
}

#[test]
fn set_keeps_the_untyped_grammar_for_an_unknown_target_and_a_text_the_type_refuses() {
    // Specification 11: a text of the untyped grammar that the declared type refuses (40000 for an
    // INT) and a target the program does not resolve keep the untyped grammar: they are queued as a
    // LINT and refused at the boundary as before. Catches the boundary's refusal moved forward for
    // the old grammar, which would change what a client of `set` sees.
    let queues = FakeQueues::with_program();
    assert_eq!(
        result(set_value(&queues, "global:Level", "40000")),
        json!({"status": "queued"})
    );
    assert_eq!(
        result(set_value(&queues, "global:Nothing", "1")),
        json!({"status": "queued"})
    );
    let drained: Vec<Value> = queues
        .drain_var_writes()
        .into_iter()
        .map(|write| write.value)
        .collect();
    assert_eq!(drained, [Value::LInt(40_000), Value::LInt(1)]);
}

#[test]
fn set_refuses_a_literal_its_target_does_not_take_and_queues_nothing() {
    // Specification 11: a literal of another type, a text longer than the target holds, a name that
    // is no value of its enumeration and a constant are refused with a plain reason before anything
    // is queued; a text that is no literal at all keeps the stable refusal. Catches a REAL literal
    // queued for an INT, a text cut to fit, and a constant written.
    let queues = FakeQueues::with_program();
    for (target, value, reason) in [
        ("global:Level", "3.5", "'3.5' is not a value of type INT"),
        (
            "global:Name",
            "'too long'",
            "''too long'' is longer than STRING[5] holds",
        ),
        (
            "global:Mode",
            "Purple",
            "'Purple' is not a value of Color (Red, Green, Blue)",
        ),
        ("global:Limit", "8", "Limit is a constant"),
        ("global:Nothing", "3.5", "unsupported value '3.5'"),
    ] {
        let response = set_value(&queues, target, value);
        assert_eq!(response.error.as_deref(), Some(reason), "{target} {value}");
    }
    assert!(
        queues.drain_var_writes().is_empty(),
        "a refusal queues nothing"
    );
}

#[test]
fn var_force_holds_a_typed_value_and_a_platform_without_its_program_keeps_the_untyped_grammar() {
    // Specification 11: `var.force` types its value as `set` does, so `Setpoint` is held at the
    // REAL 2.5; on a platform without its program at hand the untyped grammar applies, which refuses
    // 2.5. Catches a force left untyped while `set` is typed.
    let queues = FakeQueues::with_program();
    let forced = var_force(
        41,
        Some(json!({"target": "global:Setpoint", "value": "2.5"})),
        &queues,
    );
    assert_eq!(result(forced), json!({"status": "forced"}));
    assert_eq!(queues.forced_vars()[0].value, Value::Real(2.5));

    let without = FakeQueues::new();
    let refused = var_force(
        42,
        Some(json!({"target": "global:Setpoint", "value": "2.5"})),
        &without,
    );
    assert_eq!(refused.error.as_deref(), Some("unsupported value '2.5'"));
}
