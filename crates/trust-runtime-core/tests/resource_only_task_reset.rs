//! Task readiness after a resource-only commit (spec 34 HOST-09, resource-only row).
#![cfg(feature = "std")]
use std::sync::Arc;
use trust_runtime_core::{
    bytecode::{SectionData, SectionId},
    value::Duration,
    vm::{PreparationLimits, PreparedModule},
};
const FIXTURE: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

/// Spec 34 HOST-09, resource-only row: replacing task metadata stages scheduler readiness at
/// the current logical time. After the commit every task reports the commit time as its last
/// run, with no SINGLE baseline and no overrun count carried over.
#[test]
fn task_metadata_commit_resets_each_task_readiness_to_the_current_logical_time() {
    let limits = PreparationLimits::default();
    let (prepared, mut raw) = PreparedModule::from_bytes_with_artifact(FIXTURE, limits).unwrap();
    let mut state = Arc::new(prepared).instantiate_owned(0).unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    let Some(SectionData::ResourceMeta(meta)) = raw.section_mut(SectionId::ResourceMeta) else {
        panic!("resource")
    };
    meta.resources[0].memory_size += 8;
    let next = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    state
        .prepare_resource_reconfiguration(next, 0, true)
        .unwrap()
        .commit()
        .unwrap();
    assert!(!state.task_states().is_empty());
    for task in state.task_states() {
        assert_eq!(task.last_run, Duration::from_millis(30));
        assert!(!task.last_single);
        assert_eq!(task.overrun_count, 0);
    }
    assert_eq!(state.current_time(), Duration::from_millis(30));
}
