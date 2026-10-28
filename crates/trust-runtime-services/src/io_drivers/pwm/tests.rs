use alloc::format;
use alloc::vec;

use super::*;

/// The concrete, flat address of `area` and `size` at the byte offset `byte`: specification 11
/// §6.5 reads `%QW2` as the word at byte offset 2 of the output image.
fn flat(area: IoArea, size: IoSize, byte: u32) -> IoAddress {
    IoAddress {
        area,
        size,
        byte,
        bit: 0,
        path: vec![byte],
        wildcard: false,
    }
}

/// The refusal of `key` for `reason`.
fn refused(key: &str, reason: &str) -> Result<PwmParams, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The parameters of one output on `GPIO4` at `%QW0`, with `fields`, more JSON members, after
/// `address`.
fn output_with(fields: &str) -> Result<PwmParams, ParamsError> {
    PwmParams::from_json(&format!(
        r#"{{"outputs":[{{"address":"%QW0",{fields},"line":"GPIO4"}}]}}"#
    ))
}

/// A table with an output on a chip's pin with `initial` and one on a sysfs channel without reads
/// into exactly those entries in their order, `initial` 0 where it is left out (specification 11
/// §6.5). Catches an entry dropped or reordered, a frequency or duty read from the wrong key, and
/// a default other than 0.
#[test]
fn a_full_table_reads_into_its_entries() {
    let params = PwmParams::from_json(concat!(
        r#"{"outputs":[{"address":"%QW0","frequency_hz":1000,"initial":2500,"line":"GPIO4"},"#,
        r#"{"address":"%QW2","frequency_hz":20000,"line":"pwmchip0/pwm1"}]}"#
    ));
    assert_eq!(
        params,
        Ok(PwmParams {
            outputs: vec![
                PwmOutput {
                    address: flat(IoArea::Output, IoSize::Word, 0),
                    line: "GPIO4".into(),
                    frequency_hz: 1_000,
                    initial: 2_500,
                },
                PwmOutput {
                    address: flat(IoArea::Output, IoSize::Word, 2),
                    line: "pwmchip0/pwm1".into(),
                    frequency_hz: 20_000,
                    initial: 0,
                },
            ],
        })
    );
    assert_eq!(
        PwmParams::from_json("{}"),
        Ok(PwmParams { outputs: vec![] })
    );
}

/// `frequency_hz` is required, an integer from 1 to 1,000,000, both ends taken (specification 11
/// §6.5). Catches an output without a frequency started at some default, a frequency of 0 that
/// would divide the period by zero, and a frequency above the range handed to the hardware.
#[test]
fn a_frequency_is_required_from_1_hz_to_1_mhz() {
    let reason = |given: &str| {
        refused(
            "outputs[0].frequency_hz",
            &format!("must be an integer from 1 to 1000000, not {given}"),
        )
    };
    assert_eq!(
        PwmParams::from_json(r#"{"outputs":[{"address":"%QW0","line":"GPIO4"}]}"#),
        refused(
            "outputs[0].frequency_hz",
            "is required, an integer from 1 to 1000000"
        )
    );
    assert_eq!(output_with(r#""frequency_hz":0"#), reason("0"));
    assert_eq!(output_with(r#""frequency_hz":1000001"#), reason("1000001"));
    assert_eq!(output_with(r#""frequency_hz":2.5"#), reason("2.5"));
    assert_eq!(output_with(r#""frequency_hz":"1000""#), reason(r#""1000""#));
    let lowest = output_with(r#""frequency_hz":1"#).expect("1 Hz is taken");
    assert_eq!(lowest.outputs[0].frequency_hz, 1);
    let highest = output_with(r#""frequency_hz":1000000"#).expect("1 MHz is taken");
    assert_eq!(highest.outputs[0].frequency_hz, 1_000_000);
}

/// `initial` is a duty cycle from 0 to 10,000 hundredths of a percent, 10,000 taken (specification
/// 11 §6.5). Catches a negative duty wrapped to a large one, and an `initial` above 100 % taken as
/// something the output never shows.
#[test]
fn an_initial_duty_above_100_percent_is_refused() {
    let reason = |given: &str| {
        refused(
            "outputs[0].initial",
            &format!("must be an integer from 0 to 10000, not {given}"),
        )
    };
    assert_eq!(
        output_with(r#""frequency_hz":1000,"initial":10001"#),
        reason("10001")
    );
    assert_eq!(
        output_with(r#""frequency_hz":1000,"initial":-1"#),
        reason("-1")
    );
    let full = output_with(r#""frequency_hz":1000,"initial":10000"#).expect("100 % is taken");
    assert_eq!(full.outputs[0].initial, 10_000);
}

/// The shape, the address and the lines follow the shared fail-closed rules (specification 11
/// §6.5): the root a table, `outputs` an array of tables, no unknown key, a concrete, flat `%QW`
/// word as each address and once in the list, and each line once. Catches a typing mistake such as
/// `duty` ignored, a duty cycle read from the input image or from a double word, and two outputs
/// on one pin.
#[test]
fn an_output_of_another_shape_is_refused() {
    let address = |given: &str| {
        refused(
            "outputs[0].address",
            &format!(r#"must be a concrete, flat %QW word such as "%QW0", not {given}"#),
        )
    };
    assert_eq!(
        PwmParams::from_json(r#""pwm""#),
        refused("params", r#"must be a table, not "pwm""#)
    );
    assert_eq!(
        PwmParams::from_json(r#"{"frequency_hz":1000}"#),
        refused("frequency_hz", "unknown key; the pwm driver takes outputs")
    );
    assert_eq!(
        PwmParams::from_json(r#"{"outputs":1}"#),
        refused("outputs", "must be an array of tables, not 1")
    );
    assert_eq!(
        PwmParams::from_json(r#"{"outputs":[1]}"#),
        refused("outputs[0]", "must be a table, not 1")
    );
    assert_eq!(
        output_with(r#""duty":5000,"frequency_hz":1000"#),
        refused(
            "outputs[0].duty",
            "unknown key; a pwm output takes address, frequency_hz, initial and line"
        )
    );
    assert_eq!(
        PwmParams::from_json(r#"{"outputs":[{"frequency_hz":1000,"line":"GPIO4"}]}"#),
        refused(
            "outputs[0].address",
            r#"is required, a concrete, flat %QW word such as "%QW0""#
        )
    );
    assert_eq!(
        PwmParams::from_json(
            r#"{"outputs":[{"address":"%IW0","frequency_hz":1000,"line":"GPIO4"}]}"#
        ),
        address(r#""%IW0""#)
    );
    assert_eq!(
        PwmParams::from_json(
            r#"{"outputs":[{"address":"%QD0","frequency_hz":1000,"line":"GPIO4"}]}"#
        ),
        address(r#""%QD0""#)
    );
    assert_eq!(
        PwmParams::from_json(r#"{"outputs":[{"address":"%QW0","frequency_hz":1000}]}"#),
        refused(
            "outputs[0].line",
            "is required, a non-empty string naming the line"
        )
    );
    assert_eq!(
        PwmParams::from_json(concat!(
            r#"{"outputs":[{"address":"%QW0","frequency_hz":1000,"line":"GPIO4"},"#,
            r#"{"address":"%QW2","frequency_hz":50,"line":"GPIO4"}]}"#
        )),
        refused(
            "outputs[1].line",
            r#""GPIO4" is used by outputs[0].line already; a line is used once"#
        )
    );
    assert_eq!(
        PwmParams::from_json(concat!(
            r#"{"outputs":[{"address":"%QW0","frequency_hz":1000,"line":"GPIO4"},"#,
            r#"{"address":"%QW0","frequency_hz":50,"line":"GPIO5"}]}"#
        )),
        refused(
            "outputs[1].address",
            r#""%QW0" overlaps the bytes of outputs[0].address ("%QW0"); the entries of a list use bytes of their own"#
        )
    );
}

/// The duty resolution is the largest number of bits with `frequency_hz × 2^bits ≤ clock_hz`, at
/// most the timer's, and none when not even one bit fits (specification 11 §6.5: the finest duty
/// resolution the hardware has at the frequency; a frequency it cannot make is refused). Catches a
/// resolution one bit too fine, whose counter would not reach its top within the period, one bit
/// too coarse, and a frequency the timer cannot make accepted.
#[test]
fn the_resolution_is_the_finest_the_clock_makes_at_the_frequency() {
    // 1,000 × 2^16 = 65,536,000 ≤ 80,000,000 < 1,000 × 2^17 = 131,072,000: 16 bits, at most 14.
    assert_eq!(resolution_bits(80_000_000, 1_000, 14), Some(14));
    assert_eq!(resolution_bits(80_000_000, 1_000, 20), Some(16));
    // ESP-IDF's example of 5 kHz: 80,000,000 / 5,000 = 16,000, and 2^13 = 8,192 ≤ 16,000 < 16,384.
    assert_eq!(resolution_bits(80_000_000, 5_000, 20), Some(13));
    // Equal is enough: 1,000 × 2^16 = 65,536,000; a hertz of clock less is 15 bits.
    assert_eq!(resolution_bits(65_536_000, 1_000, 20), Some(16));
    assert_eq!(resolution_bits(65_535_999, 1_000, 20), Some(15));
    // 1 MHz: 80 clocks per period, 2^6 = 64 ≤ 80 < 128.
    assert_eq!(resolution_bits(80_000_000, 1_000_000, 20), Some(6));
    // 40 MHz is one bit, 2 × 40,000,000 = 80,000,000; a hertz more is none, 80,000,002 > 80 MHz.
    assert_eq!(resolution_bits(80_000_000, 40_000_000, 20), Some(1));
    assert_eq!(resolution_bits(80_000_000, 40_000_001, 20), None);
    // A timer of no bits makes nothing, and no timer makes 0 Hz.
    assert_eq!(resolution_bits(80_000_000, 1_000, 0), None);
    assert_eq!(resolution_bits(80_000_000, 0, 20), None);
}

/// The compare value is `duty × 2^bits / 10,000`, rounded to the nearest count, a half up, and a
/// duty above 10,000 is 100 % (specification 11 §6.5: a larger value drives 100 %). Catches a
/// compare value truncated, a duty above 100 % that overflows the counter or wraps to a small one,
/// and an overflow at a resolution the counter cannot hold.
#[test]
fn the_compare_value_is_the_duty_share_of_the_full_count() {
    // 5,000 × 1,024 / 10,000 = 512 exactly.
    assert_eq!(duty_ticks(5_000, 10), 512);
    assert_eq!(duty_ticks(0, 10), 0);
    // 100 % is the full count, 2^10 = 1,024, and so is any duty above it.
    assert_eq!(duty_ticks(10_000, 10), 1_024);
    assert_eq!(duty_ticks(10_001, 10), 1_024);
    assert_eq!(duty_ticks(u16::MAX, 10), 1_024);
    // 1,875 × 8 / 10,000 = 1.5, a half, is 2; 1,874 × 8 / 10,000 = 1.4992 is 1.
    assert_eq!(duty_ticks(1_875, 3), 2);
    assert_eq!(duty_ticks(1_874, 3), 1);
    // 3,333 × 16,384 / 10,000 = 5,460.7872, nearest 5,461.
    assert_eq!(duty_ticks(3_333, 14), 5_461);
    // At 31 bits 100 % is 2^31; at 40 bits 0.01 % is 2^40 / 10,000 = 109,951,162.78, nearest
    // 109,951,163; a count above u32::MAX is u32::MAX, and 0 % is 0 at any resolution.
    assert_eq!(duty_ticks(10_000, 31), 2_147_483_648);
    assert_eq!(duty_ticks(1, 40), 109_951_163);
    assert_eq!(duty_ticks(10_000, 32), u32::MAX);
    assert_eq!(duty_ticks(1, 200), u32::MAX);
    assert_eq!(duty_ticks(0, 200), 0);
}

/// A sysfs channel's `duty_cycle` is `period_ns × duty / 10,000` nanoseconds, rounded down, and
/// the whole period above 100 % (specification 11 §6.5). The period of 999 Hz is
/// 1,000,000,000 / 999 = 1,001,001 ns, rounded down. Catches a duty cycle rounded up past what the
/// period allows, a duty above 100 % written as longer than the period, which the kernel refuses,
/// and an overflow of the product.
#[test]
fn the_sysfs_duty_cycle_is_the_duty_share_of_the_period() {
    // 25 % of 1,000,000 ns at 1 kHz.
    assert_eq!(duty_nanos(2_500, 1_000_000), 250_000);
    // 1,001,001 × 3,333 / 10,000 = 333,633.6333, rounded down 333,633.
    assert_eq!(duty_nanos(3_333, 1_001_001), 333_633);
    // 0.01 % of 1,000 ns is 0.1 ns, rounded down 0.
    assert_eq!(duty_nanos(1, 1_000), 0);
    assert_eq!(duty_nanos(0, 1_001_001), 0);
    assert_eq!(duty_nanos(10_000, 1_001_001), 1_001_001);
    assert_eq!(duty_nanos(12_000, 1_001_001), 1_001_001);
    // (2^64 − 1) × 9,999 / 10,000 = 18,444,899,399,302,180,659.84, rounded down.
    assert_eq!(duty_nanos(9_999, u64::MAX), 18_444_899_399_302_180_659);
}
