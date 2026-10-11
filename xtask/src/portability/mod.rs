//! Host-side packaging and evidence checks for the F401 reference composition.
mod build;
mod build_artifacts;
mod bundle;
mod database_runner;
mod elf;
mod map;
mod source_identity;
#[cfg(all(test, unix))]
mod supply_chain_tests;
mod trace;
#[cfg(all(test, unix))]
mod worker_gate_tests;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const FIXTURES: &str = "crates/trust-runtime/tests/fixtures/portability/stbc-2.0";

pub(super) fn run(root: &Path, args: Vec<String>) -> Result<()> {
    match args.as_slice() {
        [action, output] if action == "freeze-source" => source_identity::freeze(root, output),
        [action, input] if action == "check-source" => source_identity::check(root, input),
        [action, input] if action == "database-gate" => database_runner::run(root, input),
        [action, output] if action == "build-firmware" => build::firmware(root, output),
        [action, gpio, output] if action == "pack" => {
            let main = fs::read(root.join(FIXTURES).join("program-v2.stbc"))?;
            let numeric = fs::read(root.join(FIXTURES).join("numeric-v2.stbc"))?;
            let gpio = fs::read(gpio)?;
            let bytes = bundle::pack([&main, &numeric, &gpio])?;
            fs::write(output, &bytes)?;
            println!("{}", serde_json::json!({"bytes":bytes.len(), "sha256":digest(&bytes),
                "main_sha256":digest(&main), "numeric_sha256":digest(&numeric), "gpio_sha256":digest(&gpio)}));
            Ok(())
        }
        [action, input, output] if action == "inspect" => {
            let bytes = fs::read(input).context("read linked firmware")?;
            write_report(output, elf::inspect(&bytes)?)
        }
        [action, current, baseline, output] if action == "footprint" => {
            let current = fs::read_to_string(current).context("read new linker map")?;
            let baseline = fs::read_to_string(baseline).context("read retained baseline linker map")?;
            write_report(output, map::compare(&current, &baseline)?)
        }
        [action, input, output] if action == "verify" => {
            let raw = fs::read(input).context("read physical UART capture")?;
            anyhow::ensure!(raw.len() <= 256 * 1024, "UART capture exceeds reference bound");
            let text = std::str::from_utf8(&raw).context("UART capture is not UTF-8")?;
            let oracle = fs::read_to_string(root.join(FIXTURES).join("expected-a4-trace.csv"))?;
            let mut report = trace::verify(text, &oracle)?;
            report["uart_sha256"] = digest(&raw).into();
            write_report(output, report)
        }
        _ => bail!("expected portability build-firmware <config.json>, pack <gpio.stbc> <application.bin>, inspect <firmware.elf> <report.json>, verify <uart.txt> <report.json>, footprint <current.map> <baseline.map> <report.json>, freeze-source <manifest.json>, check-source <manifest.json>, or database-gate <generated-environment>"),
    }
}

fn write_report(path: &str, report: Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod digest_tests {
    #[test]
    fn artifact_digest_preserves_standard_lowercase_sha256() {
        assert_eq!(
            super::digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
