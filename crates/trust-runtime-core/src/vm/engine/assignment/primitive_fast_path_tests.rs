use super::*;

use crate::bytecode::{BytecodeModule, SectionData, SectionId, TypeData, TypeEntry, TypeKind};
use crate::value::{
    DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue, TimeOfDayValue,
};
use crate::vm::PreparedModule;
use alloc::{vec, vec::Vec};
use core::cell::Cell;

fn decoded_module() -> BytecodeModule {
    BytecodeModule::decode(include_bytes!(
        "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    ))
    .unwrap()
}

fn append_primitives(module: &mut BytecodeModule, primitives: &[(u16, u16)]) -> Vec<u32> {
    let SectionData::TypeTable(types) = module.section_mut(SectionId::TypeTable).unwrap() else {
        panic!("type table")
    };
    let first = types.entries.len() as u32;
    for (prim_id, max_length) in primitives {
        types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: *prim_id,
                max_length: *max_length,
            },
        });
    }
    (first..first + primitives.len() as u32).collect()
}

fn prepared(module: &BytecodeModule) -> PreparedModule {
    PreparedModule::from_bytes(
        &module.encode().unwrap(),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap()
}

fn exact_non_text_values() -> Vec<(u16, Value)> {
    let mut values = vec![
        (1, Value::Bool(true)),
        (2, Value::Byte(1)),
        (3, Value::Word(2)),
        (4, Value::DWord(3)),
        (5, Value::LWord(4)),
        (6, Value::SInt(-5)),
        (7, Value::Int(-6)),
        (8, Value::DInt(-7)),
        (9, Value::LInt(-8)),
        (10, Value::USInt(9)),
        (11, Value::UInt(10)),
        (12, Value::UDInt(11)),
        (13, Value::ULInt(12)),
        (14, Value::Real(1.25)),
        (15, Value::LReal(2.5)),
        (16, Value::Time(Duration::from_nanos(-13))),
        (17, Value::LTime(Duration::from_nanos(-14))),
        (18, Value::Date(DateValue::new(15))),
        (19, Value::LDate(LDateValue::new(16))),
        (20, Value::Tod(TimeOfDayValue::new(17))),
        (21, Value::LTod(LTimeOfDayValue::new(18))),
        (22, Value::Dt(DateTimeValue::new(19))),
        (23, Value::Ldt(LDateTimeValue::new(20))),
        (26, Value::Char(b'x')),
        (27, Value::WChar(u16::from(b'y'))),
    ];
    for value in [
        Value::SInt(-21),
        Value::Int(-22),
        Value::DInt(-23),
        Value::LInt(-24),
        Value::USInt(25),
        Value::UInt(26),
        Value::UDInt(27),
        Value::ULInt(28),
        Value::Null,
    ] {
        values.push((0x0100, value));
    }
    values
}

#[test]
fn exact_non_text_primitives_and_any_int_consume_no_work_allowance() {
    let cases = exact_non_text_values();
    let primitives: Vec<_> = cases.iter().map(|(prim_id, _)| (*prim_id, 0)).collect();
    let mut module = decoded_module();
    let type_ids = append_primitives(&mut module, &primitives);
    let prepared = prepared(&module);
    let services = &crate::vm::engine::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);

    for ((_, expected), type_id) in cases.iter().zip(type_ids) {
        let allocation_before = state.data.resources.construction_bytes.get();
        state.data.resources.work_budget.reset(1);
        assert_eq!(
            state.normalize_assignment_value(type_id, expected.clone()),
            Ok(expected.clone()),
            "exact primitive type {type_id}"
        );
        // TIME-08: exact-match normalization is bookkeeping and charges nothing.
        assert_eq!(state.data.resources.work_budget.remaining(), 1);
        assert_eq!(
            state.data.resources.construction_bytes.get(),
            allocation_before
        );
    }
}

#[test]
fn mismatches_still_use_legacy_widening_and_reject_narrowing() {
    let mut module = decoded_module();
    let type_ids = append_primitives(&mut module, &[(7, 0), (8, 0), (0x0100, 0)]);
    let prepared = prepared(&module);
    let services = &crate::vm::engine::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);

    state.data.resources.work_budget.reset(1);
    assert_eq!(
        state.normalize_assignment_value(type_ids[1], Value::SInt(-91)),
        Ok(Value::DInt(-91)),
        "widening must retain the original conversion path"
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 1);

    state.data.resources.work_budget.reset(1);
    assert_eq!(
        state.normalize_assignment_value(type_ids[0], Value::DInt(91)),
        Err(RuntimeError::TypeMismatch),
        "a mismatched primitive cannot become a narrowing conversion"
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 1);

    state.data.resources.work_budget.reset(1);
    assert_eq!(
        state.normalize_assignment_value(type_ids[2], Value::Real(1.0)),
        Err(RuntimeError::TypeMismatch),
        "ANY_INT accepts only NULL and integer variants"
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 1);
}

#[test]
fn aliases_and_subranges_keep_their_bounds_without_work_charges() {
    let mut module = decoded_module();
    let SectionData::TypeTable(types) = module.section_mut(SectionId::TypeTable).unwrap() else {
        panic!("type table")
    };
    let base = types.entries.len() as u32;
    types.entries.extend([
        TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 7,
                max_length: 0,
            },
        },
        TypeEntry {
            kind: TypeKind::Alias,
            name_idx: None,
            data: TypeData::Alias {
                target_type_id: base,
            },
        },
        TypeEntry {
            kind: TypeKind::Subrange,
            name_idx: None,
            data: TypeData::Subrange {
                base_type_id: base,
                lower: -5,
                upper: 5,
            },
        },
    ]);
    let prepared = prepared(&module);
    let services = &crate::vm::engine::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);

    // TIME-08: alias levels are bookkeeping, so the full allowance is left after the walk.
    state.data.resources.work_budget.reset(2);
    assert_eq!(
        state.normalize_assignment_value(base + 1, Value::Int(4)),
        Ok(Value::Int(4))
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 2);

    state.data.resources.work_budget.reset(1);
    assert!(matches!(
        state.normalize_assignment_value(base + 2, Value::Int(6)),
        Err(RuntimeError::SubrangeViolation {
            value: 6,
            lower: -5,
            upper: 5
        })
    ));
    assert_eq!(state.data.resources.work_budget.remaining(), 1);
}

#[derive(Default)]
struct DeadlineProbe {
    calls: Cell<usize>,
    expired: Cell<bool>,
}

impl crate::vm::engine::services::ExecutionServices for DeadlineProbe {
    fn deadline_exceeded(&self) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.expired.get()
    }

    fn has_wall_clock(&self) -> bool {
        false
    }

    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
}

#[test]
fn exact_match_normalization_is_uncharged_and_samples_no_deadline() {
    let mut module = decoded_module();
    let type_id = append_primitives(&mut module, &[(7, 0)])[0];
    let prepared = prepared(&module);
    let probe = DeadlineProbe::default();
    let mut data = EngineState::construct_data(&prepared, 0, &probe).unwrap();
    probe.calls.set(0);
    let state = EngineState::borrow(&mut data, &prepared, &probe);

    // TIME-08: exact-match normalization is bookkeeping. An empty allowance neither refuses
    // the match nor samples the deadline, because nothing is charged to the stride.
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.normalize_assignment_value(type_id, Value::Int(7)),
        Ok(Value::Int(7))
    );
    assert_eq!(probe.calls.get(), 0);

    state.data.resources.work_budget.reset(64);
    assert!(!state.data.resources.work_budget.charge(31).unwrap());
    probe.expired.set(true);
    assert_eq!(
        state.normalize_assignment_value(type_id, Value::Int(7)),
        Ok(Value::Int(7))
    );
    assert_eq!(probe.calls.get(), 0);
    assert_eq!(state.data.resources.work_budget.remaining(), 33);
}
