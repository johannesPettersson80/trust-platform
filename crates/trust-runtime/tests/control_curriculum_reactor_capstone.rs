use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const DT: f64 = 0.1;
const CYCLES: usize = 1_500;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root() -> PathBuf {
    project_path("figures")
}

fn values(trace: &[Sample], select: impl Fn(&Sample) -> f64) -> Vec<f64> {
    trace.iter().map(select).collect()
}

fn plot_bounds<'a>(series: impl IntoIterator<Item = &'a [f64]>) -> (f64, f64) {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for values in series {
        for value in values {
            minimum = minimum.min(*value);
            maximum = maximum.max(*value);
        }
    }
    let padding = ((maximum - minimum) * 0.08).max(0.01);
    (minimum - padding, maximum + padding)
}

fn project_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/79-reactor")
        .join(relative)
}

fn project_file(relative: &str) -> String {
    let path = project_path(relative);
    assert!(
        path.is_file(),
        "Lesson 79 executable reactor capstone is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 79 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 79 program before configuration")
        .replace(
            "\n    OperatingMode : DINT :=",
            "\n    OperatingMode AT %ID192 : DINT :=",
        )
        .replace(
            "\n    ReactorSetpoint : LREAL :=",
            "\n    ReactorSetpoint AT %IL200 : LREAL :=",
        )
        .replace(
            "\n    JacketCommissioningSetpoint : LREAL :=",
            "\n    JacketCommissioningSetpoint AT %IL208 : LREAL :=",
        )
        .replace(
            "\n    FeedThermalLoad : LREAL :=",
            "\n    FeedThermalLoad AT %IL216 : LREAL :=",
        )
        .replace(
            "\n    ManualUtilityDemand : LREAL :=",
            "\n    ManualUtilityDemand AT %IL224 : LREAL :=",
        )
        .replace(
            "\n    CycleTime : LREAL :=",
            "\n    CycleTime AT %IL232 : LREAL :=",
        )
        .replace(
            "\n    FeedLoadQualityGood : BOOL :=",
            "\n    FeedLoadQualityGood AT %IX240.0 : BOOL :=",
        )
        .replace(
            "\n    ReactorQualityGood : BOOL :=",
            "\n    ReactorQualityGood AT %IX240.1 : BOOL :=",
        )
        .replace(
            "\n    JacketQualityGood : BOOL :=",
            "\n    JacketQualityGood AT %IX240.2 : BOOL :=",
        )
        .replace(
            "\n    HeatingAvailable : BOOL :=",
            "\n    HeatingAvailable AT %IX240.3 : BOOL :=",
        )
        .replace(
            "\n    CoolingAvailable : BOOL :=",
            "\n    CoolingAvailable AT %IX240.4 : BOOL :=",
        )
        .replace(
            "FeedbackReactorTemperature : LREAL :=",
            "FeedbackReactorTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "FeedbackJacketTemperature : LREAL :=",
            "FeedbackJacketTemperature AT %IL8 : LREAL :=",
        )
        .replace(
            "IntegratedReactorTemperature : LREAL :=",
            "IntegratedReactorTemperature AT %IL16 : LREAL :=",
        )
        .replace(
            "IntegratedJacketTemperature : LREAL :=",
            "IntegratedJacketTemperature AT %IL24 : LREAL :=",
        )
        .replace(
            "FeedbackJacketSetpoint : LREAL :=",
            "FeedbackJacketSetpoint AT %ML0 : LREAL :=",
        )
        .replace(
            "FeedbackFeedforward : LREAL :=",
            "FeedbackFeedforward AT %ML8 : LREAL :=",
        )
        .replace(
            "FeedbackNormalDemand : LREAL :=",
            "FeedbackNormalDemand AT %ML16 : LREAL :=",
        )
        .replace(
            "FeedbackProtectionDemand : LREAL :=",
            "FeedbackProtectionDemand AT %ML24 : LREAL :=",
        )
        .replace(
            "FeedbackSelectedDemand : LREAL :=",
            "FeedbackSelectedDemand AT %ML32 : LREAL :=",
        )
        .replace(
            "FeedbackHeatingRequested : LREAL :=",
            "FeedbackHeatingRequested AT %ML40 : LREAL :=",
        )
        .replace(
            "FeedbackCoolingRequested : LREAL :=",
            "FeedbackCoolingRequested AT %ML48 : LREAL :=",
        )
        .replace(
            "IntegratedJacketSetpoint : LREAL :=",
            "IntegratedJacketSetpoint AT %ML56 : LREAL :=",
        )
        .replace(
            "IntegratedFeedforward : LREAL :=",
            "IntegratedFeedforward AT %ML64 : LREAL :=",
        )
        .replace(
            "IntegratedNormalDemand : LREAL :=",
            "IntegratedNormalDemand AT %ML72 : LREAL :=",
        )
        .replace(
            "IntegratedProtectionDemand : LREAL :=",
            "IntegratedProtectionDemand AT %ML80 : LREAL :=",
        )
        .replace(
            "IntegratedSelectedDemand : LREAL :=",
            "IntegratedSelectedDemand AT %ML88 : LREAL :=",
        )
        .replace(
            "IntegratedHeatingRequested : LREAL :=",
            "IntegratedHeatingRequested AT %ML96 : LREAL :=",
        )
        .replace(
            "IntegratedCoolingRequested : LREAL :=",
            "IntegratedCoolingRequested AT %ML104 : LREAL :=",
        )
        .replace(
            "FeedbackProtectionActive : BOOL :=",
            "FeedbackProtectionActive AT %MX112.0 : BOOL :=",
        )
        .replace(
            "IntegratedProtectionActive : BOOL :=",
            "IntegratedProtectionActive AT %MX112.1 : BOOL :=",
        )
        .replace(
            "FeedbackTrackingActive : BOOL :=",
            "FeedbackTrackingActive AT %MX112.2 : BOOL :=",
        )
        .replace(
            "IntegratedTrackingActive : BOOL :=",
            "IntegratedTrackingActive AT %MX112.3 : BOOL :=",
        )
        .replace(
            "AuthorityFault : BOOL :=",
            "AuthorityFault AT %MX112.4 : BOOL :=",
        )
        .replace(
            "FeedbackAuthority : INT :=",
            "FeedbackAuthority AT %MW116 : INT :=",
        )
        .replace(
            "IntegratedAuthority : INT :=",
            "IntegratedAuthority AT %MW118 : INT :=",
        )
        .replace(
            "ArchitectureRevision : DINT :=",
            "ArchitectureRevision AT %MD120 : DINT :=",
        )
        .replace(
            "ControllerAuto : BOOL :=",
            "ControllerAuto AT %MX124.0 : BOOL :=",
        )
        .replace(
            "FeedbackHeatingApplied : LREAL :=",
            "FeedbackHeatingApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "FeedbackCoolingApplied : LREAL :=",
            "FeedbackCoolingApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "FeedbackLoadApplied : LREAL :=",
            "FeedbackLoadApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "IntegratedHeatingApplied : LREAL :=",
            "IntegratedHeatingApplied AT %QL24 : LREAL :=",
        )
        .replace(
            "IntegratedCoolingApplied : LREAL :=",
            "IntegratedCoolingApplied AT %QL32 : LREAL :=",
        )
        .replace(
            "IntegratedLoadApplied : LREAL :=",
            "IntegratedLoadApplied AT %QL40 : LREAL :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "control/14-integrated-capstones/79-reactor/src/ReactorTrain.st",
            project_file("src/ReactorTrain.st"),
        ),
        SourceFile::with_path(
            "control/14-integrated-capstones/79-reactor/src/Main.st",
            mapped_source(),
        ),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 79 Structured Text and Control library")
}

fn write_lreal(runtime: &mut Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).unwrap(),
            Value::LWord(value.to_bits()),
        )
        .unwrap();
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(&IoAddress::parse(address).unwrap(), Value::Bool(value))
        .unwrap();
}

fn write_dint(runtime: &mut Runtime, address: &str, value: i32) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).unwrap(),
            Value::DWord(value as u32),
        )
        .unwrap();
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL at {address}, got {value:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL at {address}, got {value:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &str) -> i32 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::DWord(value) => value as i32,
        value => panic!("expected DINT at {address}, got {value:?}"),
    }
}

#[derive(Clone, Debug)]
struct Sample {
    time: f64,
    mode: i32,
    reactor_setpoint: f64,
    reactor: [f64; 2],
    jacket: [f64; 2],
    jacket_setpoint: [f64; 2],
    feedforward: [f64; 2],
    normal_demand: [f64; 2],
    protection_demand: [f64; 2],
    selected_demand: [f64; 2],
    heating_requested: [f64; 2],
    cooling_requested: [f64; 2],
    heating_applied: [f64; 2],
    cooling_applied: [f64; 2],
    protection_active: [bool; 2],
    tracking_active: [bool; 2],
    authority: [i16; 2],
    feed_load: f64,
    feed_quality: bool,
    reactor_quality: bool,
    heating_available: bool,
    cooling_available: bool,
    authority_fault: bool,
    event_id: f64,
    recovered: bool,
    revision: i32,
}

fn read_int(runtime: &Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Word(value) => value as i16,
        value => panic!("expected INT at {address}, got {value:?}"),
    }
}

fn run_sequence(inject_faults: bool) -> Vec<Sample> {
    let mut runtime = build_runtime();
    let mut simulation = SimulationController::new(
        SimulationConfig::load(project_path("simulation.toml"))
            .expect("load Lesson 79 reactor configuration"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::with_capacity(CYCLES);
    let mut manual_demand = 50.0;

    for cycle in 0..CYCLES {
        let time = cycle as f64 * DT;
        let mode = if time < 10.0 {
            1
        } else if inject_faults && (75.0..77.0).contains(&time) {
            3
        } else if (100.0..105.0).contains(&time) {
            3
        } else if time >= 120.0 {
            4
        } else {
            2
        };
        let reactor_setpoint = if time < 10.0 { 80.0 } else { 82.0 };
        let jacket_test = if (1.0..9.0).contains(&time) {
            82.0
        } else {
            80.0
        };
        let feed_load = if (30.0..45.0).contains(&time) || (60.0..65.0).contains(&time) {
            4.0
        } else if (70.0..78.0).contains(&time) {
            -12.0
        } else {
            0.0
        };
        let feed_quality = !(60.0..65.0).contains(&time) && !(70.0..78.0).contains(&time);
        let reactor_quality = !inject_faults || !(110.0..112.0).contains(&time);
        let heating_available = !inject_faults || !(50.0..52.0).contains(&time);
        let cooling_available = !inject_faults || !(75.0..77.0).contains(&time);
        let event_id = if time < 10.0 {
            1.0
        } else if (30.0..45.0).contains(&time) {
            3.0
        } else if inject_faults && (50.0..52.0).contains(&time) {
            4.0
        } else if (60.0..65.0).contains(&time) {
            5.0
        } else if (70.0..78.0).contains(&time) {
            6.0
        } else if (100.0..105.0).contains(&time) {
            7.0
        } else if inject_faults && (110.0..112.0).contains(&time) {
            8.0
        } else if time >= 120.0 {
            9.0
        } else {
            2.0
        };
        if cycle == 1_000 {
            manual_demand = trace
                .last()
                .map_or(50.0, |sample: &Sample| sample.selected_demand[1]);
        }
        write_dint(&mut runtime, "%ID192", mode);
        write_lreal(&mut runtime, "%IL200", reactor_setpoint);
        write_lreal(&mut runtime, "%IL208", jacket_test);
        write_lreal(&mut runtime, "%IL216", feed_load);
        let manual_demand_for_scan = if inject_faults && (75.0..77.0).contains(&time) {
            -30.0
        } else {
            manual_demand
        };
        write_lreal(&mut runtime, "%IL224", manual_demand_for_scan);
        write_lreal(&mut runtime, "%IL232", DT);
        write_bool(&mut runtime, "%IX240.0", feed_quality);
        write_bool(&mut runtime, "%IX240.1", reactor_quality);
        write_bool(&mut runtime, "%IX240.2", true);
        write_bool(&mut runtime, "%IX240.3", heating_available);
        write_bool(&mut runtime, "%IX240.4", cooling_available);
        runtime
            .execute_cycle()
            .expect("execute Lesson 79 reactor scan");

        let heating_applied = ["%QL0", "%QL24"].map(|address| read_lreal(&runtime, address));
        let cooling_applied = ["%QL8", "%QL32"].map(|address| read_lreal(&runtime, address));
        assert!(heating_applied
            .iter()
            .chain(cooling_applied.iter())
            .all(|value| value.is_finite() && (0.0..=100.0).contains(value)));
        assert!(
            (0..2).all(|index| heating_applied[index] <= 0.01 || cooling_applied[index] <= 0.01)
        );

        let now = Duration::from_millis(((cycle + 1) * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance jacketed reactors");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish reactor feedback");
        trace.push(Sample {
            time,
            mode,
            reactor_setpoint,
            reactor: ["%IL0", "%IL16"].map(|address| read_lreal(&runtime, address)),
            jacket: ["%IL8", "%IL24"].map(|address| read_lreal(&runtime, address)),
            jacket_setpoint: ["%ML0", "%ML56"].map(|address| read_lreal(&runtime, address)),
            feedforward: ["%ML8", "%ML64"].map(|address| read_lreal(&runtime, address)),
            normal_demand: ["%ML16", "%ML72"].map(|address| read_lreal(&runtime, address)),
            protection_demand: ["%ML24", "%ML80"].map(|address| read_lreal(&runtime, address)),
            selected_demand: ["%ML32", "%ML88"].map(|address| read_lreal(&runtime, address)),
            heating_requested: ["%ML40", "%ML96"].map(|address| read_lreal(&runtime, address)),
            cooling_requested: ["%ML48", "%ML104"].map(|address| read_lreal(&runtime, address)),
            heating_applied,
            cooling_applied,
            protection_active: ["%MX112.0", "%MX112.1"].map(|address| read_bool(&runtime, address)),
            tracking_active: ["%MX112.2", "%MX112.3"].map(|address| read_bool(&runtime, address)),
            authority: ["%MW116", "%MW118"].map(|address| read_int(&runtime, address)),
            feed_load,
            feed_quality,
            reactor_quality,
            heating_available,
            cooling_available,
            authority_fault: read_bool(&runtime, "%MX112.4"),
            event_id,
            recovered: inject_faults
                && ((52.0..60.0).contains(&time)
                    || (77.0..100.0).contains(&time)
                    || (112.0..120.0).contains(&time))
                && !read_bool(&runtime, "%MX112.4"),
            revision: read_dint(&runtime, "%MD120"),
        });
    }
    trace
}

fn iae(trace: &[Sample], train: usize, start: f64, end: f64) -> f64 {
    trace
        .iter()
        .filter(|sample| sample.time >= start && sample.time < end)
        .map(|sample| (sample.reactor_setpoint - sample.reactor[train]).abs() * DT)
        .sum()
}

fn maximum_valve_jump(trace: &[Sample], train: usize) -> f64 {
    trace
        .windows(2)
        .map(|pair| {
            (pair[1].heating_applied[train] - pair[0].heating_applied[train])
                .abs()
                .max((pair[1].cooling_applied[train] - pair[0].cooling_applied[train]).abs())
        })
        .fold(0.0, f64::max)
}

fn maximum_departure_from_pre_event(trace: &[Sample], train: usize, start: f64, end: f64) -> f64 {
    let baseline = trace
        .iter()
        .rev()
        .find(|sample| sample.time < start)
        .expect("sample immediately before event")
        .reactor[train];
    trace
        .iter()
        .filter(|sample| sample.time >= start && sample.time < end)
        .map(|sample| (sample.reactor[train] - baseline).abs())
        .fold(0.0, f64::max)
}

fn applied_valve_total_variation(trace: &[Sample], train: usize, start: f64, end: f64) -> f64 {
    trace
        .windows(2)
        .filter(|pair| pair[1].time >= start && pair[1].time < end)
        .map(|pair| {
            (pair[1].heating_applied[train] - pair[0].heating_applied[train]).abs()
                + (pair[1].cooling_applied[train] - pair[0].cooling_applied[train]).abs()
        })
        .sum()
}

#[derive(Debug)]
struct IdentificationSample {
    time: f64,
    manual_demand: f64,
    feed_load: f64,
    reactor: f64,
    jacket: f64,
    heating_applied: f64,
}

fn run_identification_sequence() -> Vec<IdentificationSample> {
    let mut runtime = build_runtime();
    let mut simulation = SimulationController::new(
        SimulationConfig::load(project_path("simulation.toml"))
            .expect("load Lesson 79 identification configuration"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial identification state");
    let mut trace = Vec::with_capacity(CYCLES);
    for cycle in 0..CYCLES {
        let time = cycle as f64 * DT;
        let manual_demand = if (10.0..60.0).contains(&time) {
            55.0
        } else {
            50.0
        };
        let feed_load = if (90.0..130.0).contains(&time) {
            4.0
        } else {
            0.0
        };
        write_dint(&mut runtime, "%ID192", 3);
        write_lreal(&mut runtime, "%IL200", 80.0);
        write_lreal(&mut runtime, "%IL208", 80.0);
        write_lreal(&mut runtime, "%IL216", feed_load);
        write_lreal(&mut runtime, "%IL224", manual_demand);
        write_lreal(&mut runtime, "%IL232", DT);
        for address in ["%IX240.0", "%IX240.1", "%IX240.2", "%IX240.3", "%IX240.4"] {
            write_bool(&mut runtime, address, true);
        }
        runtime
            .execute_cycle()
            .expect("execute Lesson 79 identification scan");
        let now = Duration::from_millis(((cycle + 1) * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance identification reactor");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish identification response");
        trace.push(IdentificationSample {
            time,
            manual_demand,
            feed_load,
            reactor: read_lreal(&runtime, "%IL16"),
            jacket: read_lreal(&runtime, "%IL24"),
            heating_applied: read_lreal(&runtime, "%QL24"),
        });
    }
    trace
}

fn mean_identification(
    trace: &[IdentificationSample],
    start: usize,
    end: usize,
    select: impl Fn(&IdentificationSample) -> f64,
) -> f64 {
    trace[start..end].iter().map(select).sum::<f64>() / (end - start) as f64
}

fn response_time_constant(
    trace: &[IdentificationSample],
    start: usize,
    baseline: f64,
    final_value: f64,
    select: impl Fn(&IdentificationSample) -> f64,
) -> f64 {
    let target = baseline + 0.632_120_558_8 * (final_value - baseline);
    let rising = final_value >= baseline;
    let index = trace[start..]
        .iter()
        .position(|sample| {
            let value = select(sample);
            if rising {
                value >= target
            } else {
                value <= target
            }
        })
        .expect("identification response reaches 63.2 percent");
    index as f64 * DT
}

#[test]
fn control_curriculum_reactor_capstone_publishes_control_studio_auto_mode() {
    let mut runtime = build_runtime();
    let mut simulation = SimulationController::new(
        SimulationConfig::load(project_path("simulation.toml"))
            .expect("load Lesson 79 reactor configuration"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial reactor state");
    write_lreal(&mut runtime, "%IL200", 80.0);
    write_lreal(&mut runtime, "%IL208", 80.0);
    write_lreal(&mut runtime, "%IL216", 0.0);
    write_lreal(&mut runtime, "%IL224", 50.0);
    write_lreal(&mut runtime, "%IL232", DT);
    write_dint(&mut runtime, "%ID192", 2);
    write_bool(&mut runtime, "%IX240.0", true);
    write_bool(&mut runtime, "%IX240.1", true);
    write_bool(&mut runtime, "%IX240.2", true);
    write_bool(&mut runtime, "%IX240.3", true);
    write_bool(&mut runtime, "%IX240.4", true);
    runtime.execute_cycle().expect("execute Production scan");
    assert!(
        read_bool(&runtime, "%MX124.0"),
        "Control Studio requires an explicit Boolean Auto indication in Production"
    );

    write_dint(&mut runtime, "%ID192", 1);
    runtime
        .execute_cycle()
        .expect("execute InnerCommissioning scan");
    assert!(
        !read_bool(&runtime, "%MX124.0"),
        "non-Production modes must not authorize a Control Studio setpoint test"
    );
}

#[test]
fn control_curriculum_reactor_capstone_coordinates_tracked_authority() {
    let root = project_path("");
    assert!(
        root.is_dir(),
        "Lesson 79 executable reactor capstone is missing: {}",
        root.display()
    );
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 79 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 79);
    assert_eq!(lesson["prerequisites"][0], "control-capstone-servo");
    assert_eq!(lesson["nextLesson"], "control-capstone-gravity-tank");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_reactor_capstone_coordinates_tracked_authority"
    );
    SimulationConfig::load(project_path("simulation.toml"))
        .expect("Lesson 79 configuration must be executable");

    let identification = run_identification_sequence();
    let baseline_jacket = mean_identification(&identification, 80, 100, |sample| sample.jacket);
    let baseline_reactor = mean_identification(&identification, 80, 100, |sample| sample.reactor);
    let valve_jacket_final = mean_identification(&identification, 550, 600, |sample| sample.jacket);
    let valve_reactor_final =
        mean_identification(&identification, 550, 600, |sample| sample.reactor);
    let restored_reactor = mean_identification(&identification, 850, 900, |sample| sample.reactor);
    let feed_reactor_final =
        mean_identification(&identification, 1_250, 1_300, |sample| sample.reactor);
    let jacket_gain = (valve_jacket_final - baseline_jacket) / 5.0;
    let reactor_gain = (valve_reactor_final - baseline_reactor) / 5.0;
    let feed_gain = (feed_reactor_final - restored_reactor) / 4.0;
    let inner_time_constant = response_time_constant(
        &identification,
        100,
        baseline_jacket,
        valve_jacket_final,
        |sample| sample.jacket,
    );
    let outer_time_constant = response_time_constant(
        &identification,
        100,
        baseline_reactor,
        valve_reactor_final,
        |sample| sample.reactor,
    );
    let time_scale_ratio = outer_time_constant / inner_time_constant;
    let feedforward_gain = -feed_gain / reactor_gain;
    assert!(jacket_gain.is_finite() && jacket_gain > 0.0);
    assert!(reactor_gain.is_finite() && reactor_gain > 0.0);
    assert!(feed_gain.is_finite() && feed_gain < 0.0);
    assert!(time_scale_ratio >= 3.0);
    assert!(feedforward_gain.is_finite() && feedforward_gain > 0.0);

    let trace = run_sequence(false);
    assert!(trace.iter().all(|sample| sample.revision == 79));
    assert!(trace.iter().all(|sample| {
        (0..2).all(|index| {
            (sample.heating_requested[index] - sample.heating_applied[index]).abs() < 1.0e-9
                && (sample.cooling_requested[index] - sample.cooling_applied[index]).abs() < 1.0e-9
        })
    }));
    let commissioning_closest = trace
        .iter()
        .filter(|sample| (8.0..9.0).contains(&sample.time))
        .map(|sample| (sample.jacket[1] - 82.0).abs())
        .fold(f64::INFINITY, f64::min);
    assert!(
        commissioning_closest <= 0.3,
        "commissioning jacket closest error={commissioning_closest:.3} degC"
    );
    let feedback_iae = iae(&trace, 0, 30.0, 50.0);
    let integrated_iae = iae(&trace, 1, 30.0, 50.0);
    let feedback_departure = maximum_departure_from_pre_event(&trace, 0, 30.0, 50.0);
    let integrated_departure = maximum_departure_from_pre_event(&trace, 1, 30.0, 50.0);
    let feedback_valve_travel = applied_valve_total_variation(&trace, 0, 30.0, 50.0);
    let integrated_valve_travel = applied_valve_total_variation(&trace, 1, 30.0, 50.0);
    assert!(
        integrated_iae <= 0.75 * feedback_iae,
        "feedforward IAE {integrated_iae:.3} versus feedback {feedback_iae:.3}"
    );
    assert!(
        integrated_departure <= feedback_departure,
        "feedforward departure {integrated_departure:.3} versus feedback {feedback_departure:.3}"
    );
    let onset = trace.iter().find(|sample| sample.time >= 30.0).unwrap();
    assert!((onset.feedforward[0]).abs() < 1.0e-9);
    assert!((onset.feedforward[1] - 2.1).abs() <= 0.01);
    assert!(trace
        .iter()
        .filter(|sample| !sample.feed_quality)
        .all(|sample| sample.feedforward.iter().all(|value| value.abs() < 1.0e-9)));
    assert!(trace.iter().any(|sample| sample.protection_active[1]));
    let protected_peak = trace
        .iter()
        .filter(|sample| (70.0..90.0).contains(&sample.time))
        .map(|sample| sample.reactor[1])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        protected_peak < 86.0,
        "protected peak={protected_peak:.3} degC"
    );
    assert!(trace
        .iter()
        .any(|sample| sample.protection_active[1] && sample.tracking_active[1]));
    assert!(trace
        .iter()
        .filter(|sample| sample.protection_active[1])
        .all(|sample| sample.authority[1] == 3));
    assert!(maximum_valve_jump(&trace, 1) <= 5.0 + 1.0e-9);
    let final_sample = trace.last().unwrap();
    assert_eq!(final_sample.mode, 4);
    assert!(final_sample.reactor[1] <= 75.0);
    assert!(final_sample.heating_applied[1].abs() < 1.0e-9);
    assert!(final_sample.cooling_applied[1].abs() < 1.0e-9);

    let fault_trace = run_sequence(true);
    assert!(fault_trace.iter().any(|sample| sample.authority_fault));
    assert!(fault_trace
        .iter()
        .filter(|sample| sample.authority_fault)
        .all(|sample| sample.heating_applied[1].abs() < 1.0e-9
            && sample.cooling_applied[1].abs() < 1.0e-9));
    assert!(fault_trace.iter().all(|sample| {
        sample
            .normal_demand
            .iter()
            .chain(sample.protection_demand.iter())
            .chain(sample.selected_demand.iter())
            .chain(sample.jacket_setpoint.iter())
            .all(|value| value.is_finite())
    }));
    assert!(fault_trace
        .iter()
        .any(|sample| !sample.heating_available && sample.authority_fault));
    let unavailable_cooling_peak_request = fault_trace
        .iter()
        .filter(|sample| !sample.cooling_available)
        .map(|sample| sample.cooling_requested[1])
        .fold(0.0, f64::max);
    assert!(
        fault_trace
            .iter()
            .any(|sample| !sample.cooling_available && sample.authority_fault),
        "cooling-unavailable event never required cooling; peak request={unavailable_cooling_peak_request:.3}%"
    );
    assert!(fault_trace
        .iter()
        .any(|sample| !sample.reactor_quality && sample.authority_fault));
    assert!(fault_trace.iter().any(|sample| sample.recovered));

    println!(
        "reactor capstone: identified jacket/reactor/feed gains={jacket_gain:.4}/{reactor_gain:.4}/{feed_gain:.4}, time constants={inner_time_constant:.2}/{outer_time_constant:.2}s, time-scale ratio={time_scale_ratio:.1}, Kff={feedforward_gain:.3}; feedback/integrated cold-load IAE={feedback_iae:.2}/{integrated_iae:.2} degC*s; departure={feedback_departure:.3}/{integrated_departure:.3} degC; valve travel={feedback_valve_travel:.2}/{integrated_valve_travel:.2} pp; protected peak={protected_peak:.3} degC; max valve jump={:.2} pp; shutdown final={:.3} degC",
        maximum_valve_jump(&trace, 1),
        final_sample.reactor[1],
    );

    let time = values(&trace, |sample| sample.time);
    let setpoint = values(&trace, |sample| sample.reactor_setpoint);
    let feedback_reactor = values(&trace, |sample| sample.reactor[0]);
    let integrated_reactor = values(&trace, |sample| sample.reactor[1]);
    let integrated_jacket = values(&trace, |sample| sample.jacket[1]);
    let integrated_jacket_setpoint = values(&trace, |sample| sample.jacket_setpoint[1]);
    let feedforward = values(&trace, |sample| sample.feedforward[1]);
    let normal_demand = values(&trace, |sample| sample.normal_demand[1]);
    let protection_demand = values(&trace, |sample| sample.protection_demand[1]);
    let selected_demand = values(&trace, |sample| sample.selected_demand[1]);
    let heating_requested = values(&trace, |sample| sample.heating_requested[1]);
    let cooling_requested = values(&trace, |sample| sample.cooling_requested[1]);
    let heating_applied = values(&trace, |sample| sample.heating_applied[1]);
    let cooling_applied = values(&trace, |sample| sample.cooling_applied[1]);
    let mode = values(&trace, |sample| sample.mode as f64);
    let authority = values(&trace, |sample| sample.authority[1] as f64);
    let protection_active = values(&trace, |sample| f64::from(sample.protection_active[1]));
    let tracking_active = values(&trace, |sample| f64::from(sample.tracking_active[1]));
    let feed_quality = values(&trace, |sample| f64::from(sample.feed_quality));
    let revision = values(&trace, |sample| sample.revision as f64);
    let feed_load = values(&trace, |sample| sample.feed_load);
    let event_id = values(&trace, |sample| sample.event_id);
    let fault_time = values(&fault_trace, |sample| sample.time);
    let authority_fault = values(&fault_trace, |sample| f64::from(sample.authority_fault));
    let fault_event_id = values(&fault_trace, |sample| sample.event_id);
    let fault_feed_load = values(&fault_trace, |sample| sample.feed_load);
    let fault_reactor_quality = values(&fault_trace, |sample| f64::from(sample.reactor_quality));
    let fault_heating_available =
        values(&fault_trace, |sample| f64::from(sample.heating_available));
    let fault_cooling_available =
        values(&fault_trace, |sample| f64::from(sample.cooling_available));
    let fault_recovered = values(&fault_trace, |sample| f64::from(sample.recovered));
    let fault_heating = values(&fault_trace, |sample| sample.heating_applied[1]);
    let fault_cooling = values(&fault_trace, |sample| sample.cooling_applied[1]);
    let identification_time = identification
        .iter()
        .map(|sample| sample.time)
        .collect::<Vec<_>>();
    let identification_manual = identification
        .iter()
        .map(|sample| sample.manual_demand)
        .collect::<Vec<_>>();
    let identification_feed = identification
        .iter()
        .map(|sample| sample.feed_load)
        .collect::<Vec<_>>();
    let identification_reactor = identification
        .iter()
        .map(|sample| sample.reactor)
        .collect::<Vec<_>>();
    let identification_jacket = identification
        .iter()
        .map(|sample| sample.jacket)
        .collect::<Vec<_>>();
    let identification_heating = identification
        .iter()
        .map(|sample| sample.heating_applied)
        .collect::<Vec<_>>();
    let (temperature_min, temperature_max) = plot_bounds([
        setpoint.as_slice(),
        feedback_reactor.as_slice(),
        integrated_reactor.as_slice(),
    ]);
    let (utility_min, utility_max) =
        plot_bounds([heating_applied.as_slice(), cooling_applied.as_slice()]);
    let summary = format!(
        "case,status,cold_feed_iae_degC_s,cold_feed_departure_degC,valve_travel_pp,protected_peak_degC,maximum_valve_jump_pp,shutdown_final_degC,identified_jacket_gain,identified_reactor_gain,identified_feed_gain,identified_jacket_tau_s,identified_reactor_tau_s,identified_time_scale_ratio,identified_feedforward_gain\nidentification,qualified,,,,,,,,{jacket_gain:.6},{reactor_gain:.6},{feed_gain:.6},{inner_time_constant:.6},{outer_time_constant:.6},{time_scale_ratio:.6},{feedforward_gain:.6}\nfeedback_only,baseline,{feedback_iae:.6},{feedback_departure:.6},{feedback_valve_travel:.6},,,,,,,,,,\nintegrated,qualified,{integrated_iae:.6},{integrated_departure:.6},{integrated_valve_travel:.6},{protected_peak:.6},{:.6},{:.6},,,,,,,\nheating_unavailable,safe_state_then_recovered,,,,,,,,,,,,,,,\ncooling_unavailable,safe_state_then_recovered,,,,,,,,,,,,,,,\nreactor_quality_invalid,safe_state_then_recovered,,,,,,,,,,,,,,,\nshutdown,qualified,,,,,,{:.6},,,,,,,\n",
        maximum_valve_jump(&trace, 1),
        final_sample.reactor[1],
        final_sample.reactor[1],
    );
    evidence::retain(
        &evidence_root(),
        "reactor-capstone-evidence",
        "One authority path coordinates the reactor",
        "One inspectable path handles cascade, feedforward, protection, modes, and shutdown.",
        &[
            evidence::Panel {
                title: "Reactor temperature",
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: temperature_min,
                y_maximum: temperature_max,
                series: vec![
                    evidence::Series {
                        name: "Setpoint",
                        color: "#111827",
                        dashed: true,
                        x: &time,
                        y: &setpoint,
                    },
                    evidence::Series {
                        name: "Feedback only",
                        color: "#6b7280",
                        dashed: false,
                        x: &time,
                        y: &feedback_reactor,
                    },
                    evidence::Series {
                        name: "Integrated architecture",
                        color: "#2563eb",
                        dashed: false,
                        x: &time,
                        y: &integrated_reactor,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied utilities — steam red; cooling water blue",
                x_unit: "time (s)",
                y_unit: "valve (%)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: utility_min,
                y_maximum: utility_max,
                series: vec![
                    evidence::Series {
                        name: "Steam",
                        color: "#dc2626",
                        dashed: false,
                        x: &time,
                        y: &heating_applied,
                    },
                    evidence::Series {
                        name: "Cooling water",
                        color: "#2563eb",
                        dashed: false,
                        x: &time,
                        y: &cooling_applied,
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Cascade",
                name: "Jacket setpoint",
                x: &time,
                y: &integrated_jacket_setpoint,
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Cascade",
                name: "Jacket temperature",
                x: &time,
                y: &integrated_jacket,
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Feedforward",
                name: "Qualified contribution",
                x: &time,
                y: &feedforward,
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Feedforward",
                name: "Feed quality",
                x: &time,
                y: &feed_quality,
                x_unit: "time (s)",
                y_unit: "valid (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Selector",
                name: "Normal demand",
                x: &time,
                y: &normal_demand,
                x_unit: "time (s)",
                y_unit: "demand (%)",
            },
            evidence::RetainedSeries {
                panel: "Selector",
                name: "Protection demand",
                x: &time,
                y: &protection_demand,
                x_unit: "time (s)",
                y_unit: "demand (%)",
            },
            evidence::RetainedSeries {
                panel: "Selector",
                name: "Selected demand",
                x: &time,
                y: &selected_demand,
                x_unit: "time (s)",
                y_unit: "demand (%)",
            },
            evidence::RetainedSeries {
                panel: "Requested utilities",
                name: "Steam requested",
                x: &time,
                y: &heating_requested,
                x_unit: "time (s)",
                y_unit: "valve (%)",
            },
            evidence::RetainedSeries {
                panel: "Requested utilities",
                name: "Cooling requested",
                x: &time,
                y: &cooling_requested,
                x_unit: "time (s)",
                y_unit: "valve (%)",
            },
            evidence::RetainedSeries {
                panel: "Authority",
                name: "Operating mode",
                x: &time,
                y: &mode,
                x_unit: "time (s)",
                y_unit: "mode",
            },
            evidence::RetainedSeries {
                panel: "Authority",
                name: "Selected authority",
                x: &time,
                y: &authority,
                x_unit: "time (s)",
                y_unit: "authority",
            },
            evidence::RetainedSeries {
                panel: "Authority",
                name: "Protection active",
                x: &time,
                y: &protection_active,
                x_unit: "time (s)",
                y_unit: "state (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Authority",
                name: "Tracking active",
                x: &time,
                y: &tracking_active,
                x_unit: "time (s)",
                y_unit: "state (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Revision",
                name: "Architecture revision",
                x: &time,
                y: &revision,
                x_unit: "time (s)",
                y_unit: "revision",
            },
            evidence::RetainedSeries {
                panel: "Events",
                name: "Event identity",
                x: &time,
                y: &event_id,
                x_unit: "time (s)",
                y_unit: "event",
            },
            evidence::RetainedSeries {
                panel: "Events",
                name: "Raw feed thermal load",
                x: &time,
                y: &feed_load,
                x_unit: "time (s)",
                y_unit: "thermal load (degC)",
            },
            evidence::RetainedSeries {
                panel: "Identification",
                name: "Manual utility demand",
                x: &identification_time,
                y: &identification_manual,
                x_unit: "time (s)",
                y_unit: "utility demand (%)",
            },
            evidence::RetainedSeries {
                panel: "Identification",
                name: "Applied heating",
                x: &identification_time,
                y: &identification_heating,
                x_unit: "time (s)",
                y_unit: "valve (%)",
            },
            evidence::RetainedSeries {
                panel: "Identification",
                name: "Feed thermal load",
                x: &identification_time,
                y: &identification_feed,
                x_unit: "time (s)",
                y_unit: "thermal load (degC)",
            },
            evidence::RetainedSeries {
                panel: "Identification",
                name: "Jacket response",
                x: &identification_time,
                y: &identification_jacket,
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Identification",
                name: "Reactor response",
                x: &identification_time,
                y: &identification_reactor,
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Authority fault",
                x: &fault_time,
                y: &authority_fault,
                x_unit: "time (s)",
                y_unit: "state (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Event identity",
                x: &fault_time,
                y: &fault_event_id,
                x_unit: "time (s)",
                y_unit: "event",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Raw feed thermal load",
                x: &fault_time,
                y: &fault_feed_load,
                x_unit: "time (s)",
                y_unit: "thermal load (degC)",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Reactor quality",
                x: &fault_time,
                y: &fault_reactor_quality,
                x_unit: "time (s)",
                y_unit: "good (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Heating available",
                x: &fault_time,
                y: &fault_heating_available,
                x_unit: "time (s)",
                y_unit: "available (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Cooling available",
                x: &fault_time,
                y: &fault_cooling_available,
                x_unit: "time (s)",
                y_unit: "available (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Recovered",
                x: &fault_time,
                y: &fault_recovered,
                x_unit: "time (s)",
                y_unit: "qualified (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Steam applied",
                x: &fault_time,
                y: &fault_heating,
                x_unit: "time (s)",
                y_unit: "valve (%)",
            },
            evidence::RetainedSeries {
                panel: "Fault recovery",
                name: "Cooling applied",
                x: &fault_time,
                y: &fault_cooling,
                x_unit: "time (s)",
                y_unit: "valve (%)",
            },
        ],
        &summary,
    );
}
