//! Bounded handoff of authorized method commands to the existing Hmi.Display boundary
//! (specification 39, §21.2 item 2). HTTP handlers queue a call only after resolving the control
//! and checking the session, the role, the interlock and the inputs; the runtime makes one call
//! per boundary, between scans.
use super::member_paths::{member_value, MemberPath};
use crate::runtime::{CompiledRuntime, InvocationError};
use crate::{error::RuntimeError, value::Value};
use smol_str::SmolStr;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use std::time::Instant;

const CAPACITY: usize = 32;
// One call per boundary: each gets the same budget, independent of queue position. Distinct
// operator commands cannot overwrite one another before the FB's next cyclic call.
const PER_BOUNDARY: usize = 1;
/// The statements and loop passes the calls of one boundary may use together.
const INSTRUCTION_BUDGET: u32 = 32768;

/// One method call.
pub(crate) struct Invocation {
    pub path: MemberPath,
    pub interface: String,
    pub method: String,
    pub inputs: Vec<(SmolStr, Value)>,
    /// The text member read right after the call when the method returns FALSE: why the
    /// instance did not accept the command (§21.2 item 3).
    pub reason: Option<MemberPath>,
}

#[derive(Debug)]
pub(crate) enum Reply {
    /// The method's result, and the reason text when it returned FALSE and the instance has one.
    Returned(Value, Option<String>),
    /// Not called: the call was withdrawn or is not allowed.
    Refused(String),
    /// The call started and failed; its effect is unknown.
    Fault(String),
}

struct Pending {
    invocation: Invocation,
    generation: u64,
    deadline: Instant,
    reply: Sender<Reply>,
}

#[derive(Default)]
pub(crate) struct MethodExchange {
    generation: AtomicU64,
    pending: Mutex<VecDeque<Pending>>,
}

impl MethodExchange {
    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Invalidate unstarted work when the active bundle or PLC program changes.
    pub(crate) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn enqueue(
        &self,
        invocation: Invocation,
        generation: u64,
        deadline: Instant,
    ) -> Result<Receiver<Reply>, String> {
        if invocation.inputs.len() > 32 {
            return Err("method call exceeds its bounds".into());
        }
        let mut queue = self
            .pending
            .lock()
            .map_err(|_| "HMI method queue unavailable")?;
        if generation != self.generation() {
            return Err("HMI or PLC program changed; reload the view".into());
        }
        if deadline <= Instant::now() {
            return Err("command expired before queueing".into());
        }
        if queue.len() >= CAPACITY {
            return Err("HMI method queue full".into());
        }
        let (reply, receiver) = mpsc::channel();
        queue.push_back(Pending {
            invocation,
            generation,
            deadline,
            reply,
        });
        Ok(receiver)
    }

    /// Makes the boundary's call between scans, after a scan in which the program called
    /// `Hmi.Display` (§21.2 item 2), and answers it.
    ///
    /// # Errors
    ///
    /// The fault of a method that faulted; the resource has faulted then, as at a faulted scan,
    /// and the work queued before it is withdrawn.
    pub(crate) fn serve(&self, runtime: &mut CompiledRuntime) -> Result<(), RuntimeError> {
        let mut budget = INSTRUCTION_BUDGET;
        self.serve_with(|invocation| {
            let value = runtime.invoke_hmi_method(
                &invocation.path,
                &invocation.interface,
                &invocation.method,
                &invocation.inputs,
                &mut budget,
            )?;
            // Read at the same boundary, before any scan changes it.
            let reason = match (&value, &invocation.reason) {
                (Value::Bool(false), Some(path)) => {
                    match runtime.with_program_view(|program| member_value(path, program)) {
                        Some(Value::String(text)) => Some(text.to_string()),
                        _ => None,
                    }
                }
                _ => None,
            };
            Ok((value, reason))
        })
    }

    fn serve_with(
        &self,
        mut execute: impl FnMut(&Invocation) -> Result<(Value, Option<String>), InvocationError>,
    ) -> Result<(), RuntimeError> {
        for _ in 0..PER_BOUNDARY {
            // Contention defers work to another Hmi.Display call instead of blocking the PLC.
            let pending = match self.pending.try_lock() {
                Ok(mut queue) => queue.pop_front(),
                Err(_) => return Ok(()),
            };
            let Some(pending) = pending else {
                break;
            };
            let refusal = if pending.generation != self.generation() {
                Some("HMI or PLC program changed; reload the view")
            } else if pending.deadline <= Instant::now() {
                Some("command expired before execution")
            } else {
                None
            };
            if let Some(reason) = refusal {
                let _ = pending.reply.send(Reply::Refused(reason.into()));
                continue;
            }
            // From this point execution has started. A lost response must never trigger replay.
            let reply = match execute(&pending.invocation) {
                Ok((value, reason)) => Reply::Returned(value, reason),
                Err(InvocationError::Refused(reason)) => Reply::Refused(reason),
                Err(InvocationError::Fault(error)) => {
                    let _ = pending.reply.send(Reply::Fault(error.to_string()));
                    self.invalidate();
                    return Err(error);
                }
            };
            let _ = pending.reply.send(reply);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn call(value: i32) -> Invocation {
        Invocation {
            path: MemberPath::parse("Main.Pump").unwrap(),
            interface: "ITest".into(),
            method: "Request".into(),
            inputs: vec![("Value".into(), Value::DInt(value))],
            reason: None,
        }
    }
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(60)
    }
    fn returned(call: &Invocation) -> Result<(Value, Option<String>), InvocationError> {
        Ok((call.inputs[0].1.clone(), None))
    }

    #[test]
    fn distinct_requests_queued_before_one_scan_are_not_coalesced() {
        let exchange = MethodExchange::default();
        let a = exchange.enqueue(call(1), 0, deadline()).unwrap();
        let b = exchange.enqueue(call(2), 0, deadline()).unwrap();
        let mut seen = Vec::new();
        exchange
            .serve_with(|call| {
                seen.push(call.inputs[0].1.clone());
                returned(call)
            })
            .unwrap();
        assert_eq!(seen, vec![Value::DInt(1)]);
        assert!(
            b.try_recv().is_err(),
            "second intent waits for the next boundary"
        );
        exchange
            .serve_with(|call| {
                seen.push(call.inputs[0].1.clone());
                returned(call)
            })
            .unwrap();
        assert_eq!(seen, vec![Value::DInt(1), Value::DInt(2)]);
        assert!(matches!(
            a.try_recv(),
            Ok(Reply::Returned(Value::DInt(1), None))
        ));
        assert!(matches!(
            b.try_recv(),
            Ok(Reply::Returned(Value::DInt(2), None))
        ));
    }

    #[test]
    fn normal_warm_restart_discards_old_generation_requests_without_changing_restart_rules() {
        let mut harness = crate::harness::TestHarness::from_source(
            "PROGRAM Main VAR Marker : BOOL; END_VAR END_PROGRAM",
        )
        .unwrap();
        let exchange = std::sync::Arc::new(super::super::exchange::HmiExchange::default());
        harness.runtime_mut().set_hmi_exchange(exchange.clone());
        let response = exchange
            .methods
            .enqueue(call(1), exchange.methods.generation(), deadline())
            .unwrap();
        harness.restart(crate::RestartMode::Warm).unwrap();
        exchange
            .methods
            .serve_with(|_| panic!("pre-restart request executed"))
            .unwrap();
        assert!(matches!(response.try_recv(), Ok(Reply::Refused(_))));
    }

    #[test]
    fn capacity_and_per_scan_limits_preserve_existing_requests() {
        let exchange = MethodExchange::default();
        let receivers: Vec<_> = (0..CAPACITY)
            .map(|i| exchange.enqueue(call(i as i32), 0, deadline()).unwrap())
            .collect();
        assert!(exchange.enqueue(call(99), 0, deadline()).is_err());
        let mut count = 0;
        exchange
            .serve_with(|_| {
                count += 1;
                Ok((Value::Bool(true), None))
            })
            .unwrap();
        assert_eq!(count, PER_BOUNDARY);
        assert!(receivers[PER_BOUNDARY].try_recv().is_err());
    }

    #[test]
    fn stale_and_expired_requests_never_execute() {
        let exchange = MethodExchange::default();
        let old = exchange.enqueue(call(1), 0, deadline()).unwrap();
        exchange.invalidate();
        assert!(exchange.enqueue(call(2), 0, deadline()).is_err());
        exchange
            .serve_with(|_| panic!("stale request executed"))
            .unwrap();
        assert!(matches!(old.try_recv(), Ok(Reply::Refused(_))));
        let expired = exchange.enqueue(call(3), 1, deadline()).unwrap();
        exchange
            .pending
            .lock()
            .unwrap()
            .front_mut()
            .unwrap()
            .deadline = Instant::now();
        exchange
            .serve_with(|_| panic!("expired request executed"))
            .unwrap();
        assert!(matches!(expired.try_recv(), Ok(Reply::Refused(_))));
    }

    #[test]
    fn a_refusing_method_brings_the_instances_reason_read_at_the_same_boundary() {
        let mut harness = crate::harness::TestHarness::from_source(
            r#"
INTERFACE IRun
METHOD Start : BOOL
END_METHOD
END_INTERFACE
FUNCTION_BLOCK Motor IMPLEMENTS IRun
VAR_OUTPUT
    CommandText : STRING[80] := 'Auto: switch to Manual';
END_VAR
METHOD PUBLIC Start : BOOL
    Start := FALSE;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    M1 : Motor;
END_VAR
    M1();
END_PROGRAM
"#,
        )
        .unwrap();
        harness.cycle();
        let exchange = MethodExchange::default();
        let reply = exchange
            .enqueue(
                Invocation {
                    path: MemberPath::parse("Main.M1").unwrap(),
                    interface: "IRun".into(),
                    method: "Start".into(),
                    inputs: Vec::new(),
                    reason: Some(MemberPath::parse("Main.M1.CommandText").unwrap()),
                },
                0,
                deadline(),
            )
            .unwrap();
        exchange.serve(harness.runtime_mut()).unwrap();
        match reply.try_recv() {
            Ok(Reply::Returned(Value::Bool(false), Some(reason))) => {
                assert_eq!(reason, "Auto: switch to Manual")
            }
            other => panic!("a refusal with the instance's reason: {other:?}"),
        }
    }
}
