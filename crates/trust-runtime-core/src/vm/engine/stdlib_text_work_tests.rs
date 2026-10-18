//! TIME-02 and TIME-08 in the engine: a text call charges its arguments before its body runs
//! and its new result after it. A refused result charge discards the result.
use super::*;
use crate::error::ExecutionLimitKind;
use crate::vm::call::bindings::{VmNativeArg, VmNativeArgValue};
use crate::vm::call::stdlib::dispatch_native_stdlib_call;
use crate::vm::{VmFrame, VmTrap};
use smol_str::SmolStr;

fn expression(value: Value) -> VmNativeArg {
    VmNativeArg {
        name: None,
        value: VmNativeArgValue::Expr(value),
    }
}

fn empty_frame() -> VmFrame {
    VmFrame {
        activation: None,
        pou_id: None,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    }
}

fn concat_args() -> [VmNativeArg; 2] {
    [
        expression(Value::String("abcdefgh".into())),
        expression(Value::String("ijklmnop".into())),
    ]
}

#[test]
fn concat_charges_its_arguments_then_one_unit_and_its_result_bytes() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(100);
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let name = SmolStr::new("CONCAT");
    let mut frame = empty_frame();
    let value =
        dispatch_native_stdlib_call(&mut state, &mut frame, &name, &name, None, &concat_args())
            .unwrap();
    assert_eq!(value, Value::String("abcdefghijklmnop".into()));
    // TIME-08: 16 units for the two argument bytes, then one unit for the new result.
    assert_eq!(state.data.resources.work_budget.remaining(), 83);
    // Each 8-byte STRING argument copies its 8 bytes, and the new 16-byte result costs its 16
    // bytes (TIME-08): 8 + 8 + 16 = 32.
    assert_eq!(state.data.resources.construction_bytes.get(), 32);
}

#[test]
fn text_result_refused_by_its_unit_is_discarded() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.construction_bytes.set(0);
    // The arguments take exactly 16 units, so the result's single unit is refused.
    data.resources.work_budget.reset(16);
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let name = SmolStr::new("CONCAT");
    let mut frame = empty_frame();
    let error =
        dispatch_native_stdlib_call(&mut state, &mut frame, &name, &name, None, &concat_args())
            .unwrap_err();
    assert!(matches!(
        error,
        VmTrap::Runtime(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 16,
            requested: 1,
            limit: 16,
        })
    ));
    // Only the two 8-byte argument copies were admitted before the refused result: 8 + 8 = 16.
    assert_eq!(state.data.resources.construction_bytes.get(), 16);
}

#[test]
fn text_result_refused_by_its_bytes_is_discarded() {
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    // The two 8-byte arguments fit (8 + 8 = 16 bytes); the 16-byte result is one byte too many
    // for a limit of 31.
    prepared.limits.max_construction_bytes = 31;
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(100);
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let name = SmolStr::new("CONCAT");
    let mut frame = empty_frame();
    let error =
        dispatch_native_stdlib_call(&mut state, &mut frame, &name, &name, None, &concat_args())
            .unwrap_err();
    assert!(matches!(
        error,
        VmTrap::Runtime(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::AllocationBytes,
            used,
            requested,
            limit,
        }) if used == 16 && requested == 16 && limit == 31
    ));
    assert_eq!(state.data.resources.construction_bytes.get(), 16);
}

/// Spec 34 TIME-08, standard-library text: a WSTRING result is charged its copy allocation, two
/// bytes per UTF-16 code unit, and each WSTRING argument pays its own copy first.
#[test]
fn a_wstring_result_charges_two_bytes_per_utf16_unit() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(100);
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let name = SmolStr::new("CONCAT");
    let mut frame = empty_frame();
    // "a" plus U+1F600 is 5 UTF-8 bytes and 3 UTF-16 units; "b" is 1 byte and 1 unit.
    let args = [
        expression(Value::WString("a\u{1F600}".into())),
        expression(Value::WString("b".into())),
    ];
    let value =
        dispatch_native_stdlib_call(&mut state, &mut frame, &name, &name, None, &args).unwrap();
    assert_eq!(value, Value::WString("a\u{1F600}b".into()));
    // Work: each WSTRING argument copy pays its unit (2), the text bytes before the body (5 + 1
    // = 6), and the result its unit (1): 9 units, so 100 - 9 = 91 remain.
    assert_eq!(state.data.resources.work_budget.remaining(), 91);
    // Allocation: the argument copies, 3 units x 2 = 6 bytes and 1 unit x 2 = 2 bytes, then the
    // result's 4 units x 2 = 8 bytes: 6 + 2 + 8 = 16.
    assert_eq!(state.data.resources.construction_bytes.get(), 16);
}
