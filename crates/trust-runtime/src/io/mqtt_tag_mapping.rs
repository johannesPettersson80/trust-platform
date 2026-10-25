//! The startup adapter from a compiled program's symbols to the typed points of the `mqtt` driver
//! (specification 32 §5): it lowers each symbolic mapping to an input or an output point before the
//! driver starts, so that the driver, whose logic is the shared exchange of every runtime, takes
//! typed points only and never sees the program's storage. The mappings themselves are read by the
//! driver's shared rules (`tag`, `topic` and `direction`, specification 32 §5); this module holds
//! what needs the program: resolving each tag, choosing its process-image address, and the links
//! that copy an unbound variable at the scan's boundaries.

use smol_str::SmolStr;
use toml::Value as TomlValue;
use trust_runtime_core::io_image::IoValueCodec;
use trust_runtime_core::native::image::type_kind;
use trust_runtime_core::native::memory::TypeShape;
use trust_runtime_services::io_drivers::mqtt::{Mapping, MappingDirection};

use crate::error::RuntimeError;
use crate::memory::IoArea;
use crate::CompiledRuntime;

use super::{IoAddress, IoSize, MqttIoDriver};

/// The process image a mapping of `direction` uses: a `read` mapping takes the broker's value into
/// the input image, a `write` mapping publishes the output image's value (specification 32 §5).
fn direction_area(direction: MappingDirection) -> IoArea {
    match direction {
        MappingDirection::Read => IoArea::Input,
        MappingDirection::Write => IoArea::Output,
    }
}

/// The list of the driver's parameters that the point of a mapping of `direction` joins.
fn points_key(direction: MappingDirection) -> &'static str {
    match direction {
        MappingDirection::Read => "input_points",
        MappingDirection::Write => "output_points",
    }
}

#[derive(Debug, Clone)]
struct MqttScalarLayout {
    data_type: &'static str,
    io_size: IoSize,
    width: usize,
}

#[derive(Debug, Clone)]
struct PlannedMapping {
    mapping: Mapping,
    layout: MqttScalarLayout,
    address: IoAddress,
}

/// An MQTT tag bound to a compiled program's variable at a process-image address that the program
/// does not bind (specification 44 §13.3): the computer copies it between the image and the
/// variable at the boundaries of the scan.
#[derive(Debug, Clone)]
pub struct MqttTagLink {
    /// The variable's declared full name.
    pub name: SmolStr,
    /// The address, past the program's own process image.
    pub address: IoAddress,
    /// The variable's representation in the image.
    pub codec: IoValueCodec,
}

/// Resolves MQTT tag mappings into typed point maps (specification 44 §13.3): each tag names a
/// program instance's variable, `Instance.Variable`, found through the program's symbol table. A
/// variable the program binds at a process-image address of the mapping's direction keeps that
/// address; any other takes the next address past the program's own process image, and its
/// [`MqttTagLink`] copies it at the scan's boundaries.
///
/// # Errors
///
/// [`RuntimeError::InvalidConfig`] for a tag that names no variable of a program instance, a
/// variable that is no supported MQTT scalar, or a variable bound at more than one address or at an
/// address of another width.
pub fn resolve_compiled_mqtt_tag_mappings(
    program: &CompiledRuntime,
    params: &TomlValue,
) -> Result<(TomlValue, Vec<MqttTagLink>), RuntimeError> {
    let mappings = parse_mappings(params)?;
    if mappings.is_empty() {
        return Ok((params.clone(), Vec::new()));
    }

    let mut next_input = program.process_image_bytes(IoArea::Input);
    let mut next_output = program.process_image_bytes(IoArea::Output);
    let mut planned = Vec::with_capacity(mappings.len());
    let mut links = Vec::new();
    for (index, mapping) in mappings.into_iter().enumerate() {
        let variable = program
            .program_variable(mapping.tag.as_str())
            .ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!(
                        "io.params.mappings[{index}].tag: '{}' names no variable of a program instance",
                        mapping.tag
                    )
                    .into(),
                )
            })?;
        let layout = compiled_scalar_layout(&variable.shape).ok_or_else(|| {
            RuntimeError::InvalidConfig(
                format!(
                    "io.params.mappings[{index}].tag '{}' is not a supported MQTT scalar",
                    mapping.tag
                )
                .into(),
            )
        })?;
        let area = direction_area(mapping.direction);
        let bound = program
            .bound_addresses(variable.symbol)
            .into_iter()
            .filter(|address| address.area == area)
            .collect::<Vec<_>>();
        if bound.len() > 1 {
            return Err(RuntimeError::InvalidConfig(
                "MQTT tag mapping target has multiple compatible process-image bindings".into(),
            ));
        }
        let address = match bound.into_iter().next() {
            Some(address) if address.size != layout.io_size => {
                return Err(RuntimeError::InvalidConfig(
                    "MQTT tag mapping target has an incompatible process-image width".into(),
                ));
            }
            Some(address) => address,
            None => {
                let next = match area {
                    IoArea::Input => &mut next_input,
                    IoArea::Output => &mut next_output,
                    IoArea::Memory => unreachable!("MQTT tag directions never target memory"),
                };
                let address = next_address(next, area, &layout)?;
                links.push(MqttTagLink {
                    name: variable.name.clone(),
                    address: address.clone(),
                    codec: program.io_codec(variable.symbol),
                });
                address
            }
        };
        planned.push(PlannedMapping {
            mapping,
            layout,
            address,
        });
    }
    Ok((lowered_params(params, &planned)?, links))
}

/// The address at `next` in `area` for a value of `layout`, and `next` moved past it.
fn next_address(
    next: &mut usize,
    area: IoArea,
    layout: &MqttScalarLayout,
) -> Result<IoAddress, RuntimeError> {
    let byte = u32::try_from(*next).map_err(|_| {
        RuntimeError::InvalidConfig("MQTT tag mapping process-image offset overflow".into())
    })?;
    *next = next.checked_add(layout.width).ok_or_else(|| {
        RuntimeError::InvalidConfig("MQTT tag mapping process-image size overflow".into())
    })?;
    Ok(IoAddress {
        area,
        size: layout.io_size,
        byte,
        bit: 0,
        path: vec![byte],
        wildcard: false,
    })
}

/// `params` with each planned mapping lowered into a point of the driver, checked as the driver
/// checks its parameters.
fn lowered_params(
    params: &TomlValue,
    planned: &[PlannedMapping],
) -> Result<TomlValue, RuntimeError> {
    let mut lowered = params.clone();
    for plan in planned {
        append_point(&mut lowered, plan)?;
    }
    MqttIoDriver::validate_params(&lowered)?;
    Ok(lowered)
}

/// The symbolic mappings of `params`, read with the whole table by the driver's shared rules, so
/// that a mapping is exactly `tag`, `topic` and `direction` on every runtime; none without
/// `mappings`.
fn parse_mappings(params: &TomlValue) -> Result<Vec<Mapping>, RuntimeError> {
    Ok(MqttIoDriver::read_params(params)?
        .mappings
        .unwrap_or_default())
}

fn append_point(params: &mut TomlValue, plan: &PlannedMapping) -> Result<(), RuntimeError> {
    let table = params
        .as_table_mut()
        .ok_or_else(|| RuntimeError::InvalidConfig("io.params must be a table".into()))?;
    let key = points_key(plan.mapping.direction);
    let points = table
        .entry(key)
        .or_insert_with(|| TomlValue::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| {
            RuntimeError::InvalidConfig(format!("io.params.{key} must be an array").into())
        })?;
    let mut point = toml::map::Map::new();
    point.insert(
        "topic".to_string(),
        TomlValue::String(plan.mapping.topic.to_string()),
    );
    point.insert(
        "image_offset".to_string(),
        TomlValue::Integer(i64::from(plan.address.byte)),
    );
    if matches!(plan.layout.io_size, IoSize::Bit) {
        point.insert(
            "image_bit".to_string(),
            TomlValue::Integer(i64::from(plan.address.bit)),
        );
    }
    point.insert(
        "data_type".to_string(),
        TomlValue::String(plan.layout.data_type.to_string()),
    );
    points.push(TomlValue::Table(point));
    Ok(())
}

/// The layout of a compiled program's variable of `shape`, as [`mqtt_scalar_layout`] gives an
/// interpreted program's: an elementary type's own, a subrange's base type's, and an enumeration's
/// by its size, as the type table names no base type (2 bytes as `INT`, the default base).
fn compiled_scalar_layout(shape: &TypeShape) -> Option<MqttScalarLayout> {
    let kind = match shape {
        TypeShape::Elementary(kind) | TypeShape::Subrange(kind) => *kind,
        TypeShape::Enumeration { size: 2, .. } => type_kind::INT,
        TypeShape::Enumeration { size: 4, .. } => type_kind::DINT,
        _ => return None,
    };
    let (data_type, io_size, width) = match kind {
        type_kind::BOOL => ("bool", IoSize::Bit, 1),
        type_kind::UINT | type_kind::WORD => ("u16", IoSize::Word, 2),
        type_kind::INT => ("i16", IoSize::Word, 2),
        type_kind::UDINT | type_kind::DWORD => ("u32", IoSize::DWord, 4),
        type_kind::DINT => ("i32", IoSize::DWord, 4),
        type_kind::REAL => ("f32", IoSize::DWord, 4),
        _ => return None,
    };
    Some(MqttScalarLayout {
        data_type,
        io_size,
        width,
    })
}
