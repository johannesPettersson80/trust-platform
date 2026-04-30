use super::{normalize_assignment_for_target, read_string_element, write_string_element, Value};

pub use trust_runtime_core::value::{
    array_offset_i64, checked_array_offset_i64, parse_partial_access, ref_indices_from_iter,
    single_ref_index, PartialAccess, PartialAccessError, RefIndices, RefPath, RefSegment, ValueRef,
};

#[inline]
pub(crate) fn read_value_path_borrowed<'a>(
    value: &'a Value,
    path: &[RefSegment],
) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    match &path[0] {
        RefSegment::Field(name) => match value {
            Value::Struct(struct_value) => struct_value
                .field(name.as_str())
                .and_then(|field| read_value_path_borrowed(field, &path[1..])),
            _ => None,
        },
        RefSegment::Index(indices) => match value {
            Value::Array(array) => {
                let offset = array_offset_i64(array.dimensions(), indices)?;
                array
                    .elements()
                    .get(offset)
                    .and_then(|element| read_value_path_borrowed(element, &path[1..]))
            }
            _ => None,
        },
    }
}

#[inline]
pub(crate) fn materialize_value_path(value: &Value, path: &[RefSegment]) -> Option<Value> {
    if path.is_empty() {
        return Some(value.clone());
    }

    match &path[0] {
        RefSegment::Field(name) => match value {
            Value::Struct(struct_value) => struct_value
                .field(name.as_str())
                .and_then(|field| materialize_value_path(field, &path[1..])),
            _ => None,
        },
        RefSegment::Index(indices) => match value {
            Value::Array(array) => {
                let offset = array_offset_i64(array.dimensions(), indices)?;
                array
                    .elements()
                    .get(offset)
                    .and_then(|element| materialize_value_path(element, &path[1..]))
            }
            Value::String(text) => {
                if !path[1..].is_empty() {
                    return None;
                }
                let index = single_string_index(indices)?;
                read_string_element(text.as_str(), index, false).ok()
            }
            Value::WString(text) => {
                if !path[1..].is_empty() {
                    return None;
                }
                let index = single_string_index(indices)?;
                read_string_element(text.as_str(), index, true).ok()
            }
            _ => None,
        },
    }
}

#[inline]
pub(crate) fn write_value_path(target: &mut Value, path: &[RefSegment], value: Value) -> bool {
    if path.is_empty() {
        *target = normalize_assignment_for_target(target, value);
        return true;
    }

    match &path[0] {
        RefSegment::Field(name) => match target {
            Value::Struct(struct_value) => std::sync::Arc::make_mut(struct_value)
                .field_mut(name.as_str())
                .map(|field| write_value_path(field, &path[1..], value))
                .unwrap_or(false),
            _ => false,
        },
        RefSegment::Index(indices) => match target {
            Value::Array(array) => {
                let offset = match array_offset_i64(array.dimensions(), indices) {
                    Some(offset) => offset,
                    None => return false,
                };
                array
                    .elements_mut()
                    .get_mut(offset)
                    .map(|element| write_value_path(element, &path[1..], value))
                    .unwrap_or(false)
            }
            Value::String(text) => write_string_path(text, indices, value, false)
                .map(|updated| {
                    *target = Value::String(updated.into());
                    true
                })
                .unwrap_or(false),
            Value::WString(text) => write_string_path(text, indices, value, true)
                .map(|updated| {
                    *target = Value::WString(updated);
                    true
                })
                .unwrap_or(false),
            _ => false,
        },
    }
}

#[inline]
fn single_string_index(indices: &[i64]) -> Option<i64> {
    if indices.len() != 1 {
        return None;
    }
    Some(indices[0])
}

#[inline]
fn write_string_path(text: &str, indices: &[i64], value: Value, wide: bool) -> Option<String> {
    let index = single_string_index(indices)?;
    write_string_element(text, index, value, wide).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        array_offset_i64, checked_array_offset_i64, single_ref_index, RefPath, RefSegment,
    };
    use crate::error::RuntimeError;

    #[test]
    fn array_offset_handles_extreme_bounds_without_overflow() {
        assert_eq!(
            array_offset_i64(&[(i64::MIN, i64::MAX)], &[i64::MIN]),
            Some(0)
        );
    }

    #[test]
    fn checked_array_offset_preserves_bounds_error() {
        assert_eq!(
            checked_array_offset_i64(&[(0, 1)], &[2]),
            Err(RuntimeError::IndexOutOfBounds {
                index: 2,
                lower: 0,
                upper: 1,
            })
        );
    }

    #[test]
    fn common_ref_path_helpers_preserve_segment_order() {
        let path: RefPath = vec![
            RefSegment::Field("root".into()),
            single_ref_index(1),
            RefSegment::Field("leaf".into()),
            single_ref_index(2),
        ];
        assert_eq!(path.len(), 4);
        assert!(matches!(path[0], RefSegment::Field(_)));
        assert!(matches!(path[1], RefSegment::Index(_)));
        assert!(matches!(path[2], RefSegment::Field(_)));
        assert!(matches!(path[3], RefSegment::Index(_)));
    }
}
