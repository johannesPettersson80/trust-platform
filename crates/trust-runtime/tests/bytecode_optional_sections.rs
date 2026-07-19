use trust_runtime::bytecode::{BytecodeModule, Section, SectionData};
use trust_runtime::Runtime;

#[test]
fn unknown_optional_section_is_preserved_and_ignored_by_runtime_apply() {
    const UNKNOWN_SECTION_ID: u16 = 0x7FFF;
    const PAYLOAD: &[u8] = b"optional-v1-extension";

    let mut module =
        trust_runtime::harness::bytecode_module_from_source("PROGRAM Main\nEND_PROGRAM\n")
            .expect("compile runtime-resolvable bytecode module");
    module.sections.push(Section {
        id: UNKNOWN_SECTION_ID,
        flags: 0,
        data: SectionData::Raw(PAYLOAD.to_vec()),
    });

    let bytes = module.encode().expect("encode unknown optional section");
    let decoded = BytecodeModule::decode(&bytes).expect("decode unknown optional section");
    decoded
        .validate()
        .expect("unknown optional section must not invalidate STBC v1");
    assert!(decoded.sections.iter().any(|section| {
        section.id == UNKNOWN_SECTION_ID
            && section.flags == 0
            && matches!(&section.data, SectionData::Raw(raw) if raw == PAYLOAD)
    }));

    Runtime::new()
        .apply_bytecode_bytes(&bytes, None)
        .expect("runtime apply must ignore an unknown optional section");
}
