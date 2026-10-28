//! Curriculum 38: execute the delivered ON/OFF lesson without hidden setup writes.
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::{compile, LessonRuntime};
const PROJECT: &str =
    "../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off";
const MAIN: &str = include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/src/Main.st");
fn number(runtime: &CompiledRuntime, address: &str) -> f64 {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        Value::LReal(value) => value,
        Value::DWord(value) => value as f64,
        other => panic!("unexpected {address}: {other:?}"),
    }
}
#[test]
fn delivered_on_off_experiment_starts_switches_and_stops() {
    for preparation in [0, 100] {
        let session =
            CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", MAIN)]);
        let mut runtime = compile(session);
        let debug = runtime.enable_debug();
        let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PROJECT);
        let mut simulation = SimulationController::new(
            SimulationConfig::load(project.join("simulation.toml")).unwrap(),
        );
        let mut csv = String::from(
            "simulation_time_s,experiment_time_s,pv_degC,command_percent,door_open,switch_count\n",
        );
        let mut changes = 0;
        let mut previous = 0.0;
        for scan in 0..preparation + 1202 {
            simulation
                .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%MX16.2").unwrap(),
                Value::Bool(scan >= preparation && scan < preparation + 1200),
            );
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            let pv = number(&runtime, "%IL0");
            let mv = number(&runtime, "%QL0");
            let door = number(&runtime, "%IL8");
            assert_eq!(
                number(&runtime, "%ML0"),
                2.0,
                "fixed target must not require a hidden memory write"
            );
            if scan < preparation {
                assert!((pv - 4.0).abs() < 1e-8);
                assert_eq!(mv, 0.0);
            } else if scan < preparation + 1200 {
                assert_eq!(
                    mv,
                    if pv >= 2.0 { 100.0 } else { 0.0 },
                    "feedback decision at scan {scan}"
                );
            } else {
                assert_eq!(mv, 0.0, "timeout disable must stop refrigeration");
            }
            if mv != previous {
                changes += 1;
                previous = mv;
            }
            assert_eq!(
                number(&runtime, "%MD24"),
                changes as f64,
                "switch count must count actual transitions"
            );
            if (602..898).contains(&(scan - preparation)) {
                assert_eq!(door, 1.0);
            }
            if scan < preparation + 598 || scan > preparation + 902 {
                assert_eq!(door, 0.0);
            }
            csv.push_str(&format!(
                "{:.1},{:.1},{pv:.12},{mv},{door},{changes}\n",
                scan as f64 * 0.1,
                (scan - preparation) as f64 * 0.1
            ));
            simulation
                .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                .unwrap();
        }
        assert!(
            changes > 500,
            "must demonstrate frequent switching, observed {changes}"
        );
        if let Ok(dir) = std::env::var("TRUST_LESSON2_EVIDENCE") {
            std::fs::write(
                std::path::Path::new(&dir).join(format!("preparation-{preparation}.csv")),
                csv,
            )
            .unwrap();
        }
    }
}

#[test]
fn learner_abort_stops_cooling_without_automatic_range_trip() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", MAIN)]);
    let mut runtime = compile(session);
    let debug = runtime.enable_debug();
    for abort in [false, true, true] {
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(10.1_f64.to_bits()),
        );
        debug.enqueue_io_write(IoAddress::parse("%MX16.2").unwrap(), Value::Bool(true));
        debug.enqueue_io_write(IoAddress::parse("%MX16.3").unwrap(), Value::Bool(abort));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            number(&runtime, "%QL0"),
            if abort { 0.0 } else { 100.0 },
            "README specifies learner abort, not an automatic temperature-range trip"
        );
    }
}
