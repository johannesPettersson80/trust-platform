use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const ADVANCED_SOURCE: &str = include_str!("../../../libraries/control/src/advanced_control.st");

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/10-multivariable-control/62-distillation-decoupling/figures")
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
        .join("../../examples/control/10-multivariable-control/62-distillation-decoupling")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 62 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 62 file {}: {error}", path.display()))
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-distillation-decoupling-{label}-{}-{stamp}.toml",
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

fn write_int(runtime: &mut Runtime, address: &IoAddress, value: i16) {
    runtime
        .io_mut()
        .write(address, Value::Word(value as u16))
        .expect("write INT architecture selector");
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

#[derive(Clone, Copy)]
enum Scenario {
    OverheadServo,
    BottomsServo,
    FeedDisturbance,
}

#[derive(Default)]
struct Trace {
    sp1: Vec<f64>,
    sp2: Vec<f64>,
    pv1: Vec<f64>,
    pv2: Vec<f64>,
    mv1: Vec<f64>,
    mv2: Vec<f64>,
}

#[derive(Debug)]
struct Metrics {
    cross_peak: f64,
    combined_iae: f64,
    applied_travel: f64,
    final_error: f64,
}

fn run(model_file: &str, architecture: i16, scenario: Scenario) -> Trace {
    let source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("distillation decoupling program before configuration")
        .replace("AutoMode1 : BOOL :=", "AutoMode1 AT %MX24.0 : BOOL :=")
        .replace("AutoMode2 : BOOL :=", "AutoMode2 AT %MX24.1 : BOOL :=")
        .replace("Architecture : INT :=", "Architecture AT %MW16 : INT :=")
        .replace("Setpoint1 : LREAL :=", "Setpoint1 AT %ML32 : LREAL :=")
        .replace("Setpoint2 : LREAL :=", "Setpoint2 AT %ML40 : LREAL :=")
        .replace(
            "FeedDisturbanceRequested : LREAL :=",
            "FeedDisturbanceRequested AT %ML48 : LREAL :=",
        )
        .replace(
            "RefluxApplied : LREAL :=",
            "RefluxApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "ReboilerApplied : LREAL :=",
            "ReboilerApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "FeedDisturbanceApplied : LREAL :=",
            "FeedDisturbanceApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "ProcessValue1 : LREAL :=",
            "ProcessValue1 AT %IL0 : LREAL :=",
        )
        .replace(
            "ProcessValue2 : LREAL :=",
            "ProcessValue2 AT %IL8 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("Control/advanced_control.st", ADVANCED_SOURCE),
        SourceFile::with_path("distillation-decoupling/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped distillation decoupling lesson");

    let path = temporary_config_path(model_file);
    std::fs::write(&path, project_file(model_file)).expect("write column simulation config");
    let config = SimulationConfig::load(&path).expect("load column simulation config");
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial product deviations");
    let _ = std::fs::remove_file(path);

    let auto1 = IoAddress::parse("%MX24.0").unwrap();
    let auto2 = IoAddress::parse("%MX24.1").unwrap();
    let architecture_address = IoAddress::parse("%MW16").unwrap();
    let setpoint1 = IoAddress::parse("%ML32").unwrap();
    let setpoint2 = IoAddress::parse("%ML40").unwrap();
    let disturbance = IoAddress::parse("%ML48").unwrap();
    let pv1 = IoAddress::parse("%IL0").unwrap();
    let pv2 = IoAddress::parse("%IL8").unwrap();
    let mv1 = IoAddress::parse("%QL0").unwrap();
    let mv2 = IoAddress::parse("%QL8").unwrap();
    let mut trace = Trace::default();
    for cycle in 0..2_200 {
        let event_active = cycle >= 200;
        let (sp1, sp2, load) = match scenario {
            Scenario::OverheadServo => (if event_active { 2.0 } else { 0.0 }, 0.0, 0.0),
            Scenario::BottomsServo => (0.0, if event_active { 2.0 } else { 0.0 }, 0.0),
            Scenario::FeedDisturbance => (0.0, 0.0, if event_active { 2.0 } else { 0.0 }),
        };
        write_bool(&mut runtime, &auto1, true);
        write_bool(&mut runtime, &auto2, true);
        write_int(&mut runtime, &architecture_address, architecture);
        write_lreal(&mut runtime, &setpoint1, sp1);
        write_lreal(&mut runtime, &setpoint2, sp2);
        write_lreal(&mut runtime, &disturbance, load);
        runtime
            .execute_cycle()
            .expect("execute two-loop controller");
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance coupled column");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish product deviations");
        trace.sp1.push(sp1);
        trace.sp2.push(sp2);
        trace.pv1.push(read_lreal(&runtime, &pv1));
        trace.pv2.push(read_lreal(&runtime, &pv2));
        trace.mv1.push(read_lreal(&runtime, &mv1));
        trace.mv2.push(read_lreal(&runtime, &mv2));
    }
    trace
}

fn metrics(trace: &Trace, scenario: Scenario) -> Metrics {
    let cross_peak = match scenario {
        Scenario::OverheadServo => trace.pv2[200..]
            .iter()
            .map(|value| value.abs())
            .fold(0.0, f64::max),
        Scenario::BottomsServo => trace.pv1[200..]
            .iter()
            .map(|value| value.abs())
            .fold(0.0, f64::max),
        Scenario::FeedDisturbance => 0.0,
    };
    let combined_iae = (200..trace.pv1.len())
        .map(|index| {
            ((trace.sp1[index] - trace.pv1[index]).abs()
                + (trace.sp2[index] - trace.pv2[index]).abs())
                * 0.1
        })
        .sum();
    let applied_travel = trace
        .mv1
        .windows(2)
        .chain(trace.mv2.windows(2))
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum();
    let final_error = (2_100..2_200)
        .map(|index| {
            (trace.sp1[index] - trace.pv1[index]).abs()
                + (trace.sp2[index] - trace.pv2[index]).abs()
        })
        .sum::<f64>()
        / 100.0;
    Metrics {
        cross_peak,
        combined_iae,
        applied_travel,
        final_error,
    }
}

fn assert_valid(trace: &Trace) {
    assert!(trace
        .sp1
        .iter()
        .chain(&trace.sp2)
        .chain(&trace.pv1)
        .chain(&trace.pv2)
        .chain(&trace.mv1)
        .chain(&trace.mv2)
        .all(|value| value.is_finite()));
    assert!(trace
        .pv1
        .iter()
        .chain(&trace.pv2)
        .all(|value| value.abs() <= 6.0));
    assert!(trace
        .mv1
        .iter()
        .chain(&trace.mv2)
        .all(|value| value.abs() <= 10.0));
}

#[test]
fn control_curriculum_distillation_decoupling_compares_direct_static_and_dynamic_evidence() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 62 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 62);
    assert_eq!(lesson["prerequisites"][0], "control-distillation-mimo");
    assert_eq!(lesson["nextLesson"], "control-distillation-robustness");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_distillation_decoupling_compares_direct_static_and_dynamic_evidence"
    );
    let source = project_file("src/Main.st");
    assert_eq!(source.matches("Control.PID;").count(), 2);
    assert_eq!(source.matches("Control.LowPass;").count(), 2);
    assert_eq!(source.matches("Control.StaticDecoupler2x2;").count(), 1);
    for forbidden in [
        "DirectWins",
        "StaticWins",
        "DynamicWins",
        "CrossPeak",
        "CombinedIAE",
    ] {
        assert!(
            !source.contains(forbidden),
            "ST embeds analytical verdict '{forbidden}'"
        );
    }

    let nominal_traces: Vec<[Trace; 3]> = (0..=2)
        .map(|architecture| {
            [
                run("simulation.toml", architecture, Scenario::OverheadServo),
                run("simulation.toml", architecture, Scenario::BottomsServo),
                run("simulation.toml", architecture, Scenario::FeedDisturbance),
            ]
        })
        .collect();
    for traces in &nominal_traces {
        traces.iter().for_each(assert_valid);
    }
    let nominal_overhead: Vec<_> = nominal_traces
        .iter()
        .map(|traces| metrics(&traces[0], Scenario::OverheadServo))
        .collect();
    let nominal_bottoms: Vec<_> = nominal_traces
        .iter()
        .map(|traces| metrics(&traces[1], Scenario::BottomsServo))
        .collect();
    let nominal_load: Vec<_> = nominal_traces
        .iter()
        .map(|traces| metrics(&traces[2], Scenario::FeedDisturbance))
        .collect();
    println!(
        "nominal overhead={nominal_overhead:?}; bottoms={nominal_bottoms:?}; load={nominal_load:?}"
    );
    for architecture in 0..=2 {
        assert!(nominal_overhead[architecture as usize].final_error <= 0.05);
        assert!(nominal_bottoms[architecture as usize].final_error <= 0.05);
        assert!(nominal_load[architecture as usize].final_error <= 0.05);
    }
    for metrics_by_architecture in [&nominal_overhead, &nominal_bottoms] {
        assert!(metrics_by_architecture[1].cross_peak < metrics_by_architecture[0].cross_peak);
        assert!(metrics_by_architecture[2].cross_peak < metrics_by_architecture[1].cross_peak);
        assert!(metrics_by_architecture[2].cross_peak <= 0.05);
        assert!(metrics_by_architecture[2].combined_iae <= metrics_by_architecture[0].combined_iae);
        assert!(metrics_by_architecture[2].combined_iae <= 100.0);
        assert!(metrics_by_architecture[2].applied_travel <= 6.0);
        assert!(
            metrics_by_architecture[2].applied_travel
                <= metrics_by_architecture[1].applied_travel * 1.5
        );
    }
    assert!(nominal_load
        .iter()
        .all(|metrics| metrics.combined_iae <= 40.0));

    let perturbed_direct_trace = run("simulation-perturbed.toml", 0, Scenario::OverheadServo);
    let perturbed_static_trace = run("simulation-perturbed.toml", 1, Scenario::OverheadServo);
    let perturbed_dynamic_trace = run("simulation-perturbed.toml", 2, Scenario::OverheadServo);
    assert_valid(&perturbed_direct_trace);
    assert_valid(&perturbed_static_trace);
    assert_valid(&perturbed_dynamic_trace);
    let perturbed_direct = metrics(&perturbed_direct_trace, Scenario::OverheadServo);
    let perturbed_static = metrics(&perturbed_static_trace, Scenario::OverheadServo);
    let perturbed_dynamic = metrics(&perturbed_dynamic_trace, Scenario::OverheadServo);
    assert!(perturbed_static.cross_peak < perturbed_direct.cross_peak);
    assert!(perturbed_static.final_error <= 0.1);
    assert!(perturbed_dynamic.final_error <= 0.1);
    assert!(
        perturbed_dynamic.cross_peak >= nominal_overhead[2].cross_peak * 3.0,
        "model mismatch must visibly degrade exact dynamic cancellation"
    );
    let dynamic_qualified = perturbed_dynamic.cross_peak <= perturbed_static.cross_peak * 0.8
        && perturbed_dynamic.combined_iae <= perturbed_static.combined_iae * 1.05
        && perturbed_dynamic.applied_travel <= perturbed_static.applied_travel * 1.25;
    assert!(
        !dynamic_qualified,
        "declared mismatch must reject nominal dynamic cancellation"
    );
    println!(
        "nominal overhead cross direct/static/dynamic={:.6}/{:.6}/{:.6}, bottoms={:.6}/{:.6}/{:.6}, load IAE={:.6}/{:.6}/{:.6}; perturbed direct/static/dynamic cross={:.6}/{:.6}/{:.6}, static/dynamic IAE={:.6}/{:.6}, travel={:.6}/{:.6}, dynamic qualified={dynamic_qualified}, selected fallback=static",
        nominal_overhead[0].cross_peak,
        nominal_overhead[1].cross_peak,
        nominal_overhead[2].cross_peak,
        nominal_bottoms[0].cross_peak,
        nominal_bottoms[1].cross_peak,
        nominal_bottoms[2].cross_peak,
        nominal_load[0].combined_iae,
        nominal_load[1].combined_iae,
        nominal_load[2].combined_iae,
        perturbed_direct.cross_peak,
        perturbed_static.cross_peak,
        perturbed_dynamic.cross_peak,
        perturbed_static.combined_iae,
        perturbed_dynamic.combined_iae,
        perturbed_static.applied_travel,
        perturbed_dynamic.applied_travel,
    );

    let time = (0..nominal_traces[0][0].pv1.len())
        .map(|sample| sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let (main_min, main_max) =
        plot_bounds(nominal_traces.iter().map(|traces| traces[0].pv1.as_slice()));
    let (cross_min, cross_max) =
        plot_bounds(nominal_traces.iter().map(|traces| traces[0].pv2.as_slice()));
    let summary = format!(
        "case,status,overhead_cross_peak_pp,overhead_combined_iae_pp_s,overhead_applied_travel_pp,perturbed_cross_peak_pp\ndirect,rejected,{:.6},{:.6},{:.6},{:.6}\nstatic_decoupling,selected,{:.6},{:.6},{:.6},{:.6}\ndynamic_decoupling,nominal_only,{:.6},{:.6},{:.6},{:.6}\n",
        nominal_overhead[0].cross_peak, nominal_overhead[0].combined_iae, nominal_overhead[0].applied_travel, perturbed_direct.cross_peak,
        nominal_overhead[1].cross_peak, nominal_overhead[1].combined_iae, nominal_overhead[1].applied_travel, perturbed_static.cross_peak,
        nominal_overhead[2].cross_peak, nominal_overhead[2].combined_iae, nominal_overhead[2].applied_travel, perturbed_dynamic.cross_peak,
    );
    evidence::retain(
        &evidence_root(),
        "distillation-decoupling-evidence",
        "Decoupling reduces unwanted composition movement",
        "Dynamic cancellation is strongest for the nominal model; static decoupling remains credible after the declared mismatch.",
        &[
            evidence::Panel {
                title: "Controlled overhead response", x_unit: "time (s)", y_unit: "composition deviation (pp)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(), y_minimum: main_min, y_maximum: main_max,
                series: vec![
                    evidence::Series { name: "Direct", color: "#6b7280", dashed: false, x: &time, y: &nominal_traces[0][0].pv1 },
                    evidence::Series { name: "Static", color: "#059669", dashed: false, x: &time, y: &nominal_traces[1][0].pv1 },
                    evidence::Series { name: "Dynamic", color: "#2563eb", dashed: false, x: &time, y: &nominal_traces[2][0].pv1 },
                ],
            },
            evidence::Panel {
                title: "Unwanted bottoms movement", x_unit: "time (s)", y_unit: "composition deviation (pp)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(), y_minimum: cross_min, y_maximum: cross_max,
                series: vec![
                    evidence::Series { name: "Direct", color: "#6b7280", dashed: false, x: &time, y: &nominal_traces[0][0].pv2 },
                    evidence::Series { name: "Static", color: "#059669", dashed: false, x: &time, y: &nominal_traces[1][0].pv2 },
                    evidence::Series { name: "Dynamic", color: "#2563eb", dashed: false, x: &time, y: &nominal_traces[2][0].pv2 },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Static manipulated variables", name: "Reflux applied", x: &time, y: &nominal_traces[1][0].mv1, x_unit: "time (s)", y_unit: "command (pp)" },
            evidence::RetainedSeries { panel: "Static manipulated variables", name: "Reboiler applied", x: &time, y: &nominal_traces[1][0].mv2, x_unit: "time (s)", y_unit: "command (pp)" },
            evidence::RetainedSeries { panel: "Perturbed cross response", name: "Direct", x: &time, y: &perturbed_direct_trace.pv2, x_unit: "time (s)", y_unit: "composition deviation (pp)" },
            evidence::RetainedSeries { panel: "Perturbed cross response", name: "Static", x: &time, y: &perturbed_static_trace.pv2, x_unit: "time (s)", y_unit: "composition deviation (pp)" },
            evidence::RetainedSeries { panel: "Perturbed cross response", name: "Dynamic", x: &time, y: &perturbed_dynamic_trace.pv2, x_unit: "time (s)", y_unit: "composition deviation (pp)" },
        ],
        &summary,
    );
}
