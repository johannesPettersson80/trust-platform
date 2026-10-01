//! Curriculum 38: delivered hysteresis and dwell sequence, with no hidden writes.
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;
const PROJECT:&str="../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell";
const MAIN:&str=include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/src/Main.st");
fn number(r: &Runtime, a: &str) -> f64 {
    match r.io().read(&IoAddress::parse(a).unwrap()).unwrap() {
        Value::LWord(b) => f64::from_bits(b),
        Value::DWord(v) => v as f64,
        Value::Bool(v) => {
            if v {
                1.0
            } else {
                0.0
            }
        }
        v => panic!("{a}: {v:?}"),
    }
}
#[test]
fn delivered_dwell_runs_without_hidden_setup() {
    for (width, dwell, preparation) in
        [(1.0, 8.0, 0), (1.0, 8.0, 100), (1.0, 0.0, 0), (0.0, 0.0, 0)]
    {
        // Explicit comparison builds change only the fixed width and minimum times.
        // They are executed PLC programs, not constructed expected response curves.
        let source = MAIN
            .replace(
                "HysteresisWidth := LREAL#1.0;",
                &format!("HysteresisWidth := LREAL#{width:.1};"),
            )
            .replace(
                "MinimumOnTime := LREAL#8.0;",
                &format!("MinimumOnTime := LREAL#{dwell:.1};"),
            )
            .replace(
                "MinimumOffTime := LREAL#8.0;",
                &format!("MinimumOffTime := LREAL#{dwell:.1};"),
            );
        let session =
            CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", source)]);
        let mut r = session.build_runtime().unwrap();
        r.apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
            .unwrap();
        let debug = r.enable_debug();
        let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PROJECT);
        let mut sim = SimulationController::new(
            SimulationConfig::load(project.join("simulation.toml")).unwrap(),
        );
        let mut last = 0.0;
        let mut last_change = None;
        let mut changes = 0;
        let mut inhibited = 0;
        let mut demand = false;
        let mut csv=String::from("time_s,pv_degC,requested,applied,dwell_inhibit,dwell_elapsed_s,switch_count,door_open\n");
        for scan in 0..preparation + 1202 {
            sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
                .unwrap();
            let enabled = scan >= preparation && scan < preparation + 1200;
            debug.enqueue_io_write(IoAddress::parse("%MX44.4").unwrap(), Value::Bool(enabled));
            r.advance_time(Duration::from_millis(100));
            r.execute_cycle().unwrap();
            let pv = number(&r, "%IL0");
            let applied = number(&r, "%MX44.2");
            let request = number(&r, "%MX44.1");
            let inhibit = number(&r, "%MX44.3");
            assert_eq!(
                [
                    number(&r, "%ML0"),
                    number(&r, "%ML8"),
                    number(&r, "%ML16"),
                    number(&r, "%ML24")
                ],
                [2.0, width, dwell, dwell],
                "fixed experiment settings must be published without hidden writes"
            );
            if enabled {
                if pv >= 2.0 + width * 0.5 {
                    demand = true;
                } else if pv <= 2.0 - width * 0.5 {
                    demand = false;
                }
                assert_eq!(request, if demand { 1.0 } else { 0.0 });
            } else {
                demand = false;
                assert_eq!(request, 0.0);
                assert_eq!(applied, 0.0);
            }
            if scan == preparation {
                assert_eq!(applied, 1.0, "initial OFF dwell is already complete");
            }
            if scan < preparation {
                assert!((pv - 4.0).abs() < 1e-8);
            }
            assert_eq!(number(&r, "%QL0"), applied * 100.0);
            if last != applied {
                if enabled {
                    if let Some(previous) = last_change {
                        assert!(
                            (scan - previous) as f64 >= dwell * 10.0,
                            "normal transition before 8 seconds"
                        );
                    }
                }
                last_change = Some(scan);
                changes += 1;
                last = applied;
            }
            assert_eq!(number(&r, "%MD40"), changes as f64);
            assert_eq!(inhibit, if request != applied { 1.0 } else { 0.0 });
            if inhibit == 1.0 {
                inhibited += 1;
            }
            let elapsed = number(&r, "%ML32");
            let door = number(&r, "%IL8");
            if scan < preparation + 598 || scan > preparation + 902 {
                assert_eq!(door, 0.0, "door clock starts with experiment authority");
            }
            if (602..898).contains(&(scan - preparation)) {
                assert_eq!(door, 1.0);
            }
            csv.push_str(&format!(
                "{:.1},{pv:.12},{request},{applied},{inhibit},{elapsed:.3},{changes},{door}\n",
                scan as f64 * 0.1
            ));
            sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
                .unwrap();
        }
        if dwell > 0.0 {
            assert!(inhibited > 0, "dwell must visibly postpone a request");
        } else {
            assert_eq!(inhibited, 0);
        }
        assert_eq!(changes, if width == 0.0 { 760 } else { 12 });
        if let Ok(dir) = std::env::var("TRUST_LESSON3_EVIDENCE") {
            std::fs::write(
                std::path::Path::new(&dir).join(format!(
                    "width-{width:.1}-dwell-{dwell:.1}-preparation-{preparation}.csv"
                )),
                csv,
            )
            .unwrap();
        }
    }
}
