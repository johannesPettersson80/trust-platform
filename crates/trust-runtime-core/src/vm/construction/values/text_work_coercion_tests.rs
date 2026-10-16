//! TIME-08 TEXT: coercing a supplied initializer value charges 1 unit for the constructed node,
//! plus 1 unit per byte of a STRING or WSTRING, or 4 units for a CHAR or WCHAR.
use super::*;
use crate::bytecode::{TypeData, TypeEntry, TypeKind, TypeTable};
use crate::error::ExecutionLimitKind;
use crate::value::Value;
use crate::vm::budget::ExecutionBudget;
use alloc::vec;

/// Records construction charges against one work allowance. Type recipes and instances are
/// not used by these primitive coercions, and the check of the constructed value is a no-op,
/// so the only charges are the node and the leaf copy.
struct Coercion {
    budget: ExecutionBudget,
}

impl ValueConstructionContext for Coercion {
    fn charge_value(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn strings(&self) -> &[SmolStr] {
        &[]
    }
    fn profile(&self) -> DateTimeProfile {
        DateTimeProfile::default()
    }
    fn type_recipe(&self, _type_id: u32) -> Option<u32> {
        None
    }
    fn member_recipe(&self, _type_id: u32, _member: u32) -> Option<u32> {
        None
    }
    fn evaluate_recipe(&mut self, _recipe_id: u32) -> Result<Value, RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
    fn construct_instance(
        &mut self,
        _pou_id: u32,
        _intrinsic: bool,
    ) -> Result<Value, RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
    fn charge_work(&mut self, units: usize) -> Result<(), RuntimeError> {
        self.budget.charge(units).map(|_| ())
    }
    fn charge_allocation(&mut self, _bytes: usize) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn check_value(&mut self, _type_id: u32, _value: &Value) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn enter_type(
        &mut self,
        _type_id: u32,
        _operation: ValueOperation,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn leave_type(&mut self) {}
}

/// One primitive type per entry, with the bytecode primitive ids of the encoder
/// (`crates/trust-runtime/src/bytecode/encoder/types.rs`): 0 INT (id 7; id 3 is WORD),
/// 1 STRING (id 24, unbounded), 2 WSTRING (id 25, unbounded), 3 CHAR (id 26), 4 WCHAR (id 27).
fn primitive_types() -> TypeTable {
    let primitive = |prim_id| TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id,
            max_length: 0,
        },
    };
    TypeTable {
        entries: vec![
            primitive(7),
            primitive(24),
            primitive(25),
            primitive(26),
            primitive(27),
        ],
        ..Default::default()
    }
}

/// Coerces `value` into the primitive type `type_id` under `limit` units. Returns the outcome
/// and the units left afterwards.
fn coerce(type_id: u32, value: Value, limit: usize) -> (Result<Value, RuntimeError>, usize) {
    let types = primitive_types();
    let mut context = Coercion {
        budget: ExecutionBudget::new(limit),
    };
    let outcome = construct_value(
        &types,
        &mut context,
        type_id,
        ValueConstructionMode::Coerce(value),
    );
    (outcome, context.budget.remaining())
}

#[test]
fn coerced_string_charges_one_unit_for_the_node_and_one_per_byte() {
    // "åäö" is 3 characters in 6 UTF-8 bytes: the node costs 1 and the copy 6, so 7 in all.
    let (outcome, remaining) = coerce(1, Value::String("åäö".into()), 7);
    assert_eq!(outcome.unwrap(), Value::String("åäö".into()));
    assert_eq!(remaining, 0);

    let (outcome, remaining) = coerce(1, Value::String("åäö".into()), 6);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 1,
            requested: 6,
            limit: 6,
        })
    ));
    assert_eq!(remaining, 5);
}

#[test]
fn coerced_wstring_charges_one_unit_for_the_node_and_one_per_byte() {
    // "αβγ" is 3 characters in 6 UTF-8 bytes: the node costs 1 and the copy 6, so 7 in all.
    let (outcome, remaining) = coerce(2, Value::WString("αβγ".into()), 7);
    assert_eq!(outcome.unwrap(), Value::WString("αβγ".into()));
    assert_eq!(remaining, 0);

    let (outcome, remaining) = coerce(2, Value::WString("αβγ".into()), 6);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 1,
            requested: 6,
            limit: 6,
        })
    ));
    assert_eq!(remaining, 5);
}

#[test]
fn coerced_char_charges_one_unit_for_the_node_and_four_for_the_character() {
    let (outcome, remaining) = coerce(3, Value::Char(b'a'), 5);
    assert_eq!(outcome.unwrap(), Value::Char(b'a'));
    assert_eq!(remaining, 0);

    let (outcome, remaining) = coerce(3, Value::Char(b'a'), 4);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 1,
            requested: 4,
            limit: 4,
        })
    ));
    assert_eq!(remaining, 3);
}

#[test]
fn coerced_wchar_charges_one_unit_for_the_node_and_four_for_the_character() {
    let (outcome, remaining) = coerce(4, Value::WChar(u16::from(b'a')), 5);
    assert_eq!(outcome.unwrap(), Value::WChar(u16::from(b'a')));
    assert_eq!(remaining, 0);

    let (outcome, remaining) = coerce(4, Value::WChar(u16::from(b'a')), 4);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 1,
            requested: 4,
            limit: 4,
        })
    ));
    assert_eq!(remaining, 3);
}

#[test]
fn coerced_integer_charges_one_unit_for_its_node() {
    // A scalar has no text, so its coercion is the constructed node alone.
    let (outcome, remaining) = coerce(0, Value::Int(5), 1);
    assert_eq!(outcome.unwrap(), Value::Int(5));
    assert_eq!(remaining, 0);

    let (outcome, remaining) = coerce(0, Value::Int(5), 0);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(remaining, 0);
}
