use std::path::Path;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;
fn number(r: &Runtime, a: &str) -> f64 {
    match r.io().read(&IoAddress::parse(a).unwrap()).unwrap() {
        Value::LWord(b) => f64::from_bits(b),
        v => panic!("{a}: {v:?}"),
    }
}
#[test]
fn cold_room_core_assessment_uses_eight_second_dwell_and_only_the_declared_door_load() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/80-cold-storage");
    let mut trace = String::from("case,time_s,pv_degC,command_percent,surroundings_degC\n");
    for (close_scan, preparation) in [(900, 0), (900, 100), (1050, 0)] {
        let source = std::fs::read_to_string(p.join("src/Main.st"))
            .unwrap()
            .replace(
                "DoorCloseScan : DINT := DINT#900;",
                &format!("DoorCloseScan : DINT := DINT#{close_scan};"),
            );
        let session =
            CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", source)]);
        let mut r = session.build_runtime().unwrap();
        r.apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
            .unwrap();
        let debug = r.enable_debug();
        let mut sim =
            SimulationController::new(SimulationConfig::load(p.join("simulation.toml")).unwrap());
        let mut previous = 0.0;
        let mut last_change = None;
        for scan in 0..preparation + 1202 {
            let experiment_scan = scan - preparation;
            sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
                .unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%MX16.6").unwrap(),
                Value::Bool((0..1200).contains(&experiment_scan)),
            );
            r.advance_time(Duration::from_millis(100));
            r.execute_cycle().unwrap();
            if experiment_scan == 0 {
                assert_eq!(number(&r, "%IL0"), 4.0, "same4°C cold-room reset");
                assert_eq!(
                    number(&r, "%QL0"),
                    100.0,
                    "initial OFF dwell is already qualified"
                );
            }
            let applied = number(&r, "%QL0");
            assert!(applied == 0.0 || applied == 100.0);
            if preparation == 0 && (0..1200).contains(&experiment_scan) {
                trace.push_str(&format!(
                    "{},{:.1},{:.12},{applied},{}\n",
                    if close_scan == 900 {
                        "baseline"
                    } else {
                        "door-open-to-105-s"
                    },
                    experiment_scan as f64 * 0.1,
                    number(&r, "%IL0"),
                    number(&r, "%QL8"),
                ));
            }
            if applied != previous && (0..1200).contains(&experiment_scan) {
                if let Some(last) = last_change {
                    assert!(
                        scan - last >= 80,
                        "every normal completed interval must be at least8s"
                    );
                }
                last_change = Some(scan);
                previous = applied;
            }
            if (400..1200).contains(&experiment_scan) {
                assert!(
                    (1.0..=3.0).contains(&number(&r, "%IL0")),
                    "declared product band: t={}s close_scan={close_scan} PV={}",
                    experiment_scan as f64 * 0.1,
                    number(&r, "%IL0")
                );
            }
            if experiment_scan >= 602 && experiment_scan < close_scan - 2 {
                assert_eq!(number(&r, "%QL8"), 6.0);
            }
            if experiment_scan > close_scan + 2 {
                assert_eq!(number(&r, "%QL8"), 4.0);
            }
            if experiment_scan >= 1200 {
                assert_eq!(applied, 0.0);
            }
            sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
                .unwrap();
        }
    }
    if let Ok(path) = std::env::var("TRUST_LESSON4_TRACE") {
        std::fs::write(path, trace).expect("retain executed simulator trace");
    }
}
