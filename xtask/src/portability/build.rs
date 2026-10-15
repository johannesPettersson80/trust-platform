//! The reference firmware's single build configuration, with local paths normalized.
use super::board::BoardProfile;
use anyhow::{ensure, Context, Result};
use std::{env, fs, path::Path, process::Command};

pub(super) fn firmware(root: &Path, board: &BoardProfile, report: &str) -> Result<()> {
    let root = root.canonicalize()?;
    let directory = root.join(board.firmware_dir);
    let config = fs::read_to_string(directory.join(".cargo/config.toml"))?;
    let cargo_home = env::var_os("CARGO_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".cargo")))
        .context("CARGO_HOME or HOME is required to normalize registry paths")?
        .canonicalize()?;
    let output = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()?;
    ensure!(
        output.status.success(),
        "cannot locate compiler source root"
    );
    let sysroot = String::from_utf8(output.stdout)?;
    let library = Path::new(sysroot.trim()).join("lib/rustlib/src/rust");
    let artifacts = super::build_artifacts::Artifacts::prepare(Path::new(report))?;
    let mut registry_sources = Vec::new();
    match fs::read_dir(cargo_home.join("registry/src")) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.context("read Cargo registry source root")?;
                if entry.file_type()?.is_dir() {
                    registry_sources.push(entry.path());
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("enumerate Cargo registry source roots"),
    }
    registry_sources.sort();
    let mut flags = flags(
        board,
        &config,
        &root,
        &cargo_home,
        &library,
        &registry_sources,
    )?;
    let binary_flags = artifacts.map_flags(&mut flags)?;
    let report = artifacts.report.to_str().context("non-UTF-8 report path")?;
    let mut configuration = serde_json::json!({
        "board": board.name, "target": board.target, "profile": "release", "rustflags": flags,
        "binary_rustflags": binary_flags, "manifest": directory.join("Cargo.toml"),
        "status": "started",
        "note": "Path normalization does not claim cross-checkout binary reproducibility."
    });
    super::write_report(report, configuration.clone())?;
    let mut command = Command::new("cargo");
    command
        .current_dir(directory)
        .args([
            "rustc",
            "--locked",
            "--release",
            "--target",
            board.target,
            "--bin",
            board.bin,
            "--message-format=json-render-diagnostics",
            "--",
        ])
        .args(&binary_flags)
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"));
    // The checked-in profile/target flags own the composition. Host validation
    // environments must not silently disable checks or replace the linker scripts.
    for (key, _) in env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("CARGO_PROFILE_RELEASE_") {
            command.env_remove(key);
        }
    }
    for key in ["RUSTFLAGS", "RUSTC_BOOTSTRAP", "CC", "CXX"] {
        command.env_remove(key);
    }
    let output = command.output().context("build reference firmware")?;
    use std::io::Write;
    std::io::stderr().write_all(&output.stderr)?;
    fs::write(&artifacts.cargo_log, &output.stdout)?;
    ensure!(
        output.status.success(),
        "reference firmware build failed: {}",
        output.status
    );
    configuration["artifacts"] = artifacts.retain(std::str::from_utf8(&output.stdout)?, board)?;
    configuration["status"] = "linked".into();
    super::write_report(report, configuration)
}

fn flags(
    board: &BoardProfile,
    config: &str,
    root: &Path,
    cargo_home: &Path,
    library: &Path,
    registry_sources: &[std::path::PathBuf],
) -> Result<Vec<String>> {
    let config: toml::Value = toml::from_str(config)?;
    let mut flags = config
        .get("target")
        .and_then(|targets| targets.get(board.target))
        .and_then(|target| target.get("rustflags"))
        .and_then(toml::Value::as_array)
        .context("firmware target rustflags must be an array")?
        .iter()
        .map(|flag| {
            flag.as_str()
                .map(str::to_owned)
                .context("non-string rustflag")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        !flags
            .iter()
            .any(|flag| flag.contains("machine-outliner-reruns")),
        "reference firmware requires the compiler's default outliner pass count"
    );
    for pair in board.required_rustflags {
        ensure!(
            flags
                .windows(2)
                .any(|window| window[0] == pair[0] && window[1] == pair[1]),
            "firmware target rustflags omit {} {}",
            pair[0],
            pair[1]
        );
    }
    for (source, normalized) in [
        (root.to_path_buf(), "/src"),
        (cargo_home.join("registry").join("src"), "/registry"),
        (library.to_path_buf(), "/rust-src"),
    ] {
        let source = source.to_str().context("non-UTF-8 build source path")?;
        ensure!(
            !source.contains('\x1f'),
            "build path contains flag separator"
        );
        flags.push(format!("--remap-path-prefix={source}={normalized}"));
    }
    // rustc uses the last matching prefix, so specific roots follow general ones.
    // Preserve the relative filename and distinguish every registry namespace.
    let mut specific = vec![(
        root.join("crates/trust-runtime-core/src"),
        "/core".to_owned(),
    )];
    specific.extend(
        registry_sources
            .iter()
            .enumerate()
            .map(|(index, source)| (source.clone(), format!("/registry{index}"))),
    );
    for (source, normalized) in specific {
        let source = source.to_str().context("non-UTF-8 specific source root")?;
        ensure!(
            !source.contains('\x1f'),
            "source root contains flag separator"
        );
        flags.push(format!("--remap-path-prefix={source}={normalized}"));
    }
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remapping_preserves_all_reviewed_target_flags_and_space_boundaries() {
        let config = include_str!("../../../firmware/trust-nucleo-f401re/.cargo/config.toml");
        #[cfg(windows)]
        let (root, cargo_home, library, expected) = (
            r"C:\task source",
            r"C:\cargo home",
            r"C:\rust library",
            [
                r"--remap-path-prefix=C:\task source=/src",
                r"--remap-path-prefix=C:\cargo home\registry\src=/registry",
                r"--remap-path-prefix=C:\rust library=/rust-src",
            ],
        );
        #[cfg(not(windows))]
        let (root, cargo_home, library, expected) = (
            "/task source",
            "/cargo home",
            "/rust library",
            [
                "--remap-path-prefix=/task source=/src",
                "--remap-path-prefix=/cargo home/registry/src=/registry",
                "--remap-path-prefix=/rust library=/rust-src",
            ],
        );
        let flags = flags(
            &BoardProfile::nucleo_f401re(),
            config,
            Path::new(root),
            Path::new(cargo_home),
            Path::new(library),
            &[],
        )
        .unwrap();
        assert!(flags.windows(2).any(|p| p == ["-C", "link-arg=--icf=safe"]));
        assert!(flags
            .windows(2)
            .any(|p| p == ["-C", "link-arg=-Tprofile.x"]));
        for expected in expected {
            assert!(
                flags.contains(&expected.into()),
                "missing intact flag {expected}"
            );
        }
        assert!(!flags.iter().any(|flag| flag.contains("icf=all")));
    }

    #[test]
    fn extra_outliner_pass_is_rejected_before_building_firmware() {
        let board = BoardProfile::nucleo_f401re();
        let config = include_str!("../../../firmware/trust-nucleo-f401re/.cargo/config.toml");
        let mut config: toml::Value = toml::from_str(config).unwrap();
        let target_flags = config["target"][board.target]["rustflags"]
            .as_array_mut()
            .unwrap();
        target_flags.push("-C".into());
        target_flags.push("llvm-args=-machine-outliner-reruns=1".into());
        let error = flags(
            &board,
            &toml::to_string(&config).unwrap(),
            Path::new("task"),
            Path::new("cargo"),
            Path::new("library"),
            &[],
        )
        .unwrap_err();
        assert!(error.to_string().contains("default outliner pass count"));
    }

    #[test]
    fn specific_path_namespaces_follow_general_roots_and_remain_distinct() {
        let config = include_str!("../../../firmware/trust-nucleo-f401re/.cargo/config.toml");
        let root = Path::new("task");
        let cargo_home = Path::new("cargo");
        let registries = [
            cargo_home.join("registry/src/first"),
            cargo_home.join("registry/src/second"),
        ];
        let flags = flags(
            &BoardProfile::nucleo_f401re(),
            config,
            root,
            cargo_home,
            Path::new("library"),
            &registries,
        )
        .unwrap();
        let mappings: Vec<_> = flags
            .iter()
            .filter(|flag| flag.starts_with("--remap-path-prefix="))
            .collect();
        assert_eq!(mappings.len(), 6);
        assert_eq!(
            mappings[3],
            &format!(
                "--remap-path-prefix={}=/core",
                root.join("crates/trust-runtime-core/src").display()
            )
        );
        assert_eq!(
            mappings[4],
            &format!("--remap-path-prefix={}=/registry0", registries[0].display())
        );
        assert_eq!(
            mappings[5],
            &format!("--remap-path-prefix={}=/registry1", registries[1].display())
        );
    }

    #[test]
    fn c6_profile_requires_its_linker_script() {
        let board = BoardProfile::esp32c6_devkitc_1();
        let config = "[target.riscv32imac-unknown-none-elf]\nrustflags = [\"-C\", \"link-arg=-Tlinkall.x\", \"-C\", \"link-arg=-Map=trust-esp32c6-reference.map\"]\n";
        let accepted = flags(
            &board,
            config,
            Path::new("task"),
            Path::new("cargo"),
            Path::new("library"),
            &[],
        )
        .unwrap();
        assert!(accepted
            .windows(2)
            .any(|pair| pair == ["-C", "link-arg=-Tlinkall.x"]));
        let without_linker_script = config.replace("link-arg=-Tlinkall.x", "link-arg=-Tother.x");
        assert!(flags(
            &board,
            &without_linker_script,
            Path::new("task"),
            Path::new("cargo"),
            Path::new("library"),
            &[],
        )
        .is_err());
    }
}
