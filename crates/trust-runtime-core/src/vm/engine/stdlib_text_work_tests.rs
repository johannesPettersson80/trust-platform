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
fn concat_charges_its_arguments_then_one_unit_and_one_value_for_its_result() {
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
    // Each STRING argument binds one Value; the new result costs one Value plus its 16 bytes.
    let value_bytes = core::mem::size_of::<Value>();
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        3 * value_bytes + 16
    );
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
    // Only the argument Values were admitted before the refused result.
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        2 * core::mem::size_of::<Value>()
    );
}

#[test]
fn text_result_refused_by_its_bytes_is_discarded() {
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let value_bytes = core::mem::size_of::<Value>();
    // Two argument Values fit; the result needs one more Value and 16 bytes, one byte too many.
    prepared.limits.max_construction_bytes = 3 * value_bytes + 15;
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
        }) if used == 2 * value_bytes && requested == value_bytes + 16 && limit == 3 * value_bytes + 15
    ));
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        2 * value_bytes
    );
}
