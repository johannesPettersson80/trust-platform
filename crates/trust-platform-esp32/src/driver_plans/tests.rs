use alloc::vec;

use smol_str::SmolStr;
use trust_runtime_core::deployment::SafeStateEntry;
use trust_runtime_core::io_address::IoSize;
use trust_runtime_core::memory::IoArea;
use trust_runtime_services::io_drivers::OnError;

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

/// A `pwm` output on `line` at the word `%QW<byte>`, at `frequency_hz` with `initial`.
fn output(byte: u32, line: &str, frequency_hz: u32, initial: u16) -> PwmOutput {
    PwmOutput {
        address: flat(IoArea::Output, IoSize::Word, byte),
        line: line.into(),
        frequency_hz,
        initial,
    }
}

/// Outputs of the frequencies `frequencies`, in order, on `GPIO0` upwards at `%QW0` upwards.
fn outputs_at(frequencies: &[u32]) -> Vec<PwmOutput> {
    (0_u32..)
        .zip(frequencies)
        .map(|(index, frequency)| output(index * 2, &format!("GPIO{index}"), *frequency, 0))
        .collect()
}

/// A `counter` input on `line` at the double word `%ID<byte>`, counting `mode`.
fn counter_input(byte: u32, line: &str, mode: CounterMode) -> CounterInput {
    CounterInput {
        address: flat(IoArea::Input, IoSize::DWord, byte),
        line: line.into(),
        mode,
    }
}

/// A receive of the identifier `id` (`extended` for 29 bits) into one byte at `%IB<byte>`.
fn receive(id: u32, extended: bool, byte: u32) -> CanReceive {
    CanReceive {
        id: CanId { id, extended },
        address: flat(IoArea::Input, IoSize::Byte, byte),
        length: 1,
        timeout_ms: None,
    }
}

/// `can` parameters on `line` at `bitrate` with the pins `tx` and `rx` and the `receives`.
fn can_params(
    line: &str,
    bitrate: u32,
    tx: Option<&str>,
    rx: Option<&str>,
    receives: Vec<CanReceive>,
) -> CanParams {
    CanParams {
        line: line.into(),
        tx: tx.map(SmolStr::from),
        rx: rx.map(SmolStr::from),
        bitrate,
        receive: receives,
        transmit: Vec::new(),
        on_error: OnError::Fault,
    }
}

/// A data frame of `id` (`extended` for 29 bits) with the one data byte `byte`.
fn frame(id: u32, extended: bool, byte: u8) -> CanFrame {
    CanFrame {
        id: CanId { id, extended },
        len: 1,
        data: [byte, 0, 0, 0, 0, 0, 0, 0],
    }
}

/// The health of a path faulted with `error`.
fn faulted(error: &str) -> IoDriverHealth {
    IoDriverHealth::Faulted {
        error: error.into(),
    }
}

/// The health of a path degraded with `error`.
fn degraded(error: &str) -> IoDriverHealth {
    IoDriverHealth::Degraded {
        error: error.into(),
    }
}

/// A refusal is `<driver>: <key>: <reason>`, the shared parameter refusal after the driver's name
/// (specification 11 §6.5: refused "with the key and the reason"). Catches another separator or
/// order, which `status` and the documented refusals of every driver rely on.
#[test]
fn a_refusal_names_the_driver_the_key_and_the_reason() {
    assert_eq!(
        refusal(PWM, "outputs[1].line", "GPIO4 is reserved by the firmware"),
        "pwm: outputs[1].line: GPIO4 is reserved by the firmware"
    );
}

/// Bytes land from their byte offset: [0xAA, 0xBB] at byte 2 of a 4-byte image fill bytes 2 and 3.
/// At byte 3 they would need bytes 3 and 4, and the image ends at 3, so nothing is written; at
/// `usize::MAX` the end overflows. Catches a write that keeps its first byte when the second does
/// not fit, an end check off by one, and an overflow that panics or wraps to byte 0.
#[test]
fn an_image_write_lands_at_its_byte_and_one_outside_writes_nothing() {
    let mut image = [0_u8; 4];
    assert_eq!(write_image(&mut image, 2, &[0xAA, 0xBB], "%IW2"), Ok(()));
    assert_eq!(image, [0, 0, 0xAA, 0xBB]);

    let mut image = [0_u8; 4];
    assert_eq!(
        write_image(&mut image, 3, &[0xAA, 0xBB], "%IW3"),
        Err(String::from(
            "%IW3 lies outside the process image of 4 bytes"
        ))
    );
    assert_eq!(image, [0; 4]);
    assert_eq!(
        write_image(&mut image, usize::MAX, &[1], "%IW9"),
        Err(String::from(
            "%IW9 lies outside the process image of 4 bytes"
        ))
    );
    assert_eq!(image, [0; 4]);
}

/// The word at byte 2 of [1, 2, 3, 4] is [3, 4]; at byte 3 it would need byte 4, which the image
/// lacks. Catches a read that pads a short word with zeros, which would drive an output from bytes
/// the program never wrote.
#[test]
fn an_image_read_takes_its_bytes_or_names_its_address() {
    let image = [1_u8, 2, 3, 4];
    assert_eq!(read_image::<2>(&image, 2, "%QW2"), Ok([3, 4]));
    assert_eq!(
        read_image::<2>(&image, 3, "%QW3"),
        Err(String::from(
            "%QW3 lies outside the process image of 4 bytes"
        ))
    );
    assert_eq!(
        read_image::<2>(&image, usize::MAX, "%QW9"),
        Err(String::from(
            "%QW9 lies outside the process image of 4 bytes"
        ))
    );
}

/// A path is ok after an exchange that succeeds and faulted with the error of one that fails
/// (`ChipDriver::health`). Catches a health that stays faulted after the fault went away.
#[test]
fn a_path_takes_the_health_of_its_last_exchange() {
    assert_eq!(health_of(&Ok(())), IoDriverHealth::Ok);
    assert_eq!(
        health_of(&Err(String::from(
            "%IW8 lies outside the process image of 4 bytes"
        ))),
        faulted("%IW8 lies outside the process image of 4 bytes")
    );
}

/// A peripheral serves one driver at a time and is free again when that driver's lease is dropped
/// (specification 34 STATE-08). Catches two drivers on one ADC1 or TWAI controller, which the
/// `clone_unchecked` of each driver's copy relies on never happening, and a peripheral that stays
/// held after the program that held it ended, so the next program could not claim it.
#[test]
fn a_peripheral_serves_one_driver_at_a_time() {
    let slot = PeripheralSlot::new(7_u8);
    let lease = slot.claim();
    assert_eq!(lease.as_ref().map(Lease::get), Some(&7));
    assert!(slot.claim().is_none());
    drop(lease);
    let again = slot.claim();
    assert_eq!(again.as_ref().map(Lease::get), Some(&7));
    assert!(slot.claim().is_none());
}

/// A timer runs at the finest resolution the 80 MHz clock allows, at most 14 bits, with the divider
/// 80,000,000 × 256 / (f × 2^bits) rounded to the nearest step (ESP32-C6 TRM, LED PWM Controller:
/// f = f_clock / (divider × 2^bits), the divider with 8 fractional bits):
/// - 1,000 Hz: 80,000 clock steps per period, 2^16 ≤ 80,000, so 14 bits; 20,480,000,000 /
///   16,384,000 = 1,250 exactly.
/// - 20,000 Hz: 4,000 steps, 2^11 = 2,048 ≤ 4,000 < 4,096, so 11 bits; 20,480,000,000 /
///   40,960,000 = 500.
/// - 1,000,000 Hz: 80 steps, 2^6 = 64 ≤ 80 < 128, so 6 bits; 20,480,000,000 / 64,000,000 = 320.
/// - 3,000 Hz: 26,666 steps, so 14 bits; 20,480,000,000 / 49,152,000 = 416.67, nearest 417.
/// - 5 Hz: 14 bits; 20,480,000,000 / 81,920 = 250,000, which the 18-bit divider holds.
///
/// Catches a resolution above the LEDC's 14 bits, a divider truncated (416 at 3,000 Hz, 0.16 %
/// fast), and a divider worked out from esp-hal's 40 MHz APB clock (625 at 1,000 Hz), which the
/// 80 MHz clock would run at twice the frequency, 2,000 Hz.
#[test]
fn a_timer_runs_at_the_finest_resolution_with_the_nearest_divider() {
    let plan = |frequency_hz, bits, divider| {
        Ok(TimerPlan {
            frequency_hz,
            bits,
            divider,
        })
    };
    assert_eq!(timer_plan(1_000), plan(1_000, 14, 1_250));
    assert_eq!(timer_plan(20_000), plan(20_000, 11, 500));
    assert_eq!(timer_plan(1_000_000), plan(1_000_000, 6, 320));
    assert_eq!(timer_plan(3_000), plan(3_000, 14, 417));
    assert_eq!(timer_plan(5), plan(5, 14, 250_000));
}

/// Below 5 Hz the divider at 14 bits, 1,250,000 / f, passes the 18-bit field's 262,143: 312,500 at
/// 4 Hz, 1,250,000 at 1 Hz (specification 11 §6.5: a frequency the hardware cannot make is refused
/// with both numbers). Catches a divider written into the field anyway, whose upper bits the timer
/// drops: 312,500 would run as 50,356, about 25 Hz instead of 4.
#[test]
fn a_frequency_below_5_hz_does_not_fit_the_divider() {
    assert!(timer_plan(LEDC_LOWEST_HZ).is_ok());
    for frequency in [4_u32, 1] {
        assert_eq!(
            timer_plan(frequency),
            Err(format!(
                "the C6's LEDC cannot make {frequency} Hz from its 80000000 Hz clock: its lowest \
                 frequency is 5 Hz"
            ))
        );
    }
}

/// 40,000,000 Hz leaves 2 clock steps a period, one bit with the divider 256 (1.0); 50,000,000 Hz
/// leaves 1.6 steps, not one bit, and 0 Hz has no period. Catches a timer configured with no
/// resolution or a division by zero.
#[test]
fn a_frequency_the_clock_cannot_divide_is_refused() {
    assert_eq!(
        timer_plan(40_000_000),
        Ok(TimerPlan {
            frequency_hz: 40_000_000,
            bits: 1,
            divider: 256,
        })
    );
    for frequency in [50_000_000_u32, 0] {
        assert_eq!(
            timer_plan(frequency),
            Err(format!(
                "the C6's LEDC cannot make {frequency} Hz from its 80000000 Hz clock"
            ))
        );
    }
}

/// Outputs of one frequency share a timer, the timers follow the order in which their frequencies
/// first appear, and output `i` runs on channel `i` (specification 34 Appendix D.2). Here 1,000 Hz
/// is timer 0 (14 bits, 1,250), 20,000 Hz timer 1 (11 bits, 500) and 50 Hz timer 2 (14 bits,
/// 20,480,000,000 / 819,200 = 25,000). The initial compare values are duty × 2^bits / 10,000:
/// 2,500 × 16,384 / 10,000 = 4,096; 5,000 × 2,048 / 10,000 = 1,024; 10,000 at 14 bits = 16,384;
/// 0 = 0. Catches a timer for each output, a second timer for a repeated frequency, an output on
/// the timer of another frequency, and an initial duty taken at another timer's resolution.
#[test]
fn outputs_of_one_frequency_share_a_timer() {
    let outputs = [
        output(0, "GPIO4", 1_000, 2_500),
        output(2, "GPIO5", 20_000, 5_000),
        output(4, "GPIO6", 1_000, 10_000),
        output(6, "GPIO7", 50, 0),
    ];
    let timer = |frequency_hz, bits, divider| TimerPlan {
        frequency_hz,
        bits,
        divider,
    };
    let channel = |channel, timer, bits, initial_ticks, initial_percent| PwmChannelPlan {
        channel,
        timer,
        bits,
        initial_ticks,
        initial_percent,
    };
    assert_eq!(
        pwm_plan(&outputs),
        Ok(PwmPlan {
            timers: vec![
                timer(1_000, 14, 1_250),
                timer(20_000, 11, 500),
                timer(50, 14, 25_000),
            ],
            outputs: vec![
                channel(0, 0, 14, 4_096, 25),
                channel(1, 1, 11, 1_024, 50),
                channel(2, 0, 14, 16_384, 100),
                channel(3, 2, 14, 0, 0),
            ],
        })
    );
    assert_eq!(
        pwm_plan(&[]),
        Ok(PwmPlan {
            timers: vec![],
            outputs: vec![],
        })
    );
}

/// The LEDC has six channels: six outputs of one frequency take channels 0 to 5 on one timer, and
/// a seventh output is refused before any timer is planned. Catches a seventh output started on a
/// channel the C6 lacks.
#[test]
fn a_seventh_output_is_refused_for_the_six_channels() {
    let six = pwm_plan(&outputs_at(&[1_000; 6])).map(|plan| {
        (
            plan.timers.len(),
            plan.outputs
                .iter()
                .map(|output| (output.channel, output.timer))
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!(
        six,
        Ok((1, vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)]))
    );
    assert_eq!(
        pwm_plan(&outputs_at(&[1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 4])),
        Err(String::from(
            "pwm: outputs: the C6's LEDC has 6 channels, and the driver names 7 outputs"
        ))
    );
}

/// The LEDC has four timers, one for each frequency: of 100, 200, 100, 300, 400 and 500 Hz the
/// fifth frequency is 500 Hz, at output 5, since output 2 repeats 100 Hz. Catches a count of
/// outputs instead of frequencies, which would refuse output 4, and a refusal without the key of
/// the output that brought the fifth frequency.
#[test]
fn a_fifth_frequency_is_refused_at_its_output() {
    assert_eq!(
        pwm_plan(&outputs_at(&[100, 200, 100, 300, 400, 500])),
        Err(String::from(
            "pwm: outputs[5].frequency_hz: the C6's LEDC has 4 timers, one for each frequency, \
             and 500 Hz is a fifth frequency"
        ))
    );
}

/// A frequency the timer cannot make is refused with the key of its output and both numbers
/// (specification 11 §6.5). Catches the refusal of `timer_plan` without its key.
#[test]
fn a_frequency_the_timer_cannot_make_is_refused_at_its_output() {
    assert_eq!(
        pwm_plan(&outputs_at(&[1_000, 4])),
        Err(String::from(
            "pwm: outputs[1].frequency_hz: the C6's LEDC cannot make 4 Hz from its 80000000 Hz \
             clock: its lowest frequency is 5 Hz"
        ))
    );
}

/// The whole percent nearest a duty in hundredths of a percent, a half up: 0.49 % is 0, 0.50 % is
/// 1, 25.00 % is 25, 99.49 % is 99, 99.50 % is 100, and 100 % and above it (specification 11 §6.5:
/// a larger value drives 100 %) are 100. Catches a truncation (0.50 % as 0) and a percent above 100
/// handed to esp-hal, which refuses the channel with it.
#[test]
fn the_initial_percent_is_the_nearest_whole_percent() {
    for (duty, percent) in [
        (0, 0),
        (49, 0),
        (50, 1),
        (2_500, 25),
        (9_949, 99),
        (9_950, 100),
        (10_000, 100),
        (65_535, 100),
    ] {
        assert_eq!(duty_percent(duty), percent, "duty {duty}");
    }
}

/// A pin's static level is high only when its output's safe duty is 100 % or more; an output
/// without a `WORD` safe value has the safe duty 0 (specification 11 §6.5: "an output's safe value
/// is its `[[io.safe_state]]` value, otherwise 0"). Catches 99.99 % taken as high, a `BOOL` safe
/// value taken as a duty, and a safe value read from another output's address.
#[test]
fn a_pin_rests_high_only_at_a_full_safe_duty() {
    let word = |byte| flat(IoArea::Output, IoSize::Word, byte);
    let config = Configuration {
        safe_state: vec![
            SafeStateEntry {
                address: word(0),
                value: Value::Word(10_000),
            },
            SafeStateEntry {
                address: word(2),
                value: Value::Word(9_999),
            },
            SafeStateEntry {
                address: word(4),
                value: Value::Word(65_535),
            },
            SafeStateEntry {
                address: word(6),
                value: Value::Bool(true),
            },
        ],
        ..Configuration::default()
    };
    let levels: Vec<_> = [0, 2, 4, 6, 8]
        .into_iter()
        .map(|byte| {
            (
                safe_duty(&config, &word(byte)),
                safe_high(&config, &word(byte)),
            )
        })
        .collect();
    assert_eq!(
        levels,
        vec![
            (10_000, true),
            (9_999, false),
            (65_535, true),
            (0, false),
            (0, false),
        ]
    );
}

/// A PCNT unit as the ESP32-C6 TRM's Pulse Count Controller counts: at an edge of a channel's
/// signal the channel takes the action of the edge (`POS_MODE` rising, `NEG_MODE` falling), its
/// control level keeps or reverses it (`HCTRL_MODE` high, `LCTRL_MODE` low), and the count changes
/// by it. Channel 0's signal is A and its control B, channel 1's signal B and its control A; the
/// count of the states `levels` (A, B) visited in order is returned.
fn count(setting: UnitSetting, levels: &[(bool, bool)]) -> i32 {
    let edge = |channel: ChannelSetting, before: bool, after: bool, control: bool| {
        if before == after {
            return 0;
        }
        let action = if after {
            channel.rising
        } else {
            channel.falling
        };
        let reversed = (if control { channel.high } else { channel.low }) == ControlAction::Reverse;
        match (action, reversed) {
            (EdgeAction::Hold, _) => 0,
            (EdgeAction::Increment, false) | (EdgeAction::Decrement, true) => 1,
            (EdgeAction::Increment, true) | (EdgeAction::Decrement, false) => -1,
        }
    };
    levels
        .windows(2)
        .map(|pair| {
            let ((a, b), (next_a, next_b)) = (pair[0], pair[1]);
            edge(setting.channel0, a, next_a, next_b) + edge(setting.channel1, b, next_b, next_a)
        })
        .sum()
}

/// A forward cycle of an encoder, A leading B: (A, B) = (0, 0), (1, 0), (1, 1), (0, 1), (0, 0).
const FORWARD: [(bool, bool); 5] = [
    (false, false),
    (true, false),
    (true, true),
    (false, true),
    (false, false),
];

/// A pulse input counts the `edge`s of its line and nothing else (specification 11 §6.5): three
/// cycles of a square wave on A have three rising and three falling edges, so `rising` and
/// `falling` count 3 and `both` 6; edges of B count nothing, as a pulse unit's channel 1 holds.
/// Catches the rising and falling actions swapped, and a channel 1 that counts.
#[test]
fn a_pulse_input_counts_the_edges_it_names() {
    let wave = [
        (false, false),
        (true, false),
        (false, false),
        (true, false),
        (false, false),
        (true, false),
        (false, false),
    ];
    let on_b = [(false, false), (false, true), (false, false), (true, false)];
    for (edge, edges) in [(Edge::Rising, 3), (Edge::Falling, 3), (Edge::Both, 6)] {
        assert_eq!(count(pulse_setting(edge), &wave), edges, "{edge:?}");
        // The B transitions change nothing; the final rise of A is one rising edge.
        let from_a = i32::from(edge != Edge::Falling);
        assert_eq!(count(pulse_setting(edge), &on_b), from_a, "{edge:?} on B");
    }
    assert_eq!(pulse_setting(Edge::Rising).channel1, IDLE);
}

/// An encoder counts one at each edge of either line, up while A leads B and down while B leads A,
/// four counts per cycle of A (specification 11 §6.5): each step of the forward cycle is +1, the
/// cycle +4, the cycle backwards -4, and two cycles forward and one back +4. Catches a reversal on
/// the low control level instead of the high one (the forward cycle counting -4), and a channel
/// whose actions are swapped (a cycle counting 0, its edges cancelling).
#[test]
fn an_encoder_counts_four_per_cycle_with_the_sign_of_its_lead() {
    for step in FORWARD.windows(2) {
        assert_eq!(count(QUADRATURE_SETTING, step), 1, "{step:?}");
    }
    assert_eq!(count(QUADRATURE_SETTING, &FORWARD), 4);
    let mut backward = FORWARD;
    backward.reverse();
    assert_eq!(count(QUADRATURE_SETTING, &backward), -4);
    let there_and_back: Vec<(bool, bool)> = FORWARD
        .iter()
        .chain(&FORWARD[1..])
        .chain(&backward[1..])
        .copied()
        .collect();
    assert_eq!(count(QUADRATURE_SETTING, &there_and_back), 4);
}

/// Input `i` counts on PCNT unit `i` with its own lines: a pulse input on its line alone and an
/// encoder on `line` and `line_b`, whose count is the signed one. Catches inputs on other units
/// than their order gives, and an encoder planned without its B line.
#[test]
fn inputs_count_on_their_units_in_order() {
    let inputs = [
        counter_input(
            0,
            "GPIO4",
            CounterMode::Pulse {
                edge: Edge::Falling,
            },
        ),
        counter_input(
            4,
            "GPIO5",
            CounterMode::Quadrature {
                line_b: Some("GPIO6".into()),
            },
        ),
    ];
    let plans = counter_plan(&inputs);
    assert_eq!(
        plans,
        Ok(vec![
            CounterPlan {
                unit: 0,
                line: "GPIO4",
                line_b: None,
                setting: pulse_setting(Edge::Falling),
            },
            CounterPlan {
                unit: 1,
                line: "GPIO5",
                line_b: Some("GPIO6"),
                setting: QUADRATURE_SETTING,
            },
        ])
    );
    let signed = plans.map(|plans| {
        plans
            .iter()
            .map(CounterPlan::quadrature)
            .collect::<Vec<_>>()
    });
    assert_eq!(signed, Ok(vec![false, true]));
}

/// The PCNT has four units, one for each input (specification 34 Appendix D.2): four inputs are
/// planned and a fifth is refused. Catches a fifth input counted on a unit the C6 lacks.
#[test]
fn a_fifth_input_is_refused_for_the_four_units() {
    let pulse = |index: u32| {
        counter_input(
            index * 4,
            &format!("GPIO{index}"),
            CounterMode::Pulse { edge: Edge::Rising },
        )
    };
    let four: Vec<CounterInput> = (0..4).map(pulse).collect();
    assert_eq!(counter_plan(&four).map(|plans| plans.len()), Ok(4));
    let five: Vec<CounterInput> = (0..5).map(pulse).collect();
    assert_eq!(
        counter_plan(&five),
        Err(String::from(
            "counter: inputs: the C6's PCNT has 4 counters, and the driver names 5 inputs"
        ))
    );
}

/// The C6 counts an encoder's B on a pin of its own, so a `quadrature` input without `line_b`,
/// which the shared rules accept for Linux, is refused with its key before any hardware is touched
/// (specification 11 §6.5: on a chip `line` and `line_b` are an encoder's A and B). Catches an
/// encoder counted on A alone, which has no direction, and a refusal of another input's key.
#[test]
fn an_encoder_without_its_b_line_is_refused_on_the_c6() {
    let inputs = [
        counter_input(0, "GPIO4", CounterMode::Pulse { edge: Edge::Both }),
        counter_input(4, "GPIO5", CounterMode::Quadrature { line_b: None }),
    ];
    assert_eq!(
        counter_plan(&inputs),
        Err(String::from(
            "counter: inputs[1].line_b: is required on the C6 with mode = \"quadrature\": it \
             counts the encoder's B line on a pin of its own"
        ))
    );
}

/// Each of the nine CiA 301 rates (specification 11 §6.5) has the timing of a quantum of q cycles
/// of the 40 MHz clock and a bit of 1 + tseg_1 + tseg_2 quanta, with the prescaler 2q that esp-hal
/// halves on the C6: 40,000,000 / (q × (1 + tseg_1 + tseg_2)) is the rate.
/// - 10,000: q = 200, 20 quanta, 4,000 cycles; 20,000: 100 × 20 = 2,000; 50,000: 40 × 20 = 800;
///   100,000: 20 × 20 = 400; 125,000: 16 × 20 = 320; 250,000: 8 × 20 = 160; 500,000: 4 × 20 = 80;
///   1,000,000: 2 × 20 = 40, each sampled after 16 of 20 quanta, at 80 %.
/// - 800,000: 50 cycles, not a multiple of 20 quanta of an even size, so q = 2 and 25 quanta
///   (1 + 16 + 8), sampled after 17, at 68 %.
///
/// Catches the quantum passed without esp-hal's doubling, which would run the bus at twice the
/// rate, and a timing of another rate in the table.
#[test]
fn each_cia_301_rate_has_its_bit_timing() {
    let timing = |prescaler, tseg_1, tseg_2| {
        Some(CanBitTiming {
            prescaler,
            tseg_1,
            tseg_2,
            sync_jump_width: 3,
        })
    };
    assert_eq!(can_bit_timing(10_000), timing(400, 15, 4));
    assert_eq!(can_bit_timing(20_000), timing(200, 15, 4));
    assert_eq!(can_bit_timing(50_000), timing(80, 15, 4));
    assert_eq!(can_bit_timing(100_000), timing(40, 15, 4));
    assert_eq!(can_bit_timing(125_000), timing(32, 15, 4));
    assert_eq!(can_bit_timing(250_000), timing(16, 15, 4));
    assert_eq!(can_bit_timing(500_000), timing(8, 15, 4));
    assert_eq!(can_bit_timing(800_000), timing(4, 16, 8));
    assert_eq!(can_bit_timing(1_000_000), timing(4, 15, 4));
}

/// Each timing, as the controller takes it, makes its rate exactly and fits the TRM's fields.
/// esp-hal 1.2.2 halves the prescaler on the C6 and writes half of that less one into
/// `TWAI_BAUD_PRESC`, and the TRM's quantum is 2 × (`TWAI_BAUD_PRESC` + 1) cycles, so the quantum
/// is the prescaler / 2, whole only when that is even. `TWAI_TIME_SEG1` holds 1 to 16 quanta,
/// `TWAI_TIME_SEG2` 1 to 8 and `TWAI_SYNC_JUMP_WIDTH` 1 to 4, and the jump may not exceed segment 2
/// (ISO 11898-1). Catches an odd quantum that the register halves away, and a segment the register
/// cannot hold, either of which would run the bus at another rate than the record's.
#[test]
fn each_timing_makes_its_rate_within_the_registers() {
    for rate in trust_runtime_services::io_drivers::can::BIT_RATES {
        let Some(timing) = can_bit_timing(rate) else {
            panic!("no timing for {rate} bit/s");
        };
        let halved = u32::from(timing.prescaler) / 2;
        assert_eq!(halved % 2, 0, "{rate}: the quantum must be even");
        let register = halved / 2 - 1;
        let quantum = 2 * (register + 1);
        let bit = quantum * (1 + u32::from(timing.tseg_1) + u32::from(timing.tseg_2));
        assert_eq!(TWAI_CLOCK_HZ % bit, 0, "{rate}");
        assert_eq!(TWAI_CLOCK_HZ / bit, rate);
        assert!((1..=16).contains(&timing.tseg_1), "{rate}");
        assert!((1..=8).contains(&timing.tseg_2), "{rate}");
        assert!((1..=4).contains(&timing.sync_jump_width), "{rate}");
        assert!(timing.sync_jump_width <= timing.tseg_2, "{rate}");
    }
}

/// A rate outside CiA 301 has no timing. Catches a rate between the table's entries taken as the
/// nearest one, which would put the node on a bus at the wrong rate.
#[test]
fn a_rate_outside_cia_301_has_no_timing() {
    for rate in [0, 9_999, 33_333, 83_333, 2_000_000] {
        assert_eq!(can_bit_timing(rate), None, "{rate}");
    }
}

/// Whether the controller's single filter lets a data frame of the identifier `id` through, as
/// esp-hal's `new_from_code_mask` documents it: each identifier bit where the mask is set equals
/// the code's.
fn passes(filter: AcceptanceFilter, id: u32) -> bool {
    match filter {
        AcceptanceFilter::Standard { code, mask } => {
            ((id ^ u32::from(code)) & u32::from(mask)) == 0
        }
        AcceptanceFilter::Extended { code, mask } => ((id ^ code) & mask) == 0,
    }
}

/// The filter of 0x181 and 0x182: they differ in bits 0 and 1 (0x181 ^ 0x182 = 0x003), so the mask
/// is 0x7FF without them, 0x7FC, and 0x180 to 0x183 pass while 0x184 and 0x081 do not. One
/// receive of 0x7FF matches only 0x7FF. Receives of 0x100, 0x200 and 0x7FF differ in every bit
/// (0x300 | 0x6FF = 0x7FF), so the mask is 0 and all pass. Extended 0x18FF0001 and 0x18FF0002
/// give the mask 0x1FFFFFFC. Catches a mask that keeps the differing bits instead of the shared
/// ones, which would drop a receive's own frames, and a filter that drops either receive.
#[test]
fn the_filter_matches_the_bits_every_receive_shares() {
    let pair = acceptance_filter(&[receive(0x181, false, 0), receive(0x182, false, 1)]);
    assert_eq!(
        pair,
        Some(AcceptanceFilter::Standard {
            code: 0x181,
            mask: 0x7FC,
        })
    );
    if let Some(filter) = pair {
        for id in [0x180, 0x181, 0x182, 0x183] {
            assert!(passes(filter, id), "{id:#x}");
        }
        for id in [0x184, 0x081, 0x7FF] {
            assert!(!passes(filter, id), "{id:#x}");
        }
    }
    assert_eq!(
        acceptance_filter(&[receive(0x7FF, false, 0)]),
        Some(AcceptanceFilter::Standard {
            code: 0x7FF,
            mask: 0x7FF,
        })
    );
    assert_eq!(
        acceptance_filter(&[
            receive(0x100, false, 0),
            receive(0x200, false, 1),
            receive(0x7FF, false, 2),
        ]),
        Some(AcceptanceFilter::Standard {
            code: 0x100,
            mask: 0,
        })
    );
    let extended =
        acceptance_filter(&[receive(0x18FF_0001, true, 0), receive(0x18FF_0002, true, 1)]);
    assert_eq!(
        extended,
        Some(AcceptanceFilter::Extended {
            code: 0x18FF_0001,
            mask: 0x1FFF_FFFC,
        })
    );
    if let Some(filter) = extended {
        assert!(passes(filter, 0x18FF_0001) && passes(filter, 0x18FF_0002));
        assert!(!passes(filter, 0x18FF_0004));
    }
}

/// Without a receive, and with receives of both formats, no filter is set, so every frame passes
/// as with the controller's default filter. Catches a filter of one format, which would drop the
/// other format's receives.
#[test]
fn mixed_formats_and_no_receive_keep_every_frame() {
    assert_eq!(acceptance_filter(&[]), None);
    assert_eq!(
        acceptance_filter(&[receive(0x181, false, 0), receive(0x181, true, 1)]),
        None
    );
}

/// The plan of `TWAI1` at 500,000 bit/s on GPIO4 and GPIO5 with a receive of 0x181: controller 1,
/// the 500,000 timing (prescaler 8, 15 and 4 quanta), the pins as named and the exact filter of
/// 0x181. Catches `TWAI1` planned on controller 0 and the pins swapped.
#[test]
fn a_can_plan_names_its_controller_timing_pins_and_filter() {
    let params = can_params(
        "TWAI1",
        500_000,
        Some("GPIO4"),
        Some("GPIO5"),
        vec![receive(0x181, false, 0)],
    );
    assert_eq!(
        can_plan(&params),
        Ok(CanPlan {
            controller: 1,
            timing: CanBitTiming {
                prescaler: 8,
                tseg_1: 15,
                tseg_2: 4,
                sync_jump_width: 3,
            },
            tx: "GPIO4",
            rx: "GPIO5",
            filter: Some(AcceptanceFilter::Standard {
                code: 0x181,
                mask: 0x7FF,
            }),
        })
    );
    let first = can_params("TWAI0", 125_000, Some("GPIO1"), Some("GPIO2"), vec![]);
    assert_eq!(can_plan(&first).map(|plan| plan.controller), Ok(0));
}

/// A `can` driver is refused, in this order, for a line that is not one of the C6's controllers, a
/// rate its clock cannot make, and a missing `tx` or `rx` (Linux's `can0` names no pins, so a
/// record for Linux is refused at its line first). Catches a Linux interface name taken as a
/// controller, and the refusals' texts or keys changed.
#[test]
fn a_can_plan_refuses_in_order() {
    let refused = |params: &CanParams| can_plan(params).err();
    assert_eq!(
        refused(&can_params("can0", 500_000, None, None, vec![])),
        Some(String::from(
            "can: line: can0 is not a CAN controller of the C6: its controllers are TWAI0 and TWAI1"
        ))
    );
    assert_eq!(
        refused(&can_params("TWAI1", 123, None, None, vec![])),
        Some(String::from(
            "can: bitrate: TWAI1 cannot run at 123 bit/s from its 40000000 Hz clock"
        ))
    );
    assert_eq!(
        refused(&can_params("TWAI0", 500_000, None, None, vec![])),
        Some(String::from(
            "can: tx: TWAI0 needs the pin that goes to the transceiver's input"
        ))
    );
    assert_eq!(
        refused(&can_params("TWAI0", 500_000, Some("GPIO4"), None, vec![])),
        Some(String::from(
            "can: rx: TWAI0 needs the pin that comes from the transceiver's output"
        ))
    );
}

/// A frame of an identifier that already waits replaces it in its place with the newest data, and
/// a frame of a new identifier waits after the others; a standard and an extended frame of the same
/// number are two identifiers. So the queue holds at most one frame per identifier, never more
/// than the driver's transmits. Catches a queue that grows by a frame for each cycle the bus is
/// busy, old data sent after newer, and a replaced frame moved behind the others.
#[test]
fn a_waiting_frame_takes_the_newest_data_in_its_place() {
    let mut pending = VecDeque::new();
    queue_frame(&mut pending, frame(0x100, false, 1));
    queue_frame(&mut pending, frame(0x200, false, 2));
    queue_frame(&mut pending, frame(0x100, true, 3));
    queue_frame(&mut pending, frame(0x100, false, 4));
    assert_eq!(
        pending,
        VecDeque::from([
            frame(0x100, false, 4),
            frame(0x200, false, 2),
            frame(0x100, true, 3),
        ])
    );
}

/// A received data frame carries its bytes, zero after its length; a payload longer than 8 bytes
/// gives its first 8; a remote frame carries no data and gives no frame. Catches a remote frame
/// that would write zeros over a receive's input bytes.
#[test]
fn a_received_frame_carries_its_data_and_a_remote_frame_none() {
    let id = CanId {
        id: 0x181,
        extended: false,
    };
    assert_eq!(
        received_frame(id, false, &[1, 2, 3]),
        Some(CanFrame {
            id,
            len: 3,
            data: [1, 2, 3, 0, 0, 0, 0, 0],
        })
    );
    assert_eq!(
        received_frame(id, false, &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        Some(CanFrame {
            id,
            len: 8,
            data: [1, 2, 3, 4, 5, 6, 7, 8],
        })
    );
    assert_eq!(received_frame(id, true, &[]), None);
}

/// The driver's health is its worse path, faulted before degraded before ok, and the inputs' path
/// when both are as bad. Catches a degraded output path hiding a faulted input path.
#[test]
fn the_can_health_is_the_worse_path() {
    let ok = IoDriverHealth::Ok;
    let passive = degraded("error passive: 128 or more bus errors");
    let off = faulted("bus off");
    let timeout = faulted("no frame 0x181 for 100 ms");
    assert_eq!(worse_health(&ok, &ok), ok);
    assert_eq!(worse_health(&ok, &passive), passive);
    assert_eq!(worse_health(&off, &passive), off);
    assert_eq!(worse_health(&passive, &off), off);
    assert_eq!(worse_health(&timeout, &off), timeout);
}

/// ADC1's channels 0 to 6 are GPIO0 to GPIO6 (specification 34 Appendix D.2), and another pin is
/// refused with that reason. Catches GPIO7, the first pin past ADC1, read as an analog input.
#[test]
fn adc1_channels_are_gpio0_to_gpio6() {
    let pin = GpioPin::from_number;
    assert_eq!(adc1_channel_of(pin(0)), Some(0));
    assert_eq!(adc1_channel_of(pin(4)), Some(4));
    assert_eq!(adc1_channel_of(pin(6)), Some(6));
    assert_eq!(adc1_channel_of(pin(7)), None);
    assert_eq!(adc1_channel_of(pin(23)), None);
    assert_eq!(
        not_analog_input(pin(7)),
        "GPIO7 is not an analog input of the C6: its ADC1 channels 0 to 6 are GPIO0 to GPIO6"
    );
}

/// A pad a `pwm` output published is found by its pin until the output's guard is dropped with its
/// program, and a pad is found only by its own pin (`bench-loopback`). Catches a counter that would
/// count a pad no output drives any more, after the program that published it ended, and one that
/// counts another output's pad.
#[test]
fn a_published_pad_is_found_until_its_output_is_dropped() {
    let pads = LoopbackPads::<u8>::default();
    let (gpio4, gpio5) = (GpioPin::from_number(4), GpioPin::from_number(5));
    assert_eq!(pads.find(gpio4), None);
    let first = pads.publish(gpio4, 40);
    let second = pads.publish(gpio5, 50);
    assert_eq!((pads.find(gpio4), pads.find(gpio5)), (Some(40), Some(50)));
    assert_eq!(pads.find(GpioPin::from_number(6)), None);
    drop(first);
    assert_eq!((pads.find(gpio4), pads.find(gpio5)), (None, Some(50)));
    drop(second);
    assert_eq!(pads.find(gpio5), None);
}
