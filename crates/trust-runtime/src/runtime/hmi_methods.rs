//! The host HMI's typed, interface-constrained method entry (specification 39, §21.2). No HTTP
//! handler executes here: the call runs between scans through the kernel's method call of a
//! compiled program ([`CompiledRuntime::call_method`]); the HMI checks first that the instance
//! implements the interface and the interface declares the method.

use smol_str::SmolStr;
use trust_runtime_core::native::memory::{layout, Layout};
use trust_runtime_core::native::tool::tool_symbol;

use super::CompiledRuntime;
use crate::error::RuntimeError;
use crate::hmi::commands::write::path_text;
use crate::hmi::member_paths::{locate_member, MemberPath};
use crate::value::Value;

#[derive(Debug)]
pub(crate) enum InvocationError {
    /// Not called.
    Refused(String),
    /// The call started and failed; the resource has faulted.
    Fault(RuntimeError),
}

impl CompiledRuntime {
    /// Calls `method` of the instance at `path` with typed inputs, when its type implements
    /// `interface` and the interface declares the method; the instance's body does not run. The
    /// call draws on `budget` (statements and loop passes), which it reduces by the work it used.
    pub(crate) fn invoke_hmi_method(
        &mut self,
        path: &MemberPath,
        interface: &str,
        method: &str,
        inputs: &[(SmolStr, Value)],
        budget: &mut u32,
    ) -> Result<Value, InvocationError> {
        let capable = self.with_program_view(|program| {
            let at = locate_member(path, program)?;
            let image = program.program_image();
            let instance = matches!(layout(&image, at.type_index), Ok(Layout::Instance(_)));
            // An interface declares the method a tool calls when its tool symbol names it
            // (specification 44 §5.7, §13.3).
            Some(
                instance
                    && program.implements(&at, interface)
                    && image.find_symbol(&tool_symbol(interface, method)).is_some(),
            )
        });
        match capable {
            None => {
                return Err(InvocationError::Refused(
                    "equipment instance is unavailable".into(),
                ))
            }
            Some(false) => {
                return Err(InvocationError::Refused(
                    "HMI method is not declared by the required interface".into(),
                ))
            }
            Some(true) => {}
        }
        if *budget == 0 {
            return Err(InvocationError::Refused(
                "HMI method budget is exhausted".into(),
            ));
        }
        // A recording keeps the wall clock's readings the call makes (specification 48 RU-59).
        self.arm_call_readings();
        let called = self.call_method(&path_text(path), interface, method, inputs, *budget);
        let readings = self.take_call_readings();
        match called {
            Ok((value, used)) => {
                *budget = budget.saturating_sub(used);
                // A recording keeps the call as a screen's (specification 48 RU-59).
                self.record_hmi_method_call(path, interface, method, inputs, &readings);
                Ok(value)
            }
            // A method that faulted has faulted the resource, as a faulted scan does.
            Err(error) if self.faulted() => Err(InvocationError::Fault(error)),
            Err(error) => Err(InvocationError::Refused(error.to_string())),
        }
    }

    /// Serves the HMI's method commands between scans, after a scan in which the program called
    /// `Hmi.Display` (specification 39, §21.2 item 2): one queued call, answered.
    ///
    /// # Errors
    ///
    /// The fault of a method that faulted. The resource has faulted then (the fault latched and
    /// the fault policy's output action applied), so the caller reports the error without
    /// applying it again.
    pub(crate) fn serve_hmi_methods(
        &mut self,
        exchange: &crate::hmi::exchange::HmiExchange,
    ) -> Result<(), RuntimeError> {
        exchange.methods.serve(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;
    use crate::hmi::member_paths::member_value;

    const PROGRAM: &str = r#"
INTERFACE ICounter
METHOD Add : DINT
VAR_INPUT Amount : DINT; END_VAR
END_METHOD
END_INTERFACE
FUNCTION_BLOCK Counter IMPLEMENTS ICounter
VAR_OUTPUT Total : DINT; BodyCalls : DINT; END_VAR
BodyCalls := BodyCalls + 1;
METHOD PUBLIC Add : DINT
VAR_INPUT Amount : DINT; END_VAR
    Total := Total + Amount;
    Add := Total;
END_METHOD
METHOD PUBLIC HiddenFromCapability : DINT
    Total := 999;
    HiddenFromCapability := Total;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR Items : ARRAY[1..2] OF Counter; END_VAR
END_PROGRAM
"#;

    fn value(harness: &TestHarness, path: &str) -> Option<Value> {
        let path = MemberPath::parse(path).unwrap();
        harness
            .runtime()
            .with_program_view(|program| member_value(&path, program))
    }

    #[test]
    fn typed_method_runs_on_selected_instance_without_executing_fb_body() {
        let mut harness = TestHarness::from_source(PROGRAM).unwrap();
        harness.cycle();
        let mut budget = 10000;
        let path = MemberPath::parse("Main.Items[2]").unwrap();
        let result = harness
            .runtime_mut()
            .invoke_hmi_method(
                &path,
                "ICounter",
                "Add",
                &[("Amount".into(), Value::DInt(3))],
                &mut budget,
            )
            .unwrap();
        assert_eq!(result, Value::DInt(3));
        for (path, expected) in [
            ("Main.Items[1].Total", 0),
            ("Main.Items[2].Total", 3),
            ("Main.Items[2].BodyCalls", 0),
        ] {
            assert_eq!(value(&harness, path), Some(Value::DInt(expected)));
        }
        assert!(budget < 10000);
    }

    #[test]
    fn a_screens_method_call_is_recorded_as_the_next_cycles_change() {
        // Specification 48 RU-59 and Appendix C, C.6 (kind 7): a screen's method call between two
        // scans is the next cycle's change, a screen's (source 2), with the instance by the path
        // the debugger names it by and each input as its Structured Text literal. Worked by hand:
        // the recording of 2 cycles has the call `Add` of `main.items[2]`, an element the debugger
        // names `Main.Items[2]`, with `Amount` 3, made between its two cycles, in its second, the
        // input's literal `3`. Catches a screen's call not recorded, recorded as the control
        // link's, without its literal, or with the element as the page wrote it.
        use trust_runtime_core::trace::{decode, Change, ChangeKind, ChangeSource};
        use trust_runtime_services::trace::requests::{
            RecordRequest, TraceName, TraceState, Until,
        };

        let mut harness = TestHarness::from_source(PROGRAM).unwrap();
        harness.cycle();
        let traces = crate::runtime::new_traces();
        let request = RecordRequest {
            until: Until::Cycles(2),
            task: None,
        };
        harness
            .runtime_mut()
            .start_trace(&request, (&SmolStr::new("hmi"), "test"), &traces)
            .unwrap();
        harness.cycle();
        let path = MemberPath::parse("main.items[2]").unwrap();
        harness
            .runtime_mut()
            .invoke_hmi_method(
                &path,
                "ICounter",
                "Add",
                &[("Amount".into(), Value::DInt(3))],
                &mut 10_000,
            )
            .unwrap();
        harness.cycle();

        let slot = crate::runtime::lock_traces(&traces);
        let TraceState::Ended { bytes, .. } = slot.state(TraceName::Id(1)) else {
            panic!("the recording of 2 cycles has ended");
        };
        let mut file = vec![0; bytes];
        assert_eq!(slot.part(TraceName::Id(1), 0, &mut file), Some(bytes));
        let trace = decode(&file).unwrap().trace;
        assert_eq!(trace.cycles[0].changes, vec![]);
        assert_eq!(
            trace.cycles[1].changes,
            vec![Change {
                source: ChangeSource::Screen,
                kind: ChangeKind::MethodCall {
                    instance: "Main.Items[2]".into(),
                    interface: "ICounter".into(),
                    method: "Add".into(),
                    inputs: vec![("Amount".into(), "3".into())],
                    // `Add` reads no clock.
                    readings: vec![],
                },
            }]
        );
    }

    /// A function block whose method reads the wall clock into `called`, which the program copies
    /// to `%QL8`, beside the scan's own reading at `%QL0`, in the 10 ms task `Fast`: a replay runs
    /// the recorded task activations.
    const CLOCK_PROGRAM: &str = r#"
INTERFACE IStamp
METHOD Take : BOOL
END_METHOD
END_INTERFACE
FUNCTION_BLOCK Stamper IMPLEMENTS IStamp
VAR_OUTPUT called : LDT; END_VAR
METHOD PUBLIC Take : BOOL
    called := DT_TO_LDT(CURRENT_DT());
    Take := TRUE;
END_METHOD
END_FUNCTION_BLOCK
VAR_GLOBAL
    stamp AT %QL0 : LDT;
    called AT %QL8 : LDT;
END_VAR
PROGRAM Main
VAR clock : Stamper; END_VAR
clock();
stamp := DT_TO_LDT(CURRENT_DT());
called := clock.called;
END_PROGRAM
CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;

    #[test]
    fn a_screens_method_call_keeps_the_clock_it_read_and_replays_equal() {
        // Specification 48 RU-59 and RU-60: a method call between two scans keeps the wall
        // clock's readings it made, and a replay gives them back to it. Worked by hand on this
        // computer, whose wall clock is the system's, with the harness's logical time moved 10 ms
        // before each scan of the task: the recording's first scan reads t1 into `stamp`; 3 ms
        // later a screen's `Take` of `P1.clock` reads t2, at least t1 + 3 ms, into `called`; the
        // second scan copies t2 to `%QL8`. The trace's second cycle holds the call with its one
        // reading, t2, and a replay of the trace on the same build matches both cycles. Catches a call recorded without its readings (G296): its replay reads the last
        // reading the replay gave, t1, and the second cycle's `called` differs.
        use trust_runtime_core::trace::{decode, ChangeKind};
        use trust_runtime_services::trace::requests::{
            RecordRequest, TraceName, TraceState, Until,
        };

        let mut harness = TestHarness::from_source(CLOCK_PROGRAM).unwrap();
        harness.cycle();
        let traces = crate::runtime::new_traces();
        let request = RecordRequest {
            until: Until::Cycles(2),
            task: None,
        };
        harness
            .runtime_mut()
            .start_trace(&request, (&SmolStr::new("hmi"), "test"), &traces)
            .unwrap();
        let interval = crate::value::Duration::from_millis(10);
        harness.advance_time(interval);
        harness.cycle();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let path = MemberPath::parse("P1.clock").unwrap();
        harness
            .runtime_mut()
            .invoke_hmi_method(&path, "IStamp", "Take", &[], &mut 10_000)
            .unwrap();
        harness.advance_time(interval);
        harness.cycle();

        let file = {
            let slot = crate::runtime::lock_traces(&traces);
            let TraceState::Ended { bytes, .. } = slot.state(TraceName::Id(1)) else {
                panic!("the recording of 2 cycles has ended");
            };
            let mut file = vec![0; bytes];
            assert_eq!(slot.part(TraceName::Id(1), 0, &mut file), Some(bytes));
            file
        };
        let trace = decode(&file).unwrap().trace;
        let t1 = trace.cycles[0].wall_clock_ms[0];
        let ChangeKind::MethodCall { readings, .. } = &trace.cycles[1].changes[0].kind else {
            panic!(
                "the second cycle holds the call: {:?}",
                trace.cycles[1].changes
            );
        };
        assert_eq!(readings.len(), 1, "{readings:?}");
        assert!(
            readings[0] >= t1 + 3,
            "the call read {readings:?} after the scan's {t1}"
        );

        let mut replayer = TestHarness::from_source(CLOCK_PROGRAM).unwrap();
        let report = replayer.runtime_mut().replay_trace(&file).unwrap();
        assert_eq!(report.cycles, 2);
        assert_eq!(report.first_difference, None, "{report:?}");
    }

    #[test]
    fn methods_outside_capability_and_exhausted_budget_do_not_execute() {
        let mut harness = TestHarness::from_source(PROGRAM).unwrap();
        harness.cycle();
        let path = MemberPath::parse("Main.Items[1]").unwrap();
        let mut budget = 10000;
        assert!(harness
            .runtime_mut()
            .invoke_hmi_method(&path, "ICounter", "HiddenFromCapability", &[], &mut budget)
            .is_err());
        assert!(harness
            .runtime_mut()
            .invoke_hmi_method(&path, "IUnknown", "Add", &[], &mut budget)
            .is_err());
        budget = 0;
        assert!(harness
            .runtime_mut()
            .invoke_hmi_method(
                &path,
                "ICounter",
                "Add",
                &[("Amount".into(), Value::DInt(3))],
                &mut budget
            )
            .is_err());
        assert_eq!(value(&harness, "Main.Items[1].Total"), Some(Value::DInt(0)));
    }
}
