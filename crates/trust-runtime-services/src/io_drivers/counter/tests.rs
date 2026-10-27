use alloc::format;
use alloc::vec;

use super::*;

/// The concrete, flat address of `area` and `size` at the byte offset `byte`: specification 11
/// §6.5 reads `%ID4` as the double word at byte offset 4 of the input image.
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
fn refused(key: &str, reason: &str) -> Result<CounterParams, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The parameters of one input at `%ID0` on `GPIO7`, with `fields`, more JSON members, after
/// `line`.
fn input_with(fields: &str) -> Result<CounterParams, ParamsError> {
    CounterParams::from_json(&format!(
        r#"{{"inputs":[{{"address":"%ID0","line":"GPIO7",{fields}}}]}}"#
    ))
}

/// A table of pulse counters with the default and with each `edge`, and of a quadrature counter,
/// reads into exactly those entries in their order: `mode` is `pulse` and `edge` `rising` where
/// they are left out (specification 11 §6.5). Catches an entry dropped or reordered, a default
/// other than a rising-edge pulse counter, an edge read as another, and an encoder's B line lost.
#[test]
fn a_full_table_reads_into_its_entries() {
    let params = CounterParams::from_json(concat!(
        r#"{"inputs":[{"address":"%ID0","line":"GPIO5"},"#,
        r#"{"address":"%ID4","edge":"falling","line":"GPIO6","mode":"pulse"},"#,
        r#"{"address":"%ID8","edge":"both","line":"counter0/count0"},"#,
        r#"{"address":"%ID12","line":"GPIO7","line_b":"GPIO8","mode":"quadrature"}]}"#
    ));
    assert_eq!(
        params,
        Ok(CounterParams {
            inputs: vec![
                CounterInput {
                    address: flat(IoArea::Input, IoSize::DWord, 0),
                    line: "GPIO5".into(),
                    mode: CounterMode::Pulse { edge: Edge::Rising },
                },
                CounterInput {
                    address: flat(IoArea::Input, IoSize::DWord, 4),
                    line: "GPIO6".into(),
                    mode: CounterMode::Pulse {
                        edge: Edge::Falling,
                    },
                },
                CounterInput {
                    address: flat(IoArea::Input, IoSize::DWord, 8),
                    line: "counter0/count0".into(),
                    mode: CounterMode::Pulse { edge: Edge::Both },
                },
                CounterInput {
                    address: flat(IoArea::Input, IoSize::DWord, 12),
                    line: "GPIO7".into(),
                    mode: CounterMode::Quadrature {
                        line_b: Some("GPIO8".into()),
                    },
                },
            ],
        })
    );
    assert_eq!(
        CounterParams::from_json("{}"),
        Ok(CounterParams { inputs: vec![] })
    );
}

/// `mode` is `pulse` or `quadrature` and `edge` is `rising`, `falling` or `both`; a quadrature
/// counter refuses `edge` and takes `line_b` when it is given, which the runtime's hardware then
/// requires (a chip) or refuses (Linux), and a pulse counter, the default, refuses `line_b`
/// (specification 11 §6.5). Catches a misspelt mode counted as pulses, a second line ignored by a
/// pulse counter, and a Linux encoder, which names no B line, refused by the shared rules.
#[test]
fn each_mode_takes_only_its_own_keys() {
    assert_eq!(
        input_with(r#""mode":"count""#),
        refused(
            "inputs[0].mode",
            r#"must be "pulse" or "quadrature", not "count""#
        )
    );
    assert_eq!(
        input_with(r#""edge":"up""#),
        refused(
            "inputs[0].edge",
            r#"must be "rising", "falling" or "both", not "up""#
        )
    );
    assert_eq!(
        input_with(r#""edge":"rising","line_b":"GPIO8","mode":"quadrature""#),
        refused(
            "inputs[0].edge",
            "is refused with mode = \"quadrature\": a quadrature counter counts every edge of \
             both lines"
        )
    );
    let pulse_line_b = refused(
        "inputs[0].line_b",
        r#"is refused with mode = "pulse": a pulse counter counts one line"#,
    );
    assert_eq!(
        input_with(r#""line_b":"GPIO8","mode":"pulse""#),
        pulse_line_b
    );
    assert_eq!(input_with(r#""line_b":"GPIO8""#), pulse_line_b);
    assert_eq!(
        input_with(r#""mode":"quadrature""#),
        Ok(CounterParams {
            inputs: vec![CounterInput {
                address: flat(IoArea::Input, IoSize::DWord, 0),
                line: "GPIO7".into(),
                mode: CounterMode::Quadrature { line_b: None },
            }],
        })
    );
    assert_eq!(
        input_with(r#""line_b":"","mode":"quadrature""#),
        refused(
            "inputs[0].line_b",
            r#"must be a non-empty string naming the line, not """#
        )
    );
}

/// A line is used once across the driver's entries, an encoder's `line_b` among them, and a
/// process-image address once (specification 11 §6.5). Catches an encoder whose A and B are one
/// pin, which never counts, a pin counted by two inputs, and two counters writing one double word.
#[test]
fn a_line_b_counts_as_a_line() {
    assert_eq!(
        input_with(r#""line_b":"GPIO7","mode":"quadrature""#),
        refused(
            "inputs[0].line_b",
            r#""GPIO7" is used by inputs[0].line already; a line is used once"#
        )
    );
    assert_eq!(
        CounterParams::from_json(concat!(
            r#"{"inputs":[{"address":"%ID0","line":"GPIO5"},"#,
            r#"{"address":"%ID4","line":"GPIO7","line_b":"GPIO5","mode":"quadrature"}]}"#
        )),
        refused(
            "inputs[1].line_b",
            r#""GPIO5" is used by inputs[0].line already; a line is used once"#
        )
    );
    assert_eq!(
        CounterParams::from_json(concat!(
            r#"{"inputs":[{"address":"%ID0","line":"GPIO7","line_b":"GPIO8","mode":"quadrature"},"#,
            r#"{"address":"%ID4","line":"GPIO8"}]}"#
        )),
        refused(
            "inputs[1].line",
            r#""GPIO8" is used by inputs[0].line_b already; a line is used once"#
        )
    );
    assert_eq!(
        CounterParams::from_json(concat!(
            r#"{"inputs":[{"address":"%ID0","line":"GPIO5"},"#,
            r#"{"address":"%ID0","line":"GPIO6"}]}"#
        )),
        refused(
            "inputs[1].address",
            r#""%ID0" overlaps the bytes of inputs[0].address ("%ID0"); the entries of a list use bytes of their own"#
        )
    );
}

/// The shape, the address and the line follow the shared fail-closed rules (specification 11
/// §6.5): the root a table, `inputs` an array of tables, no unknown key, a concrete, flat `%ID`
/// double word as each address, and a non-empty line. Catches a typing mistake such as `invert`
/// ignored, and a count written to a word, which would wrap at 65,535, or to the output image.
#[test]
fn an_input_of_another_shape_is_refused() {
    let address = |given: &str| {
        refused(
            "inputs[0].address",
            &format!(r#"must be a concrete, flat %ID double word such as "%ID0", not {given}"#),
        )
    };
    assert_eq!(
        CounterParams::from_json("5"),
        refused("params", "must be a table, not 5")
    );
    assert_eq!(
        CounterParams::from_json(r#"{"outputs":[]}"#),
        refused("outputs", "unknown key; the counter driver takes inputs")
    );
    assert_eq!(
        CounterParams::from_json(r#"{"inputs":"GPIO5"}"#),
        refused("inputs", r#"must be an array of tables, not "GPIO5""#)
    );
    assert_eq!(
        CounterParams::from_json(r#"{"inputs":[5]}"#),
        refused("inputs[0]", "must be a table, not 5")
    );
    assert_eq!(
        input_with(r#""invert":true"#),
        refused(
            "inputs[0].invert",
            "unknown key; a counter input takes address, edge, line, line_b and mode"
        )
    );
    assert_eq!(
        CounterParams::from_json(r#"{"inputs":[{"line":"GPIO5"}]}"#),
        refused(
            "inputs[0].address",
            r#"is required, a concrete, flat %ID double word such as "%ID0""#
        )
    );
    assert_eq!(
        CounterParams::from_json(r#"{"inputs":[{"address":"%IW0","line":"GPIO5"}]}"#),
        address(r#""%IW0""#)
    );
    assert_eq!(
        CounterParams::from_json(r#"{"inputs":[{"address":"%QD0","line":"GPIO5"}]}"#),
        address(r#""%QD0""#)
    );
    assert_eq!(
        CounterParams::from_json(r#"{"inputs":[{"address":"%ID0"}]}"#),
        refused(
            "inputs[0].line",
            "is required, a non-empty string naming the line"
        )
    );
}

/// Each read adds the hardware counter's change since the last read as a signed 16-bit
/// difference, so the count goes on across the 16-bit counter's wrap (specification 11 §6.5: a
/// chip adds the change since its last read at each cycle start). Catches a plain difference,
/// which reads the wrap as 64,000 counts down.
#[test]
fn the_count_adds_the_change_across_the_16_bit_wrap() {
    // From 32,000 to −32,000 is −64,000 as a plain difference, and −64,000 + 65,536 = 1,536 counts
    // up as a signed 16-bit difference.
    let mut count = WideCount::new(32_000);
    assert_eq!(count.update(-32_000), 1_536);
    // Then 1,000 more, to −31,000; no change adds nothing.
    assert_eq!(count.update(-31_000), 2_536);
    assert_eq!(count.update(-31_000), 2_536);
}

/// At most 32,767 counts may arrive between two reads (specification 11 §6.5). Catches a limit
/// below 32,767, and shows what lies past it: 32,768 counts read as 32,768 down.
#[test]
fn up_to_32767_counts_arrive_between_two_reads() {
    let mut count = WideCount::new(0);
    assert_eq!(count.update(32_767), 32_767);
    // The next count wraps the hardware counter to −32,768: one count more.
    assert_eq!(count.update(-32_768), 32_768);
    // 32,768 counts between two reads are a 16-bit difference of −32,768: 2^32 − 32,768.
    let mut over = WideCount::new(0);
    assert_eq!(over.update(-32_768), 4_294_934_528);
}

/// A pulse count is a `UDINT` that wraps from 4,294,967,295 to 0, and a quadrature count the same
/// 32 bits as a `DINT` that wraps from 2,147,483,647 to −2,147,483,648 (specification 11 §6.5).
/// Catches a count that saturates or stops at the top, and a count down read as a large `UDINT`
/// by a quadrature counter.
#[test]
fn a_count_wraps_as_a_udint_and_as_a_dint() {
    // 4,294,967,290 + 10 = 4,294,967,300, which is 4 past 2^32.
    let mut pulses = WideCount {
        last: 0,
        total: 4_294_967_290,
    };
    assert_eq!(pulses.update(10), 4);
    // An encoder 10 counts back from 100: −10, the 32 bits 2^32 − 10 = 4,294,967,286.
    let mut encoder = WideCount::new(100);
    assert_eq!(encoder.update(90), 4_294_967_286);
    assert_eq!(encoder.signed(), -10);
    // Then 20 forward: +10.
    assert_eq!(encoder.update(110), 10);
    assert_eq!(encoder.signed(), 10);
    // One count past 2,147,483,647 is −2,147,483,648.
    let mut top = WideCount {
        last: 0,
        total: 2_147_483_647,
    };
    top.update(1);
    assert_eq!(top.signed(), -2_147_483_648);
}
