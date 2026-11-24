//! The Live Values of a compiled program at its scan's boundaries (specification 44 §13.3;
//! specification 13, Live Values mutation lifetime), against a stand-in program that records each
//! write it takes and refuses what it does not hold: the variables `MAIN.SPEED` and `MAIN.COUNT`,
//! the retained `@program/Main/Hours`, the member `PRESET` of the instance whose symbol is 4, and
//! the first two bytes of each process image.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{FrameId, Generation, InstanceId, IoArea};
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::reflect::Located;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::trace::ChangeSource;
use trust_runtime_core::value::{Duration, Value};

use super::{compiled_cycle_start, compiled_images, ForceEffect, WriteSink};
use crate::live_values::queues::{ForcedVarTarget, LiveValueQueues, PendingVarTarget};

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
    let mut queues = LiveValueQueues::new();
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
    let mut queues = LiveValueQueues::new();
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
    let mut queues = LiveValueQueues::new();
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
    compiled_images(
        &mut queues,
        NOW,
        &inputs,
        &mut program,
        at(10),
        &mut refusals,
    )
    .expect("the inputs' boundary runs");
    assert_eq!(
        program.written,
        vec![
            (label("%IX0.0"), Value::Bool(true)),
            (label("%MB1"), Value::Byte(3)),
            (label("%IX0.1"), Value::Bool(true)),
        ]
    );
    compiled_images(
        &mut queues,
        NOW,
        &outputs,
        &mut program,
        at(10),
        &mut refusals,
    )
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
    compiled_images(
        &mut queues,
        NOW,
        &inputs,
        &mut program,
        at(20),
        &mut refusals,
    )
    .expect("the inputs' boundary runs");
    compiled_images(
        &mut queues,
        NOW,
        &outputs,
        &mut program,
        at(20),
        &mut refusals,
    )
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

#[test]
fn a_variable_force_is_applied_again_after_the_inputs_between_the_image_writes_and_forces() {
    // Specification 44 §13.3 (the CODESYS toolkit's B17): after the drivers read the inputs, the
    // queued image writes come first, then every variable force again, then the forces of the input
    // and marker images. There the program's write of a forced variable holds it at the input or
    // marker address that binds it, so that the sample hands the program the forced value. Before
    // the outputs, the image boundary applies no variable force, which the loop applies just before
    // it. Catches a variable force applied only at the cycle start, which the sample of a variable
    // bound to an input replaces before the program runs (the toolkit's forced input that its
    // program never saw), a force applied before a one-shot write of its address, which the write
    // would replace, and a force applied twice before the outputs.
    let mut queues = LiveValueQueues::new();
    queues
        .try_enqueue_io_write(address("%IB0"), Value::Byte(5))
        .expect("the write is queued");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("MAIN.SPEED".into()), Value::Int(9))
        .expect("the force is set");
    queues
        .try_upsert_forced_io(address("%IX1.0"), Value::Bool(true))
        .expect("the force is set");
    let mut program = Program::default();
    let mut refusals = Refusals::default();
    let label = |text: &str| format!("{:?}", address(text));
    compiled_images(
        &mut queues,
        NOW,
        &[IoArea::Input, IoArea::Memory],
        &mut program,
        at(10),
        &mut refusals,
    )
    .expect("the inputs' boundary runs");
    assert_eq!(
        program.written,
        vec![
            (label("%IB0"), Value::Byte(5)),
            ("MAIN.SPEED".to_string(), Value::Int(9)),
            (label("%IX1.0"), Value::Bool(true)),
        ]
    );
    compiled_images(
        &mut queues,
        NOW,
        &[IoArea::Memory, IoArea::Output],
        &mut program,
        at(10),
        &mut refusals,
    )
    .expect("the outputs' boundary runs");
    assert_eq!(
        program.written.len(),
        3,
        "no variable force before the outputs"
    );
    assert_eq!(refusals.0, Vec::new(), "nothing is refused");
}

/// What a recording sink is told: each refusal, address write and force effect, as text.
#[derive(Default)]
struct Recorded(Vec<String>);

impl WriteSink for Recorded {
    fn refused(&mut self, target: &str, _error: &RuntimeError, _time: Duration) {
        self.0.push(format!("refused {target}"));
    }

    fn records(&self) -> bool {
        true
    }

    fn written(
        &mut self,
        source: ChangeSource,
        target: &PendingVarTarget,
        _variables: &dyn CompiledVariables,
    ) {
        self.0.push(format!("wrote {target:?} {source:?}"));
    }

    fn address_written(
        &mut self,
        source: ChangeSource,
        address: &IoAddress,
        value: &Value,
        _variables: &dyn CompiledVariables,
    ) {
        self.0
            .push(format!("wrote {address:?} {value:?} {source:?}"));
    }

    fn forced(&mut self, effect: ForceEffect<'_>, _variables: &dyn CompiledVariables) {
        self.0.push(match effect {
            ForceEffect::VariableSet(target) => format!("set {target:?}"),
            ForceEffect::VariableReleased(target) => format!("released {target:?}"),
            ForceEffect::AddressSet(address, value) => format!("set {address:?} {value:?}"),
            ForceEffect::AddressReleased(address) => format!("released {address:?}"),
        });
    }

    fn force_changes_lost(&mut self) {
        self.0.push("lost".to_string());
    }
}

#[test]
fn a_force_change_is_reported_once_as_the_cycle_start_leaves_it() {
    // Specification 48 RU-59: a cycle trace records a force where it is set or released, once, as
    // the cycle start leaves it. Between two cycle starts, MAIN.SPEED is forced at 5; MAIN.COUNT is
    // forced and released; MAIN.NOTHING, which the program does not hold, is forced; %QW0 is forced
    // at 9 and then at 10. The cycle start refuses NOTHING, then reports, in the order of each
    // target's last change: SPEED set, COUNT released, %QW0 set at 10. It does not report NOTHING's
    // set, which never took effect. The next cycle start reports nothing, though the forces still
    // hold, and lost changes are reported. Catches a force reported in each cycle it holds, a change
    // reported twice, an earlier value reported, and a refused force reported as set.
    let speed = ForcedVarTarget::Global("MAIN.SPEED".into());
    let count = ForcedVarTarget::Global("MAIN.COUNT".into());
    let nothing = ForcedVarTarget::Global("MAIN.NOTHING".into());
    let level = address("%QW0");
    let mut queues = LiveValueQueues::new();
    let mut program = Program::default();
    let mut sink = Recorded::default();
    queues
        .try_upsert_forced_var(speed.clone(), Value::Int(5))
        .unwrap();
    queues
        .try_upsert_forced_var(count.clone(), Value::DInt(1))
        .unwrap();
    queues.clear_forced_vars(|target| *target == count);
    queues
        .try_upsert_forced_var(nothing.clone(), Value::Int(1))
        .unwrap();
    queues
        .try_upsert_forced_io(level.clone(), Value::Word(9))
        .unwrap();
    queues
        .try_upsert_forced_io(level.clone(), Value::Word(10))
        .unwrap();

    compiled_cycle_start(&mut queues, NOW, &mut program, at(0), &mut sink).unwrap();
    assert_eq!(
        sink.0,
        vec![
            "refused MAIN.NOTHING".to_string(),
            format!("set {speed:?}"),
            format!("released {count:?}"),
            format!("set {level:?} {:?}", Value::Word(10)),
        ]
    );
    assert_eq!(queues.force_changes, vec![]);

    sink.0.clear();
    compiled_cycle_start(&mut queues, NOW, &mut program, at(10), &mut sink).unwrap();
    assert_eq!(sink.0, Vec::<String>::new());
    queues.force_changes_lost = true;
    compiled_cycle_start(&mut queues, NOW, &mut program, at(20), &mut sink).unwrap();
    assert_eq!(sink.0, vec!["lost".to_string()]);
    assert!(!queues.force_changes_lost);
}

#[test]
fn an_address_write_is_reported_to_a_recording_sink_once_applied() {
    // Specification 48 RU-59: a cycle trace records `io.write` as the boundary applied it. %IX0.1 is
    // written TRUE and %IB5, outside the stand-in's image, is refused: the recording sink is told of
    // the first only, after it reached the program; a sink that records nothing is told of neither.
    // Catches a refused write recorded, and a write recorded before it was applied.
    let mut queues = LiveValueQueues::new();
    let mut program = Program::default();
    let mut sink = Recorded::default();
    for (text, value) in [("%IX0.1", Value::Bool(true)), ("%IB5", Value::Byte(3))] {
        queues.try_enqueue_io_write(address(text), value).unwrap();
    }
    compiled_images(
        &mut queues,
        NOW,
        &[IoArea::Input, IoArea::Memory],
        &mut program,
        at(0),
        &mut sink,
    )
    .unwrap();
    assert_eq!(
        sink.0,
        vec![
            format!(
                "wrote {:?} {:?} {:?}",
                address("%IX0.1"),
                Value::Bool(true),
                ChangeSource::ControlLink
            ),
            format!("refused {:?}", address("%IB5")),
        ]
    );
    assert_eq!(program.written.len(), 1);

    let mut silent = Refusals::default();
    queues
        .try_enqueue_io_write(address("%IX0.1"), Value::Bool(false))
        .unwrap();
    compiled_images(
        &mut queues,
        NOW,
        &[IoArea::Input, IoArea::Memory],
        &mut program,
        at(10),
        &mut silent,
    )
    .unwrap();
    assert_eq!(silent.0, vec![]);
    assert_eq!(program.written.len(), 2);
}

#[test]
fn each_applied_write_is_reported_with_its_source() {
    // Specification 48 Appendix C, C.6: the cycle start reports each write it applied with the
    // source that queued it, in arrival order: MAIN.SPEED from the control link, MAIN.COUNT from a
    // service's `variables.write`. Catches a write reported with another source.
    let speed = PendingVarTarget::Global("MAIN.SPEED".into());
    let count = PendingVarTarget::Global("MAIN.COUNT".into());
    let mut queues = LiveValueQueues::new();
    let mut program = Program::default();
    let mut sink = Recorded::default();
    queues
        .try_upsert_var_write(speed.clone(), Value::Int(1))
        .unwrap();
    queues
        .try_upsert_var_writes(vec![(count.clone(), Value::DInt(2))], ChangeSource::Service)
        .unwrap();
    compiled_cycle_start(&mut queues, NOW, &mut program, at(0), &mut sink).unwrap();
    assert_eq!(
        sink.0,
        vec![
            format!("wrote {speed:?} {:?}", ChangeSource::ControlLink),
            format!("wrote {count:?} {:?}", ChangeSource::Service),
        ]
    );
}

#[test]
fn each_applied_address_write_is_reported_with_its_source() {
    // Specification 48 Appendix C, C.6: the boundary reports each one-shot write it applied with
    // the source that queued it, in arrival order: %IX0.1 from the control link (`io.write`), %MB1
    // from Control Studio, and %IX0.2 from the control link again, after %MB1's earlier twin from
    // Control Studio was withdrawn. Catches a write reported with another source, and a source
    // that a withdrawal leaves behind for the next write.
    let mut queues = LiveValueQueues::new();
    let mut program = Program::default();
    let mut sink = Recorded::default();
    queues
        .try_enqueue_io_write(address("%IX0.1"), Value::Bool(true))
        .unwrap();
    queues
        .try_enqueue_io_write_from(address("%MB1"), Value::Byte(7), ChangeSource::ControlStudio)
        .unwrap();
    queues
        .try_enqueue_io_write_from(address("%MB1"), Value::Byte(3), ChangeSource::ControlStudio)
        .unwrap();
    queues.withdraw_io_write(&address("%MB1"), &Value::Byte(7));
    queues
        .try_enqueue_io_write(address("%IX0.2"), Value::Bool(true))
        .unwrap();
    compiled_images(
        &mut queues,
        NOW,
        &[IoArea::Input, IoArea::Memory],
        &mut program,
        at(0),
        &mut sink,
    )
    .unwrap();
    assert_eq!(
        sink.0,
        vec![
            format!(
                "wrote {:?} {:?} {:?}",
                address("%IX0.1"),
                Value::Bool(true),
                ChangeSource::ControlLink
            ),
            format!(
                "wrote {:?} {:?} {:?}",
                address("%MB1"),
                Value::Byte(3),
                ChangeSource::ControlStudio
            ),
            format!(
                "wrote {:?} {:?} {:?}",
                address("%IX0.2"),
                Value::Bool(true),
                ChangeSource::ControlLink
            ),
        ]
    );
    assert_eq!(queues.io_write_sources, vec![]);
}
