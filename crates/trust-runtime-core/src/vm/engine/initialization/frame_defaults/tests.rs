//! Direct frame defaults write and charge exactly what the declaration action does.
use super::*;
use crate::bytecode::InitializationPhase;
use crate::vm::PreparationLimits;

fn fixture() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

/// The fixture's method return (`Counter.Advance : INT`): a frame-owned default
/// without an executable body, constructed as an intrinsic scalar.
fn return_default(prepared: &PreparedModule) -> u32 {
    let id = prepared
        .initializers
        .entries
        .iter()
        .position(|entry| {
            entry.phase == InitializationPhase::Return
                && entry.stage == InitializationStage::Default
                && entry.once == InitializationOnce::None
                && entry.code_length == 0
                && entry
                    .declaration_idx
                    .and_then(|index| prepared.layout.entries.get(index as usize))
                    .is_some_and(|declaration| {
                        declaration.owner == StorageOwner::Frame
                            && declaration.default_const_idx.is_none()
                    })
        })
        .expect("fixture method return default");
    id as u32
}

fn declaration_of(prepared: &PreparedModule, id: u32) -> usize {
    prepared.initializers.entries[id as usize]
        .declaration_idx
        .expect("declaration action") as usize
}

fn frame_for(prepared: &PreparedModule, id: u32) -> VmFrame {
    let declaration = &prepared.layout.entries[declaration_of(prepared, id)];
    let slots = declaration.slot as usize + 1;
    VmFrame {
        activation: None,
        pou_id: declaration.owner_pou_id,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: alloc::vec![Value::Null; slots],
        parameter_values_present: alloc::vec![false; slots],
        runtime_instance: None,
        instance_owner: None,
    }
}

#[derive(Debug, PartialEq)]
struct Applied {
    result: Result<(), RuntimeError>,
    locals: Vec<Value>,
    work_remaining: usize,
    constructed_values: usize,
    construction_bytes: usize,
}

/// Applies `id` to a fresh frame through the direct path or the declaration action.
fn apply(prepared: &PreparedModule, id: u32, direct: bool) -> Applied {
    apply_with_limits(prepared, prepared, id, direct, 64)
}

/// As `apply`, constructing the state from `base` and executing under `prepared`
/// with `work` units, so a limit that would refuse construction itself can be tested.
fn apply_with_limits(
    base: &PreparedModule,
    prepared: &PreparedModule,
    id: u32,
    direct: bool,
    work: usize,
) -> Applied {
    let mut data = EngineState::construct_data(base, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, prepared, &services::LOGICAL_ONLY);
    state.data.resources.constructed_values = 0;
    state.data.resources.construction_bytes.set(0);
    state.data.resources.work_budget.reset(work);
    let mut frame = frame_for(prepared, id);
    let result = if direct {
        state
            .apply_direct_frame_default(id, &mut frame, 0)
            .map(|applied| assert!(applied, "the default must take the direct path"))
    } else {
        state.run_declaration_action(id, None, Some(&mut frame), 0)
    };
    Applied {
        result,
        locals: frame.locals,
        work_remaining: state.data.resources.work_budget.remaining(),
        constructed_values: state.data.resources.constructed_values,
        construction_bytes: state.data.resources.construction_bytes.get(),
    }
}

#[test]
fn intrinsic_scalar_frame_default_matches_the_declaration_action() {
    let prepared = fixture();
    let id = return_default(&prepared);
    let direct = apply(&prepared, id, true);
    assert_eq!(direct, apply(&prepared, id, false));
    assert_eq!(direct.result, Ok(()));
    assert_eq!(direct.locals.last(), Some(&Value::Int(0)));
    // TIME-08 CONSTRUCT: one work unit, one type-stack entry and one constructed node.
    assert_eq!(direct.work_remaining, 63);
    assert_eq!(direct.constructed_values, 1);
    assert_eq!(
        direct.construction_bytes,
        core::mem::size_of::<(u32, crate::vm::construction::values::ValueOperation)>()
    );
}

#[test]
fn literal_frame_defaults_match_the_declaration_action() {
    // TIME-08 COPY: a scalar copy is admitted without work; an owned text copy
    // charges one unit plus its payload. Each literal has its declaration's
    // type, as admission requires of every default constant.
    let value_bytes = core::mem::size_of::<Value>();
    for (literal, wstring, work, bytes) in [
        (Value::Int(7), false, 0, value_bytes),
        (Value::WString("A".repeat(64)), true, 1, value_bytes + 64),
    ] {
        let (mut prepared, id) = if wstring {
            with_primitive(25)
        } else {
            let prepared = fixture();
            let id = return_default(&prepared);
            (prepared, id)
        };
        let declaration = declaration_of(&prepared, id);
        if wstring {
            let wstring_type = prepared.vm.types.entries.len() as u32 - 1;
            prepared.layout.entries[declaration].type_id = Some(wstring_type);
        }
        let constant = prepared.vm.consts.len() as u32;
        prepared.vm.consts.push(literal.clone());
        prepared.layout.entries[declaration].default_const_idx = Some(constant);
        let direct = apply(&prepared, id, true);
        assert_eq!(direct, apply(&prepared, id, false));
        assert_eq!(direct.result, Ok(()));
        assert_eq!(direct.locals.last(), Some(&literal));
        assert_eq!(direct.work_remaining, 64 - work);
        assert_eq!(direct.construction_bytes, bytes);
        assert_eq!(direct.constructed_values, 0);
    }
}

#[test]
fn frame_default_depth_refusal_matches_and_charges_nothing() {
    let base = fixture();
    let mut prepared = fixture();
    prepared.limits.max_call_depth = 0;
    let id = return_default(&prepared);
    let direct = apply_with_limits(&base, &prepared, id, true, 64);
    assert_eq!(direct, apply_with_limits(&base, &prepared, id, false, 64));
    assert_eq!(
        direct.result,
        Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error())
    );
    assert_eq!(direct.work_remaining, 64);
    assert_eq!(direct.constructed_values, 0);
}

#[test]
fn other_defaults_take_the_declaration_action_without_charges() {
    let declined = |prepared: &PreparedModule, id: u32| {
        let mut data = EngineState::construct_data(prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let mut state = EngineState::borrow(&mut data, prepared, &services::LOGICAL_ONLY);
        state.data.resources.constructed_values = 0;
        state.data.resources.work_budget.reset(64);
        let mut frame = frame_for(prepared, id);
        assert_eq!(
            state.apply_direct_frame_default(id, &mut frame, 0),
            Ok(false)
        );
        assert_eq!(state.data.resources.work_budget.remaining(), 64);
        assert_eq!(state.data.resources.constructed_values, 0);
        assert!(frame.locals.iter().all(|value| *value == Value::Null));
    };
    let mut prepared = fixture();
    let id = return_default(&prepared);
    // An executable body.
    prepared.initializers.entries[id as usize].code_length = 1;
    declined(&prepared, id);
    // A static once marker.
    prepared.initializers.entries[id as usize].code_length = 0;
    prepared.initializers.entries[id as usize].once = InitializationOnce::Instance;
    declined(&prepared, id);
    // A literal that holds nested storage needs the general lifetime walk.
    prepared.initializers.entries[id as usize].once = InitializationOnce::None;
    let declaration = declaration_of(&prepared, id);
    let constant = prepared.vm.consts.len() as u32;
    prepared
        .vm
        .consts
        .push(Value::Instance(crate::memory::InstanceId(1)));
    prepared.layout.entries[declaration].default_const_idx = Some(constant);
    declined(&prepared, id);
    // Storage owned by an instance rather than the frame.
    prepared.layout.entries[declaration].default_const_idx = None;
    prepared.layout.entries[declaration].owner = StorageOwner::Instance;
    declined(&prepared, id);
}

/// Points the return default at a new primitive type of `prim_id`.
fn with_primitive(prim_id: u16) -> (PreparedModule, u32) {
    let mut prepared = fixture();
    let id = return_default(&prepared);
    let type_id = prepared.vm.types.entries.len() as u32;
    prepared.vm.types.entries.push(crate::bytecode::TypeEntry {
        kind: crate::bytecode::TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id,
            max_length: 0,
        },
    });
    let result_ref = prepared.initializers.entries[id as usize].result_ref_idx as usize;
    prepared.vm.ref_types[result_ref] = Some(type_id);
    (prepared, id)
}

#[test]
fn every_direct_scalar_default_matches_the_declaration_action() {
    let primitives = (1..=23).chain([26, 27]);
    for prim_id in primitives {
        let (prepared, id) = with_primitive(prim_id);
        let direct = apply(&prepared, id, true);
        assert_eq!(direct, apply(&prepared, id, false), "primitive {prim_id}");
        assert_eq!(direct.result, Ok(()), "primitive {prim_id}");
        assert!(
            !matches!(direct.locals.last(), Some(Value::Null) | None),
            "primitive {prim_id} must be written"
        );
    }
}

#[test]
fn direct_defaults_refuse_at_every_limit_like_the_declaration_action() {
    let base = fixture();
    let id = return_default(&base);
    let entry_bytes =
        core::mem::size_of::<(u32, crate::vm::construction::values::ValueOperation)>();
    // Work exhausted before the CONSTRUCT unit.
    let direct = apply_with_limits(&base, &base, id, true, 0);
    assert_eq!(direct, apply_with_limits(&base, &base, id, false, 0));
    assert!(direct.result.is_err());
    // The type-stack entry does not fit the allocation limit.
    let mut limited = fixture();
    limited.limits.max_construction_bytes = entry_bytes - 1;
    let direct = apply_with_limits(&base, &limited, id, true, 64);
    assert_eq!(direct, apply_with_limits(&base, &limited, id, false, 64));
    assert!(direct.result.is_err());
    // No constructed value may be added.
    let mut limited = fixture();
    limited.limits.max_construction_values = 0;
    let direct = apply_with_limits(&base, &limited, id, true, 64);
    assert_eq!(direct, apply_with_limits(&base, &limited, id, false, 64));
    assert!(direct.result.is_err());
    // A literal text copy that does not fit.
    let mut limited = fixture();
    let declaration = declaration_of(&limited, id);
    let constant = limited.vm.consts.len() as u32;
    limited.vm.consts.push(Value::WString("A".repeat(64)));
    limited.layout.entries[declaration].default_const_idx = Some(constant);
    limited.limits.max_construction_bytes = core::mem::size_of::<Value>() + 63;
    let direct = apply_with_limits(&base, &limited, id, true, 64);
    assert_eq!(direct, apply_with_limits(&base, &limited, id, false, 64));
    assert!(direct.result.is_err());
}
