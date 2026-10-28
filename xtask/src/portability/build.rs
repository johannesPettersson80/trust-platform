//! The reference firmware's single build configuration, with local paths normalized.
use super::firmware::Firmware;
use anyhow::{ensure, Context, Result};
use std::{env, fs, path::Path, process::Command};
use trust_chips::Family;

pub(super) fn firmware(root: &Path, firmware: &Firmware, report: &str) -> Result<()> {
    let root = root.canonicalize()?;
    let directory = root.join(&firmware.firmware_dir);
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
        firmware,
        &config,
        &root,
        &cargo_home,
        &library,
        &registry_sources,
    )?;
    let binary_flags = artifacts.map_flags(&mut flags)?;
    let report = artifacts.report.to_str().context("non-UTF-8 report path")?;
    // A bench build names the firmware's bench features (`bench-loopback`, `bench-trial-hang`) in
    // TRUST_FIRMWARE_FEATURES; the report records them, and a release build names none.
    let features = env::var("TRUST_FIRMWARE_FEATURES")
        .ok()
        .filter(|features| !features.trim().is_empty());
    let mut configuration = serde_json::json!({
        "firmware": firmware.name, "target": firmware.target, "profile": "release",
        "features": features,
        "rustflags": flags,
        "binary_rustflags": binary_flags, "manifest": directory.join("Cargo.toml"),
        "status": "started",
        "note": "Path normalization does not claim cross-checkout binary reproducibility."
    });
    super::write_report(report, configuration.clone())?;
    let mut command = Command::new("cargo");
    command.current_dir(directory).args([
        "rustc",
        "--locked",
        "--release",
        "--target",
        firmware.target.as_str(),
        "--bin",
        firmware.bin.as_str(),
        "--message-format=json-render-diagnostics",
    ]);
    if let Some(features) = &features {
        command.args(["--features", features.as_str()]);
    }
    command
        .arg("--")
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
    configuration["artifacts"] =
        artifacts.retain(std::str::from_utf8(&output.stdout)?, &firmware.bin)?;
    // The C6's boot-time stack paint reaches static data above the stack window, so the same rule
    // the linker guard asserts is checked here from the linked sections (specification 34, D.2).
    if firmware.family == Family::Esp32 {
        let image = fs::read(&artifacts.elf).context("read linked firmware for the stack guard")?;
        let sections = super::elf::allocated_sections(&image)?;
        configuration["stack"] = super::elf::c6_stack_region(&sections)?;
    }
    configuration["status"] = "linked".into();
    super::write_report(report, configuration)
}

fn flags(
    firmware: &Firmware,
    config: &str,
    root: &Path,
    cargo_home: &Path,
    library: &Path,
    registry_sources: &[std::path::PathBuf],
) -> Result<Vec<String>> {
    let config: toml::Value = toml::from_str(config)?;
    let mut flags = config
        .get("target")
        .and_then(|targets| targets.get(firmware.target.as_str()))
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
    for pair in firmware.required_rustflags {
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
        let config = include_str!("../../../firmware/trust-stm32u5a9/.cargo/config.toml");
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
            &Firmware::parse("stm32u5a9").unwrap(),
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
        let firmware = Firmware::parse("stm32u5a9").unwrap();
        let config = include_str!("../../../firmware/trust-stm32u5a9/.cargo/config.toml");
        let mut config: toml::Value = toml::from_str(config).unwrap();
        let target_flags = config["target"][firmware.target.as_str()]["rustflags"]
            .as_array_mut()
            .unwrap();
        target_flags.push("-C".into());
        target_flags.push("llvm-args=-machine-outliner-reruns=1".into());
        let error = flags(
            &firmware,
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
        let config = include_str!("../../../firmware/trust-stm32u5a9/.cargo/config.toml");
        let root = Path::new("task");
        let cargo_home = Path::new("cargo");
        let registries = [
            cargo_home.join("registry/src/first"),
            cargo_home.join("registry/src/second"),
        ];
        let flags = flags(
            &Firmware::parse("stm32u5a9").unwrap(),
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
    fn u5a9_profile_accepts_its_firmware_config_and_requires_the_profile_script() {
        let firmware = Firmware::parse("stm32u5a9").unwrap();
        let config = include_str!("../../../firmware/trust-stm32u5a9/.cargo/config.toml");
        let build = |config: &str| {
            flags(
                &firmware,
                config,
                Path::new("task"),
                Path::new("cargo"),
                Path::new("library"),
                &[],
            )
        };
        let accepted = build(config).unwrap();
        for expected in [
            ["-C", "link-arg=-Tlink.x"],
            ["-C", "link-arg=-Tprofile.x"],
            ["-C", "link-arg=--icf=safe"],
        ] {
            assert!(
                accepted.windows(2).any(|pair| pair == expected),
                "missing {expected:?}"
            );
        }
        // The map flag, which the artifacts step replaces, is there exactly once.
        let maps = accepted
            .windows(2)
            .filter(|pair| pair[0] == "-C" && pair[1].starts_with("link-arg=-Map="))
            .count();
        assert_eq!(maps, 1);
        // Without profile.x the linker assertions and `__firmware_load_end` would be missing.
        let without_profile = config.replace("link-arg=-Tprofile.x", "link-arg=-Tother.x");
        let error = build(&without_profile).unwrap_err().to_string();
        assert!(error.contains("omit"), "{error}");
    }

    #[test]
    fn c6_profile_requires_its_linker_script() {
        let firmware = Firmware::parse("esp32c6").unwrap();
        let config = "[target.riscv32imac-unknown-none-elf]\nrustflags = [\"-C\", \"link-arg=-Tlinkall.x\", \"-C\", \"link-arg=-Map=trust-esp32c6.map\"]\n";
        let accepted = flags(
            &firmware,
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
            &firmware,
            &without_linker_script,
            Path::new("task"),
            Path::new("cargo"),
            Path::new("library"),
            &[],
        )
        .is_err());
    }
}
