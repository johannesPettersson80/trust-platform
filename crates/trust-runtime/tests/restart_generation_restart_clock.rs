use trust_runtime::bytecode::{
    BytecodeModule, BytecodeVersion, InitializationStage, SectionData, SectionId, StorageOwner,
};
use trust_runtime::harness::CompileSession;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn artifact(body: &str) -> Vec<u8> {
    let source = format!("{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Tick (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Tick : Main; END_RESOURCE END_CONFIGURATION");
    CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC")
}

/// Source-free artifact whose `quotient` replacement initializer divides by the retained global `divisor`. The source initializer `INT#8 / INT#1` is rewritten so its denominator is loaded from `divisor` instead of a constant.
fn divisor_fixture() -> Vec<u8> {
    let bytes = artifact(
        r#"
VAR_GLOBAL RETAIN divisor : INT := INT#1; END_VAR
VAR_GLOBAL stamp : TIME := T#0ms; quotient : INT := INT#8 / INT#1; END_VAR
VAR_GLOBAL incoming AT %IB0 : BYTE; outgoing AT %QB0 : BYTE; END_VAR
PROGRAM Main outgoing := BYTE#42; END_PROGRAM
"#,
    );
    let mut module = BytecodeModule::decode(&bytes).unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let quotient = layout
        .entries
        .iter()
        .position(|entry| {
            entry.owner == StorageOwner::Global
                && strings.entries[entry.name_idx as usize] == "quotient"
        })
        .unwrap();
    let divisor = layout
        .entries
        .iter()
        .find(|entry| {
            entry.owner == StorageOwner::Global
                && strings.entries[entry.name_idx as usize] == "divisor"
        })
        .unwrap()
        .ref_idx
        .unwrap();
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        panic!("initializers")
    };
    let quotient_action = initializers
        .entries
        .iter()
        .find(|entry| {
            entry.declaration_idx == Some(quotient as u32)
                && entry.stage == InitializationStage::Explicit
        })
        .unwrap()
        .clone();
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("code")
    };
    let quotient_body = &mut code[quotient_action.code_offset as usize
        ..(quotient_action.code_offset + quotient_action.code_length) as usize];
    let denominator = quotient_body
        .windows(6)
        .enumerate()
        .filter(|(_, bytes)| bytes[0] == 0x10 && bytes[5] == 0x43)
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    assert_eq!(
        denominator.len(),
        1,
        "one denominator LOAD_CONST directly precedes DIV"
    );
    quotient_body[denominator[0]] = 0x20;
    quotient_body[denominator[0] + 1..denominator[0] + 5].copy_from_slice(&divisor.to_le_bytes());
    module.encode().unwrap()
}

/// Spec 34 §10 lines 1243-1247: a failed in-process warm restart leaves the installed state and its logical clock unchanged.
#[test]
fn failed_warm_restart_leaves_the_logical_clock_unchanged() {
    let prepared =
        PreparedModule::from_bytes(&divisor_fixture(), PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(50)).unwrap();
    state.write_global("divisor", Value::Int(0)).unwrap();
    let before = state.current_time();
    assert_eq!(before, Duration::from_millis(50));

    assert!(state.restart(RestartMode::Warm).is_err());
    assert_eq!(state.current_time(), before);
    assert!(state.fault().is_none());
}

/// Spec 34 §10 lines 1262-1263: retained resource-variable globals are visible to replacement initializers on a successful warm restart, and the retained value itself is preserved.
#[test]
fn retained_resource_global_feeds_replacement_initializers_on_warm_restart() {
    let prepared =
        PreparedModule::from_bytes(&divisor_fixture(), PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    state.write_global("divisor", Value::Int(2)).unwrap();

    state.restart(RestartMode::Warm).unwrap();

    assert_eq!(state.storage().get_global("divisor"), Some(&Value::Int(2)));
    assert_eq!(state.storage().get_global("quotient"), Some(&Value::Int(4)));
}
