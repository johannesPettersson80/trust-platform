//! Spec 45: the public state snapshot observes the actual PID without mutation.
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::{compile, value, LessonRuntime};

const LIBRARY: &str = include_str!("../../../libraries/control/src/control.st");
const CONFIGURATION: &str = r#"
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#;

fn session(source: &str) -> CompileSession {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", LIBRARY),
        SourceFile::with_path("src/Main.st", format!("{source}\n{CONFIGURATION}")),
    ])
}

fn execute(source: &str) -> CompiledRuntime {
    let mut runtime = compile(session(source));
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("execute actual PID project");
    runtime
}

fn program_value(runtime: &CompiledRuntime, name: &str) -> Value {
    value(runtime, &format!("P1.{name}")).unwrap()
}

#[test]
fn control_pid_get_state_returns_an_independent_nonmutating_snapshot() {
    // Reach the real runtime interface before compiling any call to the new
    // method. Its absence must be an assertion red, not a compiler failure.
    let probe_source = r#"
USING Control;
PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Output : LREAL;
END_VAR
Parameters.ProportionalGain := LREAL#2.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := InitialState);
Input.Setpoint := LREAL#35.0;
Input.ProcessValue := LREAL#20.0;
Input.CycleTime := LREAL#0.1;
Output := Controller.Update(Input := Input);
END_PROGRAM
"#;
    let probe = execute(probe_source);
    assert_eq!(program_value(&probe, "Output"), Value::LReal(30.0));
    // The executed program is built from these declarations (specification 44 §3).
    let declarations = session(probe_source).declarations().unwrap();
    let definition = declarations
        .metadata()
        .function_blocks()
        .get("CONTROL.PID")
        .unwrap();
    assert!(
        definition.methods.iter().any(|method| method.name.eq_ignore_ascii_case("GetState")),
        "the executed Control.PID runtime interface must dispatch the public GetState observation method"
    );

    let runtime = execute(
        r#"
USING Control;
PROGRAM Main
VAR
    Controller : Control.PID;
    ReferenceController : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    FirstRead : Control.PIDInitialState;
    SecondRead : Control.PIDInitialState;
    ChangedCopy : Control.PIDInitialState;
    AfterCopyChange : Control.PIDInitialState;
    AfterUpdate : Control.PIDInitialState;
    ReferenceAfterUpdate : Control.PIDInitialState;
    Output : LREAL;
    ReferenceOutput : LREAL;
END_VAR
Parameters.ProportionalGain := LREAL#2.0;
Parameters.IntegralGain := LREAL#0.5;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
InitialState.Setpoint := LREAL#35.0;
InitialState.SetpointRate := LREAL#1.5;
InitialState.ProcessValue := LREAL#30.0;
InitialState.ProcessValueRate := LREAL#-2.5;
InitialState.FeedForward := LREAL#4.0;
InitialState.NominalOutput := LREAL#75.0;
InitialState.SaturatedOutput := LREAL#74.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := InitialState);
ReferenceController.Configure(Parameters := Parameters);
ReferenceController.Initialize(State := InitialState);
FirstRead := Controller.GetState();
SecondRead := Controller.GetState();
ChangedCopy := FirstRead;
ChangedCopy.Setpoint := LREAL#1001.0;
ChangedCopy.SetpointRate := LREAL#1002.0;
ChangedCopy.ProcessValue := LREAL#1003.0;
ChangedCopy.ProcessValueRate := LREAL#1004.0;
ChangedCopy.FeedForward := LREAL#1005.0;
ChangedCopy.NominalOutput := LREAL#1006.0;
ChangedCopy.SaturatedOutput := LREAL#1007.0;
AfterCopyChange := Controller.GetState();
Input.Setpoint := LREAL#35.0;
Input.ProcessValue := LREAL#30.0;
Input.FeedForward := LREAL#4.0;
Input.CycleTime := LREAL#0.1;
Output := Controller.Update(Input := Input);
ReferenceOutput := ReferenceController.Update(Input := Input);
AfterUpdate := Controller.GetState();
ReferenceAfterUpdate := ReferenceController.GetState();
END_PROGRAM
"#,
    );
    let initial = program_value(&runtime, "InitialState");
    assert_eq!(program_value(&runtime, "FirstRead"), initial);
    assert_eq!(program_value(&runtime, "SecondRead"), initial);
    assert_ne!(program_value(&runtime, "ChangedCopy"), initial);
    assert_eq!(program_value(&runtime, "AfterCopyChange"), initial);
    assert_eq!(program_value(&runtime, "Output"), Value::LReal(75.25));
    assert_eq!(
        program_value(&runtime, "Output"),
        program_value(&runtime, "ReferenceOutput")
    );
    assert_eq!(
        program_value(&runtime, "AfterUpdate"),
        program_value(&runtime, "ReferenceAfterUpdate")
    );
}
