//! A chip's loop loads the installed program in place (specification 34 STATE-08 and §11.2): a
//! `restart` of the Stopped resource that finds a committed container newer than the one the program
//! came from drops the program and builds the newest container's, without a reset. The loop is the
//! board fixture's, over the fake program flash of `tests/program_store.rs`, which has the C6's units,
//! and a loader that builds the container's program on the shared engine behind the scripted cycle
//! and counts the programs alive. Every expected value is written out here from the specification and
//! the requests below, not taken from the code under test.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::fmt::Write as _;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::sha256;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::ports::CheckpointStore;
use trust_runtime_core::retain::checkpoint::Slot;
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{Duration, Value as Retained};
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

use super::engine::{device_limits, CycleSettings, DeviceServices, EngineCycle};
use super::program::Program;
use super::{
    check_retained_values, Candidate, EngineLiveValues, Loaded, ProgramLoader, ResourceCycle,
    RetainConfig, RetainSource, Runner,
};
use crate::events::EVENT_STORE_CAPACITY;
use crate::live_values::var_requests::VariableQueuePort;
use crate::loading::select::Booted;
use crate::tests::program_store::{
    committed_image, transfer_of, CheckpointProbe, FlashOp, FlashProbe, PROGRAM, SLOT_BYTES,
};
use crate::tests::runner_ports::{
    ms, CycleCall, CycleScript, Fixture, ScriptedCycle, TestClock, TestIo, TestPlatform,
    TestRunner, CHIP_MEMORY,
};
use crate::transfer::{TransferSession, PART_BYTES};

/// Why a chip that booted from an erased store has no program, as its boot says.
const ERASED: &str = "no TRSTA002 container in either slot";
/// Why a chip whose newest container failed its payload check has no program, as its boot says.
const DAMAGED: &str = "the CRC-32 of entry kind 1 does not match";
/// The reason a booting chip's loop is built with, which its load of the program replaces.
const BOOTING: &str = "the installed program is being loaded";
/// The boot number of the transfer session.
const BOOT: u32 = 3;
/// The event capacity that the record of a loaded program asks for (the C6's default).
const CAPACITY: usize = 200;
/// The passes after which a test gives up waiting for the loop to write a reply.
const MAX_PASSES: usize = 200;
/// Memory enough for the boot of any container of these tests.
const ROOM: usize = 1024 * 1024;

/// One program that the loader was asked to build.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Load {
    /// The install sequence of the container.
    sequence: u64,
    /// The mode the `restart` asked for.
    mode: RestartMode,
    /// The length of the program that the container holds.
    program_bytes: usize,
}

/// The loader of the tests: it builds the program of an admitted container on the shared engine
/// behind the scripted cycle, and checks that the program that ended is gone before it builds.
struct TestLoader {
    clock: TestClock,
    io: TestIo,
    script: Rc<RefCell<CycleScript>>,
    services: &'static DeviceServices<TestClock>,
    checkpoints: CheckpointProbe,
    /// The programs it was asked to build.
    log: Rc<RefCell<Vec<Load>>>,
    /// The reasons of the next loads, which fail with them.
    failures: Rc<RefCell<VecDeque<String>>>,
    /// The holders of the process I/O's state while no program is alive: the fixture and this loader.
    baseline: usize,
}

impl<'a> ProgramLoader<TestPlatform, ScriptedCycle<'a>> for TestLoader {
    fn room(&self) -> usize {
        ROOM
    }

    fn load(
        &mut self,
        platform: &mut TestPlatform,
        booted: Booted,
        mode: RestartMode,
    ) -> Result<Loaded<ScriptedCycle<'a>>, String> {
        // Each program holds one handle on the process I/O's state, so the count tells how many
        // programs are alive. The program that ended is dropped before the next one is built.
        assert_eq!(
            Rc::strong_count(&self.io.state),
            self.baseline,
            "the program that ended is gone before the next one is built"
        );
        self.log.borrow_mut().push(Load {
            sequence: booted.sequence,
            mode,
            program_bytes: booted.program.len(),
        });
        if let Some(reason) = self.failures.borrow_mut().pop_front() {
            return Err(reason);
        }
        let prepared = PreparedModule::from_bytes(&booted.program, device_limits(CHIP_MEMORY))
            .map_err(|error| format!("the installed application cannot be prepared: {error}"))?;
        let prepared: &'static PreparedModule = Box::leak(Box::new(prepared));
        let live =
            EngineLiveValues::for_prepared(prepared, vec![None; prepared.io_bindings().len()])
                .map_err(|error| error.to_string())?;
        platform.live_values = Rc::new(live);
        let settings = CycleSettings {
            cycle_interval: Duration::from_millis(10),
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::Halt,
            safe_outputs: Vec::new(),
        };
        let retains = booted.record.configuration.retain_mode == RetainMode::File;
        let engine = if retains {
            EngineCycle::with_retain(
                prepared,
                self.services,
                self.io.clone(),
                settings,
                RetainConfig {
                    mode: RetainMode::File,
                    store: CheckpointStore {
                        flash: Box::new(self.checkpoints.clone()),
                        program_budget: 64,
                    },
                    identity: booted.record.checkpoint_identity(),
                    save_interval: Duration::from_millis(60_000),
                    image_bytes: 256,
                },
            )
        } else {
            EngineCycle::start(prepared, self.services, self.io.clone(), settings)
        };
        let mut engine = engine
            .map_err(|error| format!("the installed application cannot be started: {error}"))?;
        let events = if retains && mode == RestartMode::Warm {
            engine
                .restart(RestartMode::Warm, RetainSource::Checkpoint)
                .map_err(|error| format!("the retained values cannot be restored: {error}"))?
        } else {
            Vec::new()
        };
        Ok(Loaded {
            cycle: ScriptedCycle::new(engine, self.clock.clone(), Rc::clone(&self.script)),
            events,
            event_capacity: CAPACITY,
        })
    }

    /// These tests load programs in place and change none online (that is
    /// `online_change/contract_tests.rs`), so a change is refused.
    fn prepare_change(
        &mut self,
        _platform: &mut TestPlatform,
        _running: &mut ScriptedCycle<'a>,
        _program: Vec<u8>,
    ) -> Result<Candidate<ScriptedCycle<'a>>, String> {
        Err(String::from("these tests change no program online"))
    }

    fn commit_change(
        &mut self,
        _platform: &mut TestPlatform,
        _running: &mut ScriptedCycle<'a>,
        _candidate: &mut Candidate<ScriptedCycle<'a>>,
    ) {
        panic!("these tests commit no online change");
    }
}

/// One device: the board fixture's ports, a program flash and a checkpoint flash, and what the
/// loader of its loop was asked and told to do.
struct Rig {
    ports: Fixture,
    flash: FlashProbe,
    checkpoints: CheckpointProbe,
    log: Rc<RefCell<Vec<Load>>>,
    failures: Rc<RefCell<VecDeque<String>>>,
    unloads: Rc<Cell<u32>>,
}

impl Rig {
    /// A device over `flash` and `checkpoints`. Its link moves frames of up to 8,192 bytes, as the
    /// chips' USB links do.
    fn new(flash: FlashProbe, checkpoints: CheckpointProbe) -> Self {
        Self {
            ports: Fixture::with_frames_of(8_192),
            flash,
            checkpoints,
            log: Rc::new(RefCell::new(Vec::new())),
            failures: Rc::new(RefCell::new(VecDeque::new())),
            unloads: Rc::new(Cell::new(0)),
        }
    }

    /// The loader of this device, counting from the programs alive now.
    fn loader(&self) -> TestLoader {
        let io = self.ports.io.clone();
        TestLoader {
            clock: self.ports.clock.clone(),
            baseline: Rc::strong_count(&io.state),
            io,
            script: Rc::clone(&self.ports.script),
            services: self.ports.services(),
            checkpoints: self.checkpoints.clone(),
            log: Rc::clone(&self.log),
            failures: Rc::clone(&self.failures),
        }
    }

    /// The loop of a device whose program is the board probe, which came from the container with
    /// install sequence `came_from`, or from none.
    fn runner(&self, came_from: Option<u64>) -> TestRunner<'_> {
        let loader = self.loader();
        let platform = TestPlatform {
            unloads: Rc::clone(&self.unloads),
            ..TestPlatform::default()
        };
        let mut checkpoints = self.checkpoints.clone();
        self.ports
            .runner_with(platform)
            .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
            .with_program_store(self.flash.store(), move |admitted| {
                check_retained_values(&mut checkpoints, admitted)
            })
            .with_program_loader(loader, came_from)
    }

    /// The loop of a device that booted without a program, for `reason`, because the container with
    /// install sequence `refused` was refused, or because the store held none.
    fn stopped_runner(&self, reason: &'static str, refused: Option<u64>) -> TestRunner<'_> {
        let loader = self.loader();
        let platform = TestPlatform {
            unloads: Rc::clone(&self.unloads),
            ..TestPlatform::default()
        };
        let mut checkpoints = self.checkpoints.clone();
        Runner::stopped(
            self.ports.link.clone(),
            self.ports.wait.clone(),
            platform,
            self.ports.clock.clone(),
            SmolStr::new_static(reason),
            EVENT_STORE_CAPACITY,
        )
        .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
        .with_program_store(self.flash.store(), move |admitted| {
            check_retained_values(&mut checkpoints, admitted)
        })
        .with_program_loader(loader, refused)
    }

    /// Uploads `container` as one transfer, in parts, and returns its id as the requests name it.
    fn upload(&self, runner: &mut TestRunner<'_>, container: &[u8]) -> String {
        let begin = self.ports.call(
            runner,
            &json!({
                "id": 1,
                "type": "transfer.begin",
                "params": {"bytes": container.len(), "sha256": hex(&sha256(container))},
            }),
        );
        let transfer = begin["result"]["transfer"]
            .as_str()
            .expect("begin names the transfer")
            .to_string();
        for (index, part) in container.chunks(PART_BYTES).enumerate() {
            let reply = self.ports.call(
                runner,
                &json!({
                    "id": 2 + index,
                    "type": "transfer.part",
                    "params": {
                        "transfer": transfer,
                        "offset": index * PART_BYTES,
                        "data": STANDARD.encode(part),
                        "crc32": format!("{:08x}", crc32::checksum(part)),
                    },
                }),
            );
            assert_eq!(reply["ok"], json!(true), "part {index} is stored: {reply}");
        }
        transfer
    }

    /// Uploads `container`, asks for its install and runs passes until the loop answers, and returns
    /// the one answer. The clock does not move.
    fn install(&self, runner: &mut TestRunner<'_>, container: &[u8], id: u64) -> Value {
        let transfer = self.upload(runner, container);
        self.ports.link.send(&json!({
            "id": id,
            "type": "program.install",
            "params": {"transfer": transfer},
        }));
        for _ in 0..=MAX_PASSES {
            runner.run_pass().expect("the test wait does not fail");
            let mut replies = self.ports.link.take_replies();
            if let Some(reply) = replies.pop() {
                assert!(replies.is_empty(), "one answer to the install: {replies:?}");
                return reply;
            }
        }
        panic!("the install was not answered within {MAX_PASSES} passes");
    }

    /// Runs the board probe for two scans and stops it, as `shutdown` does, at 20 ms on the clock.
    fn stop(&self, runner: &mut TestRunner<'_>) {
        self.ports.pass_at(runner, 0);
        self.ports.pass_at(runner, 10);
        let _ = self.ports.call_at(runner, 15, &request(1, "shutdown"));
        self.ports.pass_at(runner, 20);
        assert_eq!(runner.resource_state(), ResourceState::Stopped);
    }
}

/// The lowercase hexadecimal digits of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

fn restart(id: u64, mode: &str) -> Value {
    json!({"id": id, "type": "restart", "params": {"mode": mode}})
}

/// The number of the audit record that `reply` carries, from its id `audit-<n>`.
fn audit_number(reply: &Value) -> u32 {
    reply["audit_id"]
        .as_str()
        .and_then(|id| id.strip_prefix("audit-"))
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("an audit id in {reply}"))
}

/// A checkpoint whose `@program/Plant/activations` is `value`: the program fixture declares
/// `activations` as a retained `DINT` in its program `Plant`.
fn checkpoint_of(value: Retained) -> CheckpointProbe {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("@program/Plant/activations", value);
    CheckpointProbe::with_checkpoint(&snapshot)
}

/// The retained `activations` of the program `Plant`, in the program the loop runs now.
fn activations(runner: &TestRunner<'_>) -> Retained {
    let Program::Installed(cycle) = &runner.cycle else {
        panic!("a program is installed");
    };
    let storage = cycle.engine().state().storage();
    let Some(Retained::Instance(plant)) = storage.get_global("Plant") else {
        panic!("the installed program has the program instance Plant");
    };
    storage
        .get_instance_var(*plant, "activations")
        .cloned()
        .expect("Plant has activations")
}

/// The first scan of a program that has just started, as the scripted cycle records it.
fn first_scan() -> CycleCall {
    CycleCall::Scan {
        now: ms(0),
        deadline: None,
    }
}

#[test]
fn a_restart_after_an_install_runs_the_installed_program_without_a_reset() {
    // Specification 34 STATE-08 and §11.2 step 9: a `restart` of the Stopped resource that finds a
    // committed container newer than the one its program came from loads that container in place. The
    // board probe stops; the A4 program is installed as sequence 1 (the first commit into an empty
    // store); `restart` cold then starts it. Nothing resets: the same loop answers on the same link,
    // the transfer session goes on with the next id of its boot (3 << 32, then sequence 2), the audit
    // numbers go on, the program's events and metrics start empty, its event store has the record's
    // capacity, and the load writes no flash. Wrong behaviour caught: a restart that restarts the
    // probe in place and leaves the installed program for a reset; a new program built before the old
    // one is dropped; a loader called more than once; a platform, session or audit sequence that is
    // rebuilt; records that keep the old program's events; a load that erases or programs the store.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let mut runner = rig.runner(None);
    rig.stop(&mut runner);
    let installed = rig.install(&mut runner, &transfer_of(RetainMode::None), 40);
    assert_eq!(installed["result"]["status"], json!("installed"));
    assert_eq!(installed["result"]["sequence"], json!(1));
    let writes = rig.flash.writes();
    let _ = rig.ports.take_calls();

    let reply = rig.ports.call(&mut runner, &restart(50, "cold"));
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    let restart_audit = audit_number(&reply);
    assert_eq!(
        *rig.log.borrow(),
        [Load {
            sequence: 1,
            mode: RestartMode::Cold,
            program_bytes: PROGRAM.len(),
        }],
        "the loader built the installed container's program once, cold"
    );
    assert_eq!(rig.unloads.get(), 1, "the program that ended was dropped");
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(
        rig.ports.take_calls(),
        [CycleCall::TaskTiming(ms(0)), first_scan()],
        "the new program starts its task timing and scans at once; it is not restarted in place"
    );
    assert_eq!(
        rig.flash.writes(),
        writes,
        "the load reads the store and writes nothing"
    );

    let status = rig.ports.call(&mut runner, &request(51, "status"));
    assert_eq!(status["result"]["state"], json!("running"));
    assert_eq!(status["result"]["fault"], Value::Null);
    assert_eq!(
        status["result"]["event_capacity"],
        json!(CAPACITY),
        "the event store takes the capacity of the new record"
    );
    assert_eq!(
        audit_number(&status),
        restart_audit + 1,
        "the audit sequence of the device goes on"
    );
    let events = rig.ports.call(
        &mut runner,
        &json!({"id": 52, "type": "events.tail", "params": {"limit": 10}}),
    );
    assert_eq!(
        events["result"]["events"],
        json!([
            {"type": "cycle_end", "cycle": 0, "time_ns": 0},
            {"type": "cycle_start", "cycle": 0, "time_ns": 0},
        ]),
        "the events of the board probe are gone; the new program's first scan is all there is"
    );

    // The program that runs is the installed A4 program: its task `Periodic` has run by 60 ms.
    for at in [40, 50, 60] {
        rig.ports.pass_at(&mut runner, at);
    }
    let tasks = rig.ports.call(&mut runner, &request(53, "tasks.stats"));
    assert_eq!(tasks["result"]["tasks"][0]["name"], json!("Periodic"));

    // The transfer session of the boot goes on: the install used sequence 1 of boot 3.
    let begin = rig.ports.call(
        &mut runner,
        &json!({
            "id": 54,
            "type": "transfer.begin",
            "params": {"bytes": 5, "sha256": hex(&sha256(b"hello"))},
        }),
    );
    assert_eq!(begin["result"]["transfer"], json!("0000000300000002"));
}

#[test]
fn a_restart_without_a_newer_install_restarts_the_running_program() {
    // Specification 34 STATE-08: a `restart` that finds no committed container newer than the one its
    // program came from restarts the program in place, as before: the cycle is asked to restart with
    // the requested mode, the loader is not called, the program is not dropped, and the loop reads
    // nothing of the store but the two slot headers (60 bytes each) that tell it nothing is newer. The
    // store holds sequence 1 and the program came from it. Wrong behaviour caught: a reload on every
    // restart, which would drop and rebuild a program that nothing replaced; a check that reads the
    // whole container.
    for (mode, restart_mode) in [("cold", RestartMode::Cold), ("warm", RestartMode::Warm)] {
        let slot = |image: &[u8]| {
            let mut bytes = vec![0xff_u8; SLOT_BYTES as usize];
            bytes[..image.len()].copy_from_slice(image);
            bytes
        };
        let flash = FlashProbe::from_slots([
            slot(&committed_image(1, RetainMode::None)),
            vec![0xff_u8; SLOT_BYTES as usize],
        ]);
        let rig = Rig::new(flash, CheckpointProbe::erased());
        let mut runner = rig.runner(Some(1));
        rig.stop(&mut runner);
        let _ = rig.ports.take_calls();

        let reply = rig.ports.call_at(&mut runner, 27, &restart(2, mode));
        assert_eq!(
            reply["result"],
            json!({"status": "restart queued"}),
            "{mode}"
        );
        assert_eq!(
            rig.ports.take_calls(),
            [
                CycleCall::Restart {
                    mode: restart_mode,
                    source: RetainSource::Checkpoint,
                },
                CycleCall::TaskTiming(ms(0)),
                first_scan(),
            ],
            "{mode}: the program that stopped is restarted in place"
        );
        assert_eq!(*rig.log.borrow(), [], "{mode}: the loader was not called");
        assert_eq!(rig.unloads.get(), 0, "{mode}: the program was not dropped");
        assert_eq!(
            rig.flash.ops(),
            [
                FlashOp::Read {
                    slot: Slot::A,
                    offset: 0,
                    len: 60,
                },
                FlashOp::Read {
                    slot: Slot::B,
                    offset: 0,
                    len: 60,
                },
            ],
            "{mode}: only the two headers were read"
        );
        assert_eq!(runner.resource_state(), ResourceState::Running, "{mode}");
    }
}

#[test]
fn a_cold_and_a_warm_reload_start_with_the_requested_retained_values() {
    // Specification 34 STATE-08 and specification 11 §6.7: a warm reload gives the RETAIN variables
    // the values of the newest checkpoint, and a cold reload gives them their declared initial
    // values. The checkpoint holds `activations` as 7 (an INT, which the by-name rules migrate to the
    // DINT the program declares, and which the install's own check accepted); the container is the
    // A4 program with `mode = "file"`. Read right after the restart, before its first activation
    // (the 25 ms task of the program is not due at logical time 0): warm 7, cold 0. Wrong behaviour
    // caught: a warm reload that ignores the checkpoint; a cold reload that restores it; the two
    // modes swapped; a reload that starts every program warm.
    for (mode, expected) in [("warm", Retained::DInt(7)), ("cold", Retained::DInt(0))] {
        let rig = Rig::new(FlashProbe::erased(), checkpoint_of(Retained::Int(7)));
        let mut runner = rig.runner(None);
        rig.stop(&mut runner);
        let installed = rig.install(&mut runner, &transfer_of(RetainMode::File), 40);
        assert_eq!(installed["ok"], json!(true), "{mode}: {installed}");

        let reply = rig.ports.call(&mut runner, &restart(50, mode));
        assert_eq!(
            reply["result"],
            json!({"status": "restart queued"}),
            "{mode}"
        );
        assert_eq!(runner.resource_state(), ResourceState::Running, "{mode}");
        assert_eq!(activations(&runner), expected, "{mode}");
        let asked = if mode == "warm" {
            RestartMode::Warm
        } else {
            RestartMode::Cold
        };
        assert_eq!(rig.log.borrow()[0].mode, asked, "{mode}");
    }
}

#[test]
fn a_program_loaded_in_place_carries_its_generation_in_its_live_values() {
    // Specification 34 STATE-08, UPDATE-04 and §4.3.1: a request that names an instance by a bare
    // identity belongs to the generation that the Live Values carry. The loader builds the Live Values
    // of the program it loads, in generation 0, and a warm start of a program that keeps retained
    // values restarts the program's state over its checkpoint, which installs generation 1; a cold
    // start installs none. The loop that takes the program gives its Live Values the generation of its
    // state. Wrong behaviour caught: Live Values of a program loaded in place that stay in generation
    // 0 while the program is in generation 1, which refuse every force by instance identity after a
    // warm reload; Live Values that are moved on after a cold reload that moved the program on.
    for (mode, expected) in [("warm", Generation(1)), ("cold", Generation(0))] {
        let rig = Rig::new(FlashProbe::erased(), checkpoint_of(Retained::Int(7)));
        let mut runner = rig.runner(None);
        rig.stop(&mut runner);
        let installed = rig.install(&mut runner, &transfer_of(RetainMode::File), 40);
        assert_eq!(installed["ok"], json!(true), "{mode}: {installed}");

        let reply = rig.ports.call(&mut runner, &restart(50, mode));
        assert_eq!(
            reply["result"],
            json!({"status": "restart queued"}),
            "{mode}"
        );
        let Program::Installed(cycle) = &runner.cycle else {
            panic!("{mode}: a program is installed");
        };
        assert_eq!(cycle.engine().generation(), expected, "{mode}: the program");
        assert_eq!(
            runner.live.generation(),
            expected,
            "{mode}: the Live Values"
        );
    }
}

#[test]
fn a_reload_that_cannot_start_leaves_the_resource_faulted_and_the_next_restart_tries_again() {
    // Specification 34 STATE-08: a program that cannot start leaves the resource Faulted with
    // `no installed program: <reason>`, the old program being gone, as on a computer; the next
    // `restart` loads the newest container again. The first load fails with a reason, the second
    // succeeds. Wrong behaviour caught: a failed load that leaves the old program running; a
    // resource that is Stopped, not Faulted, or has no fault text; a next restart that is refused,
    // or restarts nothing, or does not call the loader again; a fault that survives a start.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let mut runner = rig.runner(None);
    rig.stop(&mut runner);
    let installed = rig.install(&mut runner, &transfer_of(RetainMode::None), 40);
    assert_eq!(installed["ok"], json!(true), "{installed}");
    rig.failures
        .borrow_mut()
        .push_back("the installed application cannot be prepared: not enough memory".to_string());

    let reply = rig.ports.call(&mut runner, &restart(50, "cold"));
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        runner.last_error(),
        Some(RuntimeError::ControlError(SmolStr::new(
            "no installed program: the installed application cannot be prepared: not enough memory"
        )))
    );
    let status = rig.ports.call(&mut runner, &request(51, "status"));
    assert_eq!(status["result"]["state"], json!("faulted"));
    assert_eq!(
        status["result"]["fault"],
        json!("control error 'no installed program: the installed application cannot be prepared: not enough memory'")
    );
    assert_eq!(rig.unloads.get(), 1, "the old program is gone");

    let reply = rig.ports.call(&mut runner, &restart(52, "cold"));
    assert_eq!(
        reply["result"],
        json!({"status": "restart queued"}),
        "a faulted resource takes the next restart"
    );
    assert_eq!(rig.log.borrow().len(), 2, "the loader was called again");
    assert_eq!(rig.log.borrow()[1].sequence, 1);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
}

#[test]
fn a_reload_of_a_damaged_container_faults_with_the_reason_the_boot_gives() {
    // Specification 34 §5.1 and STATE-08 (decision Q12): the newest committed container fails its
    // payload check, so the load is refused with the reason a boot gives, `the CRC-32 of entry kind 1
    // does not match`, and the other slot is not started in its place; the loader is never called.
    // Wrong behaviour caught: a damaged container that is built anyway; one that falls back to the
    // older slot; a refusal without its reason.
    let mut damaged = committed_image(3, RetainMode::None);
    damaged[70] ^= 0x01;
    let slot = |image: &[u8]| {
        let mut bytes = vec![0xff_u8; SLOT_BYTES as usize];
        bytes[..image.len()].copy_from_slice(image);
        bytes
    };
    let flash =
        FlashProbe::from_slots([slot(&damaged), slot(&committed_image(2, RetainMode::None))]);
    let rig = Rig::new(flash, CheckpointProbe::erased());
    let mut runner = rig.runner(Some(2));
    rig.stop(&mut runner);

    let reply = rig.ports.call(&mut runner, &restart(50, "warm"));
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        runner.last_error(),
        Some(RuntimeError::ControlError(SmolStr::new(
            "no installed program: the CRC-32 of entry kind 1 does not match"
        )))
    );
    assert_eq!(*rig.log.borrow(), [], "no program was built");
}

#[test]
fn a_chip_without_a_program_starts_an_installed_one_by_restart() {
    // Specification 34 STATE-08: a chip that booted without a program refuses `restart` with the
    // reason until the store holds a container newer than the one the boot refused (here none), and
    // starts the installed program by `restart` once it does. Wrong behaviour caught: a restart
    // accepted with nothing installed; a restart still refused after an install, which would leave the
    // program for a reset.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let mut runner = rig.stopped_runner(ERASED, None);

    let refused = rig.ports.call(&mut runner, &restart(1, "cold"));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["error_code"], json!("resource_not_running"));
    assert_eq!(
        refused["error"],
        json!("no installed program: no TRSTA002 container in either slot")
    );
    assert_eq!(*rig.log.borrow(), []);

    let installed = rig.install(&mut runner, &transfer_of(RetainMode::None), 40);
    assert_eq!(installed["ok"], json!(true), "{installed}");
    let started = rig.ports.call(&mut runner, &restart(2, "cold"));
    assert_eq!(started["result"], json!({"status": "restart queued"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(rig.log.borrow().len(), 1);
    assert_eq!(rig.log.borrow()[0].sequence, 1);
}

#[test]
fn a_boot_refusal_stays_until_a_newer_container_is_installed() {
    // Specification 34 STATE-08 and §5.1: the container that the boot refused (sequence 3, damaged)
    // is not newer than itself, so `restart` stays refused with the boot's reason, and the loop reads
    // only the two headers to know it. After an install, which takes sequence 4 in the other slot,
    // the store holds a newer container and `restart` loads it. Wrong behaviour caught: a refusal
    // that no install can lift; a restart accepted while the refused container is the newest.
    let mut damaged = committed_image(3, RetainMode::None);
    damaged[70] ^= 0x01;
    let mut slot_a = vec![0xff_u8; SLOT_BYTES as usize];
    slot_a[..damaged.len()].copy_from_slice(&damaged);
    let flash = FlashProbe::from_slots([slot_a, vec![0xff_u8; SLOT_BYTES as usize]]);
    let rig = Rig::new(flash, CheckpointProbe::erased());
    let mut runner = rig.stopped_runner(DAMAGED, Some(3));

    let refused = rig.ports.call(&mut runner, &restart(1, "warm"));
    assert_eq!(refused["error_code"], json!("resource_not_running"));
    assert_eq!(
        refused["error"],
        json!("no installed program: the CRC-32 of entry kind 1 does not match")
    );
    assert_eq!(
        rig.flash.ops(),
        [
            FlashOp::Read {
                slot: Slot::A,
                offset: 0,
                len: 60,
            },
            FlashOp::Read {
                slot: Slot::B,
                offset: 0,
                len: 60,
            },
        ],
        "the loop read the two headers to see that nothing is newer"
    );

    let installed = rig.install(&mut runner, &transfer_of(RetainMode::None), 40);
    assert_eq!(installed["result"]["sequence"], json!(4));
    let started = rig.ports.call(&mut runner, &restart(2, "warm"));
    assert_eq!(started["result"], json!({"status": "restart queued"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(rig.log.borrow()[0].sequence, 4);
}

/// A slot of the fake program flash that holds `image` and is erased after it.
fn slot_with(image: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xff_u8; SLOT_BYTES as usize];
    bytes[..image.len()].copy_from_slice(image);
    bytes
}

#[test]
fn a_boot_starts_the_newest_committed_program_through_the_loop() {
    // Specification 34 STATE-08 and Appendix D.2: a chip builds its loop Stopped before any program
    // and then boots the newest committed container through it, by the steps of a `restart` that
    // finds a newer container, in the warm start of a power-on. The store holds sequence 2 in slot A;
    // the boot builds it once, warm, and the resource runs it from time zero: its task timing starts
    // at 0 ms and the first pass scans at 0 ms. A stop and a `restart` then restart that program in
    // place, because it came from the newest container. Wrong behaviour caught: a boot that leaves
    // the resource Stopped with a program committed; a boot that builds another container, or builds
    // twice; a program counted as coming from no container, which the next `restart` would load again.
    let flash = FlashProbe::from_slots([
        slot_with(&committed_image(2, RetainMode::None)),
        vec![0xff_u8; SLOT_BYTES as usize],
    ]);
    let rig = Rig::new(flash, CheckpointProbe::erased());
    let mut runner = rig.stopped_runner(BOOTING, None);

    runner.boot(RestartMode::Warm);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
    assert_eq!(
        *rig.log.borrow(),
        [Load {
            sequence: 2,
            mode: RestartMode::Warm,
            program_bytes: PROGRAM.len(),
        }],
        "the boot built the newest container's program once, warm"
    );
    assert_eq!(rig.ports.take_calls(), [CycleCall::TaskTiming(ms(0))]);
    rig.ports.pass_at(&mut runner, 0);
    assert_eq!(rig.ports.take_calls(), [first_scan()]);
    let status = rig.ports.call(&mut runner, &request(1, "status"));
    assert_eq!(status["result"]["state"], json!("running"));
    assert_eq!(status["result"]["event_capacity"], json!(CAPACITY));

    rig.stop(&mut runner);
    let reply = rig.ports.call(&mut runner, &restart(50, "cold"));
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(
        rig.log.borrow().len(),
        1,
        "the program came from sequence 2, so it restarts in place"
    );
}

#[test]
fn a_boot_whose_program_cannot_start_leaves_the_chip_stopped_with_the_reason() {
    // Specification 34 STATE-08 and design §1.1 (R2): a program that cannot start at the boot leaves
    // the resource Stopped, not Faulted, with the reason, and the loop counts its container
    // (sequence 2) as the one the boot refused: `restart` is refused with `resource_not_running` and
    // `no installed program: <reason>` and builds nothing, until an install commits sequence 3 in
    // the other slot, which `restart` then loads. Wrong behaviour caught: a boot that faults the
    // resource, which would refuse the install that repairs the chip (§11.2); a `restart` that
    // builds the refused container again on every request; a refusal that no install can lift.
    let flash = FlashProbe::from_slots([
        slot_with(&committed_image(2, RetainMode::None)),
        vec![0xff_u8; SLOT_BYTES as usize],
    ]);
    let rig = Rig::new(flash, CheckpointProbe::erased());
    rig.failures
        .borrow_mut()
        .push_back("the installed application cannot be prepared: not enough memory".to_string());
    let mut runner = rig.stopped_runner(BOOTING, None);

    runner.boot(RestartMode::Warm);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    assert_eq!(runner.last_error(), None);
    let status = rig.ports.call(&mut runner, &request(1, "status"));
    assert_eq!(status["result"]["state"], json!("stopped"));
    assert_eq!(status["result"]["fault"], Value::Null);
    let refused = rig.ports.call(&mut runner, &restart(2, "warm"));
    assert_eq!(refused["error_code"], json!("resource_not_running"));
    assert_eq!(
        refused["error"],
        json!(
            "no installed program: the installed application cannot be prepared: not enough memory"
        )
    );
    assert_eq!(
        rig.log.borrow().len(),
        1,
        "the refused container is not built again"
    );

    let installed = rig.install(&mut runner, &transfer_of(RetainMode::None), 40);
    assert_eq!(installed["result"]["sequence"], json!(3));
    let started = rig.ports.call(&mut runner, &restart(3, "warm"));
    assert_eq!(started["result"], json!({"status": "restart queued"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(rig.log.borrow().len(), 2);
    assert_eq!(rig.log.borrow()[1].sequence, 3);
}

#[test]
fn a_boot_with_no_committed_container_stays_stopped_with_the_stores_reason() {
    // Specification 34 §5.1 and STATE-08: a boot over an erased store builds nothing and leaves the
    // resource Stopped with the reason the store's selection gives, `no TRSTA002 container in either
    // slot`, which `restart` names; an install then commits sequence 1, which `restart` loads. Wrong
    // behaviour caught: a boot that replaces the store's reason with its own; a loader called with
    // nothing to build; a chip that an install cannot start without a reset.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let mut runner = rig.stopped_runner(BOOTING, None);

    runner.boot(RestartMode::Warm);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let refused = rig.ports.call(&mut runner, &restart(1, "cold"));
    assert_eq!(refused["error_code"], json!("resource_not_running"));
    assert_eq!(
        refused["error"],
        json!("no installed program: no TRSTA002 container in either slot")
    );
    assert_eq!(*rig.log.borrow(), [], "nothing was built");

    let installed = rig.install(&mut runner, &transfer_of(RetainMode::None), 40);
    assert_eq!(installed["result"]["sequence"], json!(1));
    let started = rig.ports.call(&mut runner, &restart(2, "cold"));
    assert_eq!(started["result"], json!({"status": "restart queued"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(rig.log.borrow()[0].sequence, 1);
}

#[test]
fn program_active_reports_both_containers_in_bounded_reads() {
    // Specification 34 §11.2 (`program.active`): the selected program is the committed container with
    // the highest install sequence and `previous` is the other; each is reported as
    // `{sequence, program_sha256, bytes}`, the SHA-256 as its record names it and the length of the
    // program (6,412 bytes, the fixture's size). A chip reads each container's header and record only,
    // in reads of at most the step budget (256 bytes on the C6), and writes nothing: the two
    // containers hold 6,412 program bytes each, and the loop reads far fewer bytes than one program
    // in all. Slot A holds sequence 2 and slot B sequence 5. Wrong behaviour caught: a report that
    // reads a whole container into memory (a read of more than 256 bytes, or of the program); an
    // active container that is the first slot, not the highest sequence; a `previous` that repeats the
    // active container; a SHA-256 or a length taken from the wrong entry.
    let slot = |image: &[u8]| {
        let mut bytes = vec![0xff_u8; SLOT_BYTES as usize];
        bytes[..image.len()].copy_from_slice(image);
        bytes
    };
    let flash = FlashProbe::from_slots([
        slot(&committed_image(2, RetainMode::None)),
        slot(&committed_image(5, RetainMode::File)),
    ]);
    let rig = Rig::new(flash, CheckpointProbe::erased());
    let mut runner = rig.runner(Some(5));
    rig.ports.pass_at(&mut runner, 0);

    let reply = rig.ports.call(&mut runner, &request(1, "program.active"));
    let identity = |sequence: u64| {
        json!({
            "sequence": sequence,
            "program_sha256": hex(&sha256(PROGRAM)),
            "bytes": 6_412,
        })
    };
    assert_eq!(PROGRAM.len(), 6_412, "the fixture program");
    assert_eq!(
        reply["result"],
        json!({"active": identity(5), "previous": identity(2)})
    );

    let mut read_bytes = 0;
    for operation in rig.flash.ops() {
        let FlashOp::Read { len, .. } = operation else {
            panic!("program.active writes nothing: {operation:?}");
        };
        assert!(len <= 256, "a read of {len} bytes is above the step budget");
        read_bytes += len;
    }
    assert!(
        read_bytes < 6_412,
        "{read_bytes} bytes were read: no more than the headers and the records"
    );
}
