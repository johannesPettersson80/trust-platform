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
