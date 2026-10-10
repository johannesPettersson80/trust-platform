#[cfg(test)]
use super::frames::FrameStack;
#[cfg(test)]
use crate::value::{
    materialize_value_path, read_value_path_borrowed, ref_indices_from_iter, single_ref_index,
    write_value_path, RefPath,
};
#[cfg(test)]
use crate::value::{RefSegment, Value, ValueRef};
pub(super) use trust_runtime_core::vm::hosted::dispatch_refs::*;

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    use crate::memory::{FrameId, MemoryLocation};
    use crate::runtime::vm::frames::VmFrame;
    use crate::value::{ArrayValue, StructValue};
    use crate::Runtime;
    use smol_str::SmolStr;
    use trust_runtime_core::vm::hosted::context::VM_LOCAL_SENTINEL_FRAME_ID;

    #[test]
    fn peek_dynamic_ref_borrows_global_storage_value() {
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global(
            "CELL",
            Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                SmolStr::new("CELL_T"),
                IndexMap::from([(SmolStr::new("ACC"), Value::DInt(7))]),
            ))),
        );
        let reference = runtime
            .storage()
            .ref_for_global("CELL")
            .expect("global ref");
        let frames = FrameStack::default();

        let peeked = peek_dynamic_ref(&runtime, &frames, &reference).expect("peek global");
        let direct = runtime
            .storage()
            .read_by_ref_ref(&reference)
            .expect("direct global read");

        assert!(std::ptr::eq(peeked, direct));
    }

    #[test]
    fn peek_dynamic_ref_borrows_local_sentinel_value() {
        let mut frames = FrameStack::default();
        frames
            .push(VmFrame {
                parameter_values_present: Vec::new(),
                activation: None,
                pou_id: Some(0),
                return_pc: 0,
                code_start: 0,
                code_end: 0,
                local_ref_start: 0,
                local_ref_count: 1,
                locals: vec![Value::Struct(std::sync::Arc::new(
                    StructValue::from_untyped_parts(
                        SmolStr::new("LOCAL_T"),
                        IndexMap::from([(SmolStr::new("ACC"), Value::DInt(11))]),
                    ),
                ))],
                runtime_instance: None,
                instance_owner: None,
            })
            .expect("push frame");
        let runtime = Runtime::new();
        let reference = ValueRef {
            location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
            offset: 0,
            path: [RefSegment::Field(SmolStr::new("ACC"))]
                .into_iter()
                .collect(),
        };

        let peeked = peek_dynamic_ref(&runtime, &frames, &reference).expect("peek local");
        let frame = frames.current().expect("current frame");
        let direct =
            read_value_path_borrowed(frame.locals.first().expect("local slot"), &reference.path)
                .expect("direct local read");

        assert!(std::ptr::eq(peeked, direct));
    }

    #[test]
    fn dynamic_ref_field_resolves_instance_field_reference() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage_mut().create_instance("FB");
        assert!(runtime
            .storage_mut()
            .set_instance_var(instance, "ACC", Value::DInt(19)));
        runtime
            .storage_mut()
            .set_global("HOLDER", Value::Instance(instance));
        let holder = runtime
            .storage()
            .ref_for_global("HOLDER")
            .expect("holder ref");
        let frames = FrameStack::default();

        let resolved = dynamic_ref_field(&runtime, &frames, holder, SmolStr::new("ACC"))
            .expect("resolve instance field");
        let expected = runtime
            .storage()
            .ref_for_instance_recursive(instance, "ACC")
            .expect("expected ref");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn dynamic_ref_index_extends_partial_index_against_array_shape() {
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global(
            "GRID",
            Value::Array(Box::new(
                ArrayValue::from_untyped_parts(
                    vec![
                        Value::DInt(1),
                        Value::DInt(2),
                        Value::DInt(3),
                        Value::DInt(4),
                    ],
                    vec![(0, 1), (0, 1)],
                )
                .unwrap(),
            )),
        );
        let mut reference = runtime.storage().ref_for_global("GRID").expect("grid ref");
        reference.path.push(single_ref_index(0));
        let frames = FrameStack::default();

        let resolved =
            dynamic_ref_index(&runtime, &frames, reference, 1).expect("extend partial index");

        assert_eq!(
            resolved.path,
            [RefSegment::Index(ref_indices_from_iter([0, 1]))]
                .into_iter()
                .collect::<RefPath>()
        );
    }

    #[test]
    fn dynamic_ref_index_extends_nested_partial_index_against_array_shape() {
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global(
            "HOLDER",
            Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                SmolStr::new("GRID_HOLDER"),
                IndexMap::from([(
                    SmolStr::new("GRID"),
                    Value::Array(Box::new(
                        ArrayValue::from_untyped_parts(
                            vec![
                                Value::DInt(1),
                                Value::DInt(2),
                                Value::DInt(3),
                                Value::DInt(4),
                            ],
                            vec![(0, 1), (0, 1)],
                        )
                        .unwrap(),
                    )),
                )]),
            ))),
        );
        let mut reference = runtime
            .storage()
            .ref_for_global("HOLDER")
            .expect("holder ref");
        reference.path.push(RefSegment::Field(SmolStr::new("GRID")));
        reference.path.push(single_ref_index(0));
        let frames = FrameStack::default();

        let resolved = dynamic_ref_index(&runtime, &frames, reference, 1)
            .expect("extend nested partial index");

        assert_eq!(
            resolved.path,
            [
                RefSegment::Field(SmolStr::new("GRID")),
                RefSegment::Index(ref_indices_from_iter([0, 1])),
            ]
            .into_iter()
            .collect::<RefPath>()
        );
    }

    #[test]
    fn read_and_write_value_path_handle_extreme_array_bounds_without_overflow() {
        let mut value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::DInt(7)],
            vec![(i64::MIN, i64::MAX)],
        )));
        let path = [RefSegment::Index(ref_indices_from_iter([i64::MIN]))];

        let read = read_value_path_borrowed(&value, &path).expect("read extreme lower bound");
        assert_eq!(read, &Value::DInt(7));

        assert!(write_value_path(&mut value, &path, Value::DInt(9)));
        let updated =
            read_value_path_borrowed(&value, &path).expect("read updated extreme lower bound");
        assert_eq!(updated, &Value::DInt(9));
    }

    #[test]
    fn read_and_write_value_path_non_ascii_string_uses_character_elements() {
        let mut value = Value::String("ÄBC".into());
        let path = [RefSegment::Index(ref_indices_from_iter([1]))];

        let read = materialize_value_path(&value, &path).expect("read non-ascii string element");
        assert_eq!(read, Value::Char(0xC4));

        assert!(write_value_path(
            &mut value,
            &[RefSegment::Index(ref_indices_from_iter([2]))],
            Value::Char(b'X')
        ));
        assert_eq!(value, Value::String("ÄXC".into()));
    }
}
