//! The typed loop's rules against specification 10 §6.4 and against the general path's functions,
//! which a body that compact code covers must end as (specification 34 PERF-05).

use alloc::format;
use alloc::vec::Vec;

use super::*;
use crate::memory::cells::{read_scalar_cell, read_scalar_leaf};
use crate::value::DateTimeProfile;
use crate::vm::type_policy::primitive_value_matches;

use crate::program_model::BinaryOp::{
    Add, And, Div, Eq, Ge, Gt, Le, Lt, Mod, Mul, Ne, Or, Pow, Sub, Xor,
};

/// Every integer kind's cells at the edges of its range and around zero.
fn integer_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for value in [i8::MIN, -7, -1, 0, 1, 2, 7, i8::MAX] {
        cells.push(Cell::SInt(value));
    }
    for value in [i16::MIN, -7, -1, 0, 2, 7, i16::MAX] {
        cells.push(Cell::Int(value));
    }
    for value in [i32::MIN, -7, -1, 0, 2, 7, i32::MAX] {
        cells.push(Cell::DInt(value));
    }
    for value in [i64::MIN, -7, -1, 0, 2, 7, i64::MAX] {
        cells.push(Cell::LInt(value));
    }
    for value in [0, 1, 2, 7, u8::MAX] {
        cells.push(Cell::USInt(value));
    }
    for value in [0, 2, 7, u16::MAX] {
        cells.push(Cell::UInt(value));
    }
    for value in [0, 2, 7, u32::MAX] {
        cells.push(Cell::UDInt(value));
    }
    for value in [0, 2, 7, u64::MAX] {
        cells.push(Cell::ULInt(value));
    }
    cells
}

/// Specification 34 PERF-05: a body that compact code covers ends as on the general path. For every
/// pair of integer kinds, the edge values of each and every binary operation compact code has,
/// the cells give the result or the refusal that `apply_binary` gives their values.
/// Catches: a result narrowed to another width than the common type, an overflow that wraps, a
/// division or remainder by zero or by -1 that differs, a common type across signedness, and logic
/// on integers that the general path refuses.
#[test]
fn two_integers_compute_on_their_cells_as_on_their_values() {
    let profile = DateTimeProfile::default();
    let ops = [
        Add, Sub, Mul, Div, Mod, Pow, And, Or, Xor, Eq, Ne, Lt, Le, Gt, Ge,
    ];
    let cells = integer_cells();
    for &left in &cells {
        for &right in &cells {
            for op in ops {
                let on_cells = cell_binary(op, left, right)
                    .expect("two integers are computed on their cells")
                    .map(|cell| value_of(cell).expect("a result cell has a value"));
                let on_values = apply_binary(
                    op,
                    value_of(left).unwrap(),
                    value_of(right).unwrap(),
                    &profile,
                );
                assert_eq!(on_cells, on_values, "{op:?} of {left:?} and {right:?}");
            }
        }
    }
}

/// Specification 10 §6.4, worked by hand: a common type only along SINT → INT → DINT → LINT and
/// USINT → UINT → UDINT → ULINT, results checked in it, division truncated toward zero, `MOD`
/// with the sign of its first operand, and division and modulo by zero refused by name.
/// Catches: a widening across signedness, a wrapped or saturated result, a division rounded
/// otherwise, a `MOD` with the divisor's sign, and the two zero divisors confused.
#[test]
fn integer_cells_follow_the_type_promotion_rules() {
    let overflow = Some(Err(RuntimeError::Overflow));
    // 127 + 1 = 128 is above SINT's 127.
    assert_eq!(cell_binary(Add, Cell::SInt(127), Cell::SInt(1)), overflow);
    // INT 7 + SINT 5: SINT widens to INT, 12.
    assert_eq!(
        cell_binary(Add, Cell::Int(7), Cell::SInt(5)),
        Some(Ok(Cell::Int(12)))
    );
    // UINT 3 - 5 = -2 is below UINT's 0.
    assert_eq!(cell_binary(Sub, Cell::UInt(3), Cell::UInt(5)), overflow);
    // UDINT and DINT, or USINT and SINT, have no common type.
    assert_eq!(
        cell_binary(Add, Cell::UDInt(1), Cell::DInt(1)),
        Some(Err(RuntimeError::TypeMismatch))
    );
    assert_eq!(
        cell_binary(Lt, Cell::SInt(-1), Cell::USInt(0)),
        Some(Err(RuntimeError::TypeMismatch))
    );
    // -7 / 2 = -3.5, truncated toward zero to -3; -7 MOD 2 = -7 - (-3 × 2) = -1.
    assert_eq!(
        cell_binary(Div, Cell::DInt(-7), Cell::DInt(2)),
        Some(Ok(Cell::DInt(-3)))
    );
    assert_eq!(
        cell_binary(Mod, Cell::DInt(-7), Cell::DInt(2)),
        Some(Ok(Cell::DInt(-1)))
    );
    // DINT -2,147,483,648 / -1 = 2,147,483,648 is above DINT's range; its MOD -1 is 0.
    assert_eq!(
        cell_binary(Div, Cell::DInt(i32::MIN), Cell::DInt(-1)),
        overflow
    );
    assert_eq!(
        cell_binary(Mod, Cell::DInt(i32::MIN), Cell::DInt(-1)),
        Some(Ok(Cell::DInt(0)))
    );
    // A zero divisor.
    assert_eq!(
        cell_binary(Div, Cell::LInt(7), Cell::LInt(0)),
        Some(Err(RuntimeError::DivisionByZero))
    );
    assert_eq!(
        cell_binary(Mod, Cell::UDInt(7), Cell::UDInt(0)),
        Some(Err(RuntimeError::ModuloByZero))
    );
    // ULINT 18,446,744,073,709,551,615 + 1 leaves ULINT's range; 2 ** 3 = 8.
    assert_eq!(
        cell_binary(Add, Cell::ULInt(u64::MAX), Cell::ULInt(1)),
        overflow
    );
    assert_eq!(
        cell_binary(Pow, Cell::ULInt(2), Cell::ULInt(3)),
        Some(Ok(Cell::ULInt(8)))
    );
    // INT -1 < DINT 0 in DINT; UINT 7 = UDINT 7 in UDINT.
    assert_eq!(
        cell_binary(Lt, Cell::Int(-1), Cell::DInt(0)),
        Some(Ok(Cell::Bool(true)))
    );
    assert_eq!(
        cell_binary(Eq, Cell::UInt(7), Cell::UDInt(7)),
        Some(Ok(Cell::Bool(true)))
    );
}

/// The logic and equality of two BOOLs are computed on their cells as on their values (specification
/// 10 §6.3 evaluates both operands of these instructions), and every other operation on two BOOLs,
/// any pair with a value that is no integer, is left to `apply_binary`.
/// Catches: a BOOL ordering or arithmetic computed on the cells by a rule of their own, and a REAL
/// or a duration taken for an integer.
#[test]
fn two_bools_compute_their_logic_on_their_cells() {
    let profile = DateTimeProfile::default();
    for a in [false, true] {
        for b in [false, true] {
            for op in [And, Or, Xor, Eq, Ne] {
                let on_cells = cell_binary(op, Cell::Bool(a), Cell::Bool(b))
                    .expect("BOOL logic is computed on the cells")
                    .map(|cell| value_of(cell).unwrap());
                let on_values = apply_binary(op, Value::Bool(a), Value::Bool(b), &profile);
                assert_eq!(on_cells, on_values, "{op:?} of {a} and {b}");
            }
            for op in [Lt, Le, Gt, Ge, Add, Mul] {
                assert_eq!(cell_binary(op, Cell::Bool(a), Cell::Bool(b)), None);
            }
        }
    }
    assert_eq!(cell_binary(Add, Cell::Real(1.0), Cell::Real(2.0)), None);
    assert_eq!(cell_binary(Add, Cell::Time(1), Cell::Time(2)), None);
    assert_eq!(cell_binary(Add, Cell::Byte(1), Cell::Byte(2)), None);
    assert_eq!(cell_binary(Add, Cell::DInt(1), Cell::Real(2.0)), None);
}

/// A scalar cell of every kind, a cell of no value and an instance's.
fn every_cell() -> [Cell; 27] {
    [
        Cell::Bool(true),
        Cell::SInt(-1),
        Cell::Int(-1),
        Cell::DInt(-1),
        Cell::LInt(-1),
        Cell::USInt(1),
        Cell::UInt(1),
        Cell::UDInt(1),
        Cell::ULInt(1),
        Cell::Real(1.5),
        Cell::LReal(1.5),
        Cell::Byte(1),
        Cell::Word(1),
        Cell::DWord(1),
        Cell::LWord(1),
        Cell::Char(65),
        Cell::WChar(65),
        Cell::Time(1),
        Cell::LTime(1),
        Cell::Date(1),
        Cell::LDate(1),
        Cell::Tod(1),
        Cell::LTod(1),
        Cell::Dt(1),
        Cell::Ldt(1),
        Cell::Null,
        Cell::Instance(InstanceId(3)),
    ]
}

/// A cell matches a primitive only where its value does, and for every primitive but the native
/// counter slot, which compact code never names, exactly where it does; every cell the loop holds
/// has a value, which is the cell's again.
/// Catches: a store written into a leaf whose general store would convert or refuse the value, a
/// duration of either family refused for a native duration slot, and a cell that converts to
/// another value than it came from.
#[test]
fn a_cell_matches_a_primitive_as_its_value_does() {
    for cell in every_cell() {
        let value = value_of(cell).expect("every cell the loop holds has a value");
        assert_eq!(
            cell_of(&value),
            Some(cell),
            "{cell:?} and its value {value:?}"
        );
        for prim in 0..=0x0102 {
            let by_value = primitive_value_matches(prim, &value);
            let by_cell = primitive_cell_matches(prim, &cell);
            if prim == 0x0100 {
                assert!(!by_cell, "no cell matches a native counter slot");
            } else {
                assert_eq!(by_cell, by_value, "primitive {prim} and {cell:?}");
            }
        }
    }
}

/// A leaf's cell is read where its value is read and is that value, for every primitive and leaf
/// bytes of several patterns, among them a native counter slot unbound, bound to a DINT, bound to a
/// BYTE (which a counter's read refuses) and bound to no kind.
/// Catches: a typed load of a leaf that the general read refuses, and a cell decoded otherwise than
/// the value.
#[test]
fn a_leafs_cell_is_its_value() {
    let mut patterns = [[0u8; 16], [0xFF; 16], [0x80; 16], [0x01; 16]];
    patterns[2][8] = 8;
    patterns[3][8] = 2;
    for prim in 0..=0x0102 {
        for bytes in &patterns {
            for len in [1, 2, 4, 8, 9, 16] {
                let leaf = &bytes[..len];
                let value = read_scalar_leaf(prim, leaf);
                let cell = read_scalar_cell(prim, leaf).map(|cell| value_of(cell).unwrap());
                // REAL bytes may be a NaN, which equals no value; the forms compare.
                assert_eq!(
                    format!("{cell:?}"),
                    format!("{value:?}"),
                    "primitive {prim}, bytes {leaf:?}"
                );
            }
        }
    }
}
