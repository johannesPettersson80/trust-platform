//! The Live Values of a compiled program at its scan's boundaries (specification 44 §13.3;
//! specification 13, Live Values mutation lifetime), against a stand-in program that records each
//! write it takes and refuses what it does not hold: the variables `MAIN.SPEED` and `MAIN.COUNT`,
//! the retained `@program/Main/Hours`, the member `PRESET` of the instance whose symbol is 4, and
//! the first two bytes of each process image.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{FrameId, Generation, InstanceId, IoArea};
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::reflect::Located;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::value::{Duration, Value};

use super::{compiled_cycle_start, compiled_images, WriteSink};
use crate::live_values::queues::{
    ForcedVarTarget, LiveValueQueues, PendingLValueWrite, PendingVarTarget,
};

#[derive(Default)]
struct Program {
    written: Vec<(String, Value)>,
}

impl CompiledVariables for Program {
    /// The value last written to `name`; Live Values never reads.
    fn read_named(&self, name: &str) -> Result<Value, RuntimeError> {
        self.written
            .iter()
            .rev()
            .find(|(written, _)| written == name)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.into()))
    }

    fn write_named(&mut self, name: &str, value: &Value) -> Result<(), RuntimeError> {
        if !matches!(name, "MAIN.SPEED" | "MAIN.COUNT") {
            return Err(RuntimeError::UndefinedVariable(name.into()));
        }
        self.written.push((name.to_string(), value.clone()));
        Ok(())
    }

    fn write_retained(&mut self, label: &str, value: &Value) -> Result<(), RuntimeError> {
        if label != "@program/Main/Hours" {
            return Err(RuntimeError::UndefinedVariable(label.into()));
        }
        self.written.push((label.to_string(), value.clone()));
        Ok(())
    }

    fn write_member(
        &mut self,
        instance: u32,
        member: &str,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        if instance != 4 {
            return Err(RuntimeError::UndefinedVariable(
                format!("{instance}").into(),
            ));
        }
        if member != "PRESET" {
            return Err(RuntimeError::UndefinedField(member.into()));
        }
        self.written
            .push((format!("{instance}.{member}"), value.clone()));
        Ok(())
    }

    fn write_image(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        if address.byte > 1 {
            return Err(RuntimeError::IoAddress("outside the image".into()));
        }
        self.written.push((format!("{address:?}"), value));
        Ok(())
    }

    fn program_image(&self) -> Image<'_> {
        unreachable!("the stand-in holds its variables by name, without a program image")
    }

    fn read_located(&self, _at: &Located) -> Result<Value, RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }

    fn write_located(&mut self, _at: &Located, _value: &Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
}

/// Each refusal: the target's text, the error and the logical time.
#[derive(Default)]
struct Refusals(Vec<(String, RuntimeError, Duration)>);

impl WriteSink for Refusals {
    fn refused(&mut self, target: &str, error: &RuntimeError, time: Duration) {
        self.0.push((target.to_string(), error.clone(), time));
    }
}

/// The generation the stand-in program is installed in.
const NOW: Generation = Generation(1);

fn at(milliseconds: i64) -> Duration {
    Duration::from_millis(milliseconds)
}

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("the address parses")
}

#[test]
fn the_queued_writes_reach_a_compiled_program_in_arrival_order_and_a_refused_one_is_reported() {
    // Specification 44 §13.3 and specification 13: the cycle start applies the queued writes in
    // arrival order through the symbol table, a name, a retain-map key and an instance's member;
    // a name the program does not hold, an instance identity of another generation, a local of a
    // paused frame and a debugger's expression write are each refused with the error the engine
    // gives and the drain goes on. Catches a refusal that stops the drain, writes applied out of
    // order, a stale identity written, and a queue left full.
    let mut queues = LiveValueQueues::<u8>::new();
    for (target, value) in [
        (PendingVarTarget::Global("MAIN.SPEED".into()), Value::Int(5)),
        (
            PendingVarTarget::Global("MAIN.MISSING".into()),
            Value::Int(1),
        ),
        (
            PendingVarTarget::Retain("@program/Main/Hours".into()),
            Value::DInt(7),
        ),
        (
            PendingVarTarget::Instance(Generation(0), InstanceId(4), "PRESET".into()),
            Value::DInt(3),
        ),
        (
            PendingVarTarget::Instance(NOW, InstanceId(4), "PRESET".into()),
            Value::DInt(9),
        ),
        (
            PendingVarTarget::Local(FrameId(2), "n".into()),
            Value::Int(4),
        ),
        (PendingVarTarget::Global("MAIN.COUNT".into()), Value::Int(6)),
    ] {
        queues
            .try_upsert_var_write(target, value)
            .expect("the write is queued");
    }
    queues.enqueue_lvalue_write(PendingLValueWrite {
        frame_id: None,
        target: 7,
        value: Value::Bool(true),
    });
    let mut program = Program::default();
    let mut refusals = Refusals::default();
    compiled_cycle_start(&mut queues, NOW, &mut program, at(10), &mut refusals)
        .expect("no refusal faults the cycle");
    assert_eq!(
        program.written,
        vec![
            ("MAIN.SPEED".to_string(), Value::Int(5)),
            ("@program/Main/Hours".to_string(), Value::DInt(7)),
            ("4.PRESET".to_string(), Value::DInt(9)),
            ("MAIN.COUNT".to_string(), Value::Int(6)),
        ]
    );
    assert_eq!(
        refusals.0,
        vec![
            (
                "MAIN.MISSING".to_string(),
                RuntimeError::UndefinedVariable("MAIN.MISSING".into()),
                at(10)
            ),
            (
                "4.PRESET".to_string(),
                RuntimeError::ReferenceLifetime,
                at(10)
            ),
            ("2.n".to_string(), RuntimeError::ReferenceLifetime, at(10)),
            (
                "7".to_string(),
                RuntimeError::ProfileUnsupported(SmolStr::new_static(
                    "a debugger's expression write to a compiled program"
                )),
                at(10)
            ),
        ]
    );
    assert!(queues.is_idle(), "every queued write is consumed");
}

#[test]
fn a_force_holds_a_compiled_variable_each_cycle_until_released_and_a_refused_one_is_withdrawn() {
    // Specification 13, Live Values mutation lifetime: a force is applied at every cycle start
    // until it is released; a force the program refuses (a name it does not hold, a retain-map
    // key it does not have, an instance of another generation) is reported once and withdrawn.
    // Three cycles: the first applies `MAIN.COUNT` and refuses the other three, the second applies
    // `MAIN.COUNT` again and refuses nothing, and after the release the third applies nothing.
    // Catches a force applied once only, a refused force reported every cycle, and a released
    // force still applied.
    let mut queues = LiveValueQueues::<u8>::new();
    for (target, value) in [
        (ForcedVarTarget::Global("MAIN.COUNT".into()), Value::Int(3)),
        (ForcedVarTarget::Global("MAIN.GONE".into()), Value::Int(1)),
        (
            ForcedVarTarget::Retain("@program/Main/Days".into()),
            Value::DInt(2),
        ),
        (
            ForcedVarTarget::Instance(Generation(0), InstanceId(4), "PRESET".into()),
            Value::DInt(2),
        ),
    ] {
        queues
            .try_upsert_forced_var(target, value)
            .expect("the force is set");
    }
    let mut program = Program::default();
    let mut refusals = Refusals::default();
    for time in [10, 20] {
        compiled_cycle_start(&mut queues, NOW, &mut program, at(time), &mut refusals)
            .expect("no refusal faults the cycle");
    }
    let count = ("MAIN.COUNT".to_string(), Value::Int(3));
    assert_eq!(program.written, vec![count.clone(), count.clone()]);
    assert_eq!(
        refusals.0,
        vec![
            (
                "MAIN.GONE".to_string(),
                RuntimeError::UndefinedVariable("MAIN.GONE".into()),
                at(10)
            ),
            (
                "@program/Main/Days".to_string(),
                RuntimeError::ReferenceLifetime,
                at(10)
            ),
            (
                "4.PRESET".to_string(),
                RuntimeError::ReferenceLifetime,
                at(10)
            ),
        ]
    );
    assert_eq!(
        queues.forced_vars.len(),
        1,
        "the refused forces are withdrawn"
    );
    queues.clear_forced_vars(|target| *target == ForcedVarTarget::Global("MAIN.COUNT".into()));
    compiled_cycle_start(&mut queues, NOW, &mut program, at(30), &mut refusals)
        .expect("the cycle runs");
    assert_eq!(program.written.len(), 2, "a released force is not applied");
}

#[test]
fn the_image_writes_and_forces_reach_a_compiled_program_at_their_boundaries() {
    // Specification 44 §13.3: after the drivers read the inputs, the queued image writes in
    // arrival order and then the forces of the input and marker images; before the outputs reach
    // the drivers, the forces of the output image, each scan. A forced address the program
    // refuses is reported and withdrawn. Catches an output force applied before the scan, which
    // the program would overwrite, an input force applied after the scan sampled the inputs, a
    // one-shot write applied twice, and a refused force applied again.
    let mut queues = LiveValueQueues::<u8>::new();
    queues
        .try_enqueue_io_write(address("%IX0.0"), Value::Bool(true))
        .expect("the write is queued");
    queues
        .try_enqueue_io_write(address("%MB1"), Value::Byte(3))
        .expect("the write is queued");
    for (text, value) in [
        ("%QX0.1", Value::Bool(true)),
        ("%IX0.1", Value::Bool(true)),
        ("%QB5", Value::Byte(9)),
    ] {
        queues
            .try_upsert_forced_io(address(text), value)
            .expect("the force is set");
    }
    let mut program = Program::default();
    let mut refusals = Refusals::default();
    let inputs = [IoArea::Input, IoArea::Memory];
    let outputs = [IoArea::Output];
    let label = |text: &str| format!("{:?}", address(text));
    compiled_images(&mut queues, &inputs, &mut program, at(10), &mut refusals)
        .expect("the inputs' boundary runs");
    assert_eq!(
        program.written,
        vec![
            (label("%IX0.0"), Value::Bool(true)),
            (label("%MB1"), Value::Byte(3)),
            (label("%IX0.1"), Value::Bool(true)),
        ]
    );
    compiled_images(&mut queues, &outputs, &mut program, at(10), &mut refusals)
        .expect("the outputs' boundary runs");
    assert_eq!(program.written[3..], [(label("%QX0.1"), Value::Bool(true))]);
    assert_eq!(
        refusals.0,
        vec![(
            label("%QB5"),
            RuntimeError::IoAddress("outside the image".into()),
            at(10)
        )]
    );
    // The next scan: the forces again, the one-shot writes not, the refused force not.
    compiled_images(&mut queues, &inputs, &mut program, at(20), &mut refusals)
        .expect("the inputs' boundary runs");
    compiled_images(&mut queues, &outputs, &mut program, at(20), &mut refusals)
        .expect("the outputs' boundary runs");
    assert_eq!(
        program.written[4..],
        [
            (label("%IX0.1"), Value::Bool(true)),
            (label("%QX0.1"), Value::Bool(true)),
        ]
    );
    assert_eq!(refusals.0.len(), 1);
}
