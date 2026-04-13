use std::sync::Arc;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::memory::InstanceId;

use super::{
    DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue, ValueRef,
};

/// Array value with bounds tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayValue {
    pub elements: Vec<Value>,
    pub dimensions: Vec<(i64, i64)>,
}

/// Struct value with named fields.
#[derive(Debug, Clone, PartialEq)]
pub struct StructValue {
    pub type_name: SmolStr,
    pub fields: IndexMap<SmolStr, Value>,
}

/// Enum value storing both name and numeric value.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumValue {
    pub type_name: SmolStr,
    pub variant_name: SmolStr,
    pub numeric_value: i64,
}

/// Runtime value representation for IEC 61131-3 types.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),

    SInt(i8),
    Int(i16),
    DInt(i32),
    LInt(i64),

    USInt(u8),
    UInt(u16),
    UDInt(u32),
    ULInt(u64),

    Real(f32),
    LReal(f64),

    Byte(u8),
    Word(u16),
    DWord(u32),
    LWord(u64),

    Time(Duration),
    LTime(Duration),
    Date(DateValue),
    LDate(LDateValue),
    Tod(TimeOfDayValue),
    LTod(LTimeOfDayValue),
    Dt(DateTimeValue),
    Ldt(LDateTimeValue),

    String(SmolStr),
    WString(String),
    Char(u8),
    WChar(u16),

    Array(Box<ArrayValue>),
    Struct(Arc<StructValue>),
    Enum(Box<EnumValue>),

    Reference(Option<ValueRef>),
    Instance(InstanceId),

    Null,
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Value::Int(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::DInt(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::LInt(value)
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Value::USInt(value)
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Value::UInt(value)
    }
}
