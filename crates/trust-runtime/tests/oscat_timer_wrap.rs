//! The OSCAT PLC timers wrap as upstream OSCAT's do (specification 31 §2.1): `T_PLC_MS` after 2^32
//! ms, about 49.7 days, and `T_PLC_US` after 2^32 us, about 71.6 minutes, and every block that reads
//! them takes the time between two readings across the wrap (§2.4). truST checks integer arithmetic
//! (specification 10), so a block that subtracted the readings as integers faulted at a wrap, one
//! that took them as DINT faulted at 2^31, and `T_PLC_US`, the milliseconds times 1000, faulted
//! itself after 71.6 minutes of uptime. The fixture `fixtures/oscat/wrap`, compiled, runs the timer
//! blocks scan after scan of 10 ms of logical time from 300 ms before a wrap, so each block starts
//! there and then meets the wrap; each must keep running, and the values below are worked out by
//! hand.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

#[path = "support/oscat_native.rs"]
mod oscat_native;

use std::sync::OnceLock;

use oscat_native::{program, real, scan};
use trust_runtime::native::{HostCode, NativeProgram};
use trust_runtime::value::{Duration, Value};

/// The scans of a run: the wrap falls between the 30th and the 32nd.
const SCANS: u32 = 60;

fn image() -> &'static [u8] {
    static IMAGE: OnceLock<&'static [u8]> = OnceLock::new();
    IMAGE.get_or_init(|| oscat_native::image("wrap"))
}

type Program = NativeProgram<'static, HostCode>;

fn value(program: &Program, name: &str) -> Value {
    oscat_native::value(program, name)
}

fn ms(value: i64) -> Value {
    Value::Time(Duration::from_millis(value))
}

/// Runs the fixture from `start_ms` for [`SCANS`] scans of 10 ms; every scan must end without a
/// fault, and `check` sees the values after each one, with the scan's number.
fn across(start_ms: i64, check: impl Fn(&Program, u32)) {
    let mut program = program(image());
    for scan_number in 1..=SCANS {
        scan(&mut program, start_ms + 10 * i64::from(scan_number - 1));
        common(&program, scan_number);
        check(&program, scan_number);
    }
}

/// What every run gives, the wrap between its 30th and 32nd scan: each cycle time is 10 ms, 10,000
/// us or 0.01 s; `M_T` has counted 10 ms a scan since the first; `TP_X`'s pulse of 400 ms ends at
/// the 41st scan; `GEN_SQ` of period 100 ms toggles every fifth scan from the sixth, so its output
/// is TRUE while the number of toggles, (scan - 1) / 5, is even; `CYCLE_TIME` has measured 10 ms
/// from the third scan as its last, least and greatest cycle; and `FT_PT1`'s lag of 1 s has closed
/// 1 - 0.99^59 of its step at the 60th scan.
fn common(runtime: &Program, scan: u32) {
    if scan > 1 {
        assert_eq!(
            value(runtime, "MAIN.CYCLEMS.TC"),
            Value::DWord(10),
            "scan {scan}"
        );
        assert_eq!(
            value(runtime, "MAIN.CYCLEUS.TC"),
            Value::DWord(10_000),
            "scan {scan}"
        );
        let seconds = real(runtime, "MAIN.CYCLESECONDS.TC");
        assert!(
            (seconds - 0.01).abs() < 1.0e-6,
            "scan {scan}: TC_S {seconds}"
        );
    }
    let since = i64::from(scan - 1) * 10;
    assert_eq!(value(runtime, "MAIN.SINCE.ET"), ms(since), "scan {scan}");
    if scan <= 40 {
        assert_eq!(
            value(runtime, "MAIN.PULSE.Q"),
            Value::Bool(true),
            "scan {scan}"
        );
        assert_eq!(value(runtime, "MAIN.PULSE.ET"), ms(since), "scan {scan}");
    } else if scan == 41 {
        assert_eq!(
            value(runtime, "MAIN.PULSE.Q"),
            Value::Bool(false),
            "scan {scan}"
        );
    }
    let even = ((scan - 1) / 5).is_multiple_of(2);
    assert_eq!(
        value(runtime, "MAIN.SQUARE.Q"),
        Value::Bool(even),
        "scan {scan}"
    );
    if scan >= 3 {
        for output in ["ct_last", "ct_min", "ct_max"] {
            assert_eq!(
                value(
                    runtime,
                    &format!("MAIN.CYCLETIME.{}", output.to_ascii_uppercase())
                ),
                ms(10),
                "scan {scan}: {output}"
            );
        }
    }
    if scan == SCANS {
        let lag = real(runtime, "MAIN.LAG.OUT");
        assert!((lag - 0.447_316_5).abs() < 1.0e-4, "FT_PT1 {lag}");
    }
}

/// Across the microsecond timer's wrap: the 31st scan is at 4,294,967 ms, whose 4,294,967,000 us
/// are below 2^32, and the 32nd at 4,294,977 ms, whose 4,294,977,000 us wrap to 9,704.
#[test]
fn the_timer_blocks_run_across_the_microsecond_wrap() {
    across(4_294_667, |runtime, scan| match scan {
        31 => assert_eq!(value(runtime, "MAIN.TIMERUS"), Value::DWord(4_294_967_000)),
        32 => assert_eq!(value(runtime, "MAIN.TIMERUS"), Value::DWord(9_704)),
        _ => {}
    });
}

/// Across 2^31 ms, where a reading taken as DINT turns negative: the milliseconds pass
/// 2,147,483,648 at the 31st scan, where the microseconds wrap to 0.
#[test]
fn the_timer_blocks_run_across_2_31_milliseconds() {
    across(2_147_483_348, |runtime, scan| {
        if scan == 31 {
            assert_eq!(value(runtime, "MAIN.TIMERMS"), Value::DWord(2_147_483_648));
            assert_eq!(value(runtime, "MAIN.TIMERUS"), Value::DWord(0));
        }
    });
}

/// Across the millisecond timer's wrap after 49.7 days: the 30th scan is at 4,294,967,286 ms and
/// the 31st at 2^32 ms, which the timer reads as 0, with the microseconds 0 too.
#[test]
fn the_timer_blocks_run_across_the_millisecond_wrap() {
    across(4_294_966_996, |runtime, scan| match scan {
        30 => assert_eq!(value(runtime, "MAIN.TIMERMS"), Value::DWord(4_294_967_286)),
        31 => {
            assert_eq!(value(runtime, "MAIN.TIMERMS"), Value::DWord(0));
            assert_eq!(value(runtime, "MAIN.TIMERUS"), Value::DWord(0));
        }
        32 => assert_eq!(value(runtime, "MAIN.TIMERMS"), Value::DWord(10)),
        _ => {}
    });
}
