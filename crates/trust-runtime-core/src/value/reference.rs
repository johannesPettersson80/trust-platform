use alloc::{string::String, vec, vec::Vec};

use smol_str::SmolStr;

use crate::{error::RuntimeError, memory::MemoryLocation};

/// Reference path segment within composite values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RefSegment {
    /// Array/string index path segment.
    Index(RefIndices),
    /// Struct/union field path segment.
    Field(SmolStr),
}

/// Reference path indices for one array/string segment.
pub type RefIndices = Vec<i64>;
/// Reference path within a composite value.
pub type RefPath = Vec<RefSegment>;

/// Reference to a value in memory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValueRef {
    /// Memory area that owns the referenced root value.
    pub location: MemoryLocation,
    /// Slot offset within the owning memory area.
    pub offset: usize,
    /// Nested path from the root value to the referenced sub-value.
    pub path: RefPath,
}

pub fn ref_indices_from_iter<I>(indices: I) -> RefIndices
where
    I: IntoIterator<Item = i64>,
{
    indices.into_iter().collect()
}

#[inline]
#[must_use]
pub fn single_ref_index(index: i64) -> RefSegment {
    RefSegment::Index(vec![index])
}

#[inline]
#[must_use]
pub fn array_offset_i64(dimensions: &[(i64, i64)], indices: &[i64]) -> Option<usize> {
    if dimensions.len() != indices.len() {
        return None;
    }
    let mut offset: i128 = 0;
    let mut stride: i128 = 1;
    for ((lower, upper), index) in dimensions.iter().zip(indices).rev() {
        if index < lower || index > upper {
            return None;
        }
        let lower = i128::from(*lower);
        let upper = i128::from(*upper);
        let index = i128::from(*index);
        let len = upper.checked_sub(lower)?.checked_add(1)?;
        let relative = index.checked_sub(lower)?;
        offset = offset.checked_add(relative.checked_mul(stride)?)?;
        stride = stride.checked_mul(len)?;
    }
    usize::try_from(offset).ok()
}

#[inline]
pub fn checked_array_offset_i64(
    dimensions: &[(i64, i64)],
    indices: &[i64],
) -> Result<usize, RuntimeError> {
    if dimensions.len() != indices.len() {
        return Err(RuntimeError::TypeMismatch);
    }
    for ((lower, upper), index) in dimensions.iter().zip(indices) {
        if index < lower || index > upper {
            return Err(RuntimeError::IndexOutOfBounds {
                index: *index,
                lower: *lower,
                upper: *upper,
            });
        }
    }
    array_offset_i64(dimensions, indices).ok_or(RuntimeError::TypeMismatch)
}

/// Parsed IEC partial access suffix such as `%X0`, `%B1`, `%W0`, or `%D0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialAccess {
    /// Bit access.
    Bit(u8),
    /// Byte access.
    Byte(u8),
    /// Word access.
    Word(u8),
    /// Double-word access.
    DWord(u8),
}

/// Error produced by reading or writing a partial access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialAccessError {
    /// Partial access index outside the valid width of the target value.
    IndexOutOfBounds { index: i64, lower: i64, upper: i64 },
    /// Partial access is not valid for the target value type.
    TypeMismatch,
}

#[must_use]
pub fn parse_partial_access(text: &str) -> Option<PartialAccess> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(stripped) = text.strip_prefix('%') {
        let mut chars = stripped.chars();
        let prefix = chars.next()?;
        let digits: String = chars.collect();
        let index = parse_access_index(&digits)?;
        return match prefix.to_ascii_uppercase() {
            'X' => Some(PartialAccess::Bit(index)),
            'B' => Some(PartialAccess::Byte(index)),
            'W' => Some(PartialAccess::Word(index)),
            'D' => Some(PartialAccess::DWord(index)),
            _ => None,
        };
    }
    if text.chars().all(|c| c.is_ascii_digit() || c == '_') {
        let index = parse_access_index(text)?;
        return Some(PartialAccess::Bit(index));
    }
    None
}

fn parse_access_index(text: &str) -> Option<u8> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    let value: u64 = cleaned.parse().ok()?;
    u8::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        array_offset_i64, checked_array_offset_i64, parse_partial_access, ref_indices_from_iter,
        single_ref_index, PartialAccess, RefPath, RefSegment,
    };
    use crate::error::RuntimeError;
    use alloc::vec;

    #[test]
    fn array_offset_handles_extreme_bounds_without_overflow() {
        assert_eq!(
            array_offset_i64(&[(i64::MIN, i64::MAX)], &[i64::MIN]),
            Some(0)
        );
        assert_eq!(
            array_offset_i64(&[(i64::MIN, i64::MAX)], &[i64::MAX]),
            Some(usize::MAX)
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
            RefSegment::Field("items".into()),
            single_ref_index(1),
            single_ref_index(2),
            RefSegment::Field("value".into()),
        ];

        assert_eq!(ref_indices_from_iter([1, 2, 3]), vec![1, 2, 3]);
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn partial_access_parser_accepts_iec_suffixes_and_bare_bits() {
        assert_eq!(parse_partial_access("%X1"), Some(PartialAccess::Bit(1)));
        assert_eq!(parse_partial_access("%B2"), Some(PartialAccess::Byte(2)));
        assert_eq!(parse_partial_access("%W3"), Some(PartialAccess::Word(3)));
        assert_eq!(parse_partial_access("%D4"), Some(PartialAccess::DWord(4)));
        assert_eq!(parse_partial_access("1_0"), Some(PartialAccess::Bit(10)));
        assert_eq!(parse_partial_access("%Q1"), None);
    }
}
