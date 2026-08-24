use super::*;

use crate::harness::{CompileSession, SourceFile, TestHarness};
use crate::value::{Duration, Value};
use crate::{RestartMode, RetainPolicy};

fn compile_runtime(source: &str) -> Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .unwrap_or_else(|error| panic!("source must compile: {error}"))
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("source must fail compilation"),
        Err(error) => error.to_string(),
    }
}

fn assert_error_contains(source: &str, expected: &str) {
    let error = compile_error(source);
    assert!(
        error.contains(expected),
        "expected error containing {expected:?}, got {error:?}"
    );
}

fn int_value(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::SInt(value)) => i64::from(*value),
        Some(Value::Int(value)) => i64::from(*value),
        Some(Value::DInt(value)) => i64::from(*value),
        Some(Value::LInt(value)) => *value,
        other => panic!("expected signed integer, got {other:?}"),
    }
}

#[test]
fn configuration_contract_requires_a_program_without_configuration() {
    assert_error_contains(
        "VAR_GLOBAL\n    value : INT;\nEND_VAR\n",
        "missing PROGRAM declaration",
    );
}

#[test]
fn configuration_contract_default_assembly_instantiates_every_program_once() {
    let runtime = compile_runtime(
        r#"
PROGRAM Alpha
VAR
    a : INT;
END_VAR
END_PROGRAM

PROGRAM Beta
VAR
    b : INT;
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(runtime.programs().len(), 2);
    assert!(matches!(
        runtime.storage().get_global("Alpha"),
        Some(Value::Instance(_))
    ));
    assert!(matches!(
        runtime.storage().get_global("Beta"),
        Some(Value::Instance(_))
    ));
    assert!(runtime.has_background_programs());
}

#[test]
fn configuration_contract_rejects_multiple_configuration_declarations() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION First
PROGRAM P1 : Main;
END_CONFIGURATION
CONFIGURATION Second
PROGRAM P2 : Main;
END_CONFIGURATION
"#,
        "multiple CONFIGURATION",
    );
}

#[test]
fn configuration_contract_resource_name_becomes_runtime_identity() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION Cell
RESOURCE PackagingLine ON PLC
PROGRAM MainInstance : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );

    assert_eq!(runtime.resource_name(), "PackagingLine");
}

#[test]
fn configuration_contract_reports_all_unbound_programs_in_declaration_order() {
    let error = compile_error(
        r#"
PROGRAM Zebra
END_PROGRAM
PROGRAM Alpha
END_PROGRAM
PROGRAM Bound
END_PROGRAM
CONFIGURATION C
PROGRAM P : Bound;
END_CONFIGURATION
"#,
    );

    assert!(error.contains("unbound PROGRAM declaration"));
    let zebra = error.find("Zebra").expect("Zebra must be named");
    let alpha = error.find("Alpha").expect("Alpha must be named");
    assert!(zebra < alpha, "declaration order must be retained: {error}");
}

#[test]
fn configuration_contract_explicit_extras_bind_unconfigured_types_once() {
    let source = SourceFile::new(
        r#"
PROGRAM Main
VAR
    value : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
END_CONFIGURATION
"#,
    );
    let runtime = CompileSession::from_sources(vec![source])
        .with_extra_program_instances(["main", "MAIN", "Main"])
        .build_runtime()
        .expect("case variants must deduplicate");

    assert_eq!(runtime.programs().len(), 1);
    assert!(runtime
        .programs()
        .values()
        .any(|program| program.name.eq_ignore_ascii_case("main")));
}

#[test]
fn configuration_contract_extra_instance_must_name_a_declaration() {
    let source = SourceFile::new(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
END_CONFIGURATION
"#,
    );
    let error = match CompileSession::from_sources(vec![source])
        .with_extra_program_instances(["Missing"])
        .build_runtime()
    {
        Ok(_) => panic!("unknown extra instance must fail"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("extra PROGRAM instance 'Missing'"));
}

#[test]
fn configuration_contract_configured_instance_suppresses_same_named_extra() {
    let source = SourceFile::new(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
PROGRAM main : Main;
END_CONFIGURATION
"#,
    );
    let runtime = CompileSession::from_sources(vec![source])
        .with_extra_program_instances(["MAIN"])
        .build_runtime()
        .expect("configured name must suppress matching extra");
    assert_eq!(runtime.programs().len(), 1);
}

#[test]
fn configuration_contract_duplicate_instance_names_are_case_insensitive() {
    assert_error_contains(
        r#"
PROGRAM One
END_PROGRAM
PROGRAM Two
END_PROGRAM
CONFIGURATION C
PROGRAM Station : One;
PROGRAM station : Two;
END_CONFIGURATION
"#,
        "duplicate declaration of 'station'",
    );
}

#[test]
fn configuration_contract_rejects_two_instances_of_one_program_type() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
PROGRAM First : Main;
PROGRAM Second : Main;
END_CONFIGURATION
"#,
        "multiple instances of the same PROGRAM type",
    );
}

#[test]
fn configuration_contract_unknown_program_type_names_the_type() {
    assert_error_contains(
        r#"
CONFIGURATION C
PROGRAM P : MissingProgram;
END_CONFIGURATION
"#,
        "unknown program type for 'P'",
    );
}

#[test]
fn configuration_contract_qualified_name_resolves_namespaced_program_type() {
    let runtime = compile_runtime(
        r#"
NAMESPACE Cell
PROGRAM Main
VAR
    count : INT;
END_VAR
END_PROGRAM
END_NAMESPACE

CONFIGURATION C
PROGRAM P : Cell.Main;
END_CONFIGURATION
"#,
    );

    assert!(runtime
        .programs()
        .values()
        .any(|program| program.name == "P"));
}

#[test]
fn configuration_contract_task_optional_fields_default_to_zero_and_untriggered() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK Idle (PRIORITY := 0);
PROGRAM P WITH Idle : Main;
END_CONFIGURATION
"#,
    );
    let task = &runtime.tasks()[0];
    assert_eq!(task.name, "Idle");
    assert_eq!(task.interval, Duration::ZERO);
    assert_eq!(task.priority, 0);
    assert_eq!(task.single, None);
    assert_eq!(task.programs.as_slice(), ["P"]);
}

#[test]
fn configuration_contract_task_preserves_interval_single_and_priority() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Start : BOOL;
END_VAR
TASK Fast (INTERVAL := T#125ms, SINGLE := Start, PRIORITY := 7);
PROGRAM P WITH Fast : Main;
END_CONFIGURATION
"#,
    );
    let task = &runtime.tasks()[0];
    assert_eq!(task.interval, Duration::from_millis(125));
    assert_eq!(task.single.as_deref(), Some("Start"));
    assert_eq!(task.priority, 7);
}

#[test]
fn configuration_contract_task_priority_must_be_non_negative() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK Bad (PRIORITY := -1);
PROGRAM P WITH Bad : Main;
END_CONFIGURATION
"#,
        "TASK PRIORITY must be non-negative",
    );
}

#[test]
fn configuration_contract_program_task_lookup_is_case_insensitive() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK FAST (INTERVAL := T#1ms, PRIORITY := 0);
PROGRAM P WITH fast : Main;
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.tasks()[0].programs.as_slice(), ["P"]);
}

#[test]
fn configuration_contract_unknown_program_task_names_task_and_instance() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
PROGRAM MainInstance WITH MissingTask : Main;
END_CONFIGURATION
"#,
    );
    assert!(error.contains("unknown task 'MissingTask'"));
}

#[test]
fn configuration_contract_program_without_with_remains_background() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
PROGRAM Background
END_PROGRAM
CONFIGURATION C
TASK Fast (INTERVAL := T#1ms, PRIORITY := 0);
PROGRAM Scheduled WITH Fast : Main;
PROGRAM Loose : Background;
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.tasks()[0].programs.as_slice(), ["Scheduled"]);
    assert!(runtime.has_background_programs());
}

#[test]
fn configuration_contract_fb_task_binding_resolves_real_instance() {
    let runtime = compile_runtime(
        r#"
FUNCTION_BLOCK Worker
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    worker : Worker;
END_VAR
END_PROGRAM
CONFIGURATION C
TASK Fast (INTERVAL := T#1ms, PRIORITY := 0);
PROGRAM P : Main (worker WITH Fast);
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.tasks()[0].fb_instances.len(), 1);
}

#[test]
fn configuration_contract_fb_task_binding_rejects_non_fb_value() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    value : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
TASK Fast (INTERVAL := T#1ms, PRIORITY := 0);
PROGRAM P : Main (value WITH Fast);
END_CONFIGURATION
"#,
        "FB task binding must reference a function block instance",
    );
}

#[test]
fn configuration_contract_fb_task_binding_rejects_direct_address() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK Fast (INTERVAL := T#1ms, PRIORITY := 0);
PROGRAM P : Main (%QX0.0 WITH Fast);
END_CONFIGURATION
"#,
        "direct addresses are not valid FB task bindings",
    );
}

#[test]
fn configuration_contract_program_retain_fills_only_unspecified_policy() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
VAR
    inherited : INT;
END_VAR
VAR RETAIN
    explicit_retain : INT;
END_VAR
VAR NON_RETAIN
    explicit_nonretain : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM NON_RETAIN P : Main;
END_CONFIGURATION
"#,
    );
    let program = runtime
        .programs()
        .values()
        .find(|program| program.name == "P")
        .expect("configured program");
    let policy = |name: &str| {
        program
            .vars
            .iter()
            .find(|var| var.name == name)
            .map(|var| var.retain)
            .expect("variable")
    };
    assert_eq!(policy("inherited"), RetainPolicy::NonRetain);
    assert_eq!(policy("explicit_retain"), RetainPolicy::Retain);
    assert_eq!(policy("explicit_nonretain"), RetainPolicy::NonRetain);
}

#[test]
fn configuration_contract_conflicting_program_retain_policies_are_rejected_first() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    value : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM RETAIN First : Main;
PROGRAM NON_RETAIN Second : Main;
END_CONFIGURATION
"#,
        "conflicting RETAIN/NON_RETAIN qualifiers",
    );
}

#[test]
fn configuration_contract_program_retain_controls_warm_restart() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    value : INT := INT#3;
END_VAR
value := value + INT#1;
END_PROGRAM
CONFIGURATION C
PROGRAM RETAIN P : Main;
END_CONFIGURATION
"#,
    )
    .expect("retained program must compile");
    harness.cycle();
    assert_eq!(harness.get_output("value"), Some(Value::Int(4)));
    harness.restart(RestartMode::Warm).expect("warm restart");
    assert_eq!(harness.get_output("value"), Some(Value::Int(4)));
    harness.restart(RestartMode::Cold).expect("cold restart");
    assert_eq!(harness.get_output("value"), Some(Value::Int(3)));
}

#[test]
fn configuration_contract_configuration_and_resource_globals_are_shared() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    observed : INT;
END_VAR
observed := ConfigValue + ResourceValue;
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    ConfigValue : INT := INT#4;
END_VAR
RESOURCE R ON PLC
VAR_GLOBAL
    ResourceValue : INT := INT#7;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    )
    .expect("configuration globals must compile");
    harness.cycle();
    assert_eq!(harness.get_output("observed"), Some(Value::Int(11)));
}

#[test]
fn configuration_contract_global_initializers_accept_prior_constants() {
    let runtime = compile_runtime(
        r#"
VAR_GLOBAL CONSTANT
    Base : INT := INT#9;
    Derived : INT := Base + INT#2;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_eq!(int_value(runtime.storage().get_global("Base")), 9);
    assert_eq!(int_value(runtime.storage().get_global("Derived")), 11);
}

#[test]
fn configuration_contract_function_block_global_applies_member_initializer() {
    let runtime = compile_runtime(
        r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    preset : INT;
END_VAR
END_FUNCTION_BLOCK
VAR_GLOBAL
    GlobalCounter : Counter := (preset := INT#12);
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
    let id = match runtime.storage().get_global("GlobalCounter") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected FB instance, got {other:?}"),
    };
    assert_eq!(
        int_value(runtime.storage().get_instance_var(id, "preset")),
        12
    );
}

#[test]
fn configuration_contract_class_global_rejects_initializer() {
    assert_error_contains(
        r#"
CLASS Device
VAR
    id : INT;
END_VAR
END_CLASS
VAR_GLOBAL
    GlobalDevice : Device := (id := INT#1);
END_VAR
PROGRAM Main
END_PROGRAM
"#,
        "aggregate initializer requires a STRUCT, UNION, or function block type",
    );
}

#[test]
fn configuration_contract_interface_global_starts_null() {
    let runtime = compile_runtime(
        r#"
INTERFACE IDevice
END_INTERFACE
VAR_GLOBAL
    DeviceRef : IDevice;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_eq!(
        runtime.storage().get_global("DeviceRef"),
        Some(&Value::Null)
    );
}

#[test]
fn configuration_contract_task_requires_priority_field() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK MissingPriority (INTERVAL := T#10ms);
PROGRAM P WITH MissingPriority : Main;
END_CONFIGURATION
"#,
        "requires PRIORITY",
    );
}

#[test]
fn configuration_contract_task_single_requires_named_bool_storage() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK EventTask (SINGLE := TRUE, PRIORITY := 0);
PROGRAM P WITH EventTask : Main;
END_CONFIGURATION
"#,
    );
    assert!(
        error.contains("SINGLE")
            && (error.to_ascii_lowercase().contains("name")
                || error.to_ascii_lowercase().contains("storage")
                || error.to_ascii_lowercase().contains("variable")),
        "{error}"
    );
}

#[test]
fn configuration_contract_task_single_rejects_non_bool_global() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Trigger : INT;
END_VAR
TASK EventTask (SINGLE := Trigger, PRIORITY := 0);
PROGRAM P WITH EventTask : Main;
END_CONFIGURATION
"#,
    );
    assert!(
        error.contains("SINGLE") && error.contains("BOOL"),
        "{error}"
    );
}

#[test]
fn configuration_contract_task_single_rejects_unknown_name_at_assembly() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK EventTask (SINGLE := MissingTrigger, PRIORITY := 0);
PROGRAM P WITH EventTask : Main;
END_CONFIGURATION
"#,
    );
    assert!(
        error.contains("SINGLE")
            && error.contains("MissingTrigger")
            && (error.to_ascii_lowercase().contains("unknown")
                || error.to_ascii_lowercase().contains("undefined")),
        "{error}"
    );
}

#[test]
fn configuration_contract_task_single_accepts_configuration_bool_global() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Start : BOOL;
END_VAR
TASK EventTask (SINGLE := Start, PRIORITY := 2);
PROGRAM P WITH EventTask : Main;
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.tasks()[0].single.as_deref(), Some("Start"));
    assert_eq!(runtime.tasks()[0].priority, 2);
}

#[test]
fn configuration_contract_task_single_accepts_resource_bool_global() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
RESOURCE R ON PLC
VAR_GLOBAL
    Start : BOOL;
END_VAR
TASK EventTask (SINGLE := Start, PRIORITY := 1);
PROGRAM P WITH EventTask : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.tasks()[0].single.as_deref(), Some("Start"));
}

#[test]
fn configuration_contract_task_interval_requires_time_literal() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK BadInterval (INTERVAL := INT#10, PRIORITY := 0);
PROGRAM P WITH BadInterval : Main;
END_CONFIGURATION
"#,
        "INTERVAL must be a TIME literal",
    );
}

#[test]
fn configuration_contract_task_interval_must_not_be_negative() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK BadInterval (INTERVAL := T#-1ms, PRIORITY := 0);
PROGRAM P WITH BadInterval : Main;
END_CONFIGURATION
"#,
    );
    assert!(
        error.contains("INTERVAL")
            && (error.to_ascii_lowercase().contains("non-negative")
                || error.to_ascii_lowercase().contains("negative")),
        "{error}"
    );
}

#[test]
fn configuration_contract_task_priority_must_fit_u32() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK BadPriority (PRIORITY := 4294967296);
PROGRAM P WITH BadPriority : Main;
END_CONFIGURATION
"#,
    );
    assert!(
        error.contains("PRIORITY")
            && (error.to_ascii_lowercase().contains("range")
                || error.to_ascii_lowercase().contains("u32")
                || error.to_ascii_lowercase().contains("too large")),
        "{error}"
    );
}

#[test]
fn configuration_contract_task_rejects_unknown_initializer_field() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK BadField (DEADLINE := T#1ms, PRIORITY := 0);
PROGRAM P WITH BadField : Main;
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("unknown") && error.contains("DEADLINE"),
        "{error}"
    );
}

#[test]
fn configuration_contract_task_rejects_repeated_initializer_field() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
TASK DuplicatePriority (PRIORITY := 1, PRIORITY := 2);
PROGRAM P WITH DuplicatePriority : Main;
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("duplicate") && error.contains("PRIORITY"),
        "{error}"
    );
}

#[test]
fn configuration_contract_task_names_are_case_insensitively_unique() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
PROGRAM Auxiliary
END_PROGRAM
CONFIGURATION C
TASK Fast (PRIORITY := 0);
TASK fast (PRIORITY := 1);
PROGRAM P1 WITH Fast : Main;
PROGRAM P2 WITH fast : Auxiliary;
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("duplicate")
            && error.to_ascii_lowercase().contains("fast"),
        "{error}"
    );
}

#[test]
fn configuration_contract_tasks_preserve_declaration_order_and_spelling() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
PROGRAM Auxiliary
END_PROGRAM
CONFIGURATION C
TASK SlowTask (INTERVAL := T#20ms, PRIORITY := 3);
TASK FastTask (INTERVAL := T#5ms, PRIORITY := 1);
PROGRAM P1 WITH SlowTask : Main;
PROGRAM P2 WITH FastTask : Auxiliary;
END_CONFIGURATION
"#,
    );
    assert_eq!(
        runtime
            .tasks()
            .iter()
            .map(|task| (task.name.as_str(), task.interval, task.priority))
            .collect::<Vec<_>>(),
        [
            ("SlowTask", Duration::from_millis(20), 3),
            ("FastTask", Duration::from_millis(5), 1)
        ]
    );
}

#[test]
fn configuration_contract_single_trigger_runs_only_on_rising_edges() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    Count : INT;
END_VAR
Count := Count + INT#1;
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Start : BOOL;
END_VAR
TASK EventTask (SINGLE := Start, PRIORITY := 0);
PROGRAM P WITH EventTask : Main;
END_CONFIGURATION
"#,
    )
    .expect("event task");
    harness.cycle();
    assert_eq!(harness.get_output("Count"), Some(Value::Int(0)));
    harness.set_input("Start", true);
    harness.cycle();
    assert_eq!(harness.get_output("Count"), Some(Value::Int(1)));
    harness.cycle();
    assert_eq!(harness.get_output("Count"), Some(Value::Int(1)));
    harness.set_input("Start", false);
    harness.cycle();
    harness.set_input("Start", true);
    harness.cycle();
    assert_eq!(harness.get_output("Count"), Some(Value::Int(2)));
}

#[test]
fn configuration_contract_zero_interval_without_single_never_runs_task() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    Count : INT;
END_VAR
Count := Count + INT#1;
END_PROGRAM
CONFIGURATION C
TASK DisabledPeriodic (INTERVAL := T#0ms, PRIORITY := 0);
PROGRAM P WITH DisabledPeriodic : Main;
END_CONFIGURATION
"#,
    )
    .expect("zero interval task");
    for _ in 0..3 {
        harness.cycle();
    }
    assert_eq!(harness.get_output("Count"), Some(Value::Int(0)));
}

#[test]
fn configuration_contract_implicit_resource_keeps_synthetic_identity() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION CellConfiguration
TASK MainTask (PRIORITY := 0);
PROGRAM P WITH MainTask : Main;
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.resource_name(), "RESOURCE");
    assert_eq!(runtime.tasks()[0].programs.as_slice(), ["P"]);
}

#[test]
fn configuration_contract_explicit_resource_name_not_on_type_is_runtime_identity() {
    for resource_type in ["CPU", "PLC"] {
        let source = format!(
            r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION CellConfiguration
RESOURCE PackagingLine ON {resource_type}
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#
        );
        let runtime = compile_runtime(&source);
        assert_eq!(runtime.resource_name(), "PackagingLine");
    }
}

#[test]
fn configuration_contract_single_runtime_rejects_multiple_explicit_resources() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
PROGRAM Auxiliary
END_PROGRAM
CONFIGURATION C
RESOURCE FirstResource ON CPU
PROGRAM P1 : Main;
END_RESOURCE
RESOURCE SecondResource ON CPU
PROGRAM P2 : Auxiliary;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("multiple resource")
            || error.to_ascii_lowercase().contains("single-resource"),
        "{error}"
    );
}

#[test]
fn configuration_contract_rejects_mixed_implicit_and_explicit_resource_content() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
PROGRAM Auxiliary
END_PROGRAM
CONFIGURATION C
TASK ImplicitTask (PRIORITY := 0);
PROGRAM P1 WITH ImplicitTask : Main;
RESOURCE ExplicitResource ON CPU
TASK ExplicitTask (PRIORITY := 1);
PROGRAM P2 WITH ExplicitTask : Auxiliary;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("implicit")
            && error.to_ascii_lowercase().contains("explicit")
            && error.to_ascii_lowercase().contains("resource"),
        "{error}"
    );
}

#[test]
fn configuration_contract_configuration_global_is_visible_to_resource_program() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    Observed : INT;
END_VAR
Observed := ConfigurationValue;
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    ConfigurationValue : INT := INT#12;
END_VAR
RESOURCE R ON CPU
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    )
    .expect("configuration global");
    harness.cycle();
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(12)));
}

#[test]
fn configuration_contract_resource_global_is_visible_to_its_program() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    Observed : INT;
END_VAR
Observed := ResourceValue;
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL
    ResourceValue : INT := INT#15;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    )
    .expect("resource global");
    harness.cycle();
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(15)));
}

#[test]
fn configuration_contract_resource_external_links_without_duplicate_allocation() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Shared : DINT := DINT#18;
END_VAR
RESOURCE R ON CPU
VAR_EXTERNAL
    Shared : DINT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert_eq!(
        runtime.storage().get_global("Shared"),
        Some(&Value::DInt(18))
    );
    assert_eq!(
        runtime
            .storage()
            .globals()
            .keys()
            .filter(|name| name.eq_ignore_ascii_case("Shared"))
            .count(),
        1
    );
}

#[test]
fn configuration_contract_resource_external_type_must_match_global() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Shared : DINT;
END_VAR
RESOURCE R ON CPU
VAR_EXTERNAL
    Shared : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        "does not match VAR_GLOBAL type",
    );
}

#[test]
fn configuration_contract_resource_external_cannot_have_initializer() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Shared : INT;
END_VAR
RESOURCE R ON CPU
VAR_EXTERNAL
    Shared : INT := INT#4;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        "cannot declare an initial value",
    );
}

#[test]
fn configuration_contract_resource_external_constant_state_must_match() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL CONSTANT
    Shared : INT := INT#4;
END_VAR
RESOURCE R ON CPU
VAR_EXTERNAL
    Shared : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        "must be CONSTANT",
    );
}

#[test]
fn configuration_contract_configuration_constant_initializes_resource_global() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL CONSTANT
    Base : INT := INT#7;
END_VAR
RESOURCE R ON CPU
VAR_GLOBAL
    Derived : INT := Base + INT#5;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.storage().get_global("Base"), Some(&Value::Int(7)));
    assert_eq!(
        runtime.storage().get_global("Derived"),
        Some(&Value::Int(12))
    );
}

#[test]
fn configuration_contract_resource_constant_chain_preserves_declaration_order() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL CONSTANT
    Base : INT := INT#3;
    Middle : INT := Base + INT#2;
    ResultValue : INT := Middle * INT#4;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.storage().get_global("Base"), Some(&Value::Int(3)));
    assert_eq!(runtime.storage().get_global("Middle"), Some(&Value::Int(5)));
    assert_eq!(
        runtime.storage().get_global("ResultValue"),
        Some(&Value::Int(20))
    );
}

#[test]
fn configuration_contract_rejects_configuration_resource_global_collision() {
    let error = compile_error(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Shared : INT;
END_VAR
RESOURCE R ON CPU
VAR_GLOBAL
    shared : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("duplicate global")
            && error.to_ascii_lowercase().contains("shared"),
        "{error}"
    );
}

#[test]
fn configuration_contract_rejects_root_resource_global_collision() {
    let error = compile_error(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
PROGRAM Main
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL
    SHARED : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("duplicate global")
            && error.to_ascii_lowercase().contains("shared"),
        "{error}"
    );
}

#[test]
fn configuration_contract_rejects_program_resource_global_collision() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR_GLOBAL
    Shared : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL
    Shared : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("duplicate global")
            && error.to_ascii_lowercase().contains("shared"),
        "{error}"
    );
}

#[test]
fn configuration_contract_effective_globals_keep_unqualified_runtime_names() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION CellConfiguration
VAR_GLOBAL
    ConfigurationValue : INT;
END_VAR
RESOURCE CellResource ON CPU
VAR_GLOBAL
    ResourceValue : DINT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert!(runtime.globals().contains_key("ConfigurationValue"));
    assert!(runtime.globals().contains_key("ResourceValue"));
    assert!(!runtime
        .globals()
        .contains_key("CellConfiguration.ConfigurationValue"));
    assert!(!runtime.globals().contains_key("CellResource.ResourceValue"));
}

#[test]
fn configuration_contract_resource_global_retain_policies_are_preserved() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL RETAIN
    Retained : INT;
END_VAR
VAR_GLOBAL NON_RETAIN
    Reinitialized : INT;
END_VAR
VAR_GLOBAL PERSISTENT
    PersistentValue : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert_eq!(runtime.globals()["Retained"].retain, RetainPolicy::Retain);
    assert_eq!(
        runtime.globals()["Reinitialized"].retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        runtime.globals()["PersistentValue"].retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn configuration_contract_resource_global_direct_address_is_bound() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL
    InputSignal AT %IX4.2 : BOOL;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert!(runtime
        .io()
        .bindings()
        .iter()
        .any(|binding| binding.display_name.as_deref() == Some("InputSignal")));
}

#[test]
fn configuration_contract_rejects_ordinary_and_directional_sections_in_configuration() {
    for section in ["VAR", "VAR_INPUT", "VAR_OUTPUT", "VAR_IN_OUT"] {
        let source = format!(
            r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
{section}
    InvalidValue : INT;
END_VAR
PROGRAM P : Main;
END_CONFIGURATION
"#
        );
        let error = compile_error(&source);
        assert!(
            error.contains(section)
                && error
                    .to_ascii_lowercase()
                    .contains("unsupported variable section"),
            "{section}: {error}"
        );
    }
}

#[test]
fn configuration_contract_rejects_temporary_and_static_sections_in_configuration() {
    for section in ["VAR_TEMP", "VAR_STAT"] {
        let source = format!(
            r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
{section}
    InvalidValue : INT;
END_VAR
PROGRAM P : Main;
END_CONFIGURATION
"#
        );
        let error = compile_error(&source);
        assert!(
            error.contains(section)
                && error
                    .to_ascii_lowercase()
                    .contains("unsupported variable section"),
            "{section}: {error}"
        );
    }
}

#[test]
fn configuration_contract_rejects_ordinary_and_directional_sections_in_resource() {
    for section in ["VAR", "VAR_INPUT", "VAR_OUTPUT", "VAR_IN_OUT"] {
        let source = format!(
            r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
{section}
    InvalidValue : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#
        );
        let error = compile_error(&source);
        assert!(
            error.contains(section)
                && error
                    .to_ascii_lowercase()
                    .contains("unsupported variable section"),
            "{section}: {error}"
        );
    }
}

#[test]
fn configuration_contract_rejects_temporary_and_static_sections_in_resource() {
    for section in ["VAR_TEMP", "VAR_STAT"] {
        let source = format!(
            r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
{section}
    InvalidValue : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#
        );
        let error = compile_error(&source);
        assert!(
            error.contains(section)
                && error
                    .to_ascii_lowercase()
                    .contains("unsupported variable section"),
            "{section}: {error}"
        );
    }
}
