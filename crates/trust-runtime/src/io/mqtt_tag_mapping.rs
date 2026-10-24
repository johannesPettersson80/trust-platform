use serde::Deserialize;
use smol_str::SmolStr;
use toml::Value as TomlValue;
use trust_hir::{Type, TypeId};
use trust_runtime_core::io_image::IoValueCodec;
use trust_runtime_core::native::image::type_kind;
use trust_runtime_core::native::memory::TypeShape;

use crate::boundary::resolve_io_tag;
use crate::error::RuntimeError;
use crate::memory::IoArea;
use crate::value::ValueRef;
use crate::{CompiledRuntime, Runtime};

use super::{IoAddress, IoSize, IoTarget, MqttIoDriver};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MqttTagMappingToml {
    tag: String,
    topic: String,
    direction: MqttTagDirection,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum MqttTagDirection {
    Read,
    Write,
}

impl MqttTagDirection {
    fn area(self) -> IoArea {
        match self {
            Self::Read => IoArea::Input,
            Self::Write => IoArea::Output,
        }
    }

    fn points_key(self) -> &'static str {
        match self {
            Self::Read => "input_points",
            Self::Write => "output_points",
        }
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
    mapping: MqttTagMappingToml,
    layout: MqttScalarLayout,
    address: IoAddress,
}

/// An MQTT tag bound to a compiled program's variable at a process-image address that the program
/// does not bind (specification 44 §13.3): the computer copies it between the image and the
/// variable at the boundaries of the scan, as the bytecode engine's binding does.
#[derive(Debug, Clone)]
pub struct MqttTagLink {
    /// The variable's declared full name.
    pub name: SmolStr,
    /// The address, past the program's own process image.
    pub address: IoAddress,
    /// The variable's representation in the image.
    pub codec: IoValueCodec,
}

/// Resolve MQTT tag mappings into typed point maps and shared process-image bindings.
pub fn resolve_mqtt_tag_mappings(
    runtime: &mut Runtime,
    params: &TomlValue,
) -> Result<TomlValue, RuntimeError> {
    let mappings = parse_mappings(params)?;
    if mappings.is_empty() {
        return Ok(params.clone());
    }

    let mut next_input = runtime.io().inputs().len();
    let mut next_output = runtime.io().outputs().len();
    let mut planned = Vec::with_capacity(mappings.len());
    let mut bindings = Vec::with_capacity(mappings.len());
    for (index, mapping) in mappings.into_iter().enumerate() {
        let resolved = resolve_io_tag(runtime, mapping.tag.as_str()).map_err(|error| {
            RuntimeError::InvalidConfig(format!("io.params.mappings[{index}].tag: {error}").into())
        })?;
        let layout = mqtt_scalar_layout(runtime, resolved.type_id).ok_or_else(|| {
            RuntimeError::InvalidConfig(
                format!(
                    "io.params.mappings[{index}].tag '{}' is not a supported MQTT scalar",
                    mapping.tag
                )
                .into(),
            )
        })?;
        let area = mapping.direction.area();
        let existing = existing_address(runtime, &resolved.reference, area, layout.io_size)?;
        let address = if let Some(address) = existing {
            address
        } else {
            let next = match area {
                IoArea::Input => &mut next_input,
                IoArea::Output => &mut next_output,
                IoArea::Memory => unreachable!("MQTT tag directions never target memory"),
            };
            next_address(next, area, &layout)?
        };
        bindings.push((resolved.reference, address.clone(), resolved.name));
        planned.push(PlannedMapping {
            mapping,
            layout,
            address,
        });
    }

    let lowered = lowered_params(params, &planned)?;
    let memory_len = runtime.io().memory().len();
    runtime.configure_io_bindings(&bindings, [next_input, next_output, memory_len])?;
    Ok(lowered)
}

/// [`resolve_mqtt_tag_mappings`] for a compiled program (specification 44 §13.3): each tag names a
/// program instance's variable, `Instance.Variable`, found through the program's symbol table. A
/// variable the program binds at a process-image address of the mapping's direction keeps that
/// address; any other takes the next address past the program's own process image, and its
/// [`MqttTagLink`] copies it at the scan's boundaries.
///
/// # Errors
///
/// [`RuntimeError::InvalidConfig`] as [`resolve_mqtt_tag_mappings`] refuses a mapping.
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
        let area = mapping.direction.area();
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

fn parse_mappings(params: &TomlValue) -> Result<Vec<MqttTagMappingToml>, RuntimeError> {
    let table = params
        .as_table()
        .ok_or_else(|| RuntimeError::InvalidConfig("io.params must be a table".into()))?;
    let Some(value) = table.get("mappings") else {
        return Ok(Vec::new());
    };
    value
        .clone()
        .try_into()
        .map_err(|error| RuntimeError::InvalidConfig(format!("io.params.mappings: {error}").into()))
}

fn existing_address(
    runtime: &Runtime,
    reference: &ValueRef,
    area: IoArea,
    expected_size: IoSize,
) -> Result<Option<IoAddress>, RuntimeError> {
    let io = runtime.io();
    let matches = io
        .bindings()
        .iter()
        .filter(|binding| {
            binding.address.area == area
                && matches!(&binding.target, IoTarget::Reference(candidate) if candidate == reference)
        })
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(RuntimeError::InvalidConfig(
            "MQTT tag mapping target has multiple compatible process-image bindings".into(),
        ));
    }
    let Some(binding) = matches.first() else {
        return Ok(None);
    };
    if binding.address.size != expected_size {
        return Err(RuntimeError::InvalidConfig(
            "MQTT tag mapping target has an incompatible process-image width".into(),
        ));
    }
    Ok(Some(binding.address.clone()))
}

fn append_point(params: &mut TomlValue, plan: &PlannedMapping) -> Result<(), RuntimeError> {
    let table = params
        .as_table_mut()
        .ok_or_else(|| RuntimeError::InvalidConfig("io.params must be a table".into()))?;
    let points = table
        .entry(plan.mapping.direction.points_key())
        .or_insert_with(|| TomlValue::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| {
            RuntimeError::InvalidConfig(
                format!(
                    "io.params.{} must be an array",
                    plan.mapping.direction.points_key()
                )
                .into(),
            )
        })?;
    let mut point = toml::map::Map::new();
    point.insert(
        "topic".to_string(),
        TomlValue::String(plan.mapping.topic.clone()),
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

fn mqtt_scalar_layout(runtime: &Runtime, type_id: TypeId) -> Option<MqttScalarLayout> {
    let ty = runtime.registry().get(type_id)?;
    match ty {
        Type::Alias { target, .. } => mqtt_scalar_layout(runtime, *target),
        Type::Subrange { base, .. } => mqtt_scalar_layout(runtime, *base),
        Type::Enum { base, .. } => mqtt_scalar_layout(runtime, *base),
        Type::Bool => Some(MqttScalarLayout {
            data_type: "bool",
            io_size: IoSize::Bit,
            width: 1,
        }),
        Type::UInt => Some(MqttScalarLayout {
            data_type: "u16",
            io_size: IoSize::Word,
            width: 2,
        }),
        Type::Word => Some(MqttScalarLayout {
            data_type: "u16",
            io_size: IoSize::Word,
            width: 2,
        }),
        Type::Int => Some(MqttScalarLayout {
            data_type: "i16",
            io_size: IoSize::Word,
            width: 2,
        }),
        Type::UDInt => Some(MqttScalarLayout {
            data_type: "u32",
            io_size: IoSize::DWord,
            width: 4,
        }),
        Type::DWord => Some(MqttScalarLayout {
            data_type: "u32",
            io_size: IoSize::DWord,
            width: 4,
        }),
        Type::DInt => Some(MqttScalarLayout {
            data_type: "i32",
            io_size: IoSize::DWord,
            width: 4,
        }),
        Type::Real => Some(MqttScalarLayout {
            data_type: "f32",
            io_size: IoSize::DWord,
            width: 4,
        }),
        _ => None,
    }
}
