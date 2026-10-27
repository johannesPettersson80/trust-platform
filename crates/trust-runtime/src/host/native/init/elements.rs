//! The initial values of the elements of an array of function block or class instances (IEC
//! 61131-3 Ed.3 Table 11 features 5a and 5b; specification 02 §3.4): the array's initial value
//! lists one aggregate initializer per element, `[(PT := T#100ms), (PT := T#300ms)]`, in row-major
//! order with the rightmost subscript varying fastest; `n(...)` repeats what its parentheses hold
//! `n` times. An element past the list keeps the initial values its type declares, and an entry
//! past the last element is ignored. A named array type may declare the initial value
//! (`TYPE TONS : ARRAY[1..50] OF TON := [50(PT := T#100ms)]`), which every variable of the type
//! declared without one of its own takes.

use trust_hir::{Type, TypeId};

use super::super::NativeError;
use crate::program_model::{ArgValue, Expr, UnaryOp};
use crate::value::Value;

/// The most aliases between a named type and the array it names, which a malformed registry
/// cannot make endless.
const MAX_ALIASES: usize = 64;

/// The initial value of each of the `count` elements, in row-major order, that `entries`, the
/// array's initial value, gives them: `None` for an element past the list, which keeps the
/// initial values its type declares. Entries past the last element are ignored.
///
/// A repetition `n(...)` gives `n` times what its parentheses hold: the values `n((PT := T#1s))`
/// lists (the Annex A grammar), or the one aggregate initializer `n(PT := T#1s)` writes (Table 11
/// feature 5b; `docs/IEC_DECISIONS.md`, 2026-09-28). A repetition count of zero gives nothing.
///
/// # Errors
///
/// [`NativeError::Compile`] for a negative repetition count of `name`'s initial value, and
/// [`NativeError::Unsupported`] for a repetition that mixes formal and positional values.
pub(super) fn element_initializers(
    entries: &[Expr],
    count: usize,
    name: &str,
) -> Result<Vec<Option<Expr>>, NativeError> {
    let mut elements: Vec<Option<Expr>> = Vec::with_capacity(count);
    for entry in entries {
        if elements.len() == count {
            break;
        }
        match repetition(entry, name)? {
            Some((times, values)) if !values.is_empty() => {
                for value in values
                    .iter()
                    .cycle()
                    .take(times.saturating_mul(values.len()))
                {
                    if elements.len() == count {
                        break;
                    }
                    elements.push(Some(value.clone()));
                }
            }
            Some(_) => {}
            None => elements.push(Some(entry.clone())),
        }
    }
    elements.resize(count, None);
    Ok(elements)
}

/// The count and the repeated values of `entry` when it is a repetition `n(...)`, `None` for any
/// other entry.
fn repetition(entry: &Expr, name: &str) -> Result<Option<(usize, Vec<Expr>)>, NativeError> {
    let Expr::Call { target, args } = entry else {
        return Ok(None);
    };
    let Some(times) = repetition_count(target) else {
        return Ok(None);
    };
    let times = usize::try_from(times).map_err(|_| {
        NativeError::Compile(format!(
            "the repetition count {times} in the initial value of {name} is negative"
        ))
    })?;
    let mut formal = Vec::new();
    let mut positional = Vec::new();
    for arg in args {
        let ArgValue::Expr(value) = &arg.value else {
            return Err(NativeError::unsupported(format!(
                "a repetition of a variable in the initial value of {name}"
            )));
        };
        match &arg.name {
            Some(member) => formal.push((member.clone(), value.clone())),
            None => positional.push(value.clone()),
        }
    }
    match (formal.is_empty(), positional.is_empty()) {
        // `n(PT := T#100ms)` repeats the one aggregate initializer its parentheses hold.
        (false, true) => Ok(Some((times, vec![Expr::StructInitializer(formal)]))),
        (true, _) => Ok(Some((times, positional))),
        (false, false) => Err(NativeError::unsupported(format!(
            "a repetition of formal and positional values in the initial value of {name}"
        ))),
    }
}

/// The count of a repetition `n(...)`: an integer literal, with its sign; `None` for a target
/// that is no integer literal.
fn repetition_count(target: &Expr) -> Option<i128> {
    match target {
        Expr::Literal(value) => integer(value),
        Expr::Unary { op, expr } => match (op, expr.as_ref()) {
            (UnaryOp::Pos, Expr::Literal(value)) => integer(value),
            (UnaryOp::Neg, Expr::Literal(value)) => integer(value).map(|count| -count),
            _ => None,
        },
        _ => None,
    }
}

fn integer(value: &Value) -> Option<i128> {
    Some(match value {
        Value::SInt(value) => i128::from(*value),
        Value::Int(value) => i128::from(*value),
        Value::DInt(value) => i128::from(*value),
        Value::LInt(value) => i128::from(*value),
        Value::USInt(value) => i128::from(*value),
        Value::UInt(value) => i128::from(*value),
        Value::UDInt(value) => i128::from(*value),
        Value::ULInt(value) => i128::from(*value),
        _ => return None,
    })
}

/// The subscripts of the element at row-major position `position` of an array of `dims`, as an
/// IEC subscript list: `[2]`, `[1, 3]`.
pub(super) fn subscripts(dims: &[(i64, i64)], position: usize) -> String {
    let mut rest = position;
    let mut indices = vec![0i64; dims.len()];
    for (at, (lower, upper)) in dims.iter().enumerate().rev() {
        let length = usize::try_from(upper - lower + 1).unwrap_or(1).max(1);
        indices[at] = lower.saturating_add(i64::try_from(rest % length).unwrap_or(0));
        rest /= length;
    }
    let listed: Vec<String> = indices.iter().map(ToString::to_string).collect();
    format!("[{}]", listed.join(", "))
}

/// The element type of the array type `type_id` of trust-hir's registry, through its aliases.
pub(super) fn element_type(
    registry: &trust_hir::types::TypeRegistry,
    type_id: TypeId,
) -> Option<TypeId> {
    let mut current = type_id;
    for _ in 0..MAX_ALIASES {
        match registry.get(current)? {
            Type::Alias { target, .. } => current = *target,
            Type::Array { element, .. } => return Some(*element),
            _ => return None,
        }
    }
    None
}

/// The initial value the named type `type_id`, or a type it is an alias of, declares
/// (`TYPE TONS : ARRAY[1..50] OF TON := [50(PT := T#100ms)]`); `None` without one.
pub(super) fn type_default(builder: &crate::RuntimeBuilder, type_id: TypeId) -> Option<Expr> {
    let registry = builder.registry();
    let catalog = builder.initializer_catalog();
    let mut current = type_id;
    for _ in 0..MAX_ALIASES {
        if let Some(id) = catalog.type_default(current) {
            return catalog.initializer(id).cloned();
        }
        let Type::Alias { target, .. } = registry.get(current)? else {
            return None;
        };
        current = *target;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_model::CallArg;
    use crate::value::Duration;

    /// `(PT := T#<ms>ms)`, the aggregate initializer of one `TON`.
    fn preset(ms: i64) -> Expr {
        Expr::StructInitializer(vec![(
            "PT".into(),
            Expr::Literal(Value::Time(Duration::from_millis(ms))),
        )])
    }

    /// `n(values)` with positional values, as the Annex A grammar writes a repetition.
    fn repeated(count: i32, values: Vec<Expr>) -> Expr {
        Expr::Call {
            target: Box::new(Expr::Literal(Value::DInt(count))),
            args: values
                .into_iter()
                .map(|value| CallArg {
                    name: None,
                    value: ArgValue::Expr(value),
                })
                .collect(),
        }
    }

    /// The presets of each element, `None` for an element that keeps its type's declarations.
    fn presets(elements: &[Option<Expr>]) -> Vec<Option<i64>> {
        elements
            .iter()
            .map(|element| {
                element.as_ref().map(|expr| match expr {
                    Expr::StructInitializer(fields) => match &fields[0].1 {
                        Expr::Literal(Value::Time(time)) => time.as_millis(),
                        other => panic!("a preset, not {other:?}"),
                    },
                    other => panic!("an aggregate initializer, not {other:?}"),
                })
            })
            .collect()
    }

    /// Specification 02 §3.4: values fill the array in order and missing rightmost values take
    /// the element type's own: `[(PT := T#100ms), (PT := T#300ms)]` for three `TON`s gives the
    /// first two their presets and the third none. Catches a list that is not taken in order and
    /// a missing element that takes another element's value.
    #[test]
    fn listed_initializers_fill_the_elements_in_order() {
        let elements =
            element_initializers(&[preset(100), preset(300)], 3, "Listed").expect("expanded");
        assert_eq!(presets(&elements), [Some(100), Some(300), None]);
    }

    /// Specification 02 §3.4: a repetition `2((PT := T#50ms))` gives its value twice, then the next
    /// entry follows: 50, 50, 70. Catches a repetition taken once, and one whose count is taken as
    /// an element.
    #[test]
    fn a_repetition_gives_its_value_count_times() {
        let entries = [repeated(2, vec![preset(50)]), preset(70)];
        let elements = element_initializers(&entries, 3, "Repeated").expect("expanded");
        assert_eq!(presets(&elements), [Some(50), Some(50), Some(70)]);
    }

    /// Specification 02 §3.4: excess rightmost values are ignored: three presets for two elements
    /// give 1 and 2, and a repetition past the end stops at the last element. Catches an index
    /// past the array.
    #[test]
    fn entries_past_the_last_element_are_ignored() {
        let elements = element_initializers(&[preset(1), preset(2), preset(3)], 2, "Listed")
            .expect("expanded");
        assert_eq!(presets(&elements), [Some(1), Some(2)]);
        let elements =
            element_initializers(&[repeated(50, vec![preset(9)])], 3, "Many").expect("expanded");
        assert_eq!(presets(&elements), [Some(9), Some(9), Some(9)]);
    }

    /// IEC Table 11 feature 5b writes `[50(PT := T#100ms)]`: the formal values of a repetition are
    /// one aggregate initializer, repeated (`docs/IEC_DECISIONS.md`, 2026-09-28). Catches each
    /// formal value taken as an element of its own.
    #[test]
    fn a_repetition_of_formal_values_repeats_one_aggregate_initializer() {
        let entry = Expr::Call {
            target: Box::new(Expr::Literal(Value::DInt(3))),
            args: vec![CallArg {
                name: Some("PT".into()),
                value: ArgValue::Expr(Expr::Literal(Value::Time(Duration::from_millis(100)))),
            }],
        };
        let elements = element_initializers(&[entry], 3, "Timers").expect("expanded");
        assert_eq!(presets(&elements), [Some(100), Some(100), Some(100)]);
    }

    /// Specification 02 §3.4: a repetition count of zero contributes no values, and a negative one
    /// is an error. Catches a zero count that consumes an element and a negative count read as a
    /// large one.
    #[test]
    fn a_zero_count_gives_nothing_and_a_negative_count_is_refused() {
        let entries = [repeated(0, vec![preset(5)]), preset(6)];
        let elements = element_initializers(&entries, 2, "Some").expect("expanded");
        assert_eq!(presets(&elements), [Some(6), None]);
        let negative = Expr::Call {
            target: Box::new(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(Expr::Literal(Value::DInt(2))),
            }),
            args: vec![CallArg {
                name: None,
                value: ArgValue::Expr(preset(1)),
            }],
        };
        let error = element_initializers(&[negative], 2, "Bad").expect_err("a negative count");
        assert!(error.to_string().contains("negative"), "{error}");
    }

    /// Row-major order, the rightmost subscript fastest (specification 02 §3.4): position 4 of
    /// `ARRAY[1..2, 0..2]` is `[2, 1]`, position 0 of `ARRAY[5..6]` is `[5]`.
    #[test]
    fn positions_are_named_by_their_subscripts_in_row_major_order() {
        assert_eq!(subscripts(&[(1, 2), (0, 2)], 4), "[2, 1]");
        assert_eq!(subscripts(&[(5, 6)], 0), "[5]");
    }
}
