//! Scan-cost probe (measurement only): a compiled program runs through the chip's resource loop on
//! the host, one pass per 100 ms cycle, so callgrind can count what each part of a scan costs.
//! Ignored in normal runs:
//! `TRUST_SCAN_PROBE=<program.stbc> TRUST_SCAN_CYCLES=<n> <test binary> scan_cost_probe --ignored`;
//! with `TRUST_SCAN_DISASSEMBLE=1` it prints the program's instructions instead.
//!
//! Request-cost probe (measurement only): the board probe's loop runs until its cycle window is
//! full, then answers `TRUST_REQUEST_COUNT` requests of type `TRUST_REQUEST_TYPE`, one per pass, so
//! callgrind can count what answering one costs between scans (specification 34 PORT-02):
//! `TRUST_REQUEST_TYPE=status <test binary> request_cost_probe --ignored`.

extern crate std;

use crate::tests::runner_ports::Fixture;

#[test]
#[ignore = "a measurement probe, run under callgrind"]
fn scan_cost_probe() {
    let path = std::env::var("TRUST_SCAN_PROBE").expect("TRUST_SCAN_PROBE names a program");
    let cycles: u64 = std::env::var("TRUST_SCAN_CYCLES")
        .ok()
        .and_then(|count| count.parse().ok())
        .unwrap_or(1_000);
    let artifact = std::fs::read(path).expect("the program reads");
    if std::env::var_os("TRUST_SCAN_DISASSEMBLE").is_some() {
        let module = trust_runtime_core::bytecode::BytecodeModule::decode(&artifact)
            .expect("the program decodes");
        std::println!(
            "{}",
            module.disassemble().expect("the program disassembles")
        );
        return;
    }
    let fixture = Fixture::with_program(&artifact);
    fixture.script.borrow_mut().cycle_interval =
        trust_runtime_core::value::Duration::from_millis(100);
    let mut runner = fixture.runner();
    for cycle in 1..=cycles {
        fixture.pass_at(&mut runner, cycle * 100);
        let _ = fixture.take_calls();
    }
}

#[test]
#[ignore = "a measurement probe, run under callgrind"]
fn request_cost_probe() {
    let kind = std::env::var("TRUST_REQUEST_TYPE").unwrap_or_else(|_| "status".into());
    let count: u64 = std::env::var("TRUST_REQUEST_COUNT")
        .ok()
        .and_then(|count| count.parse().ok())
        .unwrap_or(100);
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().cycle_interval =
        trust_runtime_core::value::Duration::from_millis(100);
    let mut runner = fixture.runner();
    // More cycles than the metrics' window of 512 holds, so each request reads a full window.
    let mut at = 0;
    for _ in 0..600 {
        at += 100;
        fixture.pass_at(&mut runner, at);
        let _ = fixture.take_calls();
    }
    for id in 1..=count {
        at += 100;
        let reply = fixture.call_at(
            &mut runner,
            at,
            &serde_json::json!({"id": id, "type": kind}),
        );
        assert_eq!(reply["ok"], serde_json::Value::Bool(true), "{reply}");
        let _ = fixture.take_calls();
    }
}
