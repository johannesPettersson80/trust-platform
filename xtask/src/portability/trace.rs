//! Strict checks of device records; synthetic tests never constitute board evidence.
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
fn polarity(raw: u64, pressed: u64) -> Result<()> {
    ensure!(
        raw <= 1 && pressed <= 1 && raw + pressed == 1,
        "PC13 polarity mismatch"
    );
    Ok(())
}
fn stack(used: u64, remaining: u64) -> Result<()> {
    ensure!(
        used <= 16384 && remaining <= 16384 && used + remaining == 16384 && remaining >= 2048,
        "insufficient measured stack headroom"
    );
    Ok(())
}

fn record_order() -> Vec<String> {
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
    order
}

pub(super) fn verify(text: &str, oracle: &str) -> Result<Value> {
    ensure!(
        text.len() <= 256 * 1024 && text.ends_with('\n'),
        "oversized or truncated UART capture"
    );
    let expected: Vec<_> = oracle
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("time_ms") && !line.is_empty())
        .collect();
    ensure!(expected.len() == 101, "oracle must contain 101 samples");
    let order = record_order();
    let mut identities = Vec::new();
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
    for (line_index, line) in text.lines().enumerate() {
        ensure!(!done, "records after completion");
        let f: Vec<_> = line.trim_end_matches('\r').split(',').collect();
        ensure!(
            f.first() == Some(&"B1"),
            "foreign UART record at line {}",
            line_index + 1
        );
        let kind = f.get(1).copied().context("missing record kind")?;
        let key = if ["MEM", "PREP", "IO", "GPIO", "NUM", "STATE"].contains(&kind) {
            format!("{kind}:{}", f.get(2).context("missing record phase")?)
        } else {
            kind.to_owned()
        };
        ensure!(
            order.get(line_index) == Some(&key),
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
                ensure!(
                    id[0] == 0x10016433 && id[1..4] == [0x00410016, 0x30395119, 0x35363638],
                    "wrong connected reference device"
                );
                ensure!(
                    id[4] == 512 && id[5] == 84_000_000 && id[6] & ((3 << 22) | (1 << 24)) == 0,
                    "identity/profile mismatch"
                );
                if identities.is_empty() {
                    ensure!(line_index == 0, "identity must start capture");
                } else {
                    ensure!(
                        states.last() == Some(&"WATCHDOG_ARM") && id[7] & (1 << 29) != 0,
                        "missing independent watchdog reset evidence"
                    );
                }
                identities.push(id);
            }
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
                    n[0] == 0 && n[1] < 10_000 && n[2] < 840_000 && n[3] >= last_irq,
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
                        && n[4] < 840_000
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
                    n[0] <= n[1] && n[1] <= 72 * 1024 && n[3] == 0,
                    "heap exhausted or invalid accounting"
                );
                stack(n[4], n[5])?;
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
                polarity(n[0], n[1])?;
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
                polarity(n[1], n[2])?;
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
                stack(n[1], n[2])?;
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
                    peak <= 14336
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
    Ok(
        json!({"status":"passed", "identity":identities, "trace_samples":rows, "max_main_scan_us":max_scan_us,
        "numeric_state":numeric_state, "numeric_bits":numeric, "memory":memories, "gpio":gpio, "stack_depths":depths,
        "stack_peak":stack_peak, "manual_button_transition":"unverified", "physical_markings":"unverified", "optical_led":"unverified"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;
    const ORACLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/trust-runtime/tests/fixtures/portability/stbc-2.0/expected-a4-trace.csv"
    ));
    fn synthetic() -> String {
        let mut out = String::from("B1,IDENTITY,10016433,00410016,30395119,35363638,512,84000000,00000000,00000000\nB1,IO,BOOT,1,0,0,0\n");
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
        out.push_str("B1,STACK_PEAK,4096\nB1,IO,gpio-pre-safe,1,0,1,1\nB1,IO,gpio-STOP-high,1,0,0,0\nB1,IO,gpio-pre-safe,1,0,1,1\nB1,IO,gpio-FAULT,1,0,0,0\nB1,STATE,WATCHDOG_ARM\nB1,IDENTITY,10016433,00410016,30395119,35363638,512,84000000,00000000,20000000\nB1,IO,watchdog-BOOT,1,0,0,0\nB1,STATE,WATCHDOG_RESET\nB1,DONE\n");
        let mut lines: Vec<_> = out.lines().map(str::to_owned).collect();
        let mut sorted = String::new();
        for key in record_order() {
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
        assert_eq!(verify(&synthetic(), ORACLE).unwrap()["trace_samples"], 101);
    }
    #[test]
    fn missing_corrupt_duplicate_and_unsafe_records_are_rejected() {
        let good = synthetic();
        for (from, to) in [
            ("B1,DONE\n", ""),
            ("20000000\nB1,IO,watchdog", "00000000\nB1,IO,watchdog"),
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
            assert!(verify(&good.replace(from, to), ORACLE).is_err(), "{from}");
        }
        let mut reordered: Vec<_> = good.lines().collect();
        let memory_index = reordered
            .iter()
            .position(|line| line.starts_with("B1,MEM,main-prepare"))
            .unwrap();
        reordered.swap(memory_index, memory_index + 1);
        assert!(verify(&(reordered.join("\n") + "\n"), ORACLE).is_err());
        let nan = good.replace(
            &format!("{:016x}", std::f64::consts::SQRT_2.to_bits()),
            "7ff8000000000000",
        );
        assert!(verify(&nan, ORACLE).is_err());
        assert!(verify(good.trim_end(), ORACLE).is_err());
        assert!(verify(&(good + "B1,DONE\n"), ORACLE).is_err());
    }
}
