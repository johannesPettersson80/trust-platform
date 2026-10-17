//! The deployment record on a computer (spec 34 §5.1.1, HOST-02): `trust-runtime build` writes
//! it next to `program.stbc`, a project's record is admitted when the project loads, and the
//! program then runs with the record's limits. A project without a record keeps the defaults.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value as JsonValue;
use smol_str::SmolStr;
use trust_runtime::bundle_builder::build_program_stbc;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};
use trust_runtime::bytecode::BytecodeModule;
use trust_runtime::config::RuntimeBundle;
use trust_runtime::deployment::{
    project_configuration_identity, RecordWrite, DEPLOYMENT_RECORD_FILE,
};
use trust_runtime::error::{ExecutionLimitKind, RuntimeError, StableErrorCode};
use trust_runtime::harness::CompileSession;
use trust_runtime::runtime_core::deployment::{
    sha256, AdmissionRefusal, DeploymentRecord, DeploymentRecordError, LimitKind, RequestedLimits,
};
use trust_runtime::runtime_core::vm::PreparationLimits;
use trust_runtime::value::Value;

static COUNTER: AtomicU64 = AtomicU64::new(1);

/// One scan counts to 1,000, which takes more than 1,000 units of work.
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
    build_program_stbc(&root, None).expect("build the project");
    root
}

fn record_path(root: &Path) -> PathBuf {
    root.join(DEPLOYMENT_RECORD_FILE)
}

fn program(root: &Path) -> Vec<u8> {
    std::fs::read(root.join("program.stbc")).expect("read program.stbc")
}

/// Write a record of the project's program that requests `limits`.
fn write_record(root: &Path, limits: PreparationLimits) {
    let configuration = project_configuration_identity(root).expect("configuration identity");
    let record = DeploymentRecord::for_program(&program(root), limits, configuration)
        .expect("the program is admitted under these limits");
    std::fs::write(
        record_path(root),
        record.encode().expect("encode the record"),
    )
    .expect("write the record");
}

/// The project's runtime, installed the way `trust-runtime run` installs it.
fn runtime_of(bundle: &RuntimeBundle) -> trust_runtime::Runtime {
    let module = BytecodeModule::decode(&bundle.bytecode).expect("decode program.stbc");
    CompileSession::from_sources(Vec::new())
        .build_runtime_from_bytecode_with_limits(
            &module,
            Some(&bundle.runtime.resource_name),
            bundle.preparation_limits,
        )
        .expect("install the program")
}

#[test]
fn build_writes_the_record_of_the_program_it_builds() {
    let root = project("build", COUNTING);
    let report = build_program_stbc(&root, None).expect("rebuild the project");
    let RecordWrite::Written(path) = &report.deployment_record else {
        panic!("no record written: {:?}", report.deployment_record);
    };
    assert_eq!(*path, record_path(&root));
    let record = DeploymentRecord::decode(&std::fs::read(path).expect("read the record"))
        .expect("the build writes a readable record");
    let program = program(&root);
    assert_eq!(record.application.program_sha256, sha256(&program));
    assert_eq!(
        record.application.program_bytes as usize,
        program.len(),
        "the record describes program.stbc"
    );
    assert_eq!(
        record.application.configuration_sha256,
        project_configuration_identity(&root).expect("configuration identity")
    );
    assert_eq!(
        record.limits,
        RequestedLimits::from_preparation(&PreparationLimits::default()),
        "the build requests the hosted defaults"
    );
    let bundle = RuntimeBundle::load(&root).expect("the built project loads");
    assert_eq!(bundle.preparation_limits, PreparationLimits::default());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_program_runs_with_the_limits_its_record_requests() {
    let root = project("limits", COUNTING);
    let limits = PreparationLimits {
        max_work: 1_000,
        ..PreparationLimits::default()
    };
    write_record(&root, limits);
    let bundle = RuntimeBundle::load(&root).expect("the record is admitted");
    assert_eq!(bundle.preparation_limits, limits);
    let mut runtime = runtime_of(&bundle);
    assert_eq!(runtime.prepared().limits(), limits);
    let error = runtime
        .execute_cycle()
        .expect_err("the scan needs more than the requested work");
    assert!(
        matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                limit: 1_000,
                ..
            }
        ),
        "{error:?}"
    );

    // Without a record the same program runs with the defaults and completes its scan.
    std::fs::remove_file(record_path(&root)).expect("remove the record");
    let bundle = RuntimeBundle::load(&root).expect("a project without a record loads");
    assert_eq!(bundle.preparation_limits, PreparationLimits::default());
    let mut runtime = runtime_of(&bundle);
    runtime.execute_cycle().expect("the default work allowance");
    assert_eq!(
        runtime.storage().get_global("total"),
        Some(&Value::DInt(1_000))
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
fn a_request_above_the_computers_maximum_is_refused_with_its_numbers() {
    let root = project("maximum", COUNTING);
    let mut record = DeploymentRecord::decode(&std::fs::read(record_path(&root)).unwrap())
        .expect("the build writes a readable record");
    record.limits.max_call_depth = 2048;
    std::fs::write(record_path(&root), record.encode().unwrap()).expect("write the record");
    let error = RuntimeBundle::load(&root).expect_err("1,024 call levels at most");
    assert_eq!(
        error,
        RuntimeError::DeploymentRefused(AdmissionRefusal::Limit {
            limit: LimitKind::CallDepth,
            requested: 2048,
            maximum: 1024,
        })
    );
    assert_eq!(
        error.to_string(),
        "deployment refused: the record requests max_call_depth 2048, the device allows at most \
         1024"
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
        payload["deployment_record"],
        JsonValue::String(record_path(&root).display().to_string())
    );
    let record = DeploymentRecord::decode(&std::fs::read(record_path(&root)).unwrap())
        .expect("the command wrote a readable record");
    assert_eq!(record.application.program_sha256, sha256(&program(&root)));
    let _ = std::fs::remove_dir_all(root);
}
