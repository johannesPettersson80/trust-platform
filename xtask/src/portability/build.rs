//! The reference firmware's single build configuration, with local paths normalized.
use anyhow::{ensure, Context, Result};
use std::{env, fs, path::Path, process::Command};

const TARGET: &str = "thumbv7em-none-eabihf";

pub(super) fn firmware(root: &Path, report: &str) -> Result<()> {
    let root = root.canonicalize()?;
    let directory = root.join("firmware/trust-nucleo-f401re");
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
    let mut flags = flags(&config, &root, &cargo_home, &library)?;
    let binary_flags = artifacts.map_flags(&mut flags)?;
    let report = artifacts.report.to_str().context("non-UTF-8 report path")?;
    let mut configuration = serde_json::json!({
        "target": TARGET, "profile": "release", "rustflags": flags,
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
            TARGET,
            "--bin",
            "trust-nucleo-f401re",
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
    configuration["artifacts"] = artifacts.retain(std::str::from_utf8(&output.stdout)?)?;
    configuration["status"] = "linked".into();
    super::write_report(report, configuration)
}

fn flags(config: &str, root: &Path, cargo_home: &Path, library: &Path) -> Result<Vec<String>> {
    let config: toml::Value = toml::from_str(config)?;
    let mut flags = config
        .get("target")
        .and_then(|targets| targets.get(TARGET))
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
            config,
            Path::new(root),
            Path::new(cargo_home),
            Path::new(library),
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
}
