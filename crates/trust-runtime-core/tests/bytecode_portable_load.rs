#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime_core::bytecode::{BytecodeError, BytecodeModule, SectionData, SectionId};

#[test]
fn saved_container_materializes_task_metadata_without_host_or_hir() {
    let bytes = base_module().encode().expect("encode fixture");
    let module = BytecodeModule::decode(&bytes).expect("decode saved bytes");
    module.validate().expect("validate complete container");
    let metadata = module.metadata().expect("materialize metadata");
    let resource = metadata.primary_resource().expect("resource");
    assert_eq!(resource.name, "R");
    assert_eq!(resource.tasks.len(), 1);
    assert_eq!(resource.tasks[0].name, "T");
    assert_eq!(resource.tasks[0].interval.as_nanos(), 1_000_000);
    assert_eq!(resource.tasks[0].programs[0], "Main");
    assert_eq!(module.view().encode().unwrap(), bytes);
}

#[test]
fn decoded_container_is_not_semantically_validated_by_parsing() {
    let mut source = base_module();
    let Some(SectionData::PouBodies(code)) = source.section_mut(SectionId::PouBodies) else {
        panic!("fixture contains POU bodies");
    };
    code[0] = 0xff;
    let bytes = source.encode().expect("encode malformed instruction");
    let decoded = BytecodeModule::decode(&bytes).expect("container framing is valid");
    assert_eq!(decoded.validate(), Err(BytecodeError::InvalidOpcode(0xff)));
    assert_eq!(
        decoded.view().validate(),
        Err(BytecodeError::InvalidOpcode(0xff))
    );
}

#[test]
fn legacy_1_0_and_1_1_serialization_preserve_their_byte_layouts() {
    for minor in [0, 1] {
        let mut module = base_module();
        module.version.minor = minor;
        module.flags = u32::from(minor == 1);
        let bytes = module.encode().expect("encode");
        let decoded = BytecodeModule::decode(&bytes).expect("decode");
        decoded.validate().expect("validate");
        assert_eq!(decoded.version.minor, minor);
        assert_eq!(decoded.encode().unwrap(), bytes);
    }
}

#[test]
fn overflowing_pou_extent_is_rejected_before_slicing() {
    let mut module = base_module();
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("fixture POU index");
    };
    index.entries[0].code_offset = 0xffff_fffc;
    index.entries[0].code_length = 8;
    let error = module.validate().unwrap_err();
    assert_eq!(
        error,
        trust_runtime_core::bytecode::RejectionReason::PouCodeOutOfBounds.into()
    );
    assert_eq!(
        error.to_string(),
        "invalid section data: POU code out of bounds"
    );
}

#[test]
fn jump_overflow_and_non_instruction_targets_are_rejected() {
    for (offset, target) in [
        (i32::MAX, i32::MAX),
        (i32::MIN, i32::MIN + 5),
        (-4, 1),
        (1, 6),
    ] {
        let module = jump_module(offset);
        assert_eq!(
            module.validate(),
            Err(BytecodeError::InvalidJumpTarget(target))
        );
    }
    for offset in [-5, 0] {
        jump_module(offset)
            .validate()
            .expect("jump to instruction start or code end");
    }
}

fn jump_module(offset: i32) -> BytecodeModule {
    let mut module = base_module();
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("fixture POU bodies");
    };
    *code = vec![0x02];
    code.extend_from_slice(&offset.to_le_bytes());
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("fixture POU index");
    };
    index.entries[0].code_length = 5;
    module
}

#[test]
fn validated_marker_rejects_struct_built_container_bypasses() {
    use trust_runtime_core::bytecode::{RejectionReason, Section};
    let mut wrong_version = base_module();
    wrong_version.version.major = 99;
    assert!(matches!(
        wrong_version.validated(),
        Err(BytecodeError::UnsupportedVersion { major: 99, .. })
    ));
    let mut duplicate = base_module();
    duplicate.sections.push(duplicate.sections[0].clone());
    assert!(matches!(
        duplicate.validated(),
        Err(BytecodeError::InvalidSection(_))
    ));
    let mut mismatch = base_module();
    mismatch.sections[0] = Section {
        id: SectionId::StringTable.as_raw(),
        flags: 0,
        data: SectionData::Raw(vec![]),
    };
    assert_eq!(
        mismatch.validated().unwrap_err(),
        BytecodeError::from(RejectionReason::SectionPayloadMismatch)
    );
}

#[test]
fn section_borrow_outlives_the_temporary_view() {
    let module = base_module();
    let section = {
        let view = module.view();
        view.section(SectionId::PouBodies).unwrap()
    };
    assert!(matches!(section, SectionData::PouBodies(_)));
    let validated = module.validated().unwrap();
    assert!(std::ptr::eq(
        section,
        validated.section(SectionId::PouBodies).unwrap()
    ));
}
