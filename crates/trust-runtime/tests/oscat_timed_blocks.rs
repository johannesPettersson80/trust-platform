//! The OSCAT blocks that measure time take the PLC timer's milliseconds as numbers (specification
//! 31 §2.4 and §2.5; the OSCAT classic guide). `DWORD_TO_REAL` is a binary transfer (IEC 61131-3
//! Table 25; specification 07 §2.3), so a block that converted the milliseconds with it would take
//! the REAL whose bit pattern they are: 1000 ms would be 1.4E-42, a division by it overflows, and a
//! pattern that is not a number is refused. The fixture `fixtures/oscat/timed`, compiled, runs the
//! blocks scan after scan of 10 ms of logical time, and each gives the value worked out by hand
//! below.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

#[path = "support/oscat_native.rs"]
mod oscat_native;

use std::sync::OnceLock;

use oscat_native::{program, real, scan, value};
use trust_runtime::value::Value;

fn image() -> &'static [u8] {
    static IMAGE: OnceLock<&'static [u8]> = OnceLock::new();
    IMAGE.get_or_init(|| oscat_native::image("timed"))
}

/// Catches a block that takes the milliseconds as a REAL bit pattern: `M_TX`'s frequency, `FT_PT1`,
/// `FLOW_METER`, `HEAT_METER` and `FT_Profile` divide by such a pattern and overflow, and
/// `DT_SIMU` takes the pattern of its scaled milliseconds as a count and jumps days ahead.
#[test]
fn the_timed_blocks_compute_with_the_milliseconds_as_numbers() {
    let mut program = program(image());
    for number in 1..=200i64 {
        scan(&mut program, number * 10);
        match number {
            // The input rose at scan 11 (110 ms), fell at 14 (140 ms) after rising at 1 and
            // falling at 4: 30 ms high after 70 ms low, a period of 100 ms. F = 1000 / 100 Hz,
            // DC = 30 / 100.
            14 => {
                assert_eq!(real(&program, "MAIN.PULSEFREQUENCY"), 10.0);
                assert_eq!(real(&program, "MAIN.PULSEDUTY"), 0.3);
            }
            51 => {
                // The profile started at 10 ms; at 510 ms it is 500 ms into its ramp of 1000 ms
                // from 0 to 100: 100 * 500 / 1000.
                assert_eq!(real(&program, "MAIN.PROFILEOUT"), 50.0);
                // At twice the speed, the 500 ms since the clock started at 10 ms are one second.
                assert_eq!(
                    value(&program, "MAIN.SIMULATEDONESECOND"),
                    Value::Bool(true)
                );
            }
            100 => {
                // The first second (1000 ms since the timer's zero, the flow meter's first
                // reference): 99 steps of 10 ms integrated 3600 units an hour, 0.99 units, so the
                // rate is 0.99 / 1000 ms * 3,600,000 ms an hour = 3564 units an hour. The steps
                // of 0.01 add REAL rounding of a few millionths.
                let rate = real(&program, "MAIN.FLOWRATE");
                assert!((rate - 3564.0).abs() < 0.5, "flow rate {rate}");
            }
            101 => {
                // 100 steps of the lag toward 1: each closes 10 ms / 1000 ms of the gap, so the
                // output is 1 - 0.99^100 = 0.6339677, up to REAL rounding.
                let lag = real(&program, "MAIN.PT1OUT");
                assert!((lag - 0.633_967_7).abs() < 1.0e-4, "first-order lag {lag}");
                // Two seconds of simulated time in the 1000 ms since the start.
                assert_eq!(
                    value(&program, "MAIN.SIMULATEDTWOSECONDS"),
                    Value::Bool(true)
                );
                // 100 steps of 10 ms at 10 units a second delivered 10 units in the second since
                // the meter's first scan: 10 / 1000 ms * 3,600,000 ms an hour.
                let heat = real(&program, "MAIN.HEATRATE");
                assert!((heat - 36_000.0).abs() < 1.0, "heat rate {heat}");
            }
            200 => {
                // The second second: 100 steps, 1.0 unit, 3600 units an hour.
                let rate = real(&program, "MAIN.FLOWRATE");
                assert!((rate - 3600.0).abs() < 0.5, "flow rate {rate}");
            }
            _ => {}
        }
    }
}

/// `RDM` draws from the PLC timer's milliseconds as a number. At 130,750 ms the timer it rotates
/// is 16#7FAFE000, whose pattern as a REAL is not a number: taken with `DWORD_TO_REAL` the scan
/// would end with an overflow (specification 31 §2.2).
#[test]
fn a_random_number_takes_the_plc_timer_as_a_number() {
    let mut program = program(image());
    scan(&mut program, 130_750);
    let random = real(&program, "MAIN.RANDOM");
    assert!((0.0..1.0).contains(&random), "random {random}");
}
