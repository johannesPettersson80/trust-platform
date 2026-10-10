//! A valid wire record need not be executable by the selected runtime profile.
use trust_runtime_core::bytecode::{
    BytecodeModule, RefEntry, RefLocation, SectionData, SectionId, StorageOwner, StorageRole,
};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

#[test]
fn source_free_profile_rejects_raw_retain_and_io_reference_domains_before_execution() {
    let bytes =
        include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
    for (location, label) in [(RefLocation::Retain, "retain"), (RefLocation::Io, "io")] {
        let mut module = BytecodeModule::decode(bytes).unwrap();
        if location == RefLocation::Retain {
            let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout)
            else {
                panic!("saved STORAGE_LAYOUT")
            };
            let binding = layout
                .entries
                .iter()
                .find(|entry| {
                    entry.owner == StorageOwner::Global && entry.role == StorageRole::ProgramRoot
                })
                .expect("saved program root declaration")
                .ref_idx
                .unwrap();
            let Some(SectionData::RefTable(table)) = module.section_mut(SectionId::RefTable) else {
                panic!("saved REF_TABLE")
            };
            let base = &table.entries[binding as usize];
            assert_eq!(base.location, RefLocation::Global);
            let key = (base.owner_id, base.offset);
            // Rebind the existing physical root, preserving its declaration/root
            // indexes, type and construction demand. External aliases do not
            // satisfy the physical binding index required for Retain references.
            for reference in &mut table.entries {
                if reference.location == RefLocation::Global
                    && (reference.owner_id, reference.offset) == key
                {
                    reference.location = RefLocation::Retain;
                }
            }
        } else {
            let Some(SectionData::RefTable(table)) = module.section_mut(SectionId::RefTable) else {
                panic!("saved REF_TABLE")
            };
            // Raw I/O references own no declaration or construction root.
            table.entries.push(RefEntry {
                location,
                owner_id: 0,
                offset: 0,
                segments: vec![],
            });
        }
        module
            .validated_source_free(Default::default())
            .expect("raw domain has structurally complete construction metadata");
        let error = PreparedModule::from_decoded(&module, PreparationLimits::default())
            .expect_err("execution profile must reject unsupported raw storage");
        assert!(error.to_string().contains(label), "{label}: {error}");
    }
}

#[test]
fn external_global_write_cannot_replace_a_program_root() {
    let bytes =
        include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
    let prepared = PreparedModule::from_bytes(bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let root = state.storage().get_global("Plant").unwrap().clone();
    assert_eq!(
        state.write_global("Plant", root.clone()),
        Err(trust_runtime_core::error::RuntimeError::ProgramRootReplacement)
    );
    assert_eq!(state.storage().get_global("Plant"), Some(&root));
}
