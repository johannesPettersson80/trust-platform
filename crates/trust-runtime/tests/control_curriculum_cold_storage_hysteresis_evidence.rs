use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/src/Main.st");
const SIMULATION: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/simulation.toml"
);

#[derive(Clone, Copy)]
struct Sample {
    time_s: f64,
    pv: f64,
    requested: bool,
    applied: bool,
    mv: f64,
    switch_count: i32,
    dwell_elapsed_s: f64,
    dwell_inhibit: bool,
    door_open: f64,
    effective_surroundings: f64,
}

#[derive(Clone, Copy)]
struct Candidate {
    name: &'static str,
    hysteresis_width: f64,
    minimum_on_s: f64,
    minimum_off_s: f64,
}

fn temporary_config_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-curriculum-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn addressed_source() -> String {
    SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("lesson program before configuration")
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX44.0 : BOOL :=")
        .replace(
            "RequestedCooling : BOOL :=",
            "RequestedCooling AT %MX44.1 : BOOL :=",
        )
        .replace(
            "CoolingEnabled : BOOL :=",
            "CoolingEnabled AT %MX44.2 : BOOL :=",
        )
        .replace(
            "DwellInhibit : BOOL :=",
            "DwellInhibit AT %MX44.3 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %ML0 : LREAL :=")
        .replace(
            "HysteresisWidth : LREAL :=",
            "HysteresisWidth AT %ML8 : LREAL :=",
        )
        .replace(
            "MinimumOnTime : LREAL :=",
            "MinimumOnTime AT %ML16 : LREAL :=",
        )
        .replace(
            "MinimumOffTime : LREAL :=",
            "MinimumOffTime AT %ML24 : LREAL :=",
        )
        .replace("DwellElapsed : LREAL;", "DwellElapsed AT %ML32 : LREAL;")
        .replace("SwitchCount : DINT :=", "SwitchCount AT %MD40 : DINT :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "DoorOpenIndication : LREAL :=",
            "DoorOpenIndication AT %IL8 : LREAL :=",
        )
        .replace(
            "EffectiveSurroundings : LREAL :=",
            "EffectiveSurroundings AT %IL16 : LREAL :=",
        )
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        )
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image value");
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL process-image value");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL value") {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &IoAddress) -> i32 {
    match runtime.io().read(address).expect("read DINT value") {
        Value::DInt(value) => value,
        Value::DWord(value) => value as i32,
        other => panic!("expected DINT storage, got {other:?}"),
    }
}

fn compile_controller() -> Runtime {
    CompileSession::from_sources(vec![SourceFile::with_path(
        "cold-storage-hysteresis-dwell/src/Main.st",
        addressed_source(),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped hysteresis and dwell lesson")
}

fn run_candidate(candidate: Candidate) -> Vec<Sample> {
    let mut runtime = compile_controller();
    let config_path = temporary_config_path(candidate.name);
    std::fs::write(&config_path, SIMULATION).expect("write cold-storage simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load cold-storage simulation"),
    );
    let sp = IoAddress::parse("%ML0").expect("SP address");
    let hysteresis = IoAddress::parse("%ML8").expect("hysteresis address");
    let minimum_on = IoAddress::parse("%ML16").expect("minimum-on address");
    let minimum_off = IoAddress::parse("%ML24").expect("minimum-off address");
    let dwell_elapsed = IoAddress::parse("%ML32").expect("dwell address");
    let switch_count = IoAddress::parse("%MD40").expect("switch-count address");
    let auto = IoAddress::parse("%MX44.0").expect("Auto address");
    let requested = IoAddress::parse("%MX44.1").expect("requested-state address");
    let applied = IoAddress::parse("%MX44.2").expect("applied-state address");
    let dwell_inhibit = IoAddress::parse("%MX44.3").expect("dwell-inhibit address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let door_open = IoAddress::parse("%IL8").expect("door address");
    let surroundings = IoAddress::parse("%IL16").expect("surroundings address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial plant state");

    let mut trace = Vec::with_capacity(1200);
    for cycle in 1..=1200 {
        write_lreal(&mut runtime, &sp, 2.0);
        write_lreal(&mut runtime, &hysteresis, candidate.hysteresis_width);
        write_lreal(&mut runtime, &minimum_on, candidate.minimum_on_s);
        write_lreal(&mut runtime, &minimum_off, candidate.minimum_off_s);
        write_bool(&mut runtime, &auto, true);
        runtime.execute_cycle().expect("execute thermostat");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance cold-storage room");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish cold-storage feedback");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            pv: read_lreal(&runtime, &pv),
            requested: read_bool(&runtime, &requested),
            applied: read_bool(&runtime, &applied),
            mv: read_lreal(&runtime, &mv),
            switch_count: read_dint(&runtime, &switch_count),
            dwell_elapsed_s: read_lreal(&runtime, &dwell_elapsed),
            dwell_inhibit: read_bool(&runtime, &dwell_inhibit),
            door_open: read_lreal(&runtime, &door_open),
            effective_surroundings: read_lreal(&runtime, &surroundings),
        });
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

fn transition_indices(trace: &[Sample]) -> Vec<usize> {
    trace
        .windows(2)
        .enumerate()
        .filter_map(|(index, pair)| (pair[0].applied != pair[1].applied).then_some(index + 1))
        .collect()
}

fn minimum_completed_interval_s(trace: &[Sample], state: bool) -> f64 {
    let transitions = transition_indices(trace);
    transitions
        .windows(2)
        .filter_map(|pair| {
            (trace[pair[0]].applied == state).then_some((pair[1] - pair[0]) as f64 * 0.1)
        })
        .fold(f64::INFINITY, f64::min)
}

fn run_memory_and_dwell_sequence() -> Vec<(f64, bool, bool, bool)> {
    let mut runtime = compile_controller();
    let sp = IoAddress::parse("%ML0").expect("SP address");
    let hysteresis = IoAddress::parse("%ML8").expect("hysteresis address");
    let minimum_on = IoAddress::parse("%ML16").expect("minimum-on address");
    let minimum_off = IoAddress::parse("%ML24").expect("minimum-off address");
    let auto = IoAddress::parse("%MX44.0").expect("Auto address");
    let requested = IoAddress::parse("%MX44.1").expect("requested-state address");
    let applied = IoAddress::parse("%MX44.2").expect("applied-state address");
    let dwell_inhibit = IoAddress::parse("%MX44.3").expect("dwell-inhibit address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mut samples = Vec::new();
    for process_value in [2.60, 2.00, 1.40, 2.00] {
        write_lreal(&mut runtime, &sp, 2.0);
        write_lreal(&mut runtime, &hysteresis, 1.0);
        write_lreal(&mut runtime, &minimum_on, 8.0);
        write_lreal(&mut runtime, &minimum_off, 8.0);
        write_bool(&mut runtime, &auto, true);
        write_lreal(&mut runtime, &pv, process_value);
        runtime
            .execute_cycle()
            .expect("execute threshold-memory sample");
        samples.push((
            process_value,
            read_bool(&runtime, &requested),
            read_bool(&runtime, &applied),
            read_bool(&runtime, &dwell_inhibit),
        ));
    }
    samples
}

fn export_evidence(
    directory: &std::path::Path,
    traces: &[(Candidate, Vec<Sample>)],
    memory: &[(f64, bool, bool, bool)],
) {
    std::fs::create_dir_all(directory).expect("create Lesson 3 figure directory");
    let mut comparison = String::from(
        "case,time_s,setpoint_degC,pv_degC,requested_cooling,applied_cooling,mv_percent,switch_count,dwell_elapsed_s,dwell_inhibit,door_open,effective_surroundings_degC,evidence_class\n",
    );
    for (candidate, trace) in traces {
        for sample in trace {
            writeln!(
                comparison,
                "{},{:.1},2.0,{:.6},{},{},{:.1},{},{:.3},{},{:.0},{:.1},simulation",
                candidate.name,
                sample.time_s,
                sample.pv,
                u8::from(sample.requested),
                u8::from(sample.applied),
                sample.mv,
                sample.switch_count,
                sample.dwell_elapsed_s,
                u8::from(sample.dwell_inhibit),
                sample.door_open,
                sample.effective_surroundings,
            )
            .expect("write comparison row");
        }
    }
    std::fs::write(
        directory.join("hysteresis-dwell-comparison.csv"),
        comparison,
    )
    .expect("write comparison evidence");

    let mut sequence = String::from(
        "sample,pv_degC,requested_cooling,applied_cooling,dwell_inhibit,evidence_class\n",
    );
    for (index, (pv, requested, applied, inhibited)) in memory.iter().enumerate() {
        writeln!(
            sequence,
            "{},{:.2},{},{},{},simulation",
            index + 1,
            pv,
            u8::from(*requested),
            u8::from(*applied),
            u8::from(*inhibited),
        )
        .expect("write memory row");
    }
    std::fs::write(directory.join("threshold-memory.csv"), sequence)
        .expect("write threshold-memory evidence");
}

#[test]
fn control_curriculum_cold_storage_hysteresis_exports_protection_tradeoff_evidence() {
    let candidates = [
        Candidate {
            name: "single_threshold",
            hysteresis_width: 0.0,
            minimum_on_s: 0.0,
            minimum_off_s: 0.0,
        },
        Candidate {
            name: "hysteresis",
            hysteresis_width: 1.0,
            minimum_on_s: 0.0,
            minimum_off_s: 0.0,
        },
        Candidate {
            name: "hysteresis_dwell",
            hysteresis_width: 1.0,
            minimum_on_s: 8.0,
            minimum_off_s: 8.0,
        },
    ];
    let traces = candidates
        .into_iter()
        .map(|candidate| (candidate, run_candidate(candidate)))
        .collect::<Vec<_>>();
    for (_, trace) in &traces {
        assert_eq!(trace.len(), 1200);
        assert!(trace.iter().all(|sample| {
            sample.pv.is_finite()
                && sample.dwell_elapsed_s.is_finite()
                && matches!(sample.mv, 0.0 | 100.0)
                && sample.applied == (sample.mv == 100.0)
        }));
        assert!(
            trace[..599]
                .iter()
                .all(|sample| sample.door_open == 0.0 && sample.effective_surroundings == 4.0),
            "pre-door auxiliary signals differ: first=({}, {}), last=({}, {})",
            trace[0].door_open,
            trace[0].effective_surroundings,
            trace[598].door_open,
            trace[598].effective_surroundings,
        );
        assert!(trace[599..899]
            .iter()
            .all(|sample| { sample.door_open == 1.0 && sample.effective_surroundings == 6.0 }));
        assert!(trace[899..]
            .iter()
            .all(|sample| { sample.door_open == 0.0 && sample.effective_surroundings == 4.0 }));
        assert!(trace.iter().all(|sample| {
            sample.dwell_inhibit || sample.requested == sample.applied
        }), "requested and applied state may differ only while dwell protection inhibits a transition");
    }

    // SwitchCount includes the initial safe-off to commanded-on transition.
    // Counting only adjacent retained samples would silently omit that event.
    let single_transitions = traces[0].1.last().expect("single trace").switch_count as usize;
    let hysteresis_transitions =
        traces[1].1.last().expect("hysteresis trace").switch_count as usize;
    let protected_transitions = traces[2].1.last().expect("protected trace").switch_count as usize;
    assert_eq!(
        single_transitions, 760,
        "the unchanged single-threshold baseline must reproduce Lesson 2 evidence"
    );
    assert!(hysteresis_transitions * 10 < single_transitions,
        "hysteresis must materially reduce transitions: single={single_transitions}, hysteresis={hysteresis_transitions}");
    assert!(protected_transitions <= hysteresis_transitions);

    let protected = &traces[2].1;
    let minimum_on = minimum_completed_interval_s(protected, true);
    let minimum_off = minimum_completed_interval_s(protected, false);
    assert!(
        minimum_on >= 8.0 - 1.0e-9,
        "minimum completed on-time={minimum_on}"
    );
    assert!(
        minimum_off >= 8.0 - 1.0e-9,
        "minimum completed off-time={minimum_off}"
    );
    assert!(
        protected
            .iter()
            .any(|sample| sample.dwell_inhibit && sample.requested != sample.applied),
        "the retained comparison must expose at least one dwell-postponed reversal"
    );
    let door_max = protected[599..899]
        .iter()
        .map(|sample| sample.pv)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(door_max <= 3.0 + 0.05, "door-event maximum={door_max}");

    let memory = run_memory_and_dwell_sequence();
    assert_eq!(memory[0], (2.60, true, true, false));
    assert_eq!(
        memory[1],
        (2.00, true, true, false),
        "state memory must retain cooling inside the hysteresis band"
    );
    assert_eq!(
        memory[2],
        (1.40, false, true, true),
        "minimum on-time must expose a requested/applied mismatch while inhibited"
    );
    assert_eq!(
        memory[3],
        (2.00, false, true, true),
        "returning inside the band retains the requested state while applied dwell continues"
    );

    if let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR") {
        export_evidence(std::path::Path::new(&directory), &traces, &memory);
    }
}
