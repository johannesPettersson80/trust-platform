use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const PLANT_IDS: [&str; 3] = [
    "healthy-valve-bench",
    "deadband-valve-bench",
    "stiction-valve-bench",
];

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/60-valve-stiction/figures")
}

fn time_axis(samples: usize) -> Vec<f64> {
    (1..=samples).map(|sample| sample as f64 * 0.1).collect()
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

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/60-valve-stiction")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 60 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 60 file {}: {error}", path.display()))
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-valve-stiction-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL lesson input");
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL lesson input");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime
        .io()
        .read(address)
        .expect("read LREAL lesson signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

struct ValveBench {
    runtime: Runtime,
    simulation: SimulationController,
    now_ms: i64,
    auto: IoAddress,
    manual: IoAddress,
    setpoint: IoAddress,
    flows: [IoAddress; 3],
    requests: [IoAddress; 3],
}

impl ValveBench {
    fn new() -> Self {
        let source = project_file("src/Main.st")
            .split("CONFIGURATION Config")
            .next()
            .expect("valve lesson program before configuration")
            .replace("AutoMode : BOOL :=", "AutoMode AT %MX91.0 : BOOL :=")
            .replace(
                "ManualDemand : LREAL :=",
                "ManualDemand AT %ML328 : LREAL :=",
            )
            .replace(
                "FlowSetpoint : LREAL :=",
                "FlowSetpoint AT %ML336 : LREAL :=",
            )
            .replace("HealthyFlow : LREAL :=", "HealthyFlow AT %IL168 : LREAL :=")
            .replace(
                "DeadbandFlow : LREAL :=",
                "DeadbandFlow AT %IL176 : LREAL :=",
            )
            .replace(
                "StictionFlow : LREAL :=",
                "StictionFlow AT %IL184 : LREAL :=",
            )
            .replace(
                "HealthyRequest : LREAL :=",
                "HealthyRequest AT %QL184 : LREAL :=",
            )
            .replace(
                "DeadbandRequest : LREAL :=",
                "DeadbandRequest AT %QL192 : LREAL :=",
            )
            .replace(
                "StictionRequest : LREAL :=",
                "StictionRequest AT %QL200 : LREAL :=",
            );
        let mut runtime = CompileSession::from_sources(vec![
            SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
            SourceFile::with_path("valve-stiction/src/Main.st", source),
        ])
        .label_errors(true)
        .build_runtime()
        .expect("compile shipped valve-characterization lesson");

        let path = temporary_config_path();
        std::fs::write(&path, project_file("simulation.toml"))
            .expect("write valve-characterization simulation config");
        let config = SimulationConfig::load(&path).expect("load three equivalent valve benches");
        assert_eq!(config.plant_system.plants.len(), 3);
        let mut simulation = SimulationController::new(config);
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .expect("publish initial flow measurements");
        let _ = std::fs::remove_file(path);

        Self {
            runtime,
            simulation,
            now_ms: 0,
            auto: IoAddress::parse("%MX91.0").unwrap(),
            manual: IoAddress::parse("%ML328").unwrap(),
            setpoint: IoAddress::parse("%ML336").unwrap(),
            flows: ["%IL168", "%IL176", "%IL184"].map(|address| IoAddress::parse(address).unwrap()),
            requests: ["%QL184", "%QL192", "%QL200"]
                .map(|address| IoAddress::parse(address).unwrap()),
        }
    }

    fn step(&mut self, automatic: bool, manual_demand: f64, setpoint: f64) {
        write_bool(&mut self.runtime, &self.auto, automatic);
        write_lreal(&mut self.runtime, &self.manual, manual_demand);
        write_lreal(&mut self.runtime, &self.setpoint, setpoint);
        self.runtime
            .execute_cycle()
            .expect("execute three valve controllers");
        self.now_ms += 100;
        let now = Duration::from_millis(self.now_ms);
        self.simulation
            .apply_post_cycle(now, &self.runtime)
            .expect("advance valve benches");
        self.simulation
            .apply_pre_cycle(now, &mut self.runtime)
            .expect("publish valve-bench flows");
    }

    fn hold_manual(&mut self, demand: f64, cycles: usize) {
        for _ in 0..cycles {
            self.step(false, demand, 64.0);
        }
    }

    fn positions(&self) -> [f64; 3] {
        let status = self.simulation.status();
        PLANT_IDS.map(|id| {
            status
                .plants
                .iter()
                .find(|plant| plant.id == id)
                .unwrap_or_else(|| panic!("missing valve bench '{id}'"))
                .actuators[0]
                .position
        })
    }

    fn actuator_flags(&self) -> [(bool, bool); 3] {
        let status = self.simulation.status();
        PLANT_IDS.map(|id| {
            let actuator = &status
                .plants
                .iter()
                .find(|plant| plant.id == id)
                .unwrap_or_else(|| panic!("missing valve bench '{id}'"))
                .actuators[0];
            (actuator.deadband_active, actuator.stiction_active)
        })
    }
}

#[test]
fn control_curriculum_valve_characterization_distinguishes_deadband_from_stiction() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 60 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 60 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 60);
    assert_eq!(lesson["prerequisites"][0], "control-heated-air-duct-smith");
    assert_eq!(lesson["nextLesson"], "control-distillation-mimo");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_valve_characterization_distinguishes_deadband_from_stiction"
    );
    assert_eq!(
        project_file("src/Main.st").matches("Control.PID;").count(),
        3
    );
    assert_eq!(
        project_file("simulation.toml")
            .matches(r#"model = "first_order""#)
            .count(),
        3
    );

    let mut bench = ValveBench::new();
    let positions = [0.0, 20.0, 40.0, 60.0, 80.0, 100.0];
    let predicted_flows = [0.0, 4.0, 16.0, 36.0, 64.0, 100.0];
    let mut measured_curve = Vec::with_capacity(positions.len());
    for (position, prediction) in positions.into_iter().zip(predicted_flows) {
        // The full-travel points must reach the declared settling condition;
        // actuator rate plus positioner lag makes ten seconds insufficient.
        bench.hold_manual(position, 300);
        let measured = read_lreal(&bench.runtime, &bench.flows[0]);
        measured_curve.push(measured);
        assert!(
            (measured - prediction).abs() <= 0.05,
            "healthy installed-curve point {position}% predicted {prediction:.6} m3/h, measured {measured:.6} m3/h"
        );
    }

    bench.hold_manual(70.0, 100);
    let restored = bench.positions();
    assert!(restored
        .iter()
        .all(|position| (*position - 70.0).abs() <= 0.001));
    let mut opening_positions = Vec::new();
    let mut opening_flags = Vec::new();
    let mut probe_requests = Vec::with_capacity(33);
    let mut probe_stiction_positions = Vec::with_capacity(33);
    for increment in 1..=11 {
        let request = 70.0 + increment as f64 * 0.5;
        bench.step(false, request, 64.0);
        opening_positions.push(bench.positions());
        opening_flags.push(bench.actuator_flags());
        probe_requests.push(request);
        probe_stiction_positions.push(bench.positions()[2]);
    }
    assert!(
        opening_positions[0][0] > 70.0,
        "healthy stem must begin moving immediately"
    );
    assert!(opening_positions[..4]
        .iter()
        .all(|positions| (positions[1] - 70.0).abs() <= 0.001));
    assert!(opening_flags[..4].iter().all(|flags| flags[1].0));
    assert!(
        opening_positions[4][1] > 70.0,
        "deadband stem must move after 2 percentage points"
    );
    assert!(!opening_flags[4][1].0);
    assert!(opening_positions[..10]
        .iter()
        .all(|positions| (positions[2] - 70.0).abs() <= 0.001));
    assert!(opening_flags[..10].iter().all(|flags| flags[2].1));
    assert!(
        (opening_positions[10][2] - 72.0).abs() <= 0.001,
        "stiction breakaway must apply the declared two-point slip jump"
    );
    assert!(!opening_flags[10][2].1);

    let mut qualified_breakaway_probes = 1usize;
    for _ in 0..2 {
        bench.hold_manual(70.0, 100);
        let start = bench.positions()[2];
        for increment in 1..=11 {
            let request = 70.0 + increment as f64 * 0.5;
            bench.step(false, request, 64.0);
            probe_requests.push(request);
            probe_stiction_positions.push(bench.positions()[2]);
        }
        let moved = bench.positions()[2] - start;
        if (moved - 2.0).abs() <= 0.001 {
            qualified_breakaway_probes += 1;
        }
    }
    assert_eq!(qualified_breakaway_probes, 3);
    println!(
        "installed curve measured={measured_curve:?}; deadband position at requests 72/72.5={:.6}/{:.6}; stiction position at requests 75/75.5={:.6}/{:.6}; qualified restored probes={qualified_breakaway_probes}",
        opening_positions[3][1],
        opening_positions[4][1],
        opening_positions[9][2],
        opening_positions[10][2],
    );

    let mut closed_loop = ValveBench::new();
    let mut previous_stiction_active = false;
    let mut breakaways = 0usize;
    let mut peak_gaps = [0.0_f64; 3];
    let mut flows = std::array::from_fn::<Vec<f64>, 3, _>(|_| Vec::with_capacity(1_800));
    let mut gaps = std::array::from_fn::<Vec<f64>, 3, _>(|_| Vec::with_capacity(1_800));
    let mut requests = std::array::from_fn::<Vec<f64>, 3, _>(|_| Vec::with_capacity(1_800));
    let mut stem_positions = std::array::from_fn::<Vec<f64>, 3, _>(|_| Vec::with_capacity(1_800));
    let mut deadband_state = Vec::with_capacity(1_800);
    let mut stiction_state = Vec::with_capacity(1_800);
    for cycle in 0..1_800 {
        let setpoint = if cycle < 100 { 64.0 } else { 70.0 };
        closed_loop.step(true, 80.0, setpoint);
        let status = closed_loop.simulation.status();
        for index in 0..3 {
            let request = read_lreal(&closed_loop.runtime, &closed_loop.requests[index]);
            let position = status
                .plants
                .iter()
                .find(|plant| plant.id == PLANT_IDS[index])
                .unwrap_or_else(|| panic!("missing valve bench '{}'", PLANT_IDS[index]))
                .actuators[0]
                .position;
            peak_gaps[index] = peak_gaps[index].max((request - position).abs());
            gaps[index].push(request - position);
            requests[index].push(request);
            stem_positions[index].push(position);
            flows[index].push(read_lreal(&closed_loop.runtime, &closed_loop.flows[index]));
        }
        let actuator_states = PLANT_IDS.map(|id| {
            &status
                .plants
                .iter()
                .find(|plant| plant.id == id)
                .unwrap_or_else(|| panic!("missing valve bench '{id}'"))
                .actuators[0]
        });
        let stiction_active = actuator_states[2].stiction_active;
        if previous_stiction_active && !stiction_active {
            breakaways += 1;
        }
        previous_stiction_active = stiction_active;
        deadband_state.push(f64::from(actuator_states[1].deadband_active));
        stiction_state.push(f64::from(stiction_active));
        assert!(!actuator_states[0].stiction_active);
        assert!(!actuator_states[1].stiction_active);
    }
    let peak_to_peak = flows.each_ref().map(|values| {
        let window = &values[600..];
        window.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - window.iter().copied().fold(f64::INFINITY, f64::min)
    });
    let peak_flow_difference = flows[0]
        .iter()
        .zip(&flows[2])
        .map(|(healthy, stiction)| (healthy - stiction).abs())
        .fold(0.0, f64::max);
    println!(
        "automatic breakaways={breakaways}, peak request-position gaps={peak_gaps:?}, final-window flow excursions={peak_to_peak:?}, peak healthy-stiction flow difference={peak_flow_difference:.6}"
    );
    assert!(peak_gaps[2] >= 4.9);
    assert!(peak_gaps[2] > peak_gaps[0]);
    assert!(peak_flow_difference >= 1.0);

    let time = time_axis(flows[0].len());
    let (flow_min, flow_max) = plot_bounds(flows.iter().map(Vec::as_slice));
    let (gap_min, gap_max) = plot_bounds(gaps.iter().map(Vec::as_slice));
    let summary = format!(
        "case,status,peak_request_position_gap_percent,final_window_flow_excursion_m3_h\nhealthy,qualified,{:.6},{:.6}\ndeadband,diagnosed,{:.6},{:.6}\nstiction,diagnosed,{:.6},{:.6}\nstiction_breakaway_probe,qualified,2.000000,\n",
        peak_gaps[0], peak_to_peak[0], peak_gaps[1], peak_to_peak[1], peak_gaps[2], peak_to_peak[2],
    );
    let manual_positions = [0.0, 20.0, 40.0, 60.0, 80.0, 100.0];
    let probe_index = (1..=probe_requests.len())
        .map(|value| value as f64)
        .collect::<Vec<_>>();
    evidence::retain(
        &evidence_root(),
        "valve-stiction-evidence",
        "Valve tests reveal deadband and stiction",
        "A manual stroke identifies the installed curve; closed-loop request-position gaps distinguish deadband and stiction.",
        &[
            evidence::Panel {
                title: "Closed-loop flow", x_unit: "time (s)", y_unit: "flow (m3/h)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(),
                y_minimum: flow_min, y_maximum: flow_max,
                series: vec![
                    evidence::Series { name: "Healthy valve", color: "#2563eb", dashed: false, x: &time, y: &flows[0] },
                    evidence::Series { name: "Deadband", color: "#b45309", dashed: false, x: &time, y: &flows[1] },
                    evidence::Series { name: "Stiction", color: "#dc2626", dashed: false, x: &time, y: &flows[2] },
                ],
            },
            evidence::Panel {
                title: "Request minus stem position", x_unit: "time (s)", y_unit: "gap (%)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(),
                y_minimum: gap_min, y_maximum: gap_max,
                series: vec![
                    evidence::Series { name: "Healthy valve", color: "#2563eb", dashed: false, x: &time, y: &gaps[0] },
                    evidence::Series { name: "Deadband", color: "#b45309", dashed: false, x: &time, y: &gaps[1] },
                    evidence::Series { name: "Stiction", color: "#dc2626", dashed: false, x: &time, y: &gaps[2] },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Installed curve", name: "Healthy measured flow", x: &manual_positions, y: &measured_curve, x_unit: "valve position (%)", y_unit: "flow (m3/h)" },
            evidence::RetainedSeries { panel: "Restored breakaway probes", name: "Requested position", x: &probe_index, y: &probe_requests, x_unit: "probe sample", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Restored breakaway probes", name: "Measured stem position", x: &probe_index, y: &probe_stiction_positions, x_unit: "probe sample", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Healthy actuator", name: "Requested position", x: &time, y: &requests[0], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Healthy actuator", name: "Measured stem position", x: &time, y: &stem_positions[0], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Deadband actuator", name: "Requested position", x: &time, y: &requests[1], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Deadband actuator", name: "Measured stem position", x: &time, y: &stem_positions[1], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Deadband actuator", name: "Deadband active", x: &time, y: &deadband_state, x_unit: "time (s)", y_unit: "state (0/1)" },
            evidence::RetainedSeries { panel: "Stiction actuator", name: "Requested position", x: &time, y: &requests[2], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Stiction actuator", name: "Measured stem position and applied input", x: &time, y: &stem_positions[2], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Stiction actuator", name: "Stiction active", x: &time, y: &stiction_state, x_unit: "time (s)", y_unit: "state (0/1)" },
        ],
        &summary,
    );
}
