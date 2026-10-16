//! Own the ELF/map evidence produced by one final firmware link.
use super::board::BoardProfile;
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) struct Artifacts {
    pub report: PathBuf,
    pub map: PathBuf,
    pub elf: PathBuf,
    pub cargo_log: PathBuf,
}

impl Artifacts {
    pub fn prepare(report: &Path) -> Result<Self> {
        let parent = report
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let parent = parent.canonicalize()?;
        let report = parent.join(report.file_name().context("report has no file name")?);
        let map = parent.join("firmware.map");
        let elf = parent.join("firmware.elf");
        let cargo_log = parent.join("cargo-artifacts.jsonl");
        ensure!(
            report != map && report != elf && report != cargo_log,
            "report collides with firmware artifacts"
        );
        for path in [&report, &map, &elf, &cargo_log] {
            match fs::symlink_metadata(path) {
                Ok(_) => anyhow::bail!("evidence destination already exists: {}", path.display()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("inspect evidence destination"),
            }
        }
        Ok(Self {
            report,
            map,
            elf,
            cargo_log,
        })
    }

    pub fn map_flags(&self, flags: &mut Vec<String>) -> Result<Vec<String>> {
        // Preserve every reviewed option except the map's output destination.
        let positions: Vec<_> = flags
            .windows(2)
            .enumerate()
            .filter(|(_, pair)| pair[0] == "-C" && pair[1].starts_with("link-arg=-Map="))
            .map(|(index, _)| index)
            .collect();
        ensure!(
            positions.len() == 1,
            "firmware requires exactly one reviewed map flag"
        );
        flags.drain(positions[0]..positions[0] + 2);
        let map = self.map.to_str().context("non-UTF-8 map path")?;
        ensure!(!map.contains('\x1f'), "map path contains flag separator");
        Ok(vec!["-C".into(), format!("link-arg=-Map={map}")])
    }

    pub fn retain(&self, cargo_output: &str, board: &BoardProfile) -> Result<Value> {
        let executable = executable(cargo_output, board.bin)?;
        let map = fs::read(&self.map).context("final link did not produce its map")?;
        ensure!(!map.is_empty(), "final link produced an empty map");
        let elf = fs::read(&executable).context("read linked executable reported by Cargo")?;
        ensure!(!elf.is_empty(), "linked executable is empty");
        fs::write(&self.elf, &elf)?;
        Ok(serde_json::json!({
            "elf": self.elf, "elf_sha256": super::digest(&elf),
            "map": self.map, "map_sha256": super::digest(&map),
            "cargo_executable": executable,
            "cargo_log": self.cargo_log,
        }))
    }
}

fn executable(output: &str, bin: &str) -> Result<PathBuf> {
    let mut executable = None;
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let record: Value = serde_json::from_str(line).context("invalid Cargo JSON output")?;
        if record["reason"] != "compiler-artifact"
            || record["target"]["name"] != bin
            || !record["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
        {
            continue;
        }
        ensure!(
            record["fresh"] == false,
            "firmware must be linked for this evidence destination"
        );
        ensure!(
            executable.is_none(),
            "duplicate firmware executable artifacts"
        );
        let path = PathBuf::from(
            record["executable"]
                .as_str()
                .context("Cargo omitted firmware executable")?,
        );
        ensure!(path.is_absolute(), "Cargo executable path is not absolute");
        executable = Some(path);
    }
    executable.context("Cargo did not report the firmware binary")
}

#[cfg(test)]
mod tests;
