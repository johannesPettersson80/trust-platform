//! Hosted exports copy the installed artifact and keep source-aware debug sections apart.
use trust_runtime::bytecode::{
    build_module_from_runtime, build_module_from_runtime_with_sources, BytecodeModule, SectionId,
};
use trust_runtime::harness::CompileSession;
use trust_runtime::value::{RefSegment, Value};
use trust_runtime::Runtime;

const SOURCE: &str =
    "PROGRAM Main VAR counter : INT := 7; END_VAR counter := counter + 1; END_PROGRAM";

fn is_source_debug_section(id: u16) -> bool {
    id == SectionId::DebugMap as u16 || id == SectionId::DebugStringTable as u16
}

/// Spec 34 HOST-09 (lines 524-526): a source-free installation exports the sections it
/// was admitted from, even after PLC values have changed; nothing is rebuilt from them.
/// The saved bytes carry the optional source-debug sections, and the plain export omits
/// exactly those (spec 34 lines 527-528), so the expected sections are the admitted ones
/// without them.
#[test]
fn hosted_observation_source_free_export_copies_the_installed_sections() {
    let saved = CompileSession::from_source(SOURCE)
        .build_bytecode_bytes()
        .unwrap();
    let mut installed = Runtime::from_bytes(&saved, Default::default()).unwrap();
    installed
        .write_named_path(
            "Main",
            &[RefSegment::Field("counter".into())],
            Value::Int(13),
        )
        .unwrap();

    let original = BytecodeModule::decode(&saved).unwrap();
    assert!(
        original
            .sections
            .iter()
            .any(|section| is_source_debug_section(section.id)),
        "the compiled artifact carries source-debug sections for the plain export to omit"
    );
    let admitted: Vec<_> = original
        .sections
        .iter()
        .filter(|section| !is_source_debug_section(section.id))
        .collect();
    let exported = build_module_from_runtime(&installed).unwrap();
    let exported_sections: Vec<_> = exported.sections.iter().collect();
    assert_eq!(
        exported_sections, admitted,
        "the export carries the admitted sections, not the current counter value 13"
    );
}

/// Spec 34 HOST-09 (lines 527-528): source-aware export adds only the source-debug
/// sections; every other section equals the plain export of the same installation.
#[test]
fn hosted_observation_source_aware_export_keeps_the_plain_non_debug_sections() {
    let runtime = CompileSession::from_source(SOURCE).build_runtime().unwrap();
    let plain = build_module_from_runtime(&runtime).unwrap();
    let aware = build_module_from_runtime_with_sources(&runtime, &[SOURCE]).unwrap();
    assert!(
        aware.sections.len() > plain.sections.len(),
        "the source-aware export carries the instruction-to-source map"
    );
    let plain_sections: Vec<_> = plain
        .sections
        .iter()
        .filter(|section| !is_source_debug_section(section.id))
        .collect();
    let aware_sections: Vec<_> = aware
        .sections
        .iter()
        .filter(|section| !is_source_debug_section(section.id))
        .collect();
    assert_eq!(aware_sections, plain_sections);
}
