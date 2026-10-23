//! Translate seeded process-image contents into ordered configuration actions.
use super::{invalid, RuntimeBuilder};
use crate::{
    harness::{AccessPath, CompileError, ConfigInit},
    program_model::Expr,
    value::Value,
};
use trust_hir::TypeId;

pub(super) fn image_initializers(
    builder: &mut RuntimeBuilder,
) -> Result<Vec<ConfigInit>, CompileError> {
    use crate::{
        io::{IoAddress, IoSize},
        memory::IoArea,
    };
    let mut result = Vec::new();
    for (area, bytes) in [
        (IoArea::Input, builder.io.inputs()),
        (IoArea::Output, builder.io.outputs()),
        (IoArea::Memory, builder.io.memory()),
    ] {
        // Zeroes already belong to new images. A final byte records the complete
        // explicit extent even when the unused tail contains only zeroes.
        for (offset, value) in bytes.iter().enumerate() {
            if *value == 0 && offset + 1 != bytes.len() {
                continue;
            }
            let byte = u32::try_from(offset).map_err(|_| invalid("image extent overflow"))?;
            let address = IoAddress {
                area,
                size: IoSize::Byte,
                byte,
                bit: 0,
                path: vec![byte],
                wildcard: false,
            };
            result.push(ConfigInit {
                display_name: None,
                path: AccessPath::Direct {
                    text: crate::bytecode::format_authoring_io_address(&address),
                    address,
                },
                address: None,
                type_id: TypeId::BYTE,
                initializer: Some(Expr::Literal(Value::Byte(*value))),
            });
        }
    }
    let mut hierarchy = builder
        .io
        .hierarchical_seeds()
        .map(|result| result.map(|(address, value)| (address, value.clone())))
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    hierarchy.sort_by_key(|(address, _)| crate::bytecode::format_authoring_io_address(address));
    for (address, value) in hierarchy {
        if !builder
            .io
            .bindings()
            .iter()
            .any(|binding| binding.address == address)
        {
            return Err(invalid(
                "hierarchical image seed has no declared I/O binding",
            ));
        }
        let ty = match (&value, address.size) {
            (Value::String(_), IoSize::Bytes(length)) => {
                builder.registry.register_string_with_length(length)
            }
            (Value::Enum(value), _) => builder
                .registry
                .lookup(value.type_name())
                .ok_or_else(|| invalid("enum I/O seed has no declaration"))?,
            _ => crate::bytecode::authoring_value_type(&value)
                .ok_or_else(|| invalid("unsupported I/O seed value"))?,
        };
        result.push(ConfigInit {
            display_name: None,
            path: AccessPath::Direct {
                text: crate::bytecode::format_authoring_io_address(&address),
                address,
            },
            address: None,
            type_id: ty,
            initializer: Some(Expr::Literal(value)),
        });
    }
    Ok(result)
}
