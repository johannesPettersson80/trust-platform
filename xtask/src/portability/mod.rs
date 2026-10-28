//! Host-side packaging and evidence checks for the reference compositions: ESP32-C6 and
//! STM32U5A9. Firmware is built per chip, from the chip's file (specification 34, TARGET-03);
//! inspection takes the bench board.
mod board;
mod build;
mod build_artifacts;
mod database_runner;
mod elf;
mod firmware;
mod pack;
mod source_identity;
#[cfg(all(test, unix))]
mod supply_chain_tests;
#[cfg(all(test, unix))]
mod worker_gate_tests;

use anyhow::{bail, Context, Result};
use board::Board;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub(super) fn run(root: &Path, args: Vec<String>) -> Result<()> {
    match args.as_slice() {
        [action, output] if action == "freeze-source" => source_identity::freeze(root, output),
        [action, input] if action == "check-source" => source_identity::check(root, input),
        [action, input] if action == "database-gate" => database_runner::run(root, input),
        [action, name, output] if action == "build-firmware" => {
            build::firmware(root, &firmware::Firmware::parse(name)?, output)
        }
        [action, program, record, output] if action == "pack-project" => {
            let program = fs::read(program).context("read program.tpi")?;
            let record = fs::read(record).context("read deployment record")?;
            let bytes = pack::pack_project(&program, &record)?;
            fs::write(output, &bytes)?;
            println!("{}", serde_json::json!({"bytes":bytes.len(), "sha256":digest(&bytes),
                "program_sha256":digest(&program), "record_sha256":digest(&record)}));
            Ok(())
        }
        [action, name, input, output] if action == "inspect" => {
            let board = Board::parse(name)?.profile()?;
            let bytes = fs::read(input).context("read linked firmware")?;
            write_report(output, elf::inspect(&board, &bytes)?)
        }
        _ => bail!("expected portability build-firmware <chip> <report.json>, pack-project <program.tpi> <deployment-record.bin> <application.bin>, inspect <board> <firmware.elf> <report.json>, freeze-source <manifest.json>, check-source <manifest.json>, or database-gate <generated-environment>"),
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
