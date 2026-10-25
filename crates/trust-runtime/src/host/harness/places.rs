//! The places the test harness reads and writes by name (specification 10 §11.1.1): a variable of
//! the compiled program by the names of its symbols (specification 44 §5.7), then the fields and
//! array elements within that variable's value.

use smol_str::SmolStr;
use trust_runtime_core::native::variables::NameError;

use crate::boundary::BoundaryError;
use crate::error::RuntimeError;
use crate::value::{
    read_value_path_borrowed, ref_indices_from_iter, write_value_path_checked, RefSegment, Value,
};
use crate::CompiledRuntime;

/// A place of the program: the variable its leading names name, and the path into that variable's
/// value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Place {
    pub(crate) variable: SmolStr,
    pub(crate) path: Vec<RefSegment>,
}

/// One step of a place's text.
enum Step {
    Name(SmolStr),
    Index(Vec<i64>),
}

/// The steps of `text`: names joined by `.`, each followed by any number of `[i, j, ...]` index
/// lists of integers.
fn steps(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let mut rest = text.trim();
    loop {
        let end = rest.find(['.', '[']).unwrap_or(rest.len());
        let name = rest[..end].trim();
        if name.is_empty() {
            return Err("a name is missing".to_string());
        }
        steps.push(Step::Name(SmolStr::new(name)));
        rest = &rest[end..];
        while let Some(inner) = rest.strip_prefix('[') {
            let close = inner
                .find(']')
                .ok_or_else(|| "an index list is not closed".to_string())?;
            let indices = inner[..close]
                .split(',')
                .map(|index| index.trim().parse::<i64>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "an index is not an integer".to_string())?;
            steps.push(Step::Index(indices));
            rest = inner[close + 1..].trim_start();
        }
        match rest.strip_prefix('.') {
            Some(next) => rest = next,
            None if rest.is_empty() => return Ok(steps),
            None => return Err(format!("unexpected text '{rest}'")),
        }
    }
}

/// The place `text` names in `program`: the longest run of its leading names that names one
/// variable, then the fields and elements within that variable's value.
///
/// # Errors
///
/// [`BoundaryError::UnsupportedPathSyntax`] for text that is no place, [`BoundaryError::AmbiguousName`]
/// for leading names that name a variable of several programs, with the qualified candidates, and
/// [`BoundaryError::UnresolvedName`] for names that name no variable.
pub(crate) fn resolve(program: &CompiledRuntime, text: &str) -> Result<Place, BoundaryError> {
    let steps = steps(text).map_err(|reason| BoundaryError::UnsupportedPathSyntax {
        path: text.into(),
        reason,
    })?;
    let leading = steps
        .iter()
        .take_while(|step| matches!(step, Step::Name(_)))
        .count();
    for length in (1..=leading).rev() {
        let variable = steps[..length]
            .iter()
            .filter_map(|step| match step {
                Step::Name(name) => Some(name.as_str()),
                Step::Index(_) => None,
            })
            .collect::<Vec<_>>()
            .join(".");
        match program.find_variable(&variable) {
            Ok(_) => {
                let path = steps[length..]
                    .iter()
                    .map(|step| match step {
                        Step::Name(name) => RefSegment::Field(name.clone()),
                        Step::Index(indices) => {
                            RefSegment::Index(ref_indices_from_iter(indices.iter().copied()))
                        }
                    })
                    .collect();
                return Ok(Place {
                    variable: SmolStr::new(variable),
                    path,
                });
            }
            Err(NameError::Ambiguous) => {
                return Err(BoundaryError::AmbiguousName {
                    path: text.into(),
                    candidates: candidates(program, &variable),
                });
            }
            Err(NameError::Unknown) => {}
        }
    }
    Err(BoundaryError::UnresolvedName { path: text.into() })
}

/// The variables of the programs that each declare `name`, qualified by the program.
fn candidates(program: &CompiledRuntime, name: &str) -> Vec<SmolStr> {
    let Ok(state) = program.snapshot() else {
        return Vec::new();
    };
    state
        .programs()
        .into_iter()
        .map(|instance| SmolStr::new(format!("{instance}.{name}")))
        .filter(|qualified| program.find_variable(qualified).is_ok())
        .collect()
}

/// The value at `place`, as the last scan left it.
///
/// # Errors
///
/// [`RuntimeError::UndefinedField`] for a field the value does not have, and
/// [`RuntimeError::IndexOutOfBounds`] for an element it does not hold.
pub(crate) fn read(program: &CompiledRuntime, place: &Place) -> Result<Value, RuntimeError> {
    let value = program.read_variable(&place.variable)?;
    within(&value, &place.path).cloned()
}

/// Writes `value` at `place`: the whole variable, or the field or element within its value, which
/// the value takes the form of.
///
/// # Errors
///
/// As [`read`], and the refusal of a value the place does not take.
pub(crate) fn write(
    program: &mut CompiledRuntime,
    place: &Place,
    value: Value,
) -> Result<(), RuntimeError> {
    if place.path.is_empty() {
        return program.write_variable(&place.variable, &value);
    }
    let mut whole = program.read_variable(&place.variable)?;
    within(&whole, &place.path)?;
    if !write_value_path_checked(&mut whole, &place.path, value)? {
        return Err(RuntimeError::TypeMismatch);
    }
    program.write_variable(&place.variable, &whole)
}

/// The value at `path` within `value`, with the error of the first step that names nothing: a
/// field the value does not have, an index outside its array's bounds, or a step into a value that
/// has no fields or elements.
fn within<'a>(value: &'a Value, path: &[RefSegment]) -> Result<&'a Value, RuntimeError> {
    let mut current = value;
    for (step, segment) in path.iter().enumerate() {
        if let Some(next) = read_value_path_borrowed(value, &path[..=step]) {
            current = next;
            continue;
        }
        return Err(match (segment, current) {
            (RefSegment::Field(name), Value::Struct(_)) => {
                RuntimeError::UndefinedField(name.clone())
            }
            (RefSegment::Index(indices), Value::Array(array)) => indices
                .iter()
                .zip(array.dimensions())
                .find(|(index, (lower, upper))| **index < *lower || **index > *upper)
                .map_or(RuntimeError::TypeMismatch, |(index, (lower, upper))| {
                    RuntimeError::IndexOutOfBounds {
                        index: *index,
                        lower: *lower,
                        upper: *upper,
                    }
                }),
            _ => RuntimeError::TypeMismatch,
        });
    }
    Ok(current)
}
