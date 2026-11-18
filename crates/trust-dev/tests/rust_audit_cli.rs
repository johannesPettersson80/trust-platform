//! `trust-dev rust audit --db` on a project with Rust POUs (specification 48 RU-79): the scan crate
//! graph the build reads, checked against a copy of the RustSec advisory database in a folder.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, with cargo and the
//! SDK's sources; with a toolchain without them it checks nothing.

use std::path::Path;
use std::process::Command;

use trust_runtime::native::Toolchain;

fn trust_dev() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-dev")
            .expect("Cargo must provide trust-dev binary while executing integration tests"),
    )
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    std::fs::write(path, text).expect("the file");
}

/// An advisory file as the database writes one.
fn advisory(front: &str, title: &str) -> String {
    format!("```toml\n{front}\n```\n\n# {title}\n\nWhat happens and what to do.\n")
}

#[test]
fn the_audit_fails_on_a_vulnerability_of_the_graph_and_warns_of_an_informational_advisory() {
    // RU-79: the project's crate depends on its path crate helper 0.3.1, which the copy's
    // RUSTSEC-2099-0001 names (patched >= 0.3.2), and on the SDK, whose libm the copy's
    // unmaintained notice names (no version patched). The audit prints the vulnerability as an
    // error and the notice as a warning, counts the crates it checked, and fails; with the
    // vulnerability patched (>= 0.3.1), it passes with the warning. Catches an audit of another
    // graph than the build's, a warning that fails the audit, and a vulnerability that passes.
    let toolchain = Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let root = trust_test_support::unique_temp_path("trust-dev-rust-audit");
    write(
        &root.join("rust/Cargo.toml"),
        &format!(
            "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\ntrust-plc = \"={}\"\nhelper = {{ path = \"helper\" }}\n",
            env!("CARGO_PKG_VERSION")
        ),
    );
    write(
        &root.join("rust/src/lib.rs"),
        "#![cfg_attr(not(test), no_std)]\n#![forbid(unsafe_code)]\n\nuse trust_plc::prelude::*;\n\n\
         /// Its input plus one, by the helper.\n#[trust_function(name = \"PLUS_ONE\")]\n\
         #[budget(\"1us\")]\npub fn plus_one(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {\n    \
         Ok(helper::next(a))\n}\n",
    );
    write(
        &root.join("rust/helper/Cargo.toml"),
        "[package]\nname = \"helper\"\nversion = \"0.3.1\"\nedition = \"2024\"\n",
    );
    write(
        &root.join("rust/helper/src/lib.rs"),
        "#![no_std]\n#![forbid(unsafe_code)]\n\n/// `a` plus one, wrapping.\npub fn next(a: i32) -> i32 {\n    \
         a.wrapping_add(1)\n}\n",
    );
    write(
        &root.join("src/main.st"),
        "PROGRAM Line\nVAR\n    bumped : DINT;\nEND_VAR\nbumped := PLUS_ONE(41);\nEND_PROGRAM\n",
    );
    let db = trust_test_support::unique_temp_path("trust-dev-rust-audit-db");
    let vulnerability = db.join("crates/helper/RUSTSEC-2099-0001.md");
    write(
        &vulnerability,
        &advisory(
            "[advisory]\nid = \"RUSTSEC-2099-0001\"\npackage = \"helper\"\n\n[versions]\n\
             patched = [\">= 0.3.2\"]",
            "Helper reads past its buffer",
        ),
    );
    write(
        &db.join("crates/libm/RUSTSEC-2099-0002.md"),
        &advisory(
            "[advisory]\nid = \"RUSTSEC-2099-0002\"\npackage = \"libm\"\ninformational = \"unmaintained\"\n\n\
             [versions]\npatched = []",
            "libm is unmaintained",
        ),
    );
    let audit = || {
        trust_dev()
            .args(["rust", "audit", "--project"])
            .arg(&root)
            .arg("--db")
            .arg(&db)
            .output()
            .expect("run trust-dev")
    };
    let output = audit();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stdout}\n{stderr}");
    assert!(
        stdout.contains(
            "error: RUSTSEC-2099-0001 helper 0.3.1: Helper reads past its buffer (patched: >= 0.3.2)\n"
        ),
        "{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("warning: RUSTSEC-2099-0002 libm ")
            && stdout.contains(", unmaintained: libm is unmaintained (patched: none)\n"),
        "{stdout}\n{stderr}"
    );
    let summary = format!(
        "against the RustSec advisory database in {}: 1 vulnerability, 1 warning.",
        db.display()
    );
    assert!(stdout.contains(&summary), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("1 vulnerability in the scan crate graph (specification 48 RU-79)"),
        "{stderr}"
    );
    // The vulnerability patched in the version the project builds: only the warning is left.
    write(
        &vulnerability,
        &advisory(
            "[advisory]\nid = \"RUSTSEC-2099-0001\"\npackage = \"helper\"\n\n[versions]\n\
             patched = [\">= 0.3.1\"]",
            "Helper reads past its buffer",
        ),
    );
    let output = audit();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!stdout.contains("RUSTSEC-2099-0001"), "{stdout}");
    assert!(stdout.contains("0 vulnerabilities, 1 warning."), "{stdout}");
}
