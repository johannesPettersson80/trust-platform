use super::*;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::CompileSession;
use trust_runtime::runtime_core::vm::PreparationLimits;

const DIAGNOSTIC_MAX_WORK: usize = 10_000_000;

#[test]
#[ignore = "opt-in complete PLCopen CLI bytecode-constructor work capture at default and diagnostic limits"]
fn plcopen_motion_single_axis_saved_artifact_work_charges_default_and_diagnostic_limits() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis")
        .canonicalize()
        .expect("resolve the original PLCopen CLI fixture path");
    let sources_root = resolve_sources_root(&project, None).expect("resolve PLCopen fixture");
    let discovered_sources = load_sources(&sources_root).expect("load the CLI discovery sources");
    let discovered = discover_tests(&discovered_sources);
    assert_eq!(
        discovered.len(),
        14,
        "preserve the original 14 discovered ST tests"
    );

    let compile_sources = collect_project_source_files(&project, None)
        .expect("load the exact project sources used by the CLI constructor");
    let extra_program_instances = discovered
        .iter()
        .filter(|case| matches!(case.kind, TestKind::Program))
        .map(|case| case.name.clone())
        .collect::<BTreeSet<_>>();
    let session = CompileSession::from_sources(compile_sources)
        .with_extra_program_instances(extra_program_instances);
    let bytes = session
        .build_bytecode_bytes()
        .expect("build the original PLCopen fixture artifact once");
    retain_work_capture_artifact("plcopen-single-axis", &bytes);
    let artifact_hash = trust_runtime::ads::diagnostics::sha256_evidence_hash(&bytes);
    eprintln!(
        "plcopen-work-capture operation=single_axis artifact_sha256={artifact_hash} bytes={} discovered={} default_max_work={} diagnostic_max_work={DIAGNOSTIC_MAX_WORK}",
        bytes.len(),
        discovered.len(),
        PreparationLimits::default().max_work,
    );

    let default_limits = PreparationLimits::default();
    let default_result = capture_saved_artifact_runtime(
        "single_axis_default_1m",
        &bytes,
        artifact_hash.as_str(),
        default_limits,
        false,
    );
    eprintln!(
        "plcopen-work-capture operation=single_axis_default_1m artifact_sha256={artifact_hash} outcome={:?} max_work={}",
        default_result.as_ref().map(|_| ()).map_err(|error| format!("{error:?}")),
        default_limits.max_work,
    );
    drop(default_result);

    let diagnostic_limits = PreparationLimits {
        max_work: DIAGNOSTIC_MAX_WORK,
        ..default_limits
    };
    let diagnostic_result = capture_saved_artifact_runtime(
        "single_axis_diagnostic_10m",
        &bytes,
        artifact_hash.as_str(),
        diagnostic_limits,
        true,
    );
    eprintln!(
        "plcopen-work-capture operation=single_axis_diagnostic_10m artifact_sha256={artifact_hash} outcome={:?} max_work={}",
        diagnostic_result
            .as_ref()
            .map(|_| ())
            .map_err(|error| format!("{error:?}")),
        diagnostic_limits.max_work,
    );
    diagnostic_result.expect("complete the same PLCopen artifact at the diagnostic work ceiling");
}

fn capture_saved_artifact_runtime(
    operation: &str,
    bytes: &[u8],
    artifact_hash: &str,
    limits: PreparationLimits,
    require_work: bool,
) -> Result<Runtime, RuntimeError> {
    use trust_runtime_core::vm::diagnostic_timing::{
        finish_callsite_accounting_log, start_callsite_accounting_log,
    };

    start_callsite_accounting_log();
    let result = Runtime::from_bytes(bytes, limits);
    let snapshot = finish_callsite_accounting_log();
    let thread = std::thread::current();
    let test = thread.name().unwrap_or("unnamed");
    let consumed_work: u128 = snapshot.work[..snapshot.work_len]
        .iter()
        .map(|row| u128::from(row.consumed_units))
        .sum();
    eprintln!(
        "plcopen-work-capture test={test:?} operation={operation:?} artifact_sha256={artifact_hash} bytes={} max_work={} outcome={:?} work_len={} copy_len={} lifecycle_len={} index_lookup_len={} consumed_work={consumed_work} overflow_count={} recorder_busy={} valid={}",
        bytes.len(),
        limits.max_work,
        result.as_ref().map(|_| ()).map_err(|error| format!("{error:?}")),
        snapshot.work_len,
        snapshot.copy_len,
        snapshot.lifecycle_len,
        snapshot.index_lookup_len,
        snapshot.overflow_count,
        snapshot.recorder_busy,
        snapshot.is_valid(),
    );
    eprintln!("plcopen-attribution-header test={test:?} operation={operation:?} owner_query_len={} array_consumer_len={}", snapshot.owner_queries.len(), snapshot.array_consumers.len());
    for row in &snapshot.owner_queries {
        eprintln!("plcopen-owner-query-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.array_consumers {
        eprintln!("plcopen-array-consumer-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.work[..snapshot.work_len] {
        eprintln!("plcopen-work-capture-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.index_lookups[..snapshot.index_lookup_len] {
        eprintln!("plcopen-index-capture-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.lifecycle[..snapshot.lifecycle_len] {
        eprintln!("plcopen-lifecycle-capture-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.copies[..snapshot.copy_len] {
        eprintln!(
            "plcopen-copy-capture-row test={test:?} operation={operation:?} variant={} {row:?}",
            row.value_variant.name(),
        );
    }
    assert!(
        snapshot.is_valid(),
        "PLCopen saved-artifact capture is incomplete for {operation}: overflow_count={}, recorder_busy={}, outcome={:?}",
        snapshot.overflow_count,
        snapshot.recorder_busy,
        result.as_ref().map(|_| ()).map_err(|error| format!("{error:?}")),
    );
    if require_work {
        assert!(
            snapshot.work_len > 0,
            "PLCopen saved-artifact capture has no work rows for {operation}: outcome={:?}",
            result
                .as_ref()
                .map(|_| ())
                .map_err(|error| format!("{error:?}")),
        );
    }
    result
}

fn retain_work_capture_artifact(name: &str, bytes: &[u8]) {
    let Some(directory) = std::env::var_os("TRUST_WORK_CAPTURE_ARTIFACT_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create diagnostic artifact directory");
    let path = directory.join(format!("{name}.stbc"));
    std::fs::write(&path, bytes).expect("retain the exact diagnostic artifact");
    let module = trust_runtime::bytecode::BytecodeModule::decode(bytes)
        .expect("decode the same retained diagnostic artifact");
    let tables = format!(
        "POU_INDEX {:#?}\nSTRING_TABLE {:#?}\n",
        module.section(trust_runtime::bytecode::SectionId::PouIndex),
        module.section(trust_runtime::bytecode::SectionId::StringTable)
    );
    std::fs::write(directory.join(format!("{name}.tables.txt")), tables)
        .expect("retain numeric identity tables");
    let hash = trust_runtime::ads::diagnostics::sha256_evidence_hash(bytes);
    eprintln!(
        "work-capture-artifact path={path:?} artifact_sha256={hash} bytes={}",
        bytes.len()
    );
}
