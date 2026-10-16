use serde::Deserialize;
use smol_str::SmolStr;
use toml::Value as TomlValue;
use trust_hir::{Type, TypeId};

use crate::boundary::resolve_io_tag;
use crate::error::RuntimeError;
use crate::memory::IoArea;
use crate::value::ValueRef;
use crate::Runtime;

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
    tag_name: SmolStr,
    reference: ValueRef,
    layout: MqttScalarLayout,
    address: IoAddress,
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
            let byte = u32::try_from(*next).map_err(|_| {
                RuntimeError::InvalidConfig("MQTT tag mapping process-image offset overflow".into())
            })?;
            let address = IoAddress {
                area,
                size: layout.io_size,
                byte,
                bit: 0,
                path: vec![byte],
                wildcard: false,
            };
            *next = next.checked_add(layout.width).ok_or_else(|| {
                RuntimeError::InvalidConfig("MQTT tag mapping process-image size overflow".into())
            })?;
            address
        };
        planned.push(PlannedMapping {
            mapping,
            tag_name: resolved.name,
            reference: resolved.reference,
            layout,
            address,
        });
    }

    let mut lowered = params.clone();
    for plan in &planned {
        append_point(&mut lowered, plan)?;
    }
    MqttIoDriver::validate_params(&lowered)?;

    let memory_len = runtime.io().memory().len();
    let bindings = planned
        .into_iter()
        .map(|plan| (plan.reference, plan.address, plan.tag_name))
        .collect::<Vec<_>>();
    runtime.configure_io_bindings(&bindings, [next_input, next_output, memory_len])?;
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
