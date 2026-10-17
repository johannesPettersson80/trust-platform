//! The engine cycle of a device (specification 34, STATE-01 to STATE-08, CYCLE-01, CYCLE-02 and
//! §5.2): the process I/O at the engine's phases, the deadline fault, the safe outputs of a stop
//! and a fault, cold and warm restarts (also after a stop), the task timing, the execution
//! services and the device's admission: memory limits from its region and heap, and
//! trust-runtime's work limits. With the board probe (`gpio.stbc`) and the A4 program on the
//! real engine.

use alloc::vec;
use alloc::vec::Vec;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::bytecode::ValidationLimits;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::ports::MonotonicInstant;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{self, Value};
use trust_runtime_core::vm::{
    CyclePhase, ExecutionServices, PreparationLimits, PreparedModule, VM_MAX_CALL_DEPTH,
};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

use super::{device_limits, CycleSettings, DeviceMemory, DeviceServices, EngineCycle};
use crate::runner::program::Program;
use crate::runner::{ResourceCycle, RetainSource, TaskDescriptor};
use crate::status::{IoDriverHealth, IoDriverStatus};
use crate::tests::runner_ports::{
    ms, Fixture, IoStep, TestClock, TestIo, TestRunner, CHIP_MEMORY, GPIO,
};

/// The A4 program: a 25 ms task over a counter, a TON and the retained `activations`.
const A4_PROGRAM: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

/// A program that reads `CURRENT_DT`.
const CLOCK: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/clock-v2.stbc");

/// One program on the engine cycle, outside the resource loop: the test runs its scans.
struct Probe {
    clock: TestClock,
    io: TestIo,
    services: DeviceServices<TestClock>,
    prepared: PreparedModule,
}

impl Probe {
    /// `artifact` admitted as today's chips admit it.
    fn new(artifact: &[u8]) -> Self {
        let clock = TestClock::default();
        Self {
            io: TestIo::new(clock.clone()),
            services: DeviceServices::new(clock.clone()),
            prepared: PreparedModule::from_bytes(artifact, device_limits(CHIP_MEMORY))
                .expect("the program is admitted"),
            clock,
        }
    }

    /// The program started with `settings` and its task timing started at time zero, as the
    /// resource loop starts it.
    fn start(&self, settings: CycleSettings) -> EngineCycle<'_, TestClock, TestIo> {
        let mut cycle =
            EngineCycle::start(&self.prepared, &self.services, self.io.clone(), settings)
                .expect("the program starts");
        cycle.reset_task_timing(value::Duration::ZERO);
        cycle
    }

    /// One scan at `at` milliseconds, logical and on the clock, with the deadline at `deadline`
    /// milliseconds; the engine phases are recorded among the I/O steps.
    fn scan(
        &self,
        cycle: &mut EngineCycle<'_, TestClock, TestIo>,
        at: u64,
        deadline: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.clock.set(ms(at));
        let deadline = deadline.map(|deadline| MonotonicInstant::from_origin(ms(deadline)));
        cycle.run_scan(logical(at), deadline, &mut self.io.phases())
    }

    /// Runs a scan at each of `times` and returns the times at which the task ran.
    fn activations(
        &self,
        cycle: &mut EngineCycle<'_, TestClock, TestIo>,
        times: &[u64],
    ) -> Vec<u64> {
        let mut ran = Vec::new();
        for &at in times {
            self.scan(cycle, at, None).expect("the scan runs");
            if self
                .io
                .take_steps()
                .contains(&IoStep::Phase(CyclePhase::TaskStart(0)))
            {
                ran.push(at);
            }
        }
        ran
    }
}

/// Logical time `at` milliseconds.
fn logical(at: u64) -> value::Duration {
    value::Duration::from_millis(i64::try_from(at).expect("test times are small"))
}

/// The settings of the board probe's resource: 10 ms, no cycle watchdog, faults halt.
fn settings() -> CycleSettings {
    CycleSettings {
        cycle_interval: value::Duration::from_millis(10),
        watchdog: WatchdogPolicy::default(),
        fault_policy: FaultPolicy::Halt,
        safe_outputs: Vec::new(),
    }
}

/// `[[io.safe_state]]` with the board probe's output `led` off.
fn led_off() -> Vec<(IoAddress, Value)> {
    vec![(
        IoAddress::parse("%QX0.0").expect("an output address"),
        Value::Bool(false),
    )]
}

/// The steps of a scan in which no task is due, ending with the output image `outputs`.
fn scan_without_task(outputs: u8) -> [IoStep; 8] {
    [
        IoStep::Phase(CyclePhase::CycleStart),
        IoStep::Phase(CyclePhase::BeforeInputSample),
        IoStep::ReadInputs,
        IoStep::Phase(CyclePhase::AfterInputSample),
        IoStep::Phase(CyclePhase::BeforeOutputSample),
        IoStep::Phase(CyclePhase::AfterOutputSample),
        IoStep::WriteOutputs(vec![outputs]),
        IoStep::Phase(CyclePhase::CycleEnd),
    ]
}

/// The `Plant` program's variable `name` in the A4 program.
fn plant_value(cycle: &EngineCycle<'_, TestClock, TestIo>, name: &str) -> Value {
    let storage = cycle.state.storage();
    let Some(Value::Instance(plant)) = storage.get_global("Plant") else {
        panic!("the A4 program has the program instance Plant");
    };
    storage
        .get_instance_var(*plant, name)
        .cloned()
        .unwrap_or_else(|| panic!("Plant has {name}"))
}

#[test]
fn the_inputs_are_read_before_the_program_and_the_outputs_driven_where_the_engine_publishes() {
    let probe = Probe::new(GPIO);
    let mut cycle = probe.start(settings());
    probe.io.state.borrow_mut().inputs = vec![1];
    probe.scan(&mut cycle, 0, None).expect("the first scan");
    assert_eq!(
        probe.io.take_steps(),
        scan_without_task(0),
        "the first scan has no activation due, so `led` keeps its initial value"
    );
    probe
        .scan(&mut cycle, 10, None)
        .expect("the scan of the task");
    assert_eq!(
        probe.io.take_steps(),
        [
            IoStep::Phase(CyclePhase::CycleStart),
            IoStep::Phase(CyclePhase::BeforeInputSample),
            IoStep::ReadInputs,
            IoStep::Phase(CyclePhase::AfterInputSample),
            IoStep::Phase(CyclePhase::TaskStart(0)),
            IoStep::Phase(CyclePhase::TaskEnd(0)),
            IoStep::Phase(CyclePhase::BeforeOutputSample),
            IoStep::Phase(CyclePhase::AfterOutputSample),
            IoStep::WriteOutputs(vec![1]),
            IoStep::Phase(CyclePhase::CycleEnd),
        ],
        "the program saw the input read in its own scan, and the drivers got its output after \
         it ran, before the end of the scan (CYCLE-01, CYCLE-02)"
    );
    assert_eq!(cycle.state.outputs(), &[1]);
}

#[test]
fn an_input_that_cannot_be_read_fails_the_scan_before_the_program_runs() {
    let probe = Probe::new(GPIO);
    let mut cycle = probe.start(settings());
    let unreadable = RuntimeError::IoDriver(SmolStr::new("gpio line GPIO9 unreadable"));
    probe.io.state.borrow_mut().read_error = Some(unreadable.clone());
    assert_eq!(probe.scan(&mut cycle, 0, None), Err(unreadable));
    assert_eq!(
        probe.io.take_steps(),
        [
            IoStep::Phase(CyclePhase::CycleStart),
            IoStep::Phase(CyclePhase::BeforeInputSample),
            IoStep::ReadInputs,
        ],
        "no value is put in place of the input (spec 34 §7.2), and no output is driven"
    );
    assert_eq!(
        probe.scan(&mut cycle, 10, None),
        Err(RuntimeError::ResourceFaulted),
        "the engine latched the failure: no scan runs until a restart"
    );
    assert_eq!(probe.io.take_steps(), [] as [IoStep; 0]);
}

#[test]
fn an_output_that_cannot_be_driven_fails_the_scan_and_the_engine_keeps_the_last_image() {
    let probe = Probe::new(GPIO);
    let mut cycle = probe.start(settings());
    probe.scan(&mut cycle, 0, None).expect("the first scan");
    let _ = probe.io.take_steps();
    let offline = RuntimeError::IoDriver(SmolStr::new("gpio line GPIO18 offline"));
    {
        let mut io = probe.io.state.borrow_mut();
        io.inputs = vec![1];
        io.write_error = Some(offline.clone());
    }
    assert_eq!(probe.scan(&mut cycle, 10, None), Err(offline));
    assert_eq!(
        probe.io.take_steps().last(),
        Some(&IoStep::WriteOutputs(vec![1])),
        "the drivers were given the new image"
    );
    assert_eq!(
        cycle.state.outputs(),
        &[0],
        "the engine restored the output image it staged, so the failed scan published nothing"
    );
}

#[test]
fn a_scan_past_its_deadline_faults_with_execution_timeout_and_drives_no_output() {
    let probe = Probe::new(GPIO);
    let mut cycle = probe.start(settings());
    probe.scan(&mut cycle, 0, None).expect("the first scan");
    let _ = probe.io.take_steps();
    {
        let mut io = probe.io.state.borrow_mut();
        io.inputs = vec![1];
        io.read_time = ms(6);
    }
    assert_eq!(
        probe.scan(&mut cycle, 10, Some(15)),
        Err(RuntimeError::ExecutionTimeout),
        "the slow input read ends at 16 ms, after the scan's deadline"
    );
    let steps = probe.io.take_steps();
    assert!(steps.contains(&IoStep::ReadInputs));
    assert!(
        !steps
            .iter()
            .any(|step| matches!(step, IoStep::WriteOutputs(_))),
        "the engine checks the deadline before it publishes: {steps:?}"
    );
    assert_eq!(
        cycle.state.fault(),
        Some(&RuntimeError::ExecutionTimeout),
        "the engine latched the deadline fault"
    );
    assert_eq!(cycle.state.outputs(), &[0]);
    assert!(
        !probe.services.deadline_exceeded(),
        "the deadline is armed for its scan only"
    );
    assert_eq!(
        probe.scan(&mut cycle, 20, None),
        Err(RuntimeError::ResourceFaulted)
    );
}

#[test]
fn the_safe_outputs_replace_the_output_image_and_every_driver_drives_them() {
    let probe = Probe::new(GPIO);
    let mut cycle = probe.start(CycleSettings {
        safe_outputs: led_off(),
        ..settings()
    });
    probe.io.state.borrow_mut().inputs = vec![1];
    probe.scan(&mut cycle, 0, None).expect("the first scan");
    probe
        .scan(&mut cycle, 10, None)
        .expect("the scan of the task");
    assert_eq!(cycle.state.outputs(), &[1], "`led` follows the button");
    let _ = probe.io.take_steps();

    cycle
        .apply_safe_outputs()
        .expect("the safe outputs are driven");
    assert_eq!(probe.io.take_steps(), [IoStep::WriteSafeOutputs(vec![0])]);
    assert_eq!(
        cycle.state.outputs(),
        &[0],
        "the engine's output image holds the safe values, as on trust-runtime"
    );

    let shorted = RuntimeError::IoDriver(SmolStr::new("gpio line GPIO18 shorted"));
    probe.io.state.borrow_mut().safe_error = Some(shorted.clone());
    assert_eq!(
        cycle.apply_safe_outputs(),
        Err(shorted),
        "a driver's failure is reported (STATE-04)"
    );
    assert_eq!(probe.io.take_steps(), [IoStep::WriteSafeOutputs(vec![0])]);

    let probe = Probe::new(GPIO);
    let mut cycle = probe.start(settings());
    cycle
        .apply_safe_outputs()
        .expect("nothing to drive without a safe state");
    assert_eq!(
        probe.io.take_steps(),
        [] as [IoStep; 0],
        "without `[[io.safe_state]]` no output is driven, as on trust-runtime"
    );
}

#[test]
fn a_stop_and_a_fault_drive_the_safe_outputs_through_the_process_io() {
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().safe_outputs = led_off();
    fixture.io.state.borrow_mut().inputs = vec![1];
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        fixture.io.take_steps().last(),
        Some(&IoStep::WriteOutputs(vec![1]))
    );
    let _ = fixture.call_at(&mut runner, 12, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    assert_eq!(
        fixture.io.take_steps(),
        [IoStep::WriteSafeOutputs(vec![0])],
        "the stop drives the safe outputs instead of a scan (STATE-02)"
    );

    let fixture = Fixture::board_probe();
    {
        let mut script = fixture.script.borrow_mut();
        script.safe_outputs = led_off();
        script.fault_policy = FaultPolicy::SafeHalt;
    }
    fixture.io.state.borrow_mut().inputs = vec![1];
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    let _ = fixture.io.take_steps();
    let unreadable = RuntimeError::IoDriver(SmolStr::new("gpio line GPIO9 unreadable"));
    fixture.io.state.borrow_mut().read_error = Some(unreadable.clone());
    fixture.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(runner.last_error(), Some(unreadable));
    assert_eq!(
        fixture.io.take_steps(),
        [IoStep::ReadInputs, IoStep::WriteSafeOutputs(vec![0])],
        "fault policy safe_halt: the failed scan drove nothing, then the fault drove the safe \
         outputs"
    );
}

#[test]
fn warm_restarts_keep_the_retained_values_and_a_cold_restart_starts_them_anew() {
    let probe = Probe::new(A4_PROGRAM);
    let mut cycle = probe.start(settings());
    assert_eq!(
        probe.activations(&mut cycle, &[0, 10, 20, 30, 40, 50]),
        [30, 50],
        "the 25 ms task, sampled every 10 ms"
    );
    assert_eq!(plant_value(&cycle, "activations"), Value::DInt(2));
    assert_eq!(plant_value(&cycle, "result"), Value::Int(2));

    for source in [RetainSource::Checkpoint, RetainSource::Memory] {
        assert_eq!(
            cycle.restart(RestartMode::Warm, source),
            Ok(Vec::new()),
            "{source:?}: a device without a retain store has an empty checkpoint"
        );
        assert_eq!(
            plant_value(&cycle, "activations"),
            Value::DInt(2),
            "{source:?}: a warm restart keeps the RETAIN variable"
        );
        assert_eq!(
            plant_value(&cycle, "result"),
            Value::Int(0),
            "{source:?}: and starts the others with their initial values"
        );
    }

    assert_eq!(
        cycle.restart(RestartMode::Cold, RetainSource::Checkpoint),
        Ok(Vec::new())
    );
    assert_eq!(
        plant_value(&cycle, "activations"),
        Value::DInt(0),
        "a cold restart starts every variable with its initial value"
    );
    cycle.reset_task_timing(logical(50));
    assert_eq!(probe.activations(&mut cycle, &[60, 70, 80]), [80]);
    assert_eq!(plant_value(&cycle, "activations"), Value::DInt(1));
}

#[test]
fn a_restart_clears_the_latched_fault_and_the_program_scans_again() {
    for mode in [RestartMode::Cold, RestartMode::Warm] {
        let probe = Probe::new(GPIO);
        let mut cycle = probe.start(settings());
        probe.scan(&mut cycle, 0, None).expect("the first scan");
        cycle.latch_fault(RuntimeError::DivisionByZero);
        assert_eq!(
            probe.scan(&mut cycle, 10, None),
            Err(RuntimeError::ResourceFaulted),
            "{mode:?}"
        );
        cycle
            .restart(mode, RetainSource::Checkpoint)
            .expect("the program restarts");
        cycle.reset_task_timing(logical(10));
        let _ = probe.io.take_steps();
        probe
            .scan(&mut cycle, 20, None)
            .expect("the restarted program scans");
        assert!(
            probe
                .io
                .take_steps()
                .contains(&IoStep::Phase(CyclePhase::TaskStart(0))),
            "{mode:?}: its task is due one interval after the restart"
        );
    }
}

#[test]
fn the_task_timing_counts_the_next_activation_from_where_it_starts() {
    let probe = Probe::new(GPIO);
    let mut cycle = probe.start(settings());
    assert_eq!(
        cycle.tasks(),
        [TaskDescriptor {
            name: SmolStr::new("Scan"),
            priority: 0,
        }]
    );
    assert_eq!(cycle.cycle_interval(), value::Duration::from_millis(10));
    assert_eq!(cycle.watchdog(), WatchdogPolicy::default());
    assert_eq!(cycle.fault_policy(), FaultPolicy::Halt);
    assert_eq!(
        probe.activations(&mut cycle, &[0, 5, 10, 15, 20]),
        [10, 20],
        "the task first runs one interval after the start of its timing"
    );
    cycle.reset_task_timing(logical(25));
    assert_eq!(
        probe.activations(&mut cycle, &[30, 35, 40, 45]),
        [35, 45],
        "after a restart of the timing at 25 ms the task runs at 35 ms and every 10 ms after"
    );
}

#[test]
fn the_drivers_report_their_health_in_status_and_health() {
    let fixture = Fixture::board_probe();
    fixture.io.state.borrow_mut().health = vec![IoDriverStatus {
        name: SmolStr::new("gpio"),
        health: IoDriverHealth::Faulted {
            error: SmolStr::new("line GPIO18 shorted"),
        },
    }];
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let status = fixture.call(&mut runner, &json!({"id": 1, "type": "status"}));
    assert_eq!(
        status["result"]["io_drivers"],
        json!([{"name": "gpio", "status": "faulted", "error": "line GPIO18 shorted"}])
    );
    let health = fixture.call(&mut runner, &json!({"id": 2, "type": "health"}));
    assert_eq!(
        health["result"]["ok"],
        json!(false),
        "a faulted driver makes the device unhealthy, as on trust-runtime"
    );
}

#[test]
fn the_device_services_arm_the_deadline_on_the_monotonic_clock_and_have_no_wall_clock() {
    let clock = TestClock::default();
    let services = DeviceServices::new(clock.clone());
    assert!(!services.deadline_exceeded(), "no deadline is armed");
    services.arm(Some(MonotonicInstant::from_origin(ms(10))));
    clock.set(ms(9));
    assert!(!services.deadline_exceeded());
    clock.set(ms(10));
    assert!(
        services.deadline_exceeded(),
        "the deadline is reached at its instant"
    );
    services.arm(None);
    assert!(!services.deadline_exceeded());
    assert!(!services.has_wall_clock());
}

#[test]
fn a_program_that_calls_current_dt_is_refused_when_the_device_constructs_it() {
    let probe = Probe::new(CLOCK);
    let started = EngineCycle::start(
        &probe.prepared,
        &probe.services,
        probe.io.clone(),
        settings(),
    );
    assert_eq!(
        started.err(),
        Some(RuntimeError::ProfileUnsupported(SmolStr::new(
            "CURRENT_DT requires an admitted platform wall clock"
        ))),
        "a device has no wall clock until slice T6 (spec 34 §5.2)"
    );
}

#[test]
fn the_device_memory_limits_follow_the_application_region_and_the_heap() {
    let host = PreparationLimits::default();
    let bring_up = device_limits(CHIP_MEMORY);
    assert_eq!(
        bring_up,
        PreparationLimits {
            max_artifact_bytes: 16 * 1024,
            max_preparation_bytes: 512 * 1024,
            validation: ValidationLimits {
                max_scratch_bytes: 48 * 1024,
                ..host.validation
            },
            max_construction_values: 4096,
            max_construction_bytes: 128 * 1024,
            max_process_image_bytes: 64,
            max_call_depth: 4,
            ..host
        },
        "at the 72 KiB heap of the bring-up runs, the memory limits both chips ran the A4 \
         programs with"
    );
    assert_eq!(
        device_limits(DeviceMemory {
            region_bytes: 32 * 1024,
            heap_bytes: 144 * 1024,
        }),
        PreparationLimits {
            max_artifact_bytes: 32 * 1024,
            max_preparation_bytes: 1024 * 1024,
            validation: ValidationLimits {
                max_scratch_bytes: 96 * 1024,
                ..bring_up.validation
            },
            max_construction_values: 8192,
            max_construction_bytes: 256 * 1024,
            max_process_image_bytes: 128,
            max_call_depth: 8,
            ..bring_up
        },
        "the memory limits and the call depth grow with the heap"
    );
    let large = device_limits(DeviceMemory {
        region_bytes: 1024 * 1024,
        heap_bytes: 64 * 1024 * 1024,
    });
    assert_eq!(
        large.max_call_depth, VM_MAX_CALL_DEPTH,
        "the call depth is never more than trust-runtime's"
    );

    let fits = DeviceMemory {
        region_bytes: GPIO.len(),
        ..CHIP_MEMORY
    };
    assert!(PreparedModule::from_bytes(GPIO, device_limits(fits)).is_ok());
    let short = DeviceMemory {
        region_bytes: GPIO.len() - 1,
        ..CHIP_MEMORY
    };
    assert_eq!(
        PreparedModule::from_bytes(GPIO, device_limits(short)).err(),
        Some(RuntimeError::PreparationLimit),
        "a program longer than the region is refused"
    );
}

#[test]
fn a_devices_work_limits_are_trust_runtimes() {
    let host = PreparationLimits::default();
    for memory in [
        CHIP_MEMORY,
        DeviceMemory {
            region_bytes: 256 * 1024,
            heap_bytes: 145 * 1024,
        },
    ] {
        let device = device_limits(memory);
        assert_eq!(
            (
                device.max_work,
                device.max_construction_work,
                device.max_preparation_work,
                device.validation.max_work,
            ),
            (
                host.max_work,
                host.max_construction_work,
                host.max_preparation_work,
                host.validation.max_work,
            ),
            "{memory:?}: the work limits measure what the program does, not memory (TIME-08)"
        );
    }
}

/// The A4 program's retained `activations` in the loop's installed program.
fn retained_activations(runner: &TestRunner<'_>) -> Value {
    let Program::Installed(cycle) = &runner.cycle else {
        panic!("the A4 program is installed");
    };
    plant_value(cycle.engine(), "activations")
}

#[test]
fn a_warm_restart_after_a_stop_keeps_the_retained_values_and_a_cold_one_does_not() {
    // IEC 61131-3 Ed.3 §6.5.6.1: a warm restart gives the RETAIN variables the values they had
    // when the resource was stopped; a chip keeps them in its memory (spec 34 STATE-08).
    for (mode, expected) in [("warm", Value::DInt(2)), ("cold", Value::DInt(0))] {
        let fixture = Fixture::with_program(A4_PROGRAM);
        let mut runner = fixture.runner();
        for at in [0, 10, 20, 30, 40, 50] {
            fixture.pass_at(&mut runner, at);
        }
        assert_eq!(retained_activations(&runner), Value::DInt(2), "{mode}");
        let _ = fixture.call_at(&mut runner, 52, &json!({"id": 1, "type": "shutdown"}));
        fixture.pass_at(&mut runner, 60);
        assert_eq!(runner.resource_state(), ResourceState::Stopped, "{mode}");

        let reply = fixture.call_at(
            &mut runner,
            70,
            &json!({"id": 2, "type": "restart", "params": {"mode": mode}}),
        );
        assert_eq!(
            reply["result"],
            json!({"status": "restart queued"}),
            "{mode}"
        );
        assert_eq!(runner.resource_state(), ResourceState::Running, "{mode}");
        assert_eq!(
            retained_activations(&runner),
            expected,
            "{mode}: the started program's first scan has run, with no activation due"
        );
    }
}
