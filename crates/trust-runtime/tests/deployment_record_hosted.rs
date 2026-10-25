//! The deployment record on a computer (specification 34 §5.1.1; specification 44 §10.1, §10.2,
//! NX-15): `trust-runtime build` writes it next to `program.tpi`, from the image's own tables, and a
//! project's record is admitted when the project loads.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value as JsonValue};
use smol_str::SmolStr;
use trust_runtime::bundle_builder::{build_program_image, BundleBuildReport};
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};
use trust_runtime::config::RuntimeBundle;
use trust_runtime::deployment::{project_configuration_identity, DEPLOYMENT_RECORD_FILE};
use trust_runtime::error::{RuntimeError, StableErrorCode};
use trust_runtime::native::{Target, Toolchain};
use trust_runtime::runtime_core::deployment::{
    sha256, AdmissionRefusal, DeploymentRecord, DeploymentRecordError,
};
use trust_runtime::runtime_core::native::image::Image;

static COUNTER: AtomicU64 = AtomicU64::new(1);

/// One scan counts to 1,000.
const COUNTING: &str = r#"
VAR_GLOBAL
    total : DINT := 0;
END_VAR

PROGRAM Main
VAR
    index : DINT;
END_VAR
FOR index := 1 TO 1000 DO
    total := total + 1;
END_FOR;
END_PROGRAM
"#;
const OTHER: &str =
    "PROGRAM Main\nVAR\n    Counter : DINT;\nEND_VAR\nCounter := Counter + 1;\nEND_PROGRAM\n";

/// `trust-runtime build` of `root` for this computer.
fn build(root: &Path) -> anyhow::Result<BundleBuildReport> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    build_program_image(root, None, target, &toolchain, false, true)
}

fn portable_runtime_toml() -> String {
    let runtime = render_runtime_toml(&SmolStr::new("record-test"), 10);
    #[cfg(windows)]
    {
        return runtime.replacen(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            "endpoint = \"tcp://127.0.0.1:19092\"\nauth_token = \"record-test-token\"",
            1,
        );
    }
    #[cfg(not(windows))]
    runtime
}

/// A built project with `source` as its program.
fn project(name: &str, source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "trust-deployment-record-{name}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("create the project");
    std::fs::write(root.join("runtime.toml"), portable_runtime_toml()).expect("write runtime.toml");
    let io = render_io_toml(&build_io_config_auto("loopback").expect("loopback I/O"));
    std::fs::write(root.join("io.toml"), io).expect("write io.toml");
    std::fs::write(root.join("src/main.st"), source).expect("write the program");
    build(&root).expect("build the project");
    root
}

fn record_path(root: &Path) -> PathBuf {
    root.join(DEPLOYMENT_RECORD_FILE)
}

fn program(root: &Path) -> Vec<u8> {
    std::fs::read(root.join("program.tpi")).expect("read program.tpi")
}

#[test]
fn build_writes_the_record_of_the_program_it_builds() {
    let root = project("build", COUNTING);
    let report = build(&root).expect("rebuild the project");
    assert_eq!(report.record, record_path(&root));
    let record = DeploymentRecord::decode(&std::fs::read(&report.record).expect("read the record"))
        .expect("the build writes a readable record");
    let program = program(&root);
    assert_eq!(record.application.program_sha256, sha256(&program));
    assert_eq!(
        record.application.program_bytes as usize,
        program.len(),
        "the record describes program.tpi"
    );
    assert_eq!(
        record.application.configuration_sha256,
        project_configuration_identity(&root).expect("configuration identity")
    );
    // An image requests the bytes of its process images and 0 for every other limit (specification
    // 44 §10.1); this program binds no I/O, so every limit is 0.
    assert_eq!(
        record.limits.values().map(|(_, value)| value),
        [0; 11],
        "an image requests no preparation limit"
    );
    let bundle = RuntimeBundle::load(&root).expect("the built project loads");
    assert_eq!(
        bundle.image, program,
        "the project runs the image the record describes"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_record_of_another_program_is_refused() {
    let root = project("mismatch", COUNTING);
    let other = project("mismatch-other", OTHER);
    std::fs::copy(record_path(&other), record_path(&root)).expect("copy the other record");
    let error = RuntimeBundle::load(&root).expect_err("the pair does not match");
    assert_eq!(
        error,
        RuntimeError::DeploymentRefused(AdmissionRefusal::ProgramMismatch)
    );
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeDeploymentRefused
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(other);
}

#[test]
fn a_damaged_record_is_refused_with_the_reason() {
    let root = project("damaged", COUNTING);
    let mut bytes = std::fs::read(record_path(&root)).expect("read the record");
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0x01;
    std::fs::write(record_path(&root), bytes).expect("damage the record");
    let error = RuntimeBundle::load(&root).expect_err("a damaged record is refused");
    assert_eq!(
        error,
        RuntimeError::DeploymentRecordInvalid(DeploymentRecordError::Checksum)
    );
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeDeploymentRecordInvalid
    );
    assert_eq!(
        error.to_string(),
        "invalid deployment record: the CRC-32 does not match"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn build_ci_reports_the_record_it_wrote() {
    let root = project("ci", COUNTING);
    std::fs::remove_file(record_path(&root)).expect("start without a record");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo provides the trust-runtime binary to integration tests"),
    )
    .arg("build")
    .arg("--project")
    .arg(&root)
    .arg("--ci")
    .output()
    .expect("run trust-runtime build");
    assert!(
        output.status.success(),
        "build failed; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: JsonValue = serde_json::from_slice(&output.stdout).expect("the CI payload");
    assert_eq!(
        payload["record"],
        JsonValue::String(record_path(&root).display().to_string())
    );
    let record = DeploymentRecord::decode(&std::fs::read(record_path(&root)).unwrap())
        .expect("the command wrote a readable record");
    assert_eq!(record.application.program_sha256, sha256(&program(&root)));
    // Specification 44 §10.4, which replaces the device-side figure of specification 34 MEM-12: the
    // build reports the image's memory figure part by part, and its total counts the areas, the
    // stack bound, the bookkeeping and the retain capture, and not one call level of a recursion.
    // Wrong behaviour caught: no figure, a part reported under another's name, or a total that
    // leaves out a part or counts the call level.
    let figure = Image::parse(&program(&root))
        .expect("the image reads")
        .memory_figure();
    assert_eq!(
        figure.total,
        figure.areas + figure.stack + figure.bookkeeping + figure.retain_capture,
        "the total counts the four parts"
    );
    assert_eq!(
        payload["memory_figure"],
        json!({
            "areas": figure.areas,
            "stack": figure.stack,
            "bookkeeping": figure.bookkeeping,
            "retain_capture": figure.retain_capture,
            "total": figure.total,
            "call_level": figure.call_level,
        })
    );
    let _ = std::fs::remove_dir_all(root);
}
