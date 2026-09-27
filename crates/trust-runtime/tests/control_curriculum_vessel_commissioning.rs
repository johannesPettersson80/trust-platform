use std::path::Path;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

fn number(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("{address}: {other:?}"),
    }
}
fn setup(cascade: bool) -> (Runtime, SimulationController) {
    setup_target(cascade, false)
}
fn setup_target(cascade: bool, limited_request: bool) -> (Runtime, SimulationController) {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/83-reactor-control");
    let mut sources = vec![];
    for entry in std::fs::read_dir(project.join("src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|x| x.to_str()) != Some("st")
            || path.file_name().unwrap() == "tests.st"
        {
            continue;
        }
        let mut source = std::fs::read_to_string(&path).unwrap().replace(
            "UseCascade : BOOL := TRUE;",
            if cascade {
                "UseCascade : BOOL := TRUE;"
            } else {
                "UseCascade : BOOL := FALSE;"
            },
        );
        if limited_request {
            source = source.replace("SetpointC := LREAL#37.0;", "SetpointC := LREAL#100.0;");
        }
        sources.push(SourceFile::with_path(path.to_string_lossy(), source));
    }
    sources.push(SourceFile::with_path(
        "Control/control.st",
        std::fs::read_to_string(project.join("libraries/control/src/control.st")).unwrap(),
    ));
    let session = CompileSession::from_sources(sources);
    let mut runtime = session.build_runtime().unwrap();
    runtime
        .apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
        .unwrap();
    (
        runtime,
        SimulationController::new(
            SimulationConfig::load(&project.join("simulation.toml")).unwrap(),
        ),
    )
}

#[test]
fn vessel_commissioning_uses_the_taught_energy_balance_and_two_matched_arrangements() {
    for cascade in [false, true] {
        for trial in [0_u16, 1] {
            let (mut runtime, mut simulation) = setup(cascade);
            let debug = runtime.enable_debug();
            let mut peak = 35.0_f64;
            let mut minimum = 35.0_f64;
            let mut late_error = 0.0_f64;
            for scan in 0..602 {
                simulation
                    .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                    .unwrap();
                debug.enqueue_io_write(
                    IoAddress::parse("%MX240.5").unwrap(),
                    Value::Bool(scan > 0 && scan <= 600),
                );
                debug.enqueue_io_write(IoAddress::parse("%MW192").unwrap(), Value::Word(trial));
                runtime.advance_time(Duration::from_millis(100));
                runtime.execute_cycle().unwrap();
                if scan == 0 {
                    assert_eq!(
                        number(&runtime, "%IL0"),
                        35.0,
                        "vessel reset must be35°C, not80°C"
                    );
                    assert_eq!(number(&runtime, "%IL8"), 38.0);
                    assert_eq!(number(&runtime, "%QL0"), 15.0);
                }
                if scan > 0 && scan <= 600 {
                    assert_eq!(
                        number(&runtime, "%QL8"),
                        if trial == 0 { 0.0 } else { 30.0 }
                    );
                    let vessel = number(&runtime, "%IL0");
                    let jacket = number(&runtime, "%IL8");
                    assert!((30.0..=40.0).contains(&vessel) && (20.0..=55.0).contains(&jacket));
                    assert!((0.0..=100.0).contains(&number(&runtime, "%QL0")));
                    peak = peak.max(vessel);
                    minimum = minimum.min(vessel);
                    if scan >= 301 {
                        late_error =
                            late_error.max((vessel - if trial == 0 { 37.0 } else { 35.0 }).abs());
                    }
                    if trial == 1 && scan == 2 {
                        assert!(
                            jacket < 37.6,
                            "30kW jacket extraction must affect the5kJ/°C jacket first"
                        );
                        assert!(vessel > 34.98);
                    }
                }
                if scan == 601 {
                    assert_eq!(number(&runtime, "%QL8"), 0.0);
                }
                simulation
                    .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                    .unwrap();
            }
            eprintln!("cascade={cascade} trial={trial}: peak={peak:.5} minimum={minimum:.5} lateError={late_error:.5}");
        }
    }
}

#[test]
fn vessel_commissioning_manual_transfer_and_invalid_measurement_recovery_are_separate() {
    for cascade in [false, true] {
        let (mut runtime, mut simulation) = setup(cascade);
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        for manual in [true, false] {
            for (address, value) in [
                ("%MX240.5", Value::Bool(true)),
                ("%MX240.4", Value::Bool(manual)),
                ("%MW192", Value::Word(2)),
            ] {
                debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value);
            }
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            assert_eq!(
                number(&runtime, "%QL0"),
                15.0,
                "manual→auto at unchanged35/38°C must preserve15% holding output"
            );
        }
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(29.0_f64.to_bits()),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(number(&runtime, "%QL0"), 0.0);
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(35.0_f64.to_bits()),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            number(&runtime, "%QL0"),
            0.0,
            "a recovered reading cannot clear the abort latch"
        );
    }
}

#[test]
fn vessel_commissioning_retains_correct_units_and_tracks_a_deliberately_limited_request() {
    // A one-scan controller check with an intentionally unreachable target,
    // not a qualified physical experiment or a changed commissioning requirement.
    for cascade in [false, true] {
        let (mut runtime, mut simulation) = setup_target(cascade, true);
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX240.5").unwrap(), Value::Bool(true));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        let applied = number(&runtime, "%QL0");
        assert!((0.0..=100.0).contains(&applied));
        assert!(
            (number(&runtime, "%ML16") + number(&runtime, "%ML24") - number(&runtime, "%ML40"))
                .abs()
                < 1e-9,
            "actual retained heater I plus P equals nominal percent output"
        );
        if cascade {
            assert_eq!(
                number(&runtime, "%ML8"),
                50.0,
                "outer output is a limited jacket temperature in°C"
            );
        } else {
            assert_eq!(applied, 100.0);
            assert!(
                number(&runtime, "%ML16") < 15.0,
                "tracking reduces retained heater integral after saturation"
            );
        }
    }
}
