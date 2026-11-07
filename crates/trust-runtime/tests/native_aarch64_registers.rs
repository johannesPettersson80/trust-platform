//! The AArch64 module keeps no value in `x18` (specification 44 §4.4): macOS reserves the register
//! and may overwrite it when it switches threads, so a module whose code names it computes a wrong
//! value or crashes at random on a Mac with Apple silicon, and more often the busier the computer.
//! The test builds the OSCAT core fixture, the project that uses the most of the program library,
//! for `aarch64` and reads the linked module's machine code with the Rust toolchain's
//! `llvm-objdump` (`rustup component add llvm-tools`).
//!
//! It needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, its libraries built for
//! `aarch64-unknown-linux-gnu` (`scripts/native/build_toolchain_libraries.sh`). The build runs with
//! a home, configuration and data folder of the test's own, so that it signs with a key of its own.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A folder of the test's own, removed when dropped.
struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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

/// Copies the folder `from` with everything in it to `to`.
fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create the copy's folder");
    for entry in std::fs::read_dir(from).expect("read the fixture's folder") {
        let entry = entry.expect("a fixture entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("the entry's type").is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy a fixture file");
        }
    }
}

/// `llvm-objdump` of the Rust toolchain that runs the tests.
fn llvm_objdump() -> PathBuf {
    let rustc = |args: &[&str]| {
        let output = Command::new("rustc")
            .args(args)
            .output()
            .expect("run rustc");
        String::from_utf8(output.stdout).expect("rustc prints UTF-8")
    };
    let sysroot = rustc(&["--print", "sysroot"]);
    let version = rustc(&["-vV"]);
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("rustc names its host");
    let tool = Path::new(sysroot.trim())
        .join("lib/rustlib")
        .join(host.trim())
        .join("bin/llvm-objdump");
    assert!(
        tool.is_file(),
        "the test reads the module with {}: `rustup component add llvm-tools`",
        tool.display()
    );
    tool
}

/// The instruction of a disassembled `line`, the text after its address, or `None` for a line that
/// holds none (a header, a function's label).
fn instruction(line: &str) -> Option<&str> {
    let (address, instruction) = line.split_once(':')?;
    let address = address.trim();
    (!address.is_empty() && address.chars().all(|c| c.is_ascii_hexdigit())).then_some(instruction)
}

/// Whether `instruction` names `x18` or `w18` in its operands: a branch's target symbol and a comment
/// after them may spell anything.
fn names_x18(instruction: &str) -> bool {
    let operands = instruction.split(['<', '/']).next().unwrap_or_default();
    operands
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|token| token == "x18" || token == "w18")
}

#[test]
fn an_aarch64_module_names_no_x18() {
    // Specification 44 §4.4: "The AArch64 module reserves `x18`, which macOS reserves." Every
    // instruction of the linked module counts: the generated code, the program library and the
    // compiler's own functions. Until 6 November 2026 the libraries were built without the
    // reservation, and this module named x18 in 140 instructions, among them `write_parts`, which
    // `CONCAT` runs, and `find`, which `FIND` runs: on macOS, OSCAT's `DT_TO_STRF` once gave
    // '2026-0 4-15 13:14'. A build that leaves any library or compiler function without the
    // reservation names x18 again.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let folder = Folder(trust_test_support::unique_temp_path(&format!(
        "trust-aarch64-x18-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos())
    )));
    let project = folder.0.join("project");
    copy_folder(&manifest.join("tests/fixtures/oscat/core"), &project);
    // The fixture names the OSCAT library by a path from its own place; the copy names it whole.
    let library = manifest
        .join("../../libraries/oscat")
        .canonicalize()
        .expect("the OSCAT library");
    let settings = std::fs::read_to_string(project.join("trust-lsp.toml"))
        .expect("the fixture's trust-lsp.toml")
        .replace(
            "../../../../../../libraries/oscat",
            &library.display().to_string(),
        );
    std::fs::write(project.join("trust-lsp.toml"), settings).expect("write trust-lsp.toml");
    for name in ["home", "config", "data"] {
        std::fs::create_dir_all(folder.0.join(name)).expect("a user folder");
    }

    let built = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo provides the trust-runtime binary to integration tests"),
    )
    .args(["build", "--target", "aarch64", "--project"])
    .arg(&project)
    .env("HOME", folder.0.join("home"))
    .env("XDG_CONFIG_HOME", folder.0.join("config"))
    .env("XDG_DATA_HOME", folder.0.join("data"))
    .output()
    .expect("run trust-runtime build");
    assert!(
        built.status.success(),
        "the OSCAT fixture builds for aarch64\n{}",
        failure(&built)
    );
    let module = project.join("build/native/aarch64-unknown-linux-gnu/module.elf");
    let disassembled = Command::new(llvm_objdump())
        .args(["-d", "--no-show-raw-insn"])
        .arg(&module)
        .output()
        .expect("run llvm-objdump");
    assert!(
        disassembled.status.success(),
        "llvm-objdump reads {}\n{}",
        module.display(),
        failure(&disassembled)
    );
    let text = String::from_utf8_lossy(&disassembled.stdout);
    let instructions: Vec<&str> = text.lines().filter_map(instruction).collect();
    assert!(
        instructions.len() > 10_000,
        "the module holds the OSCAT library's code, not {} instructions",
        instructions.len()
    );
    let named: Vec<&str> = instructions
        .iter()
        .copied()
        .filter(|instruction| names_x18(instruction))
        .collect();
    assert!(
        named.is_empty(),
        "{} of {} instructions name x18 or w18, the first:\n{}",
        named.len(),
        instructions.len(),
        named
            .iter()
            .take(20)
            .copied()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
