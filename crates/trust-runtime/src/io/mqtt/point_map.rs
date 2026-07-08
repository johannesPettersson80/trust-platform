#[derive(Debug, Clone, Deserialize)]
struct MqttPointToml {
    topic: String,
    image_offset: usize,
    image_bit: Option<u8>,
    data_type: Option<String>,
    payload_format: Option<String>,
    metric_name: Option<String>,
    scale: Option<f64>,
    offset: Option<f64>,
}

#[derive(Debug, Clone)]
struct MqttInputPoint {
    topic: SmolStr,
    image_offset: usize,
    image_bit: Option<u8>,
    data_type: MqttPointType,
    payload_format: MqttPayloadFormat,
    scale: f64,
    offset: f64,
}

#[derive(Debug, Clone)]
struct MqttOutputPoint {
    topic: SmolStr,
    metric_name: SmolStr,
    image_offset: usize,
    image_bit: Option<u8>,
    data_type: MqttPointType,
    payload_format: MqttPayloadFormat,
    scale: f64,
    offset: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MqttPointType {
    Bool,
    U16,
    I16,
    U32,
    I32,
    F32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MqttPayloadFormat {
    Text,
    Json,
    BinaryLe,
    BinaryBe,
}

impl MqttInputPoint {
    fn from_toml(value: MqttPointToml) -> Result<Self, RuntimeError> {
        let data_type = parse_data_type(value.data_type.as_deref(), "input_points")?;
        validate_point_shape("input_points", data_type, value.image_bit)?;
        let (scale, offset) = parse_scale_offset("input_points", data_type, value.scale, value.offset)?;
        Ok(Self {
            topic: parse_point_topic("input_points", &value.topic)?,
            image_offset: value.image_offset,
            image_bit: value.image_bit,
            data_type,
            payload_format: parse_payload_format(value.payload_format.as_deref())?,
            scale,
            offset,
        })
    }
}

impl MqttOutputPoint {
    fn from_toml(value: MqttPointToml) -> Result<Self, RuntimeError> {
        let data_type = parse_data_type(value.data_type.as_deref(), "output_points")?;
        validate_point_shape("output_points", data_type, value.image_bit)?;
        let (scale, offset) =
            parse_scale_offset("output_points", data_type, value.scale, value.offset)?;
        let topic = parse_point_topic("output_points", &value.topic)?;
        let metric_name = parse_metric_name(value.metric_name.as_deref().unwrap_or(topic.as_str()))?;
        Ok(Self {
            topic,
            metric_name,
            image_offset: value.image_offset,
            image_bit: value.image_bit,
            data_type,
            payload_format: parse_payload_format(value.payload_format.as_deref())?,
            scale,
            offset,
        })
    }
}

fn parse_point_topic(context: &str, topic: &str) -> Result<SmolStr, RuntimeError> {
    let topic = topic.trim();
    if topic.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            format!("mqtt {context}.topic must not be empty").into(),
        ));
    }
    Ok(SmolStr::new(topic))
}

fn parse_metric_name(metric_name: &str) -> Result<SmolStr, RuntimeError> {
    let metric_name = metric_name.trim();
    if metric_name.is_empty() || metric_name.chars().any(char::is_control) {
        return Err(RuntimeError::InvalidConfig(
            "mqtt output_points.metric_name must not be empty or contain control characters".into(),
        ));
    }
    Ok(SmolStr::new(metric_name))
}

fn parse_data_type(value: Option<&str>, context: &str) -> Result<MqttPointType, RuntimeError> {
    let Some(value) = value else {
        return Err(RuntimeError::InvalidConfig(
            format!("mqtt {context}.data_type is required for typed point maps").into(),
        ));
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "bool" | "boolean" => Ok(MqttPointType::Bool),
        "u16" | "uint16" | "word" => Ok(MqttPointType::U16),
        "i16" | "int16" | "int" => Ok(MqttPointType::I16),
        "u32" | "uint32" | "dword" => Ok(MqttPointType::U32),
        "i32" | "int32" | "dint" => Ok(MqttPointType::I32),
        "f32" | "float" | "real" => Ok(MqttPointType::F32),
        other => Err(RuntimeError::InvalidConfig(
            format!(
                "mqtt {context}.data_type '{other}' is unsupported, expected bool, u16, i16, u32, i32, or f32"
            )
            .into(),
        )),
    }
}

fn validate_point_shape(
    context: &str,
    data_type: MqttPointType,
    image_bit: Option<u8>,
) -> Result<(), RuntimeError> {
    if let Some(bit) = image_bit {
        if bit > 7 {
            return Err(RuntimeError::InvalidConfig(
                format!("mqtt {context}.image_bit must be between 0 and 7").into(),
            ));
        }
    }
    match (data_type, image_bit) {
        (MqttPointType::Bool, Some(_)) => Ok(()),
        (MqttPointType::Bool, None) => Err(RuntimeError::InvalidConfig(
            format!("mqtt {context}: bool point maps require image_bit").into(),
        )),
        (_, Some(_)) => Err(RuntimeError::InvalidConfig(
            format!("mqtt {context}: numeric point maps must not set image_bit").into(),
        )),
        (_, None) => Ok(()),
    }
}

fn parse_scale_offset(
    context: &str,
    data_type: MqttPointType,
    scale: Option<f64>,
    offset: Option<f64>,
) -> Result<(f64, f64), RuntimeError> {
    let scale = scale.unwrap_or(1.0);
    let offset = offset.unwrap_or(0.0);
    if !scale.is_finite() || scale.abs() < f64::EPSILON {
        return Err(RuntimeError::InvalidConfig(
            format!("mqtt {context}.scale must be finite and non-zero").into(),
        ));
    }
    if !offset.is_finite() {
        return Err(RuntimeError::InvalidConfig(
            format!("mqtt {context}.offset must be finite").into(),
        ));
    }
    if data_type == MqttPointType::Bool
        && ((scale - 1.0).abs() >= f64::EPSILON || offset.abs() >= f64::EPSILON)
    {
        return Err(RuntimeError::InvalidConfig(
            format!("mqtt {context}: bool point maps do not support scale or offset").into(),
        ));
    }
    Ok((scale, offset))
}

fn parse_payload_format(value: Option<&str>) -> Result<MqttPayloadFormat, RuntimeError> {
    let Some(value) = value else {
        return Ok(MqttPayloadFormat::Text);
    };
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "text" | "string" => Ok(MqttPayloadFormat::Text),
        "json" => Ok(MqttPayloadFormat::Json),
        "binary_le" | "little_endian" | "le" => Ok(MqttPayloadFormat::BinaryLe),
        "binary_be" | "big_endian" | "be" => Ok(MqttPayloadFormat::BinaryBe),
        other => Err(RuntimeError::InvalidConfig(
            format!(
                "mqtt payload_format '{other}' is unsupported, expected text, json, binary_le, or binary_be"
            )
            .into(),
        )),
    }
}

fn decode_mqtt_point(
    point: &MqttInputPoint,
    payload: &[u8],
    image: &mut [u8],
) -> Result<(), RuntimeError> {
    match point.data_type {
        MqttPointType::Bool => {
            let value = parse_bool_payload(payload, point.payload_format)?;
            write_image_bool(image, point.image_offset, point.image_bit, value)
        }
        data_type => {
            let raw = parse_numeric_payload(payload, point.payload_format, data_type)?;
            let engineering = raw * point.scale + point.offset;
            write_image_numeric(image, point.image_offset, data_type, engineering)
        }
    }
}

fn encode_mqtt_point(point: &MqttOutputPoint, image: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    match point.data_type {
        MqttPointType::Bool => encode_bool_payload(
            read_image_bool(image, point.image_offset, point.image_bit)?,
            point.payload_format,
        ),
        data_type => {
            let engineering = read_image_numeric(image, point.image_offset, data_type)?;
            let raw = (engineering - point.offset) / point.scale;
            encode_numeric_payload(raw, point.payload_format, data_type)
        }
    }
}

fn parse_bool_payload(payload: &[u8], format: MqttPayloadFormat) -> Result<bool, RuntimeError> {
    match format {
        MqttPayloadFormat::Text => parse_bool_text(payload),
        MqttPayloadFormat::Json => match parse_json_payload(payload)? {
            serde_json::Value::Bool(value) => Ok(value),
            serde_json::Value::Number(number) => Ok(number.as_f64().unwrap_or(0.0) != 0.0),
            serde_json::Value::String(text) => parse_bool_str(&text),
            _ => Err(RuntimeError::IoDriver(
                "mqtt bool JSON payload must be bool, number, or string".into(),
            )),
        },
        MqttPayloadFormat::BinaryLe | MqttPayloadFormat::BinaryBe => {
            let Some(value) = payload.first() else {
                return Err(RuntimeError::IoDriver(
                    "mqtt bool binary payload must contain at least one byte".into(),
                ));
            };
            Ok(*value != 0)
        }
    }
}

fn parse_bool_text(payload: &[u8]) -> Result<bool, RuntimeError> {
    let text = parse_utf8_payload(payload)?;
    parse_bool_str(text.trim())
}

fn parse_bool_str(text: &str) -> Result<bool, RuntimeError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" => Ok(true),
        "false" | "0" | "off" => Ok(false),
        _ => Err(RuntimeError::IoDriver(
            format!("mqtt bool payload '{text}' is not true/false or 1/0").into(),
        )),
    }
}

fn parse_numeric_payload(
    payload: &[u8],
    format: MqttPayloadFormat,
    data_type: MqttPointType,
) -> Result<f64, RuntimeError> {
    let value = match format {
        MqttPayloadFormat::Text => parse_utf8_payload(payload)?
            .trim()
            .parse::<f64>()
            .map_err(|err| RuntimeError::IoDriver(format!("mqtt numeric payload: {err}").into())),
        MqttPayloadFormat::Json => match parse_json_payload(payload)? {
            serde_json::Value::Number(number) => number
                .as_f64()
                .ok_or_else(|| RuntimeError::IoDriver("mqtt JSON number out of range".into())),
            serde_json::Value::String(text) => text.trim().parse::<f64>().map_err(|err| {
                RuntimeError::IoDriver(format!("mqtt JSON numeric string: {err}").into())
            }),
            _ => Err(RuntimeError::IoDriver(
                "mqtt numeric JSON payload must be number or string".into(),
            )),
        },
        MqttPayloadFormat::BinaryLe | MqttPayloadFormat::BinaryBe => {
            parse_binary_numeric(payload, format, data_type)
        }
    }?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(RuntimeError::IoDriver(
            "mqtt numeric payload must be finite".into(),
        ))
    }
}

fn parse_json_payload(payload: &[u8]) -> Result<serde_json::Value, RuntimeError> {
    serde_json::from_slice(payload)
        .map_err(|err| RuntimeError::IoDriver(format!("mqtt JSON payload: {err}").into()))
}

fn parse_utf8_payload(payload: &[u8]) -> Result<&str, RuntimeError> {
    std::str::from_utf8(payload)
        .map_err(|err| RuntimeError::IoDriver(format!("mqtt text payload is not UTF-8: {err}").into()))
}

fn parse_binary_numeric(
    payload: &[u8],
    format: MqttPayloadFormat,
    data_type: MqttPointType,
) -> Result<f64, RuntimeError> {
    match data_type {
        MqttPointType::U16 => {
            let bytes = exact_binary_payload::<2>(payload, data_type)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => u16::from_le_bytes(bytes),
                MqttPayloadFormat::BinaryBe => u16::from_be_bytes(bytes),
                _ => unreachable!(),
            } as f64)
        }
        MqttPointType::I16 => {
            let bytes = exact_binary_payload::<2>(payload, data_type)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => i16::from_le_bytes(bytes),
                MqttPayloadFormat::BinaryBe => i16::from_be_bytes(bytes),
                _ => unreachable!(),
            } as f64)
        }
        MqttPointType::U32 => {
            let bytes = exact_binary_payload::<4>(payload, data_type)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => u32::from_le_bytes(bytes),
                MqttPayloadFormat::BinaryBe => u32::from_be_bytes(bytes),
                _ => unreachable!(),
            } as f64)
        }
        MqttPointType::I32 => {
            let bytes = exact_binary_payload::<4>(payload, data_type)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => i32::from_le_bytes(bytes),
                MqttPayloadFormat::BinaryBe => i32::from_be_bytes(bytes),
                _ => unreachable!(),
            } as f64)
        }
        MqttPointType::F32 => {
            let bytes = exact_binary_payload::<4>(payload, data_type)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => f32::from_le_bytes(bytes),
                MqttPayloadFormat::BinaryBe => f32::from_be_bytes(bytes),
                _ => unreachable!(),
            } as f64)
        }
        MqttPointType::Bool => Err(RuntimeError::IoDriver(
            "mqtt bool binary payload cannot be parsed as numeric".into(),
        )),
    }
}

fn exact_binary_payload<const N: usize>(
    payload: &[u8],
    data_type: MqttPointType,
) -> Result<[u8; N], RuntimeError> {
    payload.try_into().map_err(|_| {
        RuntimeError::IoDriver(
            format!(
                "mqtt binary payload for {data_type:?} must be exactly {N} byte(s), got {}",
                payload.len()
            )
            .into(),
        )
    })
}

fn encode_bool_payload(value: bool, format: MqttPayloadFormat) -> Result<Vec<u8>, RuntimeError> {
    match format {
        MqttPayloadFormat::Text => Ok(if value { b"true".to_vec() } else { b"false".to_vec() }),
        MqttPayloadFormat::Json => serde_json::to_vec(&value).map_err(|err| {
            RuntimeError::IoDriver(format!("mqtt bool JSON encode failed: {err}").into())
        }),
        MqttPayloadFormat::BinaryLe | MqttPayloadFormat::BinaryBe => Ok(vec![u8::from(value)]),
    }
}

fn encode_numeric_payload(
    value: f64,
    format: MqttPayloadFormat,
    data_type: MqttPointType,
) -> Result<Vec<u8>, RuntimeError> {
    match format {
        MqttPayloadFormat::Text => Ok(format_numeric_value(value, data_type)?.into_bytes()),
        MqttPayloadFormat::Json => {
            let json = numeric_json_value(value, data_type)?;
            serde_json::to_vec(&json).map_err(|err| {
                RuntimeError::IoDriver(format!("mqtt numeric JSON encode failed: {err}").into())
            })
        }
        MqttPayloadFormat::BinaryLe | MqttPayloadFormat::BinaryBe => {
            encode_binary_numeric(value, format, data_type)
        }
    }
}

fn format_numeric_value(value: f64, data_type: MqttPointType) -> Result<String, RuntimeError> {
    match data_type {
        MqttPointType::U16 => Ok(checked_u16(value)?.to_string()),
        MqttPointType::I16 => Ok(checked_i16(value)?.to_string()),
        MqttPointType::U32 => Ok(checked_u32(value)?.to_string()),
        MqttPointType::I32 => Ok(checked_i32(value)?.to_string()),
        MqttPointType::F32 => Ok(checked_f32(value)?.to_string()),
        MqttPointType::Bool => Err(RuntimeError::IoDriver(
            "mqtt bool cannot be formatted as numeric".into(),
        )),
    }
}

fn numeric_json_value(
    value: f64,
    data_type: MqttPointType,
) -> Result<serde_json::Value, RuntimeError> {
    match data_type {
        MqttPointType::U16 => Ok(serde_json::Value::from(checked_u16(value)?)),
        MqttPointType::I16 => Ok(serde_json::Value::from(checked_i16(value)?)),
        MqttPointType::U32 => Ok(serde_json::Value::from(checked_u32(value)?)),
        MqttPointType::I32 => Ok(serde_json::Value::from(checked_i32(value)?)),
        MqttPointType::F32 => {
            let value = checked_f32(value)? as f64;
            serde_json::Number::from_f64(value)
                .map(serde_json::Value::Number)
                .ok_or_else(|| RuntimeError::IoDriver("mqtt f32 JSON value is not finite".into()))
        }
        MqttPointType::Bool => Err(RuntimeError::IoDriver(
            "mqtt bool cannot be encoded as numeric JSON".into(),
        )),
    }
}

fn encode_binary_numeric(
    value: f64,
    format: MqttPayloadFormat,
    data_type: MqttPointType,
) -> Result<Vec<u8>, RuntimeError> {
    match data_type {
        MqttPointType::U16 => {
            let value = checked_u16(value)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => value.to_le_bytes().to_vec(),
                MqttPayloadFormat::BinaryBe => value.to_be_bytes().to_vec(),
                _ => unreachable!(),
            })
        }
        MqttPointType::I16 => {
            let value = checked_i16(value)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => value.to_le_bytes().to_vec(),
                MqttPayloadFormat::BinaryBe => value.to_be_bytes().to_vec(),
                _ => unreachable!(),
            })
        }
        MqttPointType::U32 => {
            let value = checked_u32(value)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => value.to_le_bytes().to_vec(),
                MqttPayloadFormat::BinaryBe => value.to_be_bytes().to_vec(),
                _ => unreachable!(),
            })
        }
        MqttPointType::I32 => {
            let value = checked_i32(value)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => value.to_le_bytes().to_vec(),
                MqttPayloadFormat::BinaryBe => value.to_be_bytes().to_vec(),
                _ => unreachable!(),
            })
        }
        MqttPointType::F32 => {
            let value = checked_f32(value)?;
            Ok(match format {
                MqttPayloadFormat::BinaryLe => value.to_le_bytes().to_vec(),
                MqttPayloadFormat::BinaryBe => value.to_be_bytes().to_vec(),
                _ => unreachable!(),
            })
        }
        MqttPointType::Bool => Err(RuntimeError::IoDriver(
            "mqtt bool cannot be encoded as binary numeric".into(),
        )),
    }
}

fn write_image_bool(
    image: &mut [u8],
    offset: usize,
    bit: Option<u8>,
    value: bool,
) -> Result<(), RuntimeError> {
    let bit = bit.expect("bool point shape validates image_bit");
    let byte = image.get_mut(offset).ok_or_else(|| {
        RuntimeError::IoDriver(format!("mqtt point image_offset {offset} is outside input image").into())
    })?;
    let mask = 1u8 << bit;
    if value {
        *byte |= mask;
    } else {
        *byte &= !mask;
    }
    Ok(())
}

fn read_image_bool(image: &[u8], offset: usize, bit: Option<u8>) -> Result<bool, RuntimeError> {
    let bit = bit.expect("bool point shape validates image_bit");
    let byte = image.get(offset).ok_or_else(|| {
        RuntimeError::IoDriver(format!("mqtt point image_offset {offset} is outside output image").into())
    })?;
    Ok((*byte & (1u8 << bit)) != 0)
}

fn write_image_numeric(
    image: &mut [u8],
    offset: usize,
    data_type: MqttPointType,
    value: f64,
) -> Result<(), RuntimeError> {
    match data_type {
        MqttPointType::U16 => write_image_bytes(image, offset, &checked_u16(value)?.to_le_bytes()),
        MqttPointType::I16 => write_image_bytes(image, offset, &checked_i16(value)?.to_le_bytes()),
        MqttPointType::U32 => write_image_bytes(image, offset, &checked_u32(value)?.to_le_bytes()),
        MqttPointType::I32 => write_image_bytes(image, offset, &checked_i32(value)?.to_le_bytes()),
        MqttPointType::F32 => write_image_bytes(image, offset, &checked_f32(value)?.to_le_bytes()),
        MqttPointType::Bool => Err(RuntimeError::IoDriver(
            "mqtt bool cannot be written as numeric image bytes".into(),
        )),
    }
}

fn read_image_numeric(
    image: &[u8],
    offset: usize,
    data_type: MqttPointType,
) -> Result<f64, RuntimeError> {
    match data_type {
        MqttPointType::U16 => Ok(u16::from_le_bytes(read_image_bytes::<2>(image, offset)?) as f64),
        MqttPointType::I16 => Ok(i16::from_le_bytes(read_image_bytes::<2>(image, offset)?) as f64),
        MqttPointType::U32 => Ok(u32::from_le_bytes(read_image_bytes::<4>(image, offset)?) as f64),
        MqttPointType::I32 => Ok(i32::from_le_bytes(read_image_bytes::<4>(image, offset)?) as f64),
        MqttPointType::F32 => finite_f32_value(
            f32::from_le_bytes(read_image_bytes::<4>(image, offset)?),
            "mqtt F32 process-image value",
        ),
        MqttPointType::Bool => Err(RuntimeError::IoDriver(
            "mqtt bool cannot be read as numeric image bytes".into(),
        )),
    }
}

fn write_image_bytes(image: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), RuntimeError> {
    let end = offset
        .checked_add(bytes.len())
        .ok_or_else(|| RuntimeError::IoDriver("mqtt point image range overflow".into()))?;
    if end > image.len() {
        return Err(RuntimeError::IoDriver(
            format!(
                "mqtt point image range {offset}..{end} exceeds process image size {}",
                image.len()
            )
            .into(),
        ));
    }
    image[offset..end].copy_from_slice(bytes);
    Ok(())
}

fn read_image_bytes<const N: usize>(image: &[u8], offset: usize) -> Result<[u8; N], RuntimeError> {
    let end = offset
        .checked_add(N)
        .ok_or_else(|| RuntimeError::IoDriver("mqtt point image range overflow".into()))?;
    if end > image.len() {
        return Err(RuntimeError::IoDriver(
            format!(
                "mqtt point image range {offset}..{end} exceeds process image size {}",
                image.len()
            )
            .into(),
        ));
    }
    Ok(image[offset..end]
        .try_into()
        .expect("slice length is checked"))
}

fn checked_u16(value: f64) -> Result<u16, RuntimeError> {
    let value = checked_integer(value, 0.0, u16::MAX as f64, "u16")?;
    Ok(value as u16)
}

fn checked_i16(value: f64) -> Result<i16, RuntimeError> {
    let value = checked_integer(value, i16::MIN as f64, i16::MAX as f64, "i16")?;
    Ok(value as i16)
}

fn checked_u32(value: f64) -> Result<u32, RuntimeError> {
    let value = checked_integer(value, 0.0, u32::MAX as f64, "u32")?;
    Ok(value as u32)
}

fn checked_i32(value: f64) -> Result<i32, RuntimeError> {
    let value = checked_integer(value, i32::MIN as f64, i32::MAX as f64, "i32")?;
    Ok(value as i32)
}

fn checked_integer(value: f64, min: f64, max: f64, name: &str) -> Result<i64, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::IoDriver(
            format!("mqtt {name} value is not finite").into(),
        ));
    }
    let rounded = value.round();
    if rounded < min || rounded > max {
        return Err(RuntimeError::IoDriver(
            format!("mqtt {name} value {rounded} is out of range").into(),
        ));
    }
    Ok(rounded as i64)
}

fn checked_f32(value: f64) -> Result<f32, RuntimeError> {
    if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
        return Err(RuntimeError::IoDriver(
            format!("mqtt f32 value {value} is out of range").into(),
        ));
    }
    Ok(value as f32)
}

fn finite_f32_value(value: f32, label: &str) -> Result<f64, RuntimeError> {
    if value.is_finite() {
        Ok(f64::from(value))
    } else {
        Err(RuntimeError::IoDriver(format!("{label} must be finite").into()))
    }
}

#[cfg(test)]
mod point_map_nonfinite_tests {
    use super::*;

    #[test]
    fn mapped_f32_text_payload_rejects_non_finite() {
        let err = parse_numeric_payload(b"NaN", MqttPayloadFormat::Text, MqttPointType::F32)
            .expect_err("mapped MQTT F32 text NaN must be rejected");

        assert!(
            err.to_string().contains("finite"),
            "expected finite-value diagnostic, got {err}"
        );
    }

    #[test]
    fn mapped_f32_process_image_read_rejects_non_finite() {
        let image = f32::NEG_INFINITY.to_le_bytes();

        let err = read_image_numeric(&image, 0, MqttPointType::F32)
            .expect_err("mapped MQTT F32 image infinity must be rejected");

        assert!(
            err.to_string().contains("finite"),
            "expected finite-value diagnostic, got {err}"
        );
    }
}
