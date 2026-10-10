pub use trust_runtime_core::value::{
    array_offset_i64, checked_array_offset_i64, parse_partial_access, ref_indices_from_iter,
    single_ref_index, PartialAccess, PartialAccessError, RefIndices, RefPath, RefSegment, ValueRef,
    ValueRefView,
};
pub(crate) use trust_runtime_core::value::{materialize_value_path, write_value_path_typed};
#[cfg(test)]
pub(crate) use trust_runtime_core::value::{read_value_path_borrowed, write_value_path};
#[cfg(test)]
use trust_runtime_core::value::{single_string_index, Value};

#[cfg(test)]
#[path = "reference/contract_tests.rs"]
mod contract_tests;

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
