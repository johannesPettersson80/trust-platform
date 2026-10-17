//! Strict checks of device records; synthetic tests never constitute board evidence.
use super::{board::BoardProfile, trace_profile};
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const NUMERIC: [(&str, f64); 10] = [
    ("v_sqrt", std::f64::consts::SQRT_2),
    ("v_ln", std::f64::consts::LN_2),
    ("v_log", 1.0),
    ("v_exp", std::f64::consts::E),
    ("v_sin", 0.479_425_538_604_203),
    ("v_cos", 0.877_582_561_890_372_8),
    ("v_tan", 0.546_302_489_843_790_5),
    ("v_asin", std::f64::consts::FRAC_PI_6),
    ("v_acos", std::f64::consts::FRAC_PI_3),
    ("v_atan", std::f64::consts::FRAC_PI_4),
];
const PHASES: [&str; 13] = [
    "boot",
    "main-prepare",
    "main-instantiate",
    "main-run",
    "main-dropped",
    "numeric-prepare",
    "numeric-instantiate",
    "numeric-run",
    "numeric-dropped",
    "gpio-prepare",
    "gpio-instantiate",
    "gpio-run",
    "gpio-dropped",
];
const IO_PHASES: [(&str, u64); 9] = [
    ("BOOT", 0),
    ("STOP", 0),
    ("FAULT", 0),
    ("gpio-STOP", 0),
    ("gpio-pre-safe", 1),
    ("gpio-STOP-high", 0),
    ("gpio-pre-safe", 1),
    ("gpio-FAULT", 0),
    ("watchdog-BOOT", 0),
];

fn dec(text: &str) -> Result<u64> {
    text.parse().context("invalid unsigned decimal record")
}
fn hex(text: &str) -> Result<u64> {
    u64::from_str_radix(text, 16).context("invalid hexadecimal record")
}
fn numbers(fields: &[&str]) -> Result<Vec<u64>> {
    fields.iter().map(|v| dec(v)).collect()
}
/// The button's pin level against its pressed flag: `pressed` is set exactly when the pin reads
/// the level the board's button drives it to.
fn polarity(board: &BoardProfile, raw: u64, pressed: u64) -> Result<()> {
    ensure!(
        raw <= 1 && pressed <= 1 && (pressed == 1) == (raw == u64::from(board.button_pressed_high)),
        "button polarity mismatch"
    );
    Ok(())
}
fn stack(board: &BoardProfile, used: u64, remaining: u64) -> Result<()> {
    let window = board.stack_window_bytes;
    ensure!(
        used <= window
            && remaining <= window
            && used + remaining == window
            && remaining >= board.stack_headroom_bytes,
        "insufficient measured stack headroom"
    );
    Ok(())
}

fn record_order(board: &BoardProfile) -> Vec<String> {
    let mut order: Vec<String> = [
        "IDENTITY",
        "IO:BOOT",
        "MEM:boot",
        "PREP:main-prepare",
        "MEM:main-prepare",
        "MEM:main-instantiate",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    order.extend(std::iter::repeat_n("TRACE".to_owned(), 101));
    order.extend(
        [
            "MEM:main-run",
            "IO:STOP",
            "STATE:STOP",
            "IO:FAULT",
            "STATE:FAULT",
            "MEM:main-dropped",
            "PREP:numeric-prepare",
            "MEM:numeric-prepare",
            "MEM:numeric-instantiate",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    order.extend(NUMERIC.iter().map(|(name, _)| format!("NUM:{name}")));
    order.extend(
        [
            "NUM:exact_root",
            "NUM:exact_power",
            "NUM:sum",
            "NUM_STATE",
            "MEM:numeric-run",
            "MEM:numeric-dropped",
            "PREP:gpio-prepare",
            "MEM:gpio-prepare",
            "MEM:gpio-instantiate",
            "GPIO:physical",
            "GPIO:injected",
            "GPIO:injected",
            "GPIO:injected",
            "IO:gpio-STOP",
            "MEM:gpio-run",
            "STACK",
            "STACK",
            "STACK",
            "STACK",
            "IO:gpio-pre-safe",
            "IO:gpio-STOP-high",
            "IO:gpio-pre-safe",
            "IO:gpio-FAULT",
            "MEM:gpio-dropped",
            "STACK_PEAK",
            "STATE:WATCHDOG_ARM",
            "IDENTITY",
            "IO:watchdog-BOOT",
            "STATE:WATCHDOG_RESET",
            "DONE",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    if !trace_profile::reports_clock(board) {
        return order;
    }
    // The firmware prints the clock it started with right after each IDENTITY: the first
    // boot's and the watchdog boot's.
    let mut with_clock = Vec::with_capacity(order.len() + 2);
    for key in order {
        let identity = key == "IDENTITY";
        with_clock.push(key);
        if identity {
            with_clock.push("CLOCK".to_owned());
        }
    }
    with_clock
}

pub(super) fn verify(board: &BoardProfile, text: &str, oracle: &str) -> Result<Value> {
    ensure!(
        text.len() <= 256 * 1024 && text.ends_with('\n'),
        "oversized or truncated UART capture"
    );
    let expected: Vec<_> = oracle
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("time_ms") && !line.is_empty())
        .collect();
    ensure!(expected.len() == 101, "oracle must contain 101 samples");
    let order = record_order(board);
    let mut identities = Vec::new();
    let mut clocks = Vec::new();
    let mut rows = 0usize;
    let mut numeric = BTreeMap::new();
    let mut numeric_state = None;
    let mut memories = BTreeMap::new();
    let mut preparations = BTreeSet::new();
    let mut io_index = 0;
    let mut gpio = Vec::new();
    let mut depths = Vec::new();
    let mut stack_peak = None;
    let mut states = Vec::new();
    let mut done = false;
    let mut max_scan_us = 0;
    let mut last_irq = 0;
    let mut record_index = 0;
    let mut banner_lines: u64 = 0;
    let mut boot_banner_lines: u64 = 0;
    for (line_index, line) in text.lines().enumerate() {
        ensure!(!done, "records after completion");
        if !line.starts_with("B1,") {
            trace_profile::admit_banner(
                board,
                identities.len(),
                states.last().copied(),
                boot_banner_lines,
                line,
            )
            .with_context(|| format!("UART line {}", line_index + 1))?;
            banner_lines += 1;
            boot_banner_lines += 1;
            continue;
        }
        let record = record_index;
        record_index += 1;
        let f: Vec<_> = line.trim_end_matches('\r').split(',').collect();
        let kind = f.get(1).copied().context("missing record kind")?;
        let key = if ["MEM", "PREP", "IO", "GPIO", "NUM", "STATE"].contains(&kind) {
            format!("{kind}:{}", f.get(2).context("missing record phase")?)
        } else {
            kind.to_owned()
        };
        ensure!(
            order.get(record) == Some(&key),
            "record order mismatch at line {}: {key}",
            line_index + 1
        );
        match f.get(1).copied() {
            Some("IDENTITY") => {
                ensure!(
                    f.len() == 10 && identities.len() < 2,
                    "malformed or duplicate identity"
                );
                let id = [
                    hex(f[2])?,
                    hex(f[3])?,
                    hex(f[4])?,
                    hex(f[5])?,
                    dec(f[6])?,
                    dec(f[7])?,
                    hex(f[8])?,
                    hex(f[9])?,
                ];
                trace_profile::identity(board, &id, identities.len())?;
                if identities.is_empty() {
                    ensure!(record == 0, "identity must start capture");
                } else {
                    ensure!(
                        states.last() == Some(&"WATCHDOG_ARM"),
                        "missing independent watchdog reset evidence"
                    );
                }
                identities.push(id);
            }
            Some("CLOCK") => clocks.push(
                trace_profile::clock(board, &f)
                    .with_context(|| format!("UART line {}", line_index + 1))?,
            ),
            Some("TRACE") => {
                ensure!(
                    f.len() == 13
                        && rows < expected.len()
                        && identities.len() == 1
                        && states.is_empty(),
                    "malformed or misplaced trace"
                );
                ensure!(
                    f[2..9].join(",") == expected[rows],
                    "logical trace mismatch at sample {rows}"
                );
                let n = numbers(&f[9..])?;
                ensure!(
                    n[0] == 0 && n[1] < 10_000 && n[2] < board.scan_cycle_bound && n[3] >= last_irq,
                    "scan timing, overrun or IRQ mismatch"
                );
                if rows > 0 {
                    ensure!(n[3] > last_irq, "SysTick did not advance between scans");
                }
                last_irq = n[3];
                max_scan_us = max_scan_us.max(n[1]);
                rows += 1;
            }
            Some("NUM") => {
                ensure!(
                    f.len() == 4 && numeric.insert(f[2].to_owned(), hex(f[3])?).is_none(),
                    "malformed or duplicate numeric value"
                );
            }
            Some("NUM_STATE") => {
                ensure!(
                    f.len() == 10 && numeric_state.is_none(),
                    "malformed or duplicate numeric state"
                );
                let n = numbers(&f[2..])?;
                ensure!(
                    n[0..3] == [15_000_000, 1, 40]
                        && n[3] < 10_000
                        && n[4] < board.scan_cycle_bound
                        && n[5..8] == [1_000_000_000, 0, 101],
                    "numeric state mismatch"
                );
                numeric_state = Some(n);
            }
            Some("MEM") => {
                ensure!(
                    f.len() == 11 && PHASES.contains(&f[2]) && !memories.contains_key(f[2]),
                    "malformed or duplicate memory phase"
                );
                let n = numbers(&f[3..])?;
                ensure!(
                    n[0] <= n[1] && n[1] <= board.heap_bytes && n[3] == 0,
                    "heap exhausted or invalid accounting"
                );
                stack(board, n[4], n[5])?;
                ensure!(
                    n[6] > 0 && n[6] <= 32 && n[7].is_power_of_two() && n[7] <= 8,
                    "unexpected target Value layout"
                );
                if f[2].ends_with("-dropped") || f[2] == "boot" {
                    ensure!(n[0] == 0, "live heap after drop or before preparation");
                }
                memories.insert(f[2].to_owned(), n);
            }
            Some("PREP") => {
                ensure!(
                    f.len() == 5
                        && ["main-prepare", "numeric-prepare", "gpio-prepare"].contains(&f[2])
                        && preparations.insert(f[2].to_owned()),
                    "malformed or duplicate preparation"
                );
                ensure!(
                    dec(f[3])? <= 512 * 1024 && dec(f[4])? <= 1_000_000,
                    "preparation profile exceeded"
                );
            }
            Some("IO") => {
                ensure!(
                    f.len() == 7 && io_index < IO_PHASES.len(),
                    "malformed or extra IO record"
                );
                let n = numbers(&f[3..])?;
                polarity(board, n[0], n[1])?;
                let (phase, output) = IO_PHASES[io_index];
                ensure!(
                    f[2] == phase && n[2] == output && n[3] == output,
                    "physical safe-output sequence mismatch"
                );
                io_index += 1;
            }
            Some("GPIO") => {
                ensure!(
                    f.len() == 9 && gpio.len() < 4,
                    "malformed or extra GPIO record"
                );
                let n = numbers(&f[3..])?;
                polarity(board, n[1], n[2])?;
                ensure!(
                    n[0] == (gpio.len() as u64 + 1) * 10
                        && n[3] <= 1
                        && n[3] == n[4]
                        && n[4] == n[5],
                    "GPIO image/pad mismatch"
                );
                if gpio.is_empty() {
                    ensure!(
                        f[2] == "physical" && n[3] == n[2],
                        "physical input was substituted"
                    );
                } else {
                    ensure!(
                        f[2] == "injected" && n[3] == u64::from(gpio.len() == 2),
                        "injected input sequence mismatch"
                    );
                }
                gpio.push(n);
            }
            Some("STACK") => {
                ensure!(
                    f.len() == 6 && depths.len() < 4,
                    "malformed or duplicate stack depth"
                );
                let n = numbers(&f[2..])?;
                ensure!(
                    n[0] == depths.len() as u64 + 1 && n[3] > last_irq,
                    "stack depth/IRQ sequence mismatch"
                );
                stack(board, n[1], n[2])?;
                last_irq = n[3];
                depths.push(n);
            }
            Some("STACK_PEAK") => {
                ensure!(
                    f.len() == 3 && stack_peak.is_none(),
                    "malformed or duplicate stack peak"
                );
                let peak = dec(f[2])?;
                ensure!(
                    peak <= board.stack_peak_bound()
                        && depths.iter().all(|n| n[1] <= peak)
                        && memories.values().all(|n| n[4] <= peak),
                    "invalid cumulative stack peak"
                );
                stack_peak = Some(peak);
            }
            Some("STATE") => {
                let name = *f.get(2).context("missing state name")?;
                let required = ["STOP", "FAULT", "WATCHDOG_ARM", "WATCHDOG_RESET"];
                ensure!(
                    states.len() < required.len() && name == required[states.len()],
                    "state transition mismatch"
                );
                if name == "FAULT" {
                    ensure!(
                        f.len() == 4 && f[3] == "runtime_execution_timeout",
                        "wrong deadline fault identity"
                    );
                } else {
                    ensure!(f.len() == 3, "malformed state");
                }
                if name == "STOP" {
                    ensure!(rows == 101, "STOP before trace completed");
                }
                if name == "WATCHDOG_RESET" {
                    ensure!(
                        identities.len() == 2,
                        "reset not confirmed by hardware identity"
                    );
                }
                if name == "WATCHDOG_ARM" {
                    boot_banner_lines = 0;
                }
                states.push(name);
            }
            Some("DONE") => {
                ensure!(
                    f.len() == 2 && states.last() == Some(&"WATCHDOG_RESET"),
                    "premature completion"
                );
                done = true;
            }
            Some("FAIL") => bail!("firmware reported failure: {line}"),
            _ => bail!("unknown record: {line}"),
        }
    }
    ensure!(
        done && rows == 101
            && identities.len() == 2
            && memories.len() == PHASES.len()
            && preparations.len() == 3
            && io_index == IO_PHASES.len()
            && gpio.len() == 4
            && depths.len() == 4
            && stack_peak.is_some()
            && numeric_state.is_some(),
        "incomplete hardware evidence"
    );
    ensure!(numeric.len() == 13, "missing or extra numeric values");
    for (name, reference) in NUMERIC {
        let value = f64::from_bits(*numeric.get(name).context("missing numeric field")?);
        let tolerance = 8.0 * f64::EPSILON * reference.abs().max(1.0);
        ensure!(
            value.is_finite() && (value - reference).abs() <= tolerance,
            "numeric accuracy failed: {name}"
        );
    }
    for (name, bits) in [
        ("exact_root", u64::from(3.0f32.to_bits())),
        ("exact_power", 8.0f64.to_bits()),
        ("sum", u64::from(3.75f32.to_bits())),
    ] {
        ensure!(
            numeric.get(name) == Some(&bits),
            "exact numeric mismatch: {name}"
        );
    }
    let mut report = json!({"status":"passed", "board":board.name, "boot_banner_lines":banner_lines, "identity":identities, "trace_samples":rows, "max_main_scan_us":max_scan_us,
        "numeric_state":numeric_state, "numeric_bits":numeric, "memory":memories, "gpio":gpio, "stack_depths":depths,
        "stack_peak":stack_peak, "manual_button_transition":"unverified", "physical_markings":"unverified", "optical_led":"unverified"});
    // One entry per boot, in capture order, like `identity`. Only a board that prints the record
    // has the key, so the other boards' reports are what they were.
    if trace_profile::reports_clock(board) {
        report["clock"] = Value::Array(clocks);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;
    const ORACLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/trust-runtime/tests/fixtures/portability/stbc-2.0/expected-a4-trace.csv"
    ));
    /// The complete record protocol as the C6 firmware prints it, without boot banners: the C6's
    /// IDENTITY words and a released button reading high. The other captures are made from it.
    fn synthetic() -> String {
        let mut out = format!("{C6_FIRST}\nB1,IO,BOOT,1,0,0,0\n");
        for (index, row) in ORACLE
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("time_ms") && !l.is_empty())
            .enumerate()
        {
            writeln!(out, "B1,TRACE,{row},0,100,8400,{}", index * 10 + 1).unwrap();
        }
        out.push_str("B1,IO,STOP,1,0,0,0\nB1,STATE,STOP\nB1,IO,FAULT,1,0,0,0\nB1,STATE,FAULT,runtime_execution_timeout\n");
        for (name, value) in NUMERIC {
            writeln!(out, "B1,NUM,{name},{:016x}", value.to_bits()).unwrap();
        }
        for (name, bits) in [
            ("exact_root", u64::from(3.0f32.to_bits())),
            ("exact_power", 8.0f64.to_bits()),
            ("sum", u64::from(3.75f32.to_bits())),
        ] {
            writeln!(out, "B1,NUM,{name},{bits:016x}").unwrap();
        }
        out.push_str("B1,NUM_STATE,15000000,1,40,200,16800,1000000000,0,101\n");
        for phase in PHASES {
            writeln!(out, "B1,MEM,{phase},0,4096,10,0,4096,12288,24,8").unwrap();
        }
        for phase in ["main-prepare", "numeric-prepare", "gpio-prepare"] {
            writeln!(out, "B1,PREP,{phase},12000,20000").unwrap();
        }
        out.push_str("B1,GPIO,physical,10,1,0,0,0,0\nB1,GPIO,injected,20,1,0,0,0,0\nB1,GPIO,injected,30,1,0,1,1,1\nB1,GPIO,injected,40,1,0,0,0,0\nB1,IO,gpio-STOP,1,0,0,0\n");
        for depth in 1..=4 {
            writeln!(out, "B1,STACK,{depth},4096,12288,{}", 1100 + depth * 10).unwrap();
        }
        out.push_str("B1,STACK_PEAK,4096\nB1,IO,gpio-pre-safe,1,0,1,1\nB1,IO,gpio-STOP-high,1,0,0,0\nB1,IO,gpio-pre-safe,1,0,1,1\nB1,IO,gpio-FAULT,1,0,0,0\nB1,STATE,WATCHDOG_ARM\n");
        writeln!(out, "{C6_SECOND}").unwrap();
        out.push_str("B1,IO,watchdog-BOOT,1,0,0,0\nB1,STATE,WATCHDOG_RESET\nB1,DONE\n");
        let mut lines: Vec<_> = out.lines().map(str::to_owned).collect();
        let mut sorted = String::new();
        for key in record_order(&c6()) {
            let index = lines
                .iter()
                .position(|line| {
                    let f: Vec<_> = line.split(',').collect();
                    let actual = if ["MEM", "PREP", "IO", "GPIO", "NUM", "STATE"].contains(&f[1]) {
                        format!("{}:{}", f[1], f[2])
                    } else {
                        f[1].to_owned()
                    };
                    actual == key
                })
                .expect("synthetic record exists");
            writeln!(sorted, "{}", lines.remove(index)).unwrap();
        }
        assert!(lines.is_empty());
        sorted
    }
    #[test]
    fn complete_synthetic_protocol_is_accepted_without_claiming_board_execution() {
        assert_eq!(
            verify(&c6(), &synthetic(), ORACLE).unwrap()["trace_samples"],
            101
        );
    }
    #[test]
    fn missing_corrupt_duplicate_and_unsafe_records_are_rejected() {
        let good = synthetic();
        for (from, to) in [
            ("B1,DONE\n", ""),
            ("00000007\nB1,IO,watchdog", "00000000\nB1,IO,watchdog"),
            ("B1,TRACE,30,4,1,1,0,0,25,", "B1,TRACE,30,4,1,1,0,0,30,"),
            (
                "B1,GPIO,injected,30,1,0,1,1,1",
                "B1,GPIO,injected,30,1,0,1,1,0",
            ),
            ("B1,STACK,4,4096,12288,1140", "B1,STACK,4,15000,1384,1140"),
            (
                "B1,STATE,WATCHDOG_ARM",
                "B1,STATE,WATCHDOG_ARM\nB1,STATE,WATCHDOG_ARM",
            ),
            ("B1,IO,gpio-FAULT,1,0,0,0", "B1,IO,gpio-FAULT,1,0,0,1"),
        ] {
            assert!(good.contains(from));
            assert!(
                verify(&c6(), &good.replace(from, to), ORACLE).is_err(),
                "{from}"
            );
        }
        let mut reordered: Vec<_> = good.lines().collect();
        let memory_index = reordered
            .iter()
            .position(|line| line.starts_with("B1,MEM,main-prepare"))
            .unwrap();
        reordered.swap(memory_index, memory_index + 1);
        assert!(verify(&c6(), &(reordered.join("\n") + "\n"), ORACLE).is_err());
        let nan = good.replace(
            &format!("{:016x}", std::f64::consts::SQRT_2.to_bits()),
            "7ff8000000000000",
        );
        assert!(verify(&c6(), &nan, ORACLE).is_err());
        assert!(verify(&c6(), good.trim_end(), ORACLE).is_err());
        assert!(verify(&c6(), &(good + "B1,DONE\n"), ORACLE).is_err());
    }

    const C6_FIRST: &str =
        "B1,IDENTITY,0000000d,00000002,0000206e,f11704c0,8192,160000000,40903105,00000015";
    const C6_SECOND: &str =
        "B1,IDENTITY,0000000d,00000002,0000206e,f11704c0,8192,160000000,40903105,00000007";

    fn c6() -> BoardProfile {
        BoardProfile::esp32c6_devkitc_1().unwrap()
    }

    fn banner(lines: usize) -> String {
        (0..lines)
            .map(|index| format!("ESP-ROM boot line {index}\n"))
            .collect()
    }

    /// The synthetic protocol with `before_first` banner lines before the first IDENTITY and
    /// `before_second` banner lines after `STATE,WATCHDOG_ARM`.
    fn c6_synthetic(before_first: usize, before_second: usize) -> String {
        let records = synthetic();
        let mut out = banner(before_first);
        for line in records.lines() {
            out.push_str(line);
            out.push('\n');
            if line == "B1,STATE,WATCHDOG_ARM" {
                out.push_str(&banner(before_second));
            }
        }
        out
    }

    #[test]
    fn c6_capture_with_boot_banners_is_accepted_and_counted() {
        let report = verify(&c6(), &c6_synthetic(30, 30), ORACLE).unwrap();
        assert_eq!(report["trace_samples"], 101);
        assert_eq!(report["boot_banner_lines"], 60);
        assert_eq!(report["board"], "esp32c6-devkitc-1");
        let full = verify(&c6(), &c6_synthetic(64, 64), ORACLE).unwrap();
        assert_eq!(full["boot_banner_lines"], 128);
        let power_on = c6_synthetic(30, 30).replace(",40903105,00000015", ",40903105,00000001");
        assert!(verify(&c6(), &power_on, ORACLE).is_ok());
    }

    #[test]
    fn c6_identity_mutations_are_rejected() {
        let good = c6_synthetic(30, 30);
        let mutations = [
            good.replace("0000206e,f11704c0", "0000206e,f11704c1"),
            good.replace(",40903105,", ",40903125,"),
            good.replace(",40903105,", ",4090310d,"),
            good.replace(C6_SECOND, &C6_SECOND.replace(",00000007", ",0000000b")),
            good.replace(C6_FIRST, &C6_FIRST.replace(",00000015", ",00000007")),
        ];
        for mutated in mutations {
            assert!(verify(&c6(), &mutated, ORACLE).is_err());
        }
    }

    #[test]
    fn c6_banner_mutations_are_rejected() {
        let good = c6_synthetic(30, 30);
        let late = "ESP-ROM late line\n";
        let after_first = good.replace(&format!("{C6_FIRST}\n"), &format!("{C6_FIRST}\n{late}"));
        let after_second = good.replace(&format!("{C6_SECOND}\n"), &format!("{C6_SECOND}\n{late}"));
        assert!(verify(&c6(), &after_first, ORACLE).is_err());
        assert!(verify(&c6(), &after_second, ORACLE).is_err());
        assert!(verify(&c6(), &c6_synthetic(65, 30), ORACLE).is_err());
        assert!(verify(&c6(), &c6_synthetic(30, 65), ORACLE).is_err());
        let non_printable = good.replacen("ESP-ROM boot line 3\n", "ESP-ROM boot\u{7} line 3\n", 1);
        assert!(verify(&c6(), &non_printable, ORACLE).is_err());
    }

    #[test]
    fn scan_cycle_bounds_are_board_values_for_trace_and_numeric_state() {
        let good = c6_synthetic(30, 30);
        let inside = good.replacen(",0,100,8400,", ",0,100,1599999,", 1);
        assert!(verify(&c6(), &inside, ORACLE).is_ok());
        let at_bound = good.replacen(",0,100,8400,", ",0,100,1600000,", 1);
        assert!(verify(&c6(), &at_bound, ORACLE).is_err());
        let state_at_bound = good.replace(",16800,1000000000,", ",1600000,1000000000,");
        assert!(verify(&c6(), &state_at_bound, ORACLE).is_err());
    }

    const F407_FIRST: &str =
        "B1,IDENTITY,100f6413,0034002d,34314715,35363630,1024,168000000,00000000,00000000";
    const F407_SECOND: &str =
        "B1,IDENTITY,100f6413,0034002d,34314715,35363630,1024,168000000,00000000,20000000";
    const F407_CLOCK: &str = "B1,CLOCK,HSE,7998123,8000000,168000000,measured";

    fn disc1() -> BoardProfile {
        BoardProfile::stm32f407g_disc1().unwrap()
    }

    /// The records as a board whose button reads high while pressed prints them. The C6's button
    /// reads high while released, so the pin level of every IO and GPIO record is inverted here:
    /// a released button reads low.
    fn button_high_when_pressed(records: &str) -> String {
        records
            .lines()
            .map(|line| {
                let mut fields: Vec<&str> = line.split(',').collect();
                let kind = fields[1];
                match kind {
                    "IO" => fields[3] = "0",
                    "GPIO" => fields[4] = "0",
                    _ => {}
                }
                fields.join(",") + "\n"
            })
            .collect()
    }

    /// The synthetic protocol as the DISC1 firmware prints it: the F407's IDENTITY words with a
    /// CLOCK record right after each, and a released button reading low.
    fn f407_synthetic() -> String {
        button_high_when_pressed(&synthetic())
            .replace(C6_FIRST, &format!("{F407_FIRST}\n{F407_CLOCK}"))
            .replace(C6_SECOND, &format!("{F407_SECOND}\n{F407_CLOCK}"))
    }

    #[test]
    fn f407_capture_with_a_clock_record_after_each_identity_is_accepted_and_reported() {
        let report = verify(&disc1(), &f407_synthetic(), ORACLE).unwrap();
        assert_eq!(report["status"], "passed");
        assert_eq!(report["board"], "stm32f407g-disc1");
        assert_eq!(report["trace_samples"], 101);
        assert_eq!(report["boot_banner_lines"], 0);
        assert_eq!(report["identity"].as_array().unwrap().len(), 2);
        assert_eq!(report["identity"][0][0], 0x100f_6413_u64);
        assert_eq!(report["identity"][1][7], 0x2000_0000_u64);
        let clock = serde_json::json!({"source": "HSE", "measured_hz": 7_998_123,
            "crystal_hz": 8_000_000, "sysclk_hz": 168_000_000, "note": "measured"});
        assert_eq!(report["clock"], serde_json::json!([clock, clock]));
        // The F407 admits no boot banner and no foreign line.
        let with_banner = format!("ESP-ROM boot line 0\n{}", f407_synthetic());
        assert!(verify(&disc1(), &with_banner, ORACLE).is_err());
    }

    #[test]
    fn f407_falls_back_to_the_hsi_without_failing_the_capture() {
        let hsi = "B1,CLOCK,HSI,0,0,168000000,no-capture";
        let both = f407_synthetic().replace(F407_CLOCK, hsi);
        let report = verify(&disc1(), &both, ORACLE).unwrap();
        assert_eq!(report["clock"][0]["source"], "HSI");
        assert_eq!(report["clock"][1]["note"], "no-capture");
        // The two boots measure on their own and need not agree.
        let mixed = f407_synthetic().replacen(F407_CLOCK, hsi, 1);
        let report = verify(&disc1(), &mixed, ORACLE).unwrap();
        assert_eq!(report["clock"][0]["source"], "HSI");
        assert_eq!(report["clock"][1]["source"], "HSE");
    }

    #[test]
    fn f407_needs_one_clock_record_right_after_each_identity() {
        let good = f407_synthetic();
        let first = format!("{F407_FIRST}\n{F407_CLOCK}\n");
        let second = format!("{F407_SECOND}\n{F407_CLOCK}\n");
        let io_boot = "B1,IO,BOOT,0,0,0,0\n";
        let cases = [
            // None at all, and the record of one boot missing.
            good.replace(&format!("{F407_CLOCK}\n"), ""),
            good.replace(&first, &format!("{F407_FIRST}\n")),
            good.replace(&second, &format!("{F407_SECOND}\n")),
            // Twice after the same IDENTITY.
            good.replace(&first, &format!("{first}{F407_CLOCK}\n")),
            good.replace(&second, &format!("{second}{F407_CLOCK}\n")),
            // Before the IDENTITY, and after the record that follows it.
            good.replace(&first, &format!("{F407_CLOCK}\n{F407_FIRST}\n")),
            good.replace(
                &format!("{first}{io_boot}"),
                &format!("{F407_FIRST}\n{io_boot}{F407_CLOCK}\n"),
            ),
        ];
        for capture in cases {
            assert_ne!(capture, good);
            let error = verify(&disc1(), &capture, ORACLE).unwrap_err().to_string();
            assert!(error.contains("record order mismatch"), "{error}");
        }
    }

    #[test]
    fn f407_clock_values_are_checked_in_the_capture() {
        let good = f407_synthetic();
        for (clock, expected) in [
            (
                "B1,CLOCK,HSE,7998123,8000000,84000000,measured",
                "differs from the profile",
            ),
            (
                "B1,CLOCK,HSE,7998123,0,168000000,measured",
                "outside 4 to 26 MHz",
            ),
            (
                "B1,CLOCK,HSE,7998123,27000000,168000000,measured",
                "outside 4 to 26 MHz",
            ),
            (
                "B1,CLOCK,PLL,7998123,8000000,168000000,measured",
                "unknown clock source",
            ),
            (
                "B1,CLOCK,HSE,7998123,8000000,168000000",
                "malformed clock record",
            ),
            (
                "B1,CLOCK,HSI,0,8000000,168000000,no-capture",
                "no crystal figure",
            ),
        ] {
            let capture = good.replace(F407_CLOCK, clock);
            let error = format!("{:#}", verify(&disc1(), &capture, ORACLE).unwrap_err());
            assert!(error.contains(expected), "{clock}: {error}");
            // The error names the capture line of the record.
            assert!(error.contains("UART line 2"), "{clock}: {error}");
        }
    }

    #[test]
    fn f407_identity_is_pinned_to_the_owners_chip() {
        let good = f407_synthetic();
        let mutations = [
            good.replace("100f6413", "10016433"),
            good.replace("0034002d", "0034002e"),
            good.replace("34314715", "34314716"),
            good.replace("35363630", "35363631"),
            good.replace(",1024,", ",512,"),
            good.replace(",168000000,00000000,", ",84000000,00000000,"),
            good.replace(
                F407_FIRST,
                &F407_FIRST.replace(",00000000,00000000", ",00400000,00000000"),
            ),
            good.replace(F407_SECOND, &F407_SECOND.replace(",20000000", ",00000000")),
        ];
        for mutated in mutations {
            assert_ne!(mutated, good);
            assert!(verify(&disc1(), &mutated, ORACLE).is_err());
        }
        // No board's capture is accepted by another board's profile.
        assert!(verify(&disc1(), &synthetic(), ORACLE).is_err());
        assert!(verify(&disc1(), &c6_synthetic(30, 30), ORACLE).is_err());
        assert!(verify(&c6(), &good, ORACLE).is_err());
    }

    #[test]
    fn button_polarity_is_a_board_value() {
        let good = f407_synthetic();
        // The DISC1's B1 reads high while pressed: the pin level and the pressed flag agree.
        let io_pressed = good.replace("B1,IO,STOP,0,0,0,0", "B1,IO,STOP,1,1,0,0");
        assert_ne!(io_pressed, good);
        assert!(verify(&disc1(), &io_pressed, ORACLE).is_ok());
        let gpio_pressed = good.replace(
            "B1,GPIO,physical,10,0,0,0,0,0",
            "B1,GPIO,physical,10,1,1,1,1,1",
        );
        assert_ne!(gpio_pressed, good);
        assert!(verify(&disc1(), &gpio_pressed, ORACLE).is_ok());
        // The C6's convention, high while released, is refused on the DISC1.
        for wrong in [
            good.replace("B1,IO,STOP,0,0,0,0", "B1,IO,STOP,1,0,0,0"),
            good.replace("B1,IO,STOP,0,0,0,0", "B1,IO,STOP,0,1,0,0"),
            good.replace(
                "B1,GPIO,physical,10,0,0,0,0,0",
                "B1,GPIO,physical,10,1,0,0,0,0",
            ),
            good.replace(
                "B1,GPIO,injected,20,0,0,0,0,0",
                "B1,GPIO,injected,20,0,1,0,0,0",
            ),
        ] {
            assert_ne!(wrong, good);
            let error = verify(&disc1(), &wrong, ORACLE).unwrap_err().to_string();
            assert!(error.contains("polarity"), "{error}");
        }
        // The DISC1's convention is refused on the C6, whose button reads low while pressed, and
        // the C6's own is still accepted.
        let c6_capture = c6_synthetic(30, 30);
        for refused in ["B1,IO,STOP,1,1,0,0", "B1,IO,STOP,0,0,0,0"] {
            let capture = c6_capture.replace("B1,IO,STOP,1,0,0,0", refused);
            assert_ne!(capture, c6_capture);
            assert!(verify(&c6(), &capture, ORACLE).is_err(), "{refused}");
        }
        let pressed = c6_capture.replace("B1,IO,STOP,1,0,0,0", "B1,IO,STOP,0,1,0,0");
        assert!(verify(&c6(), &pressed, ORACLE).is_ok());
    }

    #[test]
    fn a_c6_capture_still_refuses_a_clock_record() {
        let c6_capture =
            c6_synthetic(30, 30).replace(C6_FIRST, &format!("{C6_FIRST}\n{F407_CLOCK}"));
        let error = verify(&c6(), &c6_capture, ORACLE).unwrap_err().to_string();
        assert!(
            error.contains("record order mismatch") && error.contains("CLOCK"),
            "{error}"
        );
    }

    #[test]
    fn reports_keep_their_keys_and_only_the_f407_adds_a_clock_entry() {
        let keys = |report: &Value| -> BTreeSet<String> {
            report.as_object().unwrap().keys().cloned().collect()
        };
        let expected: BTreeSet<String> = [
            "status",
            "board",
            "boot_banner_lines",
            "identity",
            "trace_samples",
            "max_main_scan_us",
            "numeric_state",
            "numeric_bits",
            "memory",
            "gpio",
            "stack_depths",
            "stack_peak",
            "manual_button_transition",
            "physical_markings",
            "optical_led",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let c6_report = verify(&c6(), &c6_synthetic(30, 30), ORACLE).unwrap();
        assert_eq!(keys(&c6_report), expected);
        let mut with_clock = expected;
        with_clock.insert("clock".to_owned());
        let f407 = verify(&disc1(), &f407_synthetic(), ORACLE).unwrap();
        assert_eq!(keys(&f407), with_clock);
    }

    #[test]
    fn f407_scan_cycle_bound_is_ten_milliseconds_at_168_mhz() {
        let good = f407_synthetic();
        let inside = good.replacen(",0,100,8400,", ",0,100,1679999,", 1);
        assert_ne!(inside, good);
        assert!(verify(&disc1(), &inside, ORACLE).is_ok());
        let at_bound = good.replacen(",0,100,8400,", ",0,100,1680000,", 1);
        assert!(verify(&disc1(), &at_bound, ORACLE).is_err());
        let state_inside = good.replace(",16800,1000000000,", ",1679999,1000000000,");
        assert_ne!(state_inside, good);
        assert!(verify(&disc1(), &state_inside, ORACLE).is_ok());
        let state_at_bound = good.replace(",16800,1000000000,", ",1680000,1000000000,");
        assert!(verify(&disc1(), &state_at_bound, ORACLE).is_err());
    }
}
