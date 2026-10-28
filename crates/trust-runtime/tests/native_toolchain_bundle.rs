//! The toolchain bundle end to end (specification 44 §4.6): `trust-runtime toolchain bundle` writes
//! the bundle of this computer's platform from the development toolchain at `TRUST_TOOLCHAIN`,
//! `toolchain install --from` installs it into truST's data folder, `toolchain status` names it,
//! and `build` compiles for every module target of §4.4 with the installed bundle alone.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, its libraries built
//! for every module target (`scripts/native/build_toolchain_libraries.sh`), and about 1 GB in the
//! temporary folder. Each command runs with a home, configuration and data folder of the test's own.
//! A Windows development folder has no compiler of its own (§4.6), so on Windows CI writes and
//! installs the bundle the same way before the tests, which then build with it.
#![cfg(unix)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value as JsonValue};
use smol_str::SmolStr;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};

/// A counter in a 10 ms task.
const COUNTER: &str = "
PROGRAM Counter
VAR
    Count : UDINT;
END_VAR
Count := Count + 1;
END_PROGRAM

CONFIGURATION Bundled
RESOURCE Bundled ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Counter;
END_RESOURCE
END_CONFIGURATION
";

/// A folder of the test's own, removed when dropped.
struct Fixture {
    folder: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!(
            "trust-toolchain-bundle-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        let project = folder.join("project");
        std::fs::create_dir_all(project.join("src")).expect("the project folder");
        for name in ["home", "config", "data"] {
            std::fs::create_dir_all(folder.join(name)).expect("a user folder");
        }
        let runtime = render_runtime_toml(&SmolStr::new("bundled"), 10);
        #[cfg(windows)]
        let runtime = runtime.replacen(
            "[runtime.control]\n",
            "[runtime.control]\nendpoint = \"tcp://127.0.0.1:19094\"\nauth_token = \"bundle-test-token\"\n",
            1,
        );
        std::fs::write(project.join("runtime.toml"), runtime).expect("runtime.toml");
        let io = render_io_toml(&build_io_config_auto("loopback").expect("loopback I/O"));
        std::fs::write(project.join("io.toml"), io).expect("io.toml");
        std::fs::write(project.join("src/main.st"), COUNTER).expect("the program");
        Self { folder }
    }

    fn project(&self) -> PathBuf {
        self.folder.join("project")
    }

    /// truST's data folder of the user the commands run as (specification 44 §4.6).
    fn trust_data(&self) -> PathBuf {
        if cfg!(target_os = "macos") {
            self.folder.join("home/Library/Application Support/trust")
        } else {
            self.folder.join("data/trust")
        }
    }

    /// `trust-runtime` with `args`, as a user whose folders are the fixture's, with
    /// `TRUST_TOOLCHAIN` set to `toolchain` or unset.
    fn run(&self, args: &[&std::ffi::OsStr], toolchain: Option<&OsString>) -> Output {
        let mut command = Command::new(
            std::env::var_os("CARGO_BIN_EXE_trust-runtime")
                .expect("Cargo provides the trust-runtime binary to integration tests"),
        );
        command
            .args(args)
            .env("HOME", self.folder.join("home"))
            .env("XDG_CONFIG_HOME", self.folder.join("config"))
            .env("APPDATA", self.folder.join("config"))
            .env("XDG_DATA_HOME", self.folder.join("data"))
            .env("LOCALAPPDATA", self.folder.join("data"))
            .env_remove("TRUST_TOOLCHAIN");
        if let Some(toolchain) = toolchain {
            command.env("TRUST_TOOLCHAIN", toolchain);
        }
        command.output().expect("run trust-runtime")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

fn failure(output: &Output) -> String {
    format!(
        "status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn succeeded(output: &Output) -> String {
    assert!(output.status.success(), "{}", failure(output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn os(text: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(text)
}

/// Specification 44 §4.6, end to end: the bundle written from the development toolchain installs
/// from its file into `toolchains/<version>/` of truST's data folder, whose path the last JSON line
/// names; `status` names it as installed with this release's version while `TRUST_TOOLCHAIN` is
/// unset, and names `TRUST_TOOLCHAIN` while it is set; and with the installed bundle alone, a build
/// for this computer writes its program image and a build for each module target of §4.4 succeeds.
/// Catches a bundle that leaves out a part a module links (a target's `core`, a compiler library, a
/// program library), an install the tools do not find, and a status that names the wrong
/// toolchain.
#[test]
fn an_installed_bundle_builds_for_every_module_target() {
    let development = std::env::var_os("TRUST_TOOLCHAIN")
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let fixture = Fixture::new();
    let archive = fixture.folder.join("trust-toolchain.tar.gz");

    let wrote = succeeded(&fixture.run(
        &[
            os("toolchain"),
            os("bundle"),
            development.as_os_str(),
            archive.as_os_str(),
        ],
        Some(&development),
    ));
    assert!(wrote.starts_with("Wrote "), "{wrote}");

    let version = env!("CARGO_PKG_VERSION");
    let installed = fixture.trust_data().join("toolchains").join(version);
    let events = succeeded(&fixture.run(
        &[
            os("toolchain"),
            os("install"),
            os("--from"),
            archive.as_os_str(),
            os("--json"),
        ],
        None,
    ));
    let last: JsonValue =
        serde_json::from_str(events.lines().last().expect("an event")).expect("a JSON event");
    assert_eq!(
        last,
        json!({"event": "installed", "folder": installed.display().to_string()})
    );

    let status: JsonValue = serde_json::from_str(&succeeded(
        &fixture.run(&[os("toolchain"), os("status"), os("--json")], None),
    ))
    .expect("the status");
    assert_eq!(
        status,
        json!({
            "installed": true,
            "folder": installed.display().to_string(),
            "source": "installed",
            "trust": version,
        })
    );
    let overridden: JsonValue = serde_json::from_str(&succeeded(&fixture.run(
        &[os("toolchain"), os("status"), os("--json")],
        Some(&development),
    )))
    .expect("the status");
    assert_eq!(overridden["source"], "TRUST_TOOLCHAIN");

    let project = fixture.project();
    succeeded(&fixture.run(
        &[
            os("build"),
            os("--project"),
            project.as_os_str(),
            os("--ci"),
        ],
        None,
    ));
    assert!(
        project.join("program.tpi").is_file(),
        "the build for this computer wrote no program image"
    );
    for target in [
        "riscv32imac-unknown-none-elf",
        "thumbv8m.main-none-eabihf",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    ] {
        succeeded(&fixture.run(
            &[
                os("build"),
                os("--project"),
                project.as_os_str(),
                os("--target"),
                os(target),
                os("--ci"),
            ],
            None,
        ));
        assert!(
            project
                .join("build")
                .join(format!("program.{target}.tpi"))
                .is_file(),
            "no image for {target}"
        );
    }
    assert!(Path::new(&installed).join("manifest.json").is_file());
}
