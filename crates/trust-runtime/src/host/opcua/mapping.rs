#[must_use]
pub fn map_iec_value(value: &Value) -> Option<OpcUaValue> {
    match value {
        Value::Bool(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::Boolean,
            value: OpcUaVariant::Boolean(*value),
        }),
        Value::Int(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::Int16,
            value: OpcUaVariant::Int16(*value),
        }),
        Value::DInt(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::Int32,
            value: OpcUaVariant::Int32(*value),
        }),
        Value::LInt(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::Int64,
            value: OpcUaVariant::Int64(*value),
        }),
        Value::UInt(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::UInt16,
            value: OpcUaVariant::UInt16(*value),
        }),
        Value::UDInt(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::UInt32,
            value: OpcUaVariant::UInt32(*value),
        }),
        Value::ULInt(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::UInt64,
            value: OpcUaVariant::UInt64(*value),
        }),
        Value::Real(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::Float,
            value: OpcUaVariant::Float(*value),
        }),
        Value::LReal(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::Double,
            value: OpcUaVariant::Double(*value),
        }),
        Value::String(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::String,
            value: OpcUaVariant::String(value.to_string()),
        }),
        Value::WString(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::String,
            value: OpcUaVariant::String(value.clone()),
        }),
        Value::Char(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::String,
            value: OpcUaVariant::String(char::from(*value).to_string()),
        }),
        Value::WChar(value) => char::from_u32(u32::from(*value)).map(|ch| OpcUaValue {
            data_type: OpcUaDataType::String,
            value: OpcUaVariant::String(ch.to_string()),
        }),
        Value::SInt(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::Int16,
            value: OpcUaVariant::Int16(i16::from(*value)),
        }),
        Value::USInt(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::UInt16,
            value: OpcUaVariant::UInt16(u16::from(*value)),
        }),
        Value::Byte(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::UInt16,
            value: OpcUaVariant::UInt16(u16::from(*value)),
        }),
        Value::Word(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::UInt16,
            value: OpcUaVariant::UInt16(*value),
        }),
        Value::DWord(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::UInt32,
            value: OpcUaVariant::UInt32(*value),
        }),
        Value::LWord(value) => Some(OpcUaValue {
            data_type: OpcUaDataType::UInt64,
            value: OpcUaVariant::UInt64(*value),
        }),
        _ => None,
    }
}
