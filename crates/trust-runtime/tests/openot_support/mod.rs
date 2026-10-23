pub(crate) const ST_LIBRARY_SOURCES: &[(&str, &str)] = &[
    (
        "openot_control_block.st",
        include_str!("../fixtures/openot/iec61131/src/openot_control_block.st.txt"),
    ),
    (
        "openot_crc32c.st",
        include_str!("../fixtures/openot/iec61131/src/openot_crc32c.st.txt"),
    ),
    (
        "openot_lifecycle.st",
        include_str!("../fixtures/openot/iec61131/src/openot_lifecycle.st.txt"),
    ),
    (
        "openot_message.st",
        include_str!("../fixtures/openot/iec61131/src/openot_message.st.txt"),
    ),
    (
        "openot_producer.st",
        include_str!("../fixtures/openot/iec61131/src/openot_producer.st.txt"),
    ),
    (
        "openot_records_dropped.st",
        include_str!("../fixtures/openot/iec61131/src/openot_records_dropped.st.txt"),
    ),
    (
        "openot_ring256_producer.st",
        include_str!("../fixtures/openot/iec61131/src/openot_ring256_producer.st.txt"),
    ),
    (
        "openot_ring.st",
        include_str!("../fixtures/openot/iec61131/src/openot_ring.st.txt"),
    ),
    (
        "openot_source_high_water.st",
        include_str!("../fixtures/openot/iec61131/src/openot_source_high_water.st.txt"),
    ),
    (
        "openot_value_state.st",
        include_str!("../fixtures/openot/iec61131/src/openot_value_state.st.txt"),
    ),
    (
        "openot_wire_encode.st",
        include_str!("../fixtures/openot/iec61131/src/openot_wire_encode.st.txt"),
    ),
];

pub(crate) const REACTOR_PROGRAM: &str =
    include_str!("../fixtures/openot/examples/reactor/Reactor.st.txt");

pub(crate) fn st_test_source(name: &str) -> &'static str {
    match name {
        "test_scan_records_burst.st" => {
            include_str!("../fixtures/openot/iec61131/tests/test_scan_records_burst.st.txt")
        }
        "test_conformant_value_changed_int.st" => include_str!(
            "../fixtures/openot/iec61131/tests/test_conformant_value_changed_int.st.txt"
        ),
        "test_conformant_value_changed_real.st" => include_str!(
            "../fixtures/openot/iec61131/tests/test_conformant_value_changed_real.st.txt"
        ),
        "test_conformant_condition_lifecycle.st" => include_str!(
            "../fixtures/openot/iec61131/tests/test_conformant_condition_lifecycle.st.txt"
        ),
        "test_conformant_batch_recipe.st" => {
            include_str!("../fixtures/openot/iec61131/tests/test_conformant_batch_recipe.st.txt")
        }
        "test_conformant_regulated.st" => {
            include_str!("../fixtures/openot/iec61131/tests/test_conformant_regulated.st.txt")
        }
        "test_authoring_api.st" => {
            include_str!("../fixtures/openot/iec61131/tests/test_authoring_api.st.txt")
        }
        "test_value_sampling.st" => {
            include_str!("../fixtures/openot/iec61131/tests/test_value_sampling.st.txt")
        }
        _ => panic!("unregistered frozen OpenOT ST test fixture: {name}"),
    }
}

pub(crate) fn openot_st_source_files(main_source: &str) -> Vec<trust_runtime::harness::SourceFile> {
    let mut sources = ST_LIBRARY_SOURCES
        .iter()
        .map(|(name, source)| trust_runtime::harness::SourceFile::with_path(*name, *source))
        .collect::<Vec<_>>();
    sources.push(trust_runtime::harness::SourceFile::with_path(
        "main.st",
        main_source,
    ));
    sources
}

pub(crate) fn with_work_charge_log<T, E: std::fmt::Debug>(
    operation: &str,
    action: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    use trust_runtime_core::vm::diagnostic_timing::{
        finish_callsite_accounting_log, start_callsite_accounting_log,
    };

    if std::env::var_os("TRUST_OPENOT_WORK_CHARGES").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return action();
    }
    start_callsite_accounting_log();
    let result = action();
    let snapshot = finish_callsite_accounting_log();
    let thread = std::thread::current();
    let test = thread.name().unwrap_or("unnamed");
    let consumed_work: u128 = snapshot.work[..snapshot.work_len]
        .iter()
        .map(|row| u128::from(row.consumed_units))
        .sum();
    eprintln!(
        "openot-work-charge test={test:?} operation={operation:?} result={:?} work_len={} copy_len={} lifecycle_len={} index_lookup_len={} consumed_work={consumed_work} overflow_count={} recorder_busy={} valid={}",
        result.as_ref().map(|_| ()),
        snapshot.work_len,
        snapshot.copy_len,
        snapshot.lifecycle_len,
        snapshot.index_lookup_len,
        snapshot.overflow_count,
        snapshot.recorder_busy,
        snapshot.is_valid(),
    );
    eprintln!("openot-attribution-header test={test:?} operation={operation:?} owner_query_len={} array_consumer_len={}", snapshot.owner_queries.len(), snapshot.array_consumers.len());
    for row in &snapshot.owner_queries {
        eprintln!("openot-owner-query-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.array_consumers {
        eprintln!("openot-array-consumer-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.work[..snapshot.work_len] {
        eprintln!("openot-work-charge-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.index_lookups[..snapshot.index_lookup_len] {
        eprintln!("openot-index-lookup-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.lifecycle[..snapshot.lifecycle_len] {
        eprintln!("openot-lifecycle-demand-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.copies[..snapshot.copy_len] {
        eprintln!(
            "openot-copy-admission-row test={test:?} operation={operation:?} variant={} {row:?}",
            row.value_variant.name(),
        );
    }
    assert!(
        snapshot.is_valid(),
        "OpenOT work-charge diagnostic capture is incomplete for {test}/{operation}: overflow_count={}, recorder_busy={}; original outcome: {:?}",
        snapshot.overflow_count,
        snapshot.recorder_busy,
        result.as_ref().map(|_| ()),
    );
    result
}

pub(crate) fn build_st_runtime(main_source: &str) -> trust_runtime::Runtime {
    let session =
        trust_runtime::harness::CompileSession::from_sources(openot_st_source_files(main_source))
            .label_errors(true);
    let mut runtime = with_work_charge_log("construct", || session.build_runtime())
        .unwrap_or_else(|err| panic!("build OpenOT ST runtime: {err}"));
    let bytecode = session
        .build_bytecode_bytes()
        .unwrap_or_else(|err| panic!("build OpenOT ST bytecode: {err}"));
    with_work_charge_log("install", || runtime.apply_bytecode_bytes(&bytecode, None))
        .unwrap_or_else(|err| panic!("apply OpenOT ST bytecode: {err}"));
    runtime
}
