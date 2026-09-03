use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/src/Main.st");
const SIMULATION_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/simulation.toml");

#[derive(Clone, Copy)]
struct Sample {
    time_s: f64,
    pv: f64,
    requested_mv: f64,
    applied_mv: f64,
}

fn final_window(trace: &[Sample]) -> &[Sample] {
    &trace[1_100..]
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-water-heater-p-figure-{}-{stamp}.toml",
        std::process::id()
    ))
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

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL32 : LREAL :=")
        .replace(
            "ProportionalGain : LREAL :=",
            "ProportionalGain AT %IL8 : LREAL :=",
        )
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX24.0 : BOOL :=",
        )
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "RequestedManipulatedValue : LREAL :=",
            "RequestedManipulatedValue AT %ML16 : LREAL :=",
        )
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        )
}

#[test]
fn control_curriculum_water_heater_p_applies_live_gain_and_exposes_both_outputs() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-p/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete P-control lesson");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let kp = IoAddress::parse("%IL8").expect("Kp address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let requested_mv = IoAddress::parse("%ML16").expect("requested MV address");
    let applied_mv = IoAddress::parse("%QL0").expect("applied MV address");

    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("execute safe initialization scan");
    write_lreal(&mut runtime, &sp, 35.0);
    write_lreal(&mut runtime, &pv, 20.0);
    write_lreal(&mut runtime, &kp, 2.0);
    write_bool(&mut runtime, &auto, true);
    runtime
        .execute_cycle()
        .expect("execute initial P-control scan");
    let initial_requested = read_lreal(&runtime, &requested_mv);
    let initial_applied = read_lreal(&runtime, &applied_mv);
    assert!(
        (initial_requested - 30.0).abs() <= 1.0e-9,
        "initial requested MV must be 30 percent, got {initial_requested}"
    );
    assert!(
        (initial_applied - 30.0).abs() <= 1.0e-9,
        "initial applied MV must be 30 percent, got {initial_applied}"
    );

    write_lreal(&mut runtime, &kp, 8.0);
    runtime
        .execute_cycle()
        .expect("execute scan after live gain change");
    assert_eq!(read_lreal(&runtime, &requested_mv), 120.0);
    assert_eq!(read_lreal(&runtime, &applied_mv), 100.0);
}

fn run_lesson(proportional_gain: f64) -> Vec<Sample> {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-p/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete P-control lesson");
    let config_path = temporary_config_path();
    std::fs::write(&config_path, SIMULATION_SOURCE).expect("write simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load simulation config"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let kp = IoAddress::parse("%IL8").expect("Kp address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let requested_mv = IoAddress::parse("%ML16").expect("requested MV address");
    let applied_mv = IoAddress::parse("%QL0").expect("applied MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial state");
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("execute safe initialization scan");

    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &sp, 35.0);
        write_lreal(&mut runtime, &kp, proportional_gain);
        write_bool(&mut runtime, &auto, true);
        runtime.execute_cycle().expect("execute P controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance heater plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish heater PV");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            pv: read_lreal(&runtime, &pv),
            requested_mv: read_lreal(&runtime, &requested_mv),
            applied_mv: read_lreal(&runtime, &applied_mv),
        });
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

fn export_csv(directory: &std::path::Path, low: &[Sample], high: &[Sample], aggressive: &[Sample]) {
    std::fs::create_dir_all(directory).expect("create figure directory");
    let mut csv = String::from(
        "time_s,setpoint_degC,load_event,low_kp_pv_degC,low_kp_mv_requested_percent,low_kp_mv_applied_percent,high_kp_pv_degC,high_kp_mv_requested_percent,high_kp_mv_applied_percent,aggressive_kp_pv_degC,aggressive_kp_mv_requested_percent,aggressive_kp_mv_applied_percent\n",
    );
    for ((low_sample, high_sample), aggressive_sample) in low.iter().zip(high).zip(aggressive) {
        assert!((low_sample.time_s - high_sample.time_s).abs() < f64::EPSILON);
        assert!((low_sample.time_s - aggressive_sample.time_s).abs() < f64::EPSILON);
        let event = if (low_sample.time_s - 60.0).abs() < 0.05 {
            "cold_water_flow_increase"
        } else {
            ""
        };
        writeln!(
            csv,
            "{:.1},35.0,{event},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            low_sample.time_s,
            low_sample.pv,
            low_sample.requested_mv,
            low_sample.applied_mv,
            high_sample.pv,
            high_sample.requested_mv,
            high_sample.applied_mv,
            aggressive_sample.pv,
            aggressive_sample.requested_mv,
            aggressive_sample.applied_mv,
        )
        .expect("write CSV row");
    }
    std::fs::write(directory.join("p-gain-comparison.csv"), csv)
        .expect("write retained simulation evidence");
}

#[test]
fn control_curriculum_water_heater_p_exports_reproducible_plot_evidence() {
    let low = run_lesson(2.0);
    let high = run_lesson(8.0);
    let aggressive = run_lesson(40.0);
    assert_eq!(low.len(), 1_200);
    assert_eq!(high.len(), 1_200);
    assert_eq!(aggressive.len(), 1_200);
    assert!((low[0].requested_mv - 30.0).abs() <= 1.0e-9);
    assert!((low[0].applied_mv - 30.0).abs() <= 1.0e-9,
        "Kp 2 must apply the predicted 2 * (35 - 20) = 30 percent without an unexplained rate limit; got {}", low[0].applied_mv);
    assert!((high[0].requested_mv - 120.0).abs() <= 1.0e-9);
    assert!((high[0].applied_mv - 100.0).abs() <= 1.0e-9,
        "Kp 8 must clamp the predicted 120 percent request at the taught 100 percent amplitude limit; got {}", high[0].applied_mv);
    let range = |trace: &[Sample], field: fn(Sample) -> f64| {
        let minimum = trace
            .iter()
            .map(|sample| field(*sample))
            .fold(f64::INFINITY, f64::min);
        let maximum = trace
            .iter()
            .map(|sample| field(*sample))
            .fold(f64::NEG_INFINITY, f64::max);
        maximum - minimum
    };
    let settled = |trace: &[Sample]| {
        range(final_window(trace), |sample| sample.pv) <= 0.1
            && range(final_window(trace), |sample| sample.applied_mv) <= 1.0
    };
    let mean_absolute_error = |trace: &[Sample]| {
        final_window(trace)
            .iter()
            .map(|sample| (35.0 - sample.pv).abs())
            .sum::<f64>()
            / final_window(trace).len() as f64
    };
    assert!(
        settled(&low),
        "Kp 2 must qualify as settled before error is reported"
    );
    assert!(
        settled(&high),
        "Kp 8 must qualify as settled before error is reported"
    );
    assert!(
        !settled(&aggressive),
        "Kp 40 must be reported as not settled"
    );
    let low_mean_error = mean_absolute_error(&low);
    let high_mean_error = mean_absolute_error(&high);
    assert!(
        high_mean_error < low_mean_error,
        "settled final-window error must improve: low={low_mean_error}, high={high_mean_error}"
    );
    let total_variation = |trace: &[Sample], field: fn(Sample) -> f64| -> f64 {
        trace[600..]
            .windows(2)
            .map(|pair| (field(pair[1]) - field(pair[0])).abs())
            .sum()
    };
    let high_pv_travel = total_variation(&high, |sample| sample.pv);
    let aggressive_pv_travel = total_variation(&aggressive, |sample| sample.pv);
    let high_mv_travel = total_variation(&high, |sample| sample.applied_mv);
    let aggressive_mv_travel = total_variation(&aggressive, |sample| sample.applied_mv);
    println!(
        "settled low/high/aggressive={}/{}/{}, final-window MAE low/high={low_mean_error:.6}/{high_mean_error:.6}degC, post-event PV travel low/high/aggressive={:.6}/{high_pv_travel:.6}/{aggressive_pv_travel:.6}degC, applied-MV travel low/high/aggressive={:.6}/{high_mv_travel:.6}/{aggressive_mv_travel:.6}pp",
        settled(&low),
        settled(&high),
        settled(&aggressive),
        total_variation(&low, |sample| sample.pv),
        total_variation(&low, |sample| sample.applied_mv),
    );
    assert!(high_pv_travel <= 3.0,
        "the selected Kp 8 candidate must satisfy the declared 3 degC PV-travel limit; got {high_pv_travel}");
    assert!(high_mv_travel <= 20.0,
        "the selected Kp 8 candidate must satisfy the declared 20 percent MV-travel limit; got {high_mv_travel}");
    assert!(aggressive_pv_travel >= 10.0 * high_pv_travel,
        "Kp 40 must expose reduced damping relative to Kp 8; PV travel high={high_pv_travel}, aggressive={aggressive_pv_travel}");
    assert!(aggressive_mv_travel >= 50.0 * high_mv_travel,
        "the reduced-damping case must expose its actuator cost; MV travel high={high_mv_travel}, aggressive={aggressive_mv_travel}");
    assert!(low.iter().chain(&high).chain(&aggressive).all(|sample| {
        sample.pv.is_finite()
            && sample.requested_mv.is_finite()
            && sample.applied_mv.is_finite()
            && (0.0..=100.0).contains(&sample.applied_mv)
    }));

    if let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR") {
        export_csv(&PathBuf::from(directory), &low, &high, &aggressive);
    }
}
