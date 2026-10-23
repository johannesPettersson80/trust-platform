//! Bounded exhaustive check of SIZEOF arithmetic over type-table records and runtime values.
//!
//! Specification: `docs/specs/12-bytecode.md`, primitive identifiers of version 1.x; "3.0 temporal
//! ABI" (every temporal type is eight bytes under the uniform layout, the short forms four bytes
//! under the legacy ABI; uniform SIZEOF measures the complete relative extent rounded to bytes);
//! SIZEOF returns DINT, rejects an unknown or unsupported type as a type mismatch and reports
//! overflow when the size does not fit; runtime-value sizing permits at most 128 nested values
//! counting the leaf; the SIZEOF type-evaluation depth is 128. `docs/specs/10-runtime-semantics.md`,
//! portable SIZEOF (aliases resolve to storage, structures sum fields or use the complete relative
//! extent under the uniform ABI, arrays multiply inclusive dimensions by the element size).
//! `docs/specs/02-data-types.md`: SIZEOF, array strides and relative field extents agree.
//!
//! Production under test:
//! - `trust_runtime_core::vm::sizeof_type_from_table_with_abi`
//!   (`crates/trust-runtime-core/src/vm/dispatch_sizeof.rs:25`), with `type_array_len` (`:202`), the
//!   located-structure branch (`:86-114`) and the depth bound (`:48`);
//! - `trust_runtime_core::value::size_of_value_with_abi_and_visitor`
//!   (`crates/trust-runtime-core/src/value/size.rs:181`), with `array_len_bits` (`:267`) and the
//!   value depth bound (`:208`).
//!
//! Model and bounds.
//! - Arrays: one and two dimensions drawn from eleven inclusive bound pairs: empty `(5, 4)`, wildcard
//!   `(0, i64::MAX)`, full range, and extents whose element count exceeds u64. Element kinds BOOL,
//!   INT, DINT, LINT, TIME and LTIME, under both ABIs: 2 x 6 x (11 + 11 x 11) = 1,584 cases. The
//!   oracle counts elements in u128. A byte size that fits u64 is exact; a byte size beyond u64 is
//!   `Overflow`; an empty or wildcard dimension is an unsupported type (`TypeMismatch`); an element
//!   count beyond u64 is refused with `TypeMismatch` or `Overflow`, since the specification does not
//!   choose between them.
//! - Located structures: a BOOL at bit `b` and an INT at bit `i`, with `b` and `i` in 0..=40. Uniform
//!   ABI: the complete relative extent `max(b + 1, i + 16)` rounded up to bytes, and a mismatch when
//!   `i` is not byte-aligned (only a BOOL may sit off a byte boundary). Legacy ABI: the historic
//!   aggregate sum, 3, whatever the offsets. 1,681 pairs, each checked under both ABIs.
//! - Overflow: a structure whose two fields are 2^63-byte arrays sums past u64 and is refused.
//! - Depth: a chain of arrays above an INT is accepted with at most 127 arrays (128 levels counting
//!   the leaf) and refused at 128; a type that contains itself is refused. A runtime value nested 128
//!   values deep is accepted and 129 values deep is refused with a type mismatch.
//! - Cross-route: for each small canonical array (lower bounds -2..=2, lengths 0..=3, one or two
//!   dimensions, every element kind, both ABIs), the type-table size and the runtime-value size agree
//!   after the `RuntimeError` mapping. Non-canonical arrays and wildcard arrays are outside the value
//!   route's input contract, since a canonical array holds one element per position.
//!
//! Assumptions. Primitive identifiers and widths come from specification 12, not from the production
//! table. The test runs on a 64-bit host.

use trust_runtime_core::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{
    size_of_value_with_abi_and_visitor, ArrayValue, Duration, SizeOfError, StorageAbi, Value,
};
use trust_runtime_core::vm::sizeof_type_from_table_with_abi;

const ABIS: [StorageAbi; 2] = [StorageAbi::LegacyTemporal32, StorageAbi::UniformTemporal64];
const MIN: i64 = i64::MIN;
const MAX: i64 = i64::MAX;

/// Inclusive dimension bounds for the arrays under test.
const BOUNDS: [(i64, i64); 11] = [
    (0, 0),
    (1, 3),
    (-2, 2),
    (5, 4),
    (MIN, MAX),
    (0, MAX),
    (MIN, 0),
    (MAX - 1, MAX),
    (MIN, MIN),
    (-1, MAX - 1),
    (0, (1 << 60) - 1),
];

/// Element kinds, with their specification 12 primitive identifiers.
#[derive(Clone, Copy, Debug)]
enum Elem {
    Bool,
    Int,
    DInt,
    LInt,
    Time,
    LTime,
}

impl Elem {
    const ALL: [Elem; 6] = [
        Elem::Bool,
        Elem::Int,
        Elem::DInt,
        Elem::LInt,
        Elem::Time,
        Elem::LTime,
    ];

    fn prim_id(self) -> u16 {
        match self {
            Elem::Bool => 1,
            Elem::Int => 7,
            Elem::DInt => 8,
            Elem::LInt => 9,
            Elem::Time => 16,
            Elem::LTime => 17,
        }
    }

    /// Storage width in bytes: TIME is four bytes under the legacy ABI and eight under the uniform one.
    fn width(self, abi: StorageAbi) -> u64 {
        match self {
            Elem::Bool => 1,
            Elem::Int => 2,
            Elem::DInt => 4,
            Elem::LInt | Elem::LTime => 8,
            Elem::Time => match abi {
                StorageAbi::LegacyTemporal32 => 4,
                StorageAbi::UniformTemporal64 => 8,
            },
        }
    }

    fn value(self) -> Value {
        match self {
            Elem::Bool => Value::Bool(false),
            Elem::Int => Value::Int(0),
            Elem::DInt => Value::DInt(0),
            Elem::LInt => Value::LInt(0),
            Elem::Time => Value::Time(Duration::ZERO),
            Elem::LTime => Value::LTime(Duration::ZERO),
        }
    }
}

fn primitive(prim_id: u16, max_length: u16) -> TypeEntry {
    TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id,
            max_length,
        },
    }
}

fn array_of(elem_type_id: u32, dims: Vec<(i64, i64)>) -> TypeEntry {
    TypeEntry {
        kind: TypeKind::Array,
        name_idx: None,
        data: TypeData::Array { elem_type_id, dims },
    }
}

fn field(type_id: u32, bit_offset: Option<u32>) -> Field {
    Field {
        name_idx: 0,
        type_id,
        bit_offset,
    }
}

fn table(entries: Vec<TypeEntry>) -> TypeTable {
    TypeTable {
        offsets: Vec::new(),
        entries,
    }
}

fn sizeof(types: &TypeTable, index: u32, abi: StorageAbi) -> Result<u64, RuntimeError> {
    sizeof_type_from_table_with_abi(
        types,
        index,
        abi,
        &mut |_: usize| -> Result<(), RuntimeError> { Ok(()) },
    )
}

/// Runtime-value SIZEOF, with the portable errors mapped to their runtime error.
fn value_size(value: &Value, abi: StorageAbi) -> Result<u64, RuntimeError> {
    let mut enum_size = |_: &str| -> Result<u64, SizeOfError> { Ok(0) };
    let mut structure_size = |_: &str| -> Result<Option<u64>, SizeOfError> { Ok(None) };
    let mut visit = |_: usize| -> Result<(), SizeOfError> { Ok(()) };
    size_of_value_with_abi_and_visitor(value, abi, &mut enum_size, &mut structure_size, &mut visit)
        .map_err(RuntimeError::from)
}

/// What the specification requires of an array's SIZEOF.
#[derive(Debug, PartialEq, Eq)]
enum Expected {
    Size(u64),
    /// The byte size does not fit in u64.
    Overflow,
    /// An element count beyond u64: refused with a checked error.
    TooLarge,
    /// An empty or wildcard dimension: an unsupported type.
    Unsupported,
}

fn expected_array_size(element_width: u64, dims: &[(i64, i64)]) -> Expected {
    if dims
        .iter()
        .any(|&(lower, upper)| lower == 0 && upper == MAX)
    {
        return Expected::Unsupported;
    }
    let mut count: u128 = 1;
    for &(lower, upper) in dims {
        let length = i128::from(upper) - i128::from(lower) + 1;
        let Ok(length) = u128::try_from(length) else {
            return Expected::Unsupported;
        };
        if length == 0 {
            return Expected::Unsupported;
        }
        let Some(next) = count.checked_mul(length) else {
            return Expected::TooLarge;
        };
        count = next;
    }
    if count > u128::from(u64::MAX) {
        return Expected::TooLarge;
    }
    // The count fits u64 and the width is at most eight, so the product fits u128.
    let size = count * u128::from(element_width);
    match u64::try_from(size) {
        Ok(size) => Expected::Size(size),
        Err(_) => Expected::Overflow,
    }
}

fn check_array(elem: Elem, abi: StorageAbi, dims: &[(i64, i64)]) {
    let types = table(vec![
        primitive(elem.prim_id(), 0),
        array_of(0, dims.to_vec()),
    ]);
    let actual = sizeof(&types, 1, abi);
    let context = format!("{elem:?} x {dims:?} under {abi:?}");
    match expected_array_size(elem.width(abi), dims) {
        Expected::Size(size) => assert_eq!(actual, Ok(size), "{context}"),
        Expected::Overflow => assert_eq!(actual, Err(RuntimeError::Overflow), "{context}"),
        Expected::Unsupported => assert_eq!(actual, Err(RuntimeError::TypeMismatch), "{context}"),
        Expected::TooLarge => assert!(
            matches!(
                actual,
                Err(RuntimeError::TypeMismatch | RuntimeError::Overflow)
            ),
            "{context}: {actual:?}"
        ),
    }
}

#[test]
fn array_sizes_are_exact_or_refused_by_their_checked_error_on_every_bound_pair() {
    let mut cases = 0_u32;
    for abi in ABIS {
        for elem in Elem::ALL {
            for first in BOUNDS {
                check_array(elem, abi, &[first]);
                cases += 1;
                for second in BOUNDS {
                    check_array(elem, abi, &[first, second]);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 2 * 6 * (11 + 11 * 11));
}

#[test]
fn located_struct_extents_are_exact_under_the_uniform_abi_and_summed_under_legacy() {
    let mut cases = 0_u32;
    for bool_bit in 0_u32..=40 {
        for int_bit in 0_u32..=40 {
            let types = table(vec![
                primitive(1, 0), // BOOL
                primitive(7, 0), // INT
                TypeEntry {
                    kind: TypeKind::LocatedStruct,
                    name_idx: None,
                    data: TypeData::Struct {
                        fields: vec![field(0, Some(bool_bit)), field(1, Some(int_bit))],
                    },
                },
            ]);
            let context = format!("BOOL at bit {bool_bit}, INT at bit {int_bit}");
            let uniform = sizeof(&types, 2, StorageAbi::UniformTemporal64);
            if int_bit % 8 == 0 {
                let end = (u64::from(bool_bit) + 1).max(u64::from(int_bit) + 16);
                assert_eq!(uniform, Ok(end.div_ceil(8)), "{context}");
            } else {
                assert_eq!(uniform, Err(RuntimeError::TypeMismatch), "{context}");
            }
            assert_eq!(
                sizeof(&types, 2, StorageAbi::LegacyTemporal32),
                Ok(3),
                "{context}"
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 41 * 41);
}

#[test]
fn a_structure_whose_fields_sum_past_u64_is_refused_under_both_abis() {
    let types = table(vec![
        primitive(9, 0),                           // LINT, eight bytes
        array_of(0, vec![(0, (1_i64 << 60) - 1)]), // 2^60 elements, 2^63 bytes
        TypeEntry {
            kind: TypeKind::Struct,
            name_idx: None,
            data: TypeData::Struct {
                fields: vec![field(1, None), field(1, None)],
            },
        },
    ]);
    for abi in ABIS {
        assert_eq!(sizeof(&types, 1, abi), Ok(1_u64 << 63), "{abi:?}");
        assert_eq!(
            sizeof(&types, 2, abi),
            Err(RuntimeError::Overflow),
            "{abi:?}"
        );
    }
}

/// `arrays` one-element array types above one INT leaf; the leaf is the last entry.
fn chain(arrays: u32) -> TypeTable {
    let mut entries: Vec<TypeEntry> = (0..arrays)
        .map(|level| array_of(level + 1, vec![(0, 0)]))
        .collect();
    entries.push(primitive(7, 0));
    table(entries)
}

#[test]
fn type_nesting_is_bounded_at_128_levels_counting_the_leaf() {
    for arrays in [0_u32, 1, 126, 127, 128, 129] {
        let result = sizeof(&chain(arrays), 0, StorageAbi::UniformTemporal64);
        if arrays <= 127 {
            assert_eq!(result, Ok(2), "{arrays} arrays above an INT");
        } else {
            assert!(result.is_err(), "{arrays} arrays above an INT are refused");
        }
    }
    let self_referencing = table(vec![array_of(0, vec![(0, 0)])]);
    assert!(sizeof(&self_referencing, 0, StorageAbi::LegacyTemporal32).is_err());
}

/// A runtime value nested `levels` values deep: one-element arrays around an INT leaf.
fn nested_value(levels: u32) -> Value {
    let mut value = Value::Int(0);
    for _ in 1..levels {
        value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![value],
            vec![(0, 0)],
        )));
    }
    value
}

#[test]
fn runtime_values_nest_at_most_128_values_counting_the_leaf() {
    let accepted = nested_value(128);
    let refused = nested_value(129);
    assert_eq!(
        value_size(&accepted, StorageAbi::UniformTemporal64),
        Ok(2),
        "128 nested values"
    );
    assert_eq!(
        value_size(&refused, StorageAbi::UniformTemporal64),
        Err(RuntimeError::TypeMismatch),
        "129 nested values"
    );
}

fn check_routes(elem: Elem, abi: StorageAbi, dims: &[(i64, i64)]) {
    let types = table(vec![
        primitive(elem.prim_id(), 0),
        array_of(0, dims.to_vec()),
    ]);
    let type_route = sizeof(&types, 1, abi);
    let valid = dims.iter().all(|&(lower, upper)| upper >= lower);
    // A canonical array holds one element per position; an invalid shape keeps one element so that
    // the element kind is still visible to the value route.
    let count: usize = if valid {
        dims.iter()
            .map(|&(lower, upper)| usize::try_from(upper - lower + 1).expect("small bound"))
            .product()
    } else {
        1
    };
    let value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![elem.value(); count],
        dims.to_vec(),
    )));
    assert_eq!(
        value_size(&value, abi),
        type_route,
        "{elem:?} x {dims:?} under {abi:?}"
    );
}

#[test]
fn runtime_value_sizes_agree_with_type_sizes_on_small_canonical_arrays() {
    // Lower bounds -2..=2 and lengths 0..=3; length 0 is an empty dimension.
    let axis: Vec<(i64, i64)> = (-2_i64..=2)
        .flat_map(|lower| (0_i64..=3).map(move |length| (lower, lower + length - 1)))
        .collect();
    let mut cases = 0_u32;
    for abi in ABIS {
        for elem in Elem::ALL {
            for &first in &axis {
                check_routes(elem, abi, &[first]);
                cases += 1;
                for &second in &axis {
                    check_routes(elem, abi, &[first, second]);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 2 * 6 * (20 + 20 * 20));
}
