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

/// The OpenOT library and `main_source` compiled for this computer, loaded as the harness loads a
/// program. It needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
pub(crate) fn build_st_runtime(main_source: &str) -> trust_runtime::CompiledRuntime {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use trust_runtime::native::{build, Options, Target, Toolchain};

    static BUILDS: AtomicUsize = AtomicUsize::new(0);
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!(
        "trust-openot-{}-{}",
        std::process::id(),
        BUILDS.fetch_add(1, Ordering::Relaxed)
    ));
    let built = build(
        &openot_st_source_files(main_source),
        &Options {
            target,
            hooks: true,
        },
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    let built = built.unwrap_or_else(|err| panic!("build OpenOT ST program: {err}"));
    trust_runtime::CompiledRuntime::load(built.image, "RESOURCE")
        .unwrap_or_else(|err| panic!("load OpenOT ST program: {err}"))
}
