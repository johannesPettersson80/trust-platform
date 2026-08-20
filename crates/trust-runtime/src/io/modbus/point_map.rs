use serde::Deserialize;

use super::{normalize_function_name, ModbusInputFunction, ModbusOutputFunction};
use crate::error::RuntimeError;

#[derive(Debug, Deserialize)]
pub(super) struct ModbusPointToml {
    pub(super) image_offset: usize,
    pub(super) image_bit: Option<u8>,
    pub(super) address: u16,
    pub(super) function: Option<String>,
    pub(super) data_type: Option<String>,
    pub(super) scale: Option<f64>,
    pub(super) offset: Option<f64>,
    pub(super) byte_order: Option<String>,
    pub(super) word_order: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ModbusInputPoint {
    pub(super) image_offset: usize,
    pub(super) image_bit: u8,
    pub(super) address: u16,
    pub(super) function: ModbusInputFunction,
    pub(super) data_type: ModbusPointType,
    pub(super) scale: f64,
    pub(super) offset: f64,
    pub(super) byte_order: ModbusByteOrder,
    pub(super) word_order: ModbusWordOrder,
}

impl ModbusInputPoint {
    pub(super) fn from_toml(
        value: ModbusPointToml,
        default_function: ModbusInputFunction,
    ) -> Result<Self, RuntimeError> {
        let function = value
            .function
            .as_deref()
            .map(ModbusInputFunction::parse)
            .transpose()?
            .unwrap_or(default_function);
        let data_type = parse_point_type_or_default(value.data_type.as_deref(), function)?;
        if function.is_bit_read() && data_type != ModbusPointType::Bool {
            return Err(RuntimeError::InvalidConfig(
                "io.params.input_points: coil/discrete input mappings require data_type = \"bool\""
                    .into(),
            ));
        }
        if !function.is_bit_read() && data_type == ModbusPointType::Bool {
            return Err(RuntimeError::InvalidConfig(
                "io.params.input_points: register mappings require a numeric data_type".into(),
            ));
        }
        Ok(Self {
            image_offset: value.image_offset,
            image_bit: parse_image_bit(value.image_bit)?,
            address: value.address,
            function,
            data_type,
            scale: parse_scale(value.scale)?,
            offset: parse_offset(value.offset)?,
            byte_order: parse_byte_order(value.byte_order.as_deref())?,
            word_order: parse_word_order(value.word_order.as_deref())?,
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct ModbusOutputPoint {
    pub(super) image_offset: usize,
    pub(super) image_bit: u8,
    pub(super) address: u16,
    pub(super) function: ModbusOutputFunction,
    pub(super) data_type: ModbusPointType,
    pub(super) scale: f64,
    pub(super) offset: f64,
    pub(super) byte_order: ModbusByteOrder,
    pub(super) word_order: ModbusWordOrder,
}

impl ModbusOutputPoint {
    pub(super) fn from_toml(
        value: ModbusPointToml,
        default_function: ModbusOutputFunction,
    ) -> Result<Self, RuntimeError> {
        let function = value
            .function
            .as_deref()
            .map(ModbusOutputFunction::parse)
            .transpose()?
            .unwrap_or(default_function);
        let data_type = parse_output_type_or_default(value.data_type.as_deref(), function)?;
        if function.is_coil_write() && data_type != ModbusPointType::Bool {
            return Err(RuntimeError::InvalidConfig(
                "io.params.output_points: coil mappings require data_type = \"bool\"".into(),
            ));
        }
        if function.is_register_write() && data_type == ModbusPointType::Bool {
            return Err(RuntimeError::InvalidConfig(
                "io.params.output_points: register mappings require a numeric data_type".into(),
            ));
        }
        if matches!(function, ModbusOutputFunction::SingleRegister)
            && data_type.register_count() != 1
        {
            return Err(RuntimeError::InvalidConfig(
                "io.params.output_points: write_single_register supports only 16-bit data types"
                    .into(),
            ));
        }
        Ok(Self {
            image_offset: value.image_offset,
            image_bit: parse_image_bit(value.image_bit)?,
            address: value.address,
            function,
            data_type,
            scale: parse_scale(value.scale)?,
            offset: parse_offset(value.offset)?,
            byte_order: parse_byte_order(value.byte_order.as_deref())?,
            word_order: parse_word_order(value.word_order.as_deref())?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModbusPointType {
    Bool,
    U16,
    I16,
    U32,
    I32,
    F32,
}

impl ModbusPointType {
    fn parse(value: &str) -> Result<Self, RuntimeError> {
        match normalize_function_name(value).as_str() {
            "bool" | "boolean" | "coil" => Ok(Self::Bool),
            "u16" | "uint16" | "word" => Ok(Self::U16),
            "i16" | "int16" | "int" => Ok(Self::I16),
            "u32" | "uint32" | "dword" => Ok(Self::U32),
            "i32" | "int32" | "dint" => Ok(Self::I32),
            "f32" | "real" | "float" => Ok(Self::F32),
            _ => Err(RuntimeError::InvalidConfig(
                format!(
                    "io.params.modbus point data_type: unsupported value '{value}', expected \
                     bool, u16, i16, u32, i32, or f32"
                )
                .into(),
            )),
        }
    }

    pub(super) fn register_count(self) -> u16 {
        match self {
            Self::Bool => 0,
            Self::U16 | Self::I16 => 1,
            Self::U32 | Self::I32 | Self::F32 => 2,
        }
    }

    fn image_len(self) -> usize {
        match self {
            Self::Bool => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModbusByteOrder {
    Big,
    Little,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModbusWordOrder {
    Big,
    Little,
}

impl ModbusOutputFunction {
    pub(super) fn is_coil_write(self) -> bool {
        matches!(self, Self::SingleCoil | Self::MultipleCoils)
    }

    pub(super) fn is_register_write(self) -> bool {
        matches!(self, Self::SingleRegister | Self::MultipleRegisters)
    }
}

fn parse_point_type_or_default(
    value: Option<&str>,
    function: ModbusInputFunction,
) -> Result<ModbusPointType, RuntimeError> {
    if let Some(value) = value {
        return ModbusPointType::parse(value);
    }
    if function.is_bit_read() {
        Ok(ModbusPointType::Bool)
    } else {
        Ok(ModbusPointType::U16)
    }
}

fn parse_output_type_or_default(
    value: Option<&str>,
    function: ModbusOutputFunction,
) -> Result<ModbusPointType, RuntimeError> {
    if let Some(value) = value {
        return ModbusPointType::parse(value);
    }
    if function.is_coil_write() {
        Ok(ModbusPointType::Bool)
    } else {
        Ok(ModbusPointType::U16)
    }
}

fn parse_image_bit(value: Option<u8>) -> Result<u8, RuntimeError> {
    let bit = value.unwrap_or(0);
    if bit > 7 {
        return Err(RuntimeError::InvalidConfig(
            "io.params.modbus point image_bit must be between 0 and 7".into(),
        ));
    }
    Ok(bit)
}

fn parse_scale(value: Option<f64>) -> Result<f64, RuntimeError> {
    let scale = value.unwrap_or(1.0);
    if !scale.is_finite() || scale.abs() < f64::EPSILON {
        return Err(RuntimeError::InvalidConfig(
            "io.params.modbus point scale must be finite and non-zero".into(),
        ));
    }
    Ok(scale)
}

fn parse_offset(value: Option<f64>) -> Result<f64, RuntimeError> {
    let offset = value.unwrap_or(0.0);
    if !offset.is_finite() {
        return Err(RuntimeError::InvalidConfig(
            "io.params.modbus point offset must be finite".into(),
        ));
    }
    Ok(offset)
}

fn parse_byte_order(value: Option<&str>) -> Result<ModbusByteOrder, RuntimeError> {
    match value.map(normalize_function_name).as_deref() {
        None | Some("big") | Some("be") | Some("big_endian") => Ok(ModbusByteOrder::Big),
        Some("little") | Some("le") | Some("little_endian") => Ok(ModbusByteOrder::Little),
        Some(other) => Err(RuntimeError::InvalidConfig(
            format!(
                "io.params.modbus point byte_order: unsupported value '{other}', expected big or little"
            )
            .into(),
        )),
    }
}

fn parse_word_order(value: Option<&str>) -> Result<ModbusWordOrder, RuntimeError> {
    match value.map(normalize_function_name).as_deref() {
        None | Some("big") | Some("be") | Some("big_endian") => Ok(ModbusWordOrder::Big),
        Some("little") | Some("le") | Some("little_endian") => Ok(ModbusWordOrder::Little),
        Some(other) => Err(RuntimeError::InvalidConfig(
            format!(
                "io.params.modbus point word_order: unsupported value '{other}', expected big or little"
            )
            .into(),
        )),
    }
}

pub(super) fn decode_modbus_numeric(
    data_type: ModbusPointType,
    wire: &[u8],
    byte_order: ModbusByteOrder,
    word_order: ModbusWordOrder,
) -> Result<f64, RuntimeError> {
    let normalized = normalize_modbus_register_bytes(data_type, wire, byte_order, word_order)?;
    match data_type {
        ModbusPointType::Bool => Err(RuntimeError::IoDriver(
            "modbus bool point cannot be decoded as register numeric".into(),
        )),
        ModbusPointType::U16 => Ok(u16::from_be_bytes([normalized[0], normalized[1]]) as f64),
        ModbusPointType::I16 => Ok(i16::from_be_bytes([normalized[0], normalized[1]]) as f64),
        ModbusPointType::U32 => {
            Ok(
                u32::from_be_bytes([normalized[0], normalized[1], normalized[2], normalized[3]])
                    as f64,
            )
        }
        ModbusPointType::I32 => {
            Ok(
                i32::from_be_bytes([normalized[0], normalized[1], normalized[2], normalized[3]])
                    as f64,
            )
        }
        ModbusPointType::F32 => {
            let bits =
                u32::from_be_bytes([normalized[0], normalized[1], normalized[2], normalized[3]]);
            finite_f32_value(f32::from_bits(bits), "modbus mapped F32 wire value")
        }
    }
}

pub(super) fn encode_modbus_numeric(
    data_type: ModbusPointType,
    value: f64,
    byte_order: ModbusByteOrder,
    word_order: ModbusWordOrder,
) -> Result<Vec<u8>, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::IoDriver(
            "modbus mapped numeric value must be finite".into(),
        ));
    }
    let mut bytes = match data_type {
        ModbusPointType::Bool => {
            return Err(RuntimeError::IoDriver(
                "modbus bool point cannot be encoded as register numeric".into(),
            ));
        }
        ModbusPointType::U16 => (checked_round(value, u16::MIN as f64, u16::MAX as f64)? as u16)
            .to_be_bytes()
            .to_vec(),
        ModbusPointType::I16 => (checked_round(value, i16::MIN as f64, i16::MAX as f64)? as i16)
            .to_be_bytes()
            .to_vec(),
        ModbusPointType::U32 => (checked_round(value, u32::MIN as f64, u32::MAX as f64)? as u32)
            .to_be_bytes()
            .to_vec(),
        ModbusPointType::I32 => (checked_round(value, i32::MIN as f64, i32::MAX as f64)? as i32)
            .to_be_bytes()
            .to_vec(),
        ModbusPointType::F32 => narrow_finite_f32(value, "modbus mapped F32 wire value")?
            .to_bits()
            .to_be_bytes()
            .to_vec(),
    };
    apply_modbus_order(&mut bytes, byte_order, word_order)?;
    Ok(bytes)
}

fn normalize_modbus_register_bytes(
    data_type: ModbusPointType,
    wire: &[u8],
    byte_order: ModbusByteOrder,
    word_order: ModbusWordOrder,
) -> Result<Vec<u8>, RuntimeError> {
    let expected = usize::from(data_type.register_count()) * 2;
    if wire.len() != expected {
        return Err(RuntimeError::IoDriver(
            format!(
                "modbus mapped register response must contain exactly {expected} byte(s), got {}",
                wire.len()
            )
            .into(),
        ));
    }
    let mut bytes = wire.to_vec();
    if matches!(byte_order, ModbusByteOrder::Little) {
        swap_register_bytes(&mut bytes)?;
    }
    if matches!(word_order, ModbusWordOrder::Little) {
        reverse_register_words(&mut bytes)?;
    }
    Ok(bytes)
}

pub(super) fn validate_modbus_point_maps(
    input_points: &[ModbusInputPoint],
    output_points: &[ModbusOutputPoint],
) -> Result<(), RuntimeError> {
    validate_input_points(input_points)?;
    validate_output_points(output_points)
}

fn validate_input_points(points: &[ModbusInputPoint]) -> Result<(), RuntimeError> {
    for (index, point) in points.iter().enumerate() {
        let protocol_len = point.data_type.register_count().max(1);
        let protocol_end = point.address.checked_add(protocol_len).ok_or_else(|| {
            RuntimeError::InvalidConfig(
                "io.params.input_points Modbus address range overflow".into(),
            )
        })?;
        let image_range = point_image_range(point.image_offset, point.image_bit, point.data_type)?;
        for previous in &points[..index] {
            let previous_protocol_len = previous.data_type.register_count().max(1);
            let previous_protocol_end = previous
                .address
                .checked_add(previous_protocol_len)
                .ok_or_else(|| {
                    RuntimeError::InvalidConfig(
                        "io.params.input_points Modbus address range overflow".into(),
                    )
                })?;
            if point.function == previous.function
                && ranges_overlap(
                    usize::from(point.address)..usize::from(protocol_end),
                    usize::from(previous.address)..usize::from(previous_protocol_end),
                )
            {
                return Err(RuntimeError::InvalidConfig(
                    "io.params.input_points contain duplicate or overlapping Modbus address ranges"
                        .into(),
                ));
            }
            if ranges_overlap(
                image_range.clone(),
                point_image_range(
                    previous.image_offset,
                    previous.image_bit,
                    previous.data_type,
                )?,
            ) {
                return Err(RuntimeError::InvalidConfig(
                    "io.params.input_points contain overlapping process-image ranges".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_output_points(points: &[ModbusOutputPoint]) -> Result<(), RuntimeError> {
    for (index, point) in points.iter().enumerate() {
        let protocol_len = point.data_type.register_count().max(1);
        let protocol_end = point.address.checked_add(protocol_len).ok_or_else(|| {
            RuntimeError::InvalidConfig(
                "io.params.output_points Modbus address range overflow".into(),
            )
        })?;
        let image_range = point_image_range(point.image_offset, point.image_bit, point.data_type)?;
        for previous in &points[..index] {
            let previous_protocol_len = previous.data_type.register_count().max(1);
            let previous_protocol_end = previous
                .address
                .checked_add(previous_protocol_len)
                .ok_or_else(|| {
                    RuntimeError::InvalidConfig(
                        "io.params.output_points Modbus address range overflow".into(),
                    )
                })?;
            if point.function == previous.function
                && ranges_overlap(
                    usize::from(point.address)..usize::from(protocol_end),
                    usize::from(previous.address)..usize::from(previous_protocol_end),
                )
            {
                return Err(RuntimeError::InvalidConfig(
                    "io.params.output_points contain duplicate or overlapping Modbus address ranges"
                        .into(),
                ));
            }
            if ranges_overlap(
                image_range.clone(),
                point_image_range(
                    previous.image_offset,
                    previous.image_bit,
                    previous.data_type,
                )?,
            ) {
                return Err(RuntimeError::InvalidConfig(
                    "io.params.output_points contain overlapping process-image ranges".into(),
                ));
            }
        }
    }
    Ok(())
}

fn point_image_range(
    image_offset: usize,
    image_bit: u8,
    data_type: ModbusPointType,
) -> Result<std::ops::Range<usize>, RuntimeError> {
    let start = image_offset.checked_mul(8).ok_or_else(|| {
        RuntimeError::InvalidConfig("io.params.modbus point image range overflow".into())
    })?;
    if data_type == ModbusPointType::Bool {
        let start = start.checked_add(usize::from(image_bit)).ok_or_else(|| {
            RuntimeError::InvalidConfig("io.params.modbus point image range overflow".into())
        })?;
        return Ok(start..start + 1);
    }
    let len = data_type.image_len().checked_mul(8).ok_or_else(|| {
        RuntimeError::InvalidConfig("io.params.modbus point image range overflow".into())
    })?;
    let end = start.checked_add(len).ok_or_else(|| {
        RuntimeError::InvalidConfig("io.params.modbus point image range overflow".into())
    })?;
    Ok(start..end)
}

fn ranges_overlap(left: std::ops::Range<usize>, right: std::ops::Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn apply_modbus_order(
    bytes: &mut [u8],
    byte_order: ModbusByteOrder,
    word_order: ModbusWordOrder,
) -> Result<(), RuntimeError> {
    if matches!(word_order, ModbusWordOrder::Little) {
        reverse_register_words(bytes)?;
    }
    if matches!(byte_order, ModbusByteOrder::Little) {
        swap_register_bytes(bytes)?;
    }
    Ok(())
}

fn swap_register_bytes(bytes: &mut [u8]) -> Result<(), RuntimeError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(RuntimeError::IoDriver(
            "modbus mapped register byte length must be even".into(),
        ));
    }
    for chunk in bytes.chunks_exact_mut(2) {
        chunk.swap(0, 1);
    }
    Ok(())
}

fn reverse_register_words(bytes: &mut [u8]) -> Result<(), RuntimeError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(RuntimeError::IoDriver(
            "modbus mapped register byte length must be even".into(),
        ));
    }
    let words = bytes.len() / 2;
    for idx in 0..(words / 2) {
        let left = idx * 2;
        let right = (words - 1 - idx) * 2;
        bytes.swap(left, right);
        bytes.swap(left + 1, right + 1);
    }
    Ok(())
}

pub(super) fn write_image_bool(
    image: &mut [u8],
    image_offset: usize,
    image_bit: u8,
    value: bool,
) -> Result<(), RuntimeError> {
    let byte = image.get_mut(image_offset).ok_or_else(|| {
        RuntimeError::IoDriver(
            format!("modbus point image_offset {image_offset} out of range").into(),
        )
    })?;
    if value {
        *byte |= 1 << image_bit;
    } else {
        *byte &= !(1 << image_bit);
    }
    Ok(())
}

pub(super) fn read_image_bool(
    image: &[u8],
    image_offset: usize,
    image_bit: u8,
) -> Result<bool, RuntimeError> {
    let byte = image.get(image_offset).copied().ok_or_else(|| {
        RuntimeError::IoDriver(
            format!("modbus point image_offset {image_offset} out of range").into(),
        )
    })?;
    Ok(byte & (1 << image_bit) != 0)
}

pub(super) fn write_image_numeric(
    image: &mut [u8],
    image_offset: usize,
    data_type: ModbusPointType,
    value: f64,
) -> Result<(), RuntimeError> {
    let bytes = encode_process_image_numeric(data_type, value)?;
    let range = checked_image_range(image.len(), image_offset, bytes.len())?;
    image[range].copy_from_slice(&bytes);
    Ok(())
}

pub(super) fn read_image_numeric(
    image: &[u8],
    image_offset: usize,
    data_type: ModbusPointType,
) -> Result<f64, RuntimeError> {
    let len = data_type.image_len();
    let range = checked_image_range(image.len(), image_offset, len)?;
    let bytes = &image[range];
    match data_type {
        ModbusPointType::Bool => Err(RuntimeError::IoDriver(
            "modbus bool point cannot be read as process-image numeric".into(),
        )),
        ModbusPointType::U16 => Ok(u16::from_le_bytes([bytes[0], bytes[1]]) as f64),
        ModbusPointType::I16 => Ok(i16::from_le_bytes([bytes[0], bytes[1]]) as f64),
        ModbusPointType::U32 => {
            Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64)
        }
        ModbusPointType::I32 => {
            Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64)
        }
        ModbusPointType::F32 => {
            let bits = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            finite_f32_value(
                f32::from_bits(bits),
                "modbus mapped F32 process-image value",
            )
        }
    }
}

fn encode_process_image_numeric(
    data_type: ModbusPointType,
    value: f64,
) -> Result<Vec<u8>, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::IoDriver(
            "modbus mapped process-image value must be finite".into(),
        ));
    }
    match data_type {
        ModbusPointType::Bool => Err(RuntimeError::IoDriver(
            "modbus bool point cannot be encoded as process-image numeric".into(),
        )),
        ModbusPointType::U16 => Ok((checked_round(value, u16::MIN as f64, u16::MAX as f64)?
            as u16)
            .to_le_bytes()
            .to_vec()),
        ModbusPointType::I16 => Ok((checked_round(value, i16::MIN as f64, i16::MAX as f64)?
            as i16)
            .to_le_bytes()
            .to_vec()),
        ModbusPointType::U32 => Ok((checked_round(value, u32::MIN as f64, u32::MAX as f64)?
            as u32)
            .to_le_bytes()
            .to_vec()),
        ModbusPointType::I32 => Ok((checked_round(value, i32::MIN as f64, i32::MAX as f64)?
            as i32)
            .to_le_bytes()
            .to_vec()),
        ModbusPointType::F32 => Ok(narrow_finite_f32(
            value,
            "modbus mapped F32 process-image value",
        )?
        .to_bits()
        .to_le_bytes()
        .to_vec()),
    }
}

fn checked_round(value: f64, min: f64, max: f64) -> Result<i64, RuntimeError> {
    let rounded = value.round();
    if rounded < min || rounded > max {
        return Err(RuntimeError::IoDriver(
            format!("modbus mapped numeric value {value} outside {min}..={max}").into(),
        ));
    }
    Ok(rounded as i64)
}

fn finite_f32_value(value: f32, label: &str) -> Result<f64, RuntimeError> {
    if value.is_finite() {
        Ok(f64::from(value))
    } else {
        Err(RuntimeError::IoDriver(
            format!("{label} must be finite").into(),
        ))
    }
}

fn narrow_finite_f32(value: f64, label: &str) -> Result<f32, RuntimeError> {
    let narrowed = value as f32;
    if narrowed.is_finite() {
        Ok(narrowed)
    } else {
        Err(RuntimeError::IoDriver(
            format!("{label} must remain finite after narrowing").into(),
        ))
    }
}

fn checked_image_range(
    image_len: usize,
    image_offset: usize,
    len: usize,
) -> Result<std::ops::Range<usize>, RuntimeError> {
    let end = image_offset
        .checked_add(len)
        .ok_or_else(|| RuntimeError::IoDriver("modbus point image range overflow".into()))?;
    if end > image_len {
        return Err(RuntimeError::IoDriver(
            format!("modbus point image range {image_offset}..{end} out of {image_len}").into(),
        ));
    }
    Ok(image_offset..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    include!("tests/point_map_contract.rs");

    #[test]
    fn mapped_f32_wire_decode_rejects_non_finite() {
        let wire = f32::NAN.to_bits().to_be_bytes();

        let err = decode_modbus_numeric(
            ModbusPointType::F32,
            &wire,
            ModbusByteOrder::Big,
            ModbusWordOrder::Big,
        )
        .expect_err("mapped Modbus F32 wire NaN must be rejected");

        assert!(
            err.to_string().contains("finite"),
            "expected finite-value diagnostic, got {err}"
        );
    }

    #[test]
    fn mapped_f32_process_image_read_rejects_non_finite() {
        let image = f32::INFINITY.to_bits().to_le_bytes();

        let err = read_image_numeric(&image, 0, ModbusPointType::F32)
            .expect_err("mapped Modbus F32 image infinity must be rejected");

        assert!(
            err.to_string().contains("finite"),
            "expected finite-value diagnostic, got {err}"
        );
    }

    #[test]
    fn mapped_f32_process_image_encoding_rejects_narrowing_overflow() {
        for value in [f64::MAX, -f64::MAX] {
            let mut image = [0xA5; 4];
            let error = write_image_numeric(&mut image, 0, ModbusPointType::F32, value)
                .expect_err("finite f64 that narrows to infinity must fail");
            assert!(error.to_string().contains("finite"));
            assert_eq!(image, [0xA5; 4], "failed encoding mutated the image");
        }
    }

    #[test]
    fn mapped_f32_wire_encoding_rejects_narrowing_overflow() {
        for value in [f64::MAX, -f64::MAX] {
            let error = encode_modbus_numeric(
                ModbusPointType::F32,
                value,
                ModbusByteOrder::Big,
                ModbusWordOrder::Big,
            )
            .expect_err("finite f64 that narrows to infinity must not reach the wire");
            assert!(error.to_string().contains("finite"));
        }
    }
}
