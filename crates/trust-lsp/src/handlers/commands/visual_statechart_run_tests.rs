//! The statechart companion executed (specification 17, "Statechart"): a chart's companion and
//! runtime entry, as `trust-lsp.visualCompanion` answers them, compiled and scanned on the runtime
//! at the entry's 20 ms. A transition with `after` fires when its state has been active that long:
//! its `TON` runs once per scan with `IN` TRUE while the state is active, so the call in the first
//! scan of the state samples the rising edge at `ET` T#0s and each later scan adds 20 ms
//! (specification 08 §5; `docs/IEC_DECISIONS.md` 2026-10-30); a transition that fires resets the
//! timers of its state, so that a self-transition starts them again.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// `Off` turns `On` after 100 ms; `On` repeats its own `TICK` every 60 ms of being active and
/// turns `Off` on the event `GO`.
const LAMP: &str = r#"{
  "id": "lamp",
  "initial": "Off",
  "states": {
    "Off": { "on": { "WAIT": { "target": "On", "after": 100 } } },
    "On": { "on": { "GO": "Off", "TICK": { "target": "On", "after": 60 } } }
  }
}"#;

struct Run {
    harness: TestHarness,
}

impl Run {
    fn new() -> Self {
        let files = trust_visual::companion::companion("lamp.statechart.json", LAMP)
            .unwrap_or_else(|error| panic!("a companion: {error:?}"));
        let harness = TestHarness::from_sources(&[
            files.companion_text.as_str(),
            files.runtime_entry_text.as_str(),
        ])
        .unwrap_or_else(|error| {
            panic!(
                "the companion compiles: {error}\n{}\n{}",
                files.companion_text, files.runtime_entry_text
            )
        });
        Self { harness }
    }

    /// One activation of the runtime entry's 20 ms task.
    fn scan(&mut self) {
        self.harness.advance_time(Duration::from_millis(20));
        let result = self.harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    fn value(&self, name: &str) -> Value {
        let path = format!("fb_lamp.{name}");
        self.harness
            .try_get_output(&path)
            .unwrap_or_else(|error| panic!("{path}: {error:?}"))
    }

    fn state(&self) -> String {
        match self.value("STATE_NAME") {
            Value::String(name) => name.to_string(),
            other => panic!("STATE_NAME is {other:?}"),
        }
    }

    /// The elapsed time of the `TICK` timer of `On`, in milliseconds.
    fn tick_elapsed(&self) -> i64 {
        match self.value("sc_timer_On_TICK_1.ET") {
            Value::Time(elapsed) => elapsed.as_millis(),
            other => panic!("the TICK timer's ET is {other:?}"),
        }
    }
}

/// Worked by hand, scan n at n × 20 ms: `Off`'s timer starts at `ET` 0 in scan 1 and reaches its
/// 100 ms in scan 6, which turns `On`. `On`'s `TICK` timer starts in scan 7 and reaches 60 ms in
/// scan 10, where the self-transition fires and resets it; it starts again in scan 11 and fires in
/// scan 14. `GO` in scan 15 turns `Off`, whose timer starts again in scan 16 and turns `On` in scan
/// 21. Catches timers reset before they run in every scan, which never fired (`On` never came), a
/// self-transition that does not restart its timer (it would fire in every scan after scan 10),
/// and a timer that counts the scan before its state was entered (the former timer rule: `TICK`
/// in scan 9, and `On` again in scan 20).
#[test]
fn transitions_with_after_fire_when_their_state_has_been_active_that_long() {
    let mut run = Run::new();
    let mut states = Vec::new();
    let mut ticks = Vec::new();
    for scan in 1..=21 {
        if scan == 15 {
            run.harness
                .try_set_input("fb_lamp.EV_GO", Value::Bool(true))
                .expect("raise GO");
        }
        if scan == 16 {
            run.harness
                .try_set_input("fb_lamp.EV_GO", Value::Bool(false))
                .expect("lower GO");
        }
        run.scan();
        states.push(run.state());
        ticks.push(run.tick_elapsed());
    }
    let expected_states: Vec<&str> = (1..=21)
        .map(|scan| match scan {
            1..=5 | 15..=20 => "Off",
            _ => "On",
        })
        .collect();
    assert_eq!(states, expected_states, "the active state after each scan");
    // The TICK timer at the end of each scan: 0 while Off, then 0, 20, 40 and 0 again where the
    // self-transition fires (scans 10 and 14), restarting at 0 in the scan after.
    assert_eq!(
        ticks,
        vec![0, 0, 0, 0, 0, 0, 0, 20, 40, 0, 0, 20, 40, 0, 0, 0, 0, 0, 0, 0, 0],
        "the TICK timer's elapsed time after each scan"
    );
}
