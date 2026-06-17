use serde::{Deserialize, Serialize};
use trust_runtime_core::value::{ArrayValue, Value};

/// IEC scalar data types supported by the ADS product surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum IecDataType {
    /// IEC `BOOL`.
    Bool,
    /// IEC `SINT`.
    Sint,
    /// IEC `INT`.
    Int,
    /// IEC `DINT`.
    Dint,
    /// IEC `LINT`.
    Lint,
    /// IEC `USINT`.
    Usint,
    /// IEC `UINT`.
    Uint,
    /// IEC `UDINT`.
    Udint,
    /// IEC `ULINT`.
    Ulint,
    /// IEC `REAL`.
    Real,
    /// IEC `LREAL`.
    Lreal,
    /// IEC `BYTE`.
    Byte,
    /// IEC `WORD`.
    Word,
    /// IEC `DWORD`.
    Dword,
    /// IEC `LWORD`.
    Lword,
    /// IEC `STRING(n)`.
    String,
}

/// One IEC array dimension, inclusive bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArrayDimension {
    /// Inclusive lower bound.
    pub lower: i64,
    /// Inclusive upper bound.
    pub upper: i64,
}

impl ArrayDimension {
    /// Returns the number of elements covered by this dimension.
    ///
    /// # Errors
    ///
    /// Returns [`AdsMappingError::InvalidArrayBounds`] when `upper < lower`.
    pub fn len(self) -> Result<usize, AdsMappingError> {
        if self.upper < self.lower {
            return Err(AdsMappingError::InvalidArrayBounds {
                lower: self.lower,
                upper: self.upper,
            });
        }
        let width = i128::from(self.upper) - i128::from(self.lower) + 1;
        usize::try_from(width).map_err(|_| AdsMappingError::ArrayTooLarge)
    }

    /// Returns whether the dimension is empty.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.upper < self.lower
    }
}

/// ADS type metadata needed to decode a scalar or scalar array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsDataTypeDescriptor {
    /// Source type name reported by the ADS endpoint, for example `REAL`.
    pub source_name: String,
    /// IEC scalar type used by truST.
    pub iec_type: IecDataType,
    /// Inclusive array dimensions. Empty means scalar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<ArrayDimension>,
    /// Declared character capacity for `STRING(n)`, excluding the terminator byte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string_len: Option<u16>,
}

impl AdsDataTypeDescriptor {
    /// Creates a scalar descriptor.
    #[must_use]
    pub fn scalar(source_name: impl Into<String>, iec_type: IecDataType) -> Self {
        Self {
            source_name: source_name.into(),
            iec_type,
            dimensions: Vec::new(),
            string_len: None,
        }
    }

    /// Creates a `STRING(n)` scalar descriptor.
    #[must_use]
    pub fn string(source_name: impl Into<String>, string_len: u16) -> Self {
        Self {
            source_name: source_name.into(),
            iec_type: IecDataType::String,
            dimensions: Vec::new(),
            string_len: Some(string_len),
        }
    }

    /// Returns the descriptor with array dimensions attached.
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: Vec<ArrayDimension>) -> Self {
        self.dimensions = dimensions;
        self
    }

    /// Returns true when this descriptor represents an array.
    #[must_use]
    pub fn is_array(&self) -> bool {
        !self.dimensions.is_empty()
    }

    /// Returns the number of values represented by the descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error when array bounds are invalid or overflow `usize`.
    pub fn element_count(&self) -> Result<usize, AdsMappingError> {
        if self.dimensions.is_empty() {
            return Ok(1);
        }
        self.dimensions.iter().try_fold(1usize, |acc, dimension| {
            acc.checked_mul(dimension.len()?)
                .ok_or(AdsMappingError::ArrayTooLarge)
        })
    }

    /// Returns the number of ADS bytes required by this descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error when string length is absent or array bounds are invalid.
    pub fn byte_len(&self) -> Result<usize, AdsMappingError> {
        self.scalar_byte_len()?
            .checked_mul(self.element_count()?)
            .ok_or(AdsMappingError::ArrayTooLarge)
    }

    fn scalar_byte_len(&self) -> Result<usize, AdsMappingError> {
        match self.iec_type {
            IecDataType::Bool | IecDataType::Sint | IecDataType::Usint | IecDataType::Byte => Ok(1),
            IecDataType::Int | IecDataType::Uint | IecDataType::Word => Ok(2),
            IecDataType::Dint | IecDataType::Udint | IecDataType::Real | IecDataType::Dword => {
                Ok(4)
            }
            IecDataType::Lint | IecDataType::Ulint | IecDataType::Lreal | IecDataType::Lword => {
                Ok(8)
            }
            IecDataType::String => self
                .string_len
                .map(|len| usize::from(len) + 1)
                .ok_or(AdsMappingError::MissingStringLength),
        }
    }
}

/// ADS value conversion error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdsMappingError {
    /// The ADS byte slice length does not match the descriptor.
    ByteLengthMismatch {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// A `STRING` descriptor did not include a declared length.
    MissingStringLength,
    /// A string value exceeds the declared ADS capacity.
    StringTooLong {
        /// Maximum bytes allowed before the terminator.
        max: usize,
        /// Actual UTF-8 byte length.
        actual: usize,
    },
    /// ADS bytes for `STRING` were not UTF-8.
    InvalidUtf8,
    /// A runtime value did not match the ADS descriptor.
    ValueTypeMismatch {
        /// Expected IEC type.
        expected: IecDataType,
        /// Actual runtime value kind.
        actual: &'static str,
    },
    /// A runtime array shape did not match the ADS descriptor.
    ArrayShapeMismatch {
        /// Expected dimensions.
        expected: Vec<ArrayDimension>,
        /// Actual dimensions.
        actual: Vec<(i64, i64)>,
    },
    /// Array bounds were invalid.
    InvalidArrayBounds {
        /// Inclusive lower bound.
        lower: i64,
        /// Inclusive upper bound.
        upper: i64,
    },
    /// Array element count overflowed the platform.
    ArrayTooLarge,
}

impl core::fmt::Display for AdsMappingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ByteLengthMismatch { expected, actual } => {
                write!(
                    f,
                    "ADS byte length mismatch: expected {expected}, got {actual}"
                )
            }
            Self::MissingStringLength => write!(f, "ADS STRING descriptor is missing length"),
            Self::StringTooLong { max, actual } => {
                write!(f, "ADS STRING too long: max {max} bytes, got {actual}")
            }
            Self::InvalidUtf8 => write!(f, "ADS STRING bytes are not valid UTF-8"),
            Self::ValueTypeMismatch { expected, actual } => {
                write!(
                    f,
                    "value type mismatch: expected {expected:?}, got {actual}"
                )
            }
            Self::ArrayShapeMismatch { expected, actual } => {
                write!(
                    f,
                    "array shape mismatch: expected {expected:?}, got {actual:?}"
                )
            }
            Self::InvalidArrayBounds { lower, upper } => {
                write!(f, "invalid array bounds {lower}..{upper}")
            }
            Self::ArrayTooLarge => write!(f, "array is too large"),
        }
    }
}

impl std::error::Error for AdsMappingError {}

/// Decodes ADS little-endian bytes into a runtime value.
///
/// # Errors
///
/// Returns an error when the byte slice does not match the descriptor or cannot
/// be decoded into the requested IEC type.
pub fn value_from_ads_bytes(
    descriptor: &AdsDataTypeDescriptor,
    bytes: &[u8],
) -> Result<Value, AdsMappingError> {
    let expected = descriptor.byte_len()?;
    if bytes.len() != expected {
        return Err(AdsMappingError::ByteLengthMismatch {
            expected,
            actual: bytes.len(),
        });
    }
    if descriptor.is_array() {
        decode_array(descriptor, bytes)
    } else {
        decode_scalar(descriptor, bytes)
    }
}

/// Encodes a runtime value into ADS little-endian bytes.
///
/// # Errors
///
/// Returns an error when the value does not match the descriptor or cannot fit
/// in the declared ADS type.
pub fn ads_bytes_from_value(
    descriptor: &AdsDataTypeDescriptor,
    value: &Value,
) -> Result<Vec<u8>, AdsMappingError> {
    if descriptor.is_array() {
        encode_array(descriptor, value)
    } else {
        encode_scalar(descriptor, value)
    }
}

fn decode_array(
    descriptor: &AdsDataTypeDescriptor,
    bytes: &[u8],
) -> Result<Value, AdsMappingError> {
    let scalar_width = descriptor.scalar_byte_len()?;
    let values = bytes
        .chunks_exact(scalar_width)
        .map(|chunk| {
            let scalar = AdsDataTypeDescriptor {
                source_name: descriptor.source_name.clone(),
                iec_type: descriptor.iec_type,
                dimensions: Vec::new(),
                string_len: descriptor.string_len,
            };
            decode_scalar(&scalar, chunk)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dimensions = descriptor
        .dimensions
        .iter()
        .map(|dimension| (dimension.lower, dimension.upper))
        .collect();
    Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
        values, dimensions,
    ))))
}

fn encode_array(
    descriptor: &AdsDataTypeDescriptor,
    value: &Value,
) -> Result<Vec<u8>, AdsMappingError> {
    let Value::Array(array) = value else {
        return Err(AdsMappingError::ValueTypeMismatch {
            expected: descriptor.iec_type,
            actual: value_kind(value),
        });
    };
    let expected = descriptor.dimensions.clone();
    let actual = array.dimensions().to_vec();
    if expected
        .iter()
        .map(|dimension| (dimension.lower, dimension.upper))
        .ne(actual.iter().copied())
    {
        return Err(AdsMappingError::ArrayShapeMismatch { expected, actual });
    }
    let scalar = AdsDataTypeDescriptor {
        source_name: descriptor.source_name.clone(),
        iec_type: descriptor.iec_type,
        dimensions: Vec::new(),
        string_len: descriptor.string_len,
    };
    let mut bytes = Vec::with_capacity(descriptor.byte_len()?);
    for element in array.elements() {
        bytes.extend(encode_scalar(&scalar, element)?);
    }
    Ok(bytes)
}

fn decode_scalar(
    descriptor: &AdsDataTypeDescriptor,
    bytes: &[u8],
) -> Result<Value, AdsMappingError> {
    Ok(match descriptor.iec_type {
        IecDataType::Bool => Value::Bool(bytes[0] != 0),
        IecDataType::Sint => Value::SInt(i8::from_le_bytes([bytes[0]])),
        IecDataType::Int => Value::Int(i16::from_le_bytes([bytes[0], bytes[1]])),
        IecDataType::Dint => {
            Value::DInt(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        IecDataType::Lint => Value::LInt(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        IecDataType::Usint => Value::USInt(bytes[0]),
        IecDataType::Uint => Value::UInt(u16::from_le_bytes([bytes[0], bytes[1]])),
        IecDataType::Udint => {
            Value::UDInt(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        IecDataType::Ulint => Value::ULInt(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        IecDataType::Real => {
            Value::Real(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        IecDataType::Lreal => Value::LReal(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        IecDataType::Byte => Value::Byte(bytes[0]),
        IecDataType::Word => Value::Word(u16::from_le_bytes([bytes[0], bytes[1]])),
        IecDataType::Dword => {
            Value::DWord(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        IecDataType::Lword => Value::LWord(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        IecDataType::String => decode_string(bytes)?,
    })
}

fn encode_scalar(
    descriptor: &AdsDataTypeDescriptor,
    value: &Value,
) -> Result<Vec<u8>, AdsMappingError> {
    match (descriptor.iec_type, value) {
        (IecDataType::Bool, Value::Bool(value)) => Ok(vec![u8::from(*value)]),
        (IecDataType::Sint, Value::SInt(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Int, Value::Int(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Dint, Value::DInt(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Lint, Value::LInt(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Usint, Value::USInt(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Uint, Value::UInt(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Udint, Value::UDInt(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Ulint, Value::ULInt(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Real, Value::Real(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Lreal, Value::LReal(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Byte, Value::Byte(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Word, Value::Word(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Dword, Value::DWord(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::Lword, Value::LWord(value)) => Ok(value.to_le_bytes().to_vec()),
        (IecDataType::String, Value::String(value)) => encode_string(
            usize::from(
                descriptor
                    .string_len
                    .ok_or(AdsMappingError::MissingStringLength)?,
            ),
            value.as_str(),
        ),
        (expected, actual) => Err(AdsMappingError::ValueTypeMismatch {
            expected,
            actual: value_kind(actual),
        }),
    }
}

fn decode_string(bytes: &[u8]) -> Result<Value, AdsMappingError> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let text = core::str::from_utf8(&bytes[..end]).map_err(|_| AdsMappingError::InvalidUtf8)?;
    Ok(Value::String(text.into()))
}

fn encode_string(max: usize, value: &str) -> Result<Vec<u8>, AdsMappingError> {
    if value.len() > max {
        return Err(AdsMappingError::StringTooLong {
            max,
            actual: value.len(),
        });
    }
    let mut bytes = vec![0; max + 1];
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    Ok(bytes)
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "BOOL",
        Value::SInt(_) => "SINT",
        Value::Int(_) => "INT",
        Value::DInt(_) => "DINT",
        Value::LInt(_) => "LINT",
        Value::USInt(_) => "USINT",
        Value::UInt(_) => "UINT",
        Value::UDInt(_) => "UDINT",
        Value::ULInt(_) => "ULINT",
        Value::Real(_) => "REAL",
        Value::LReal(_) => "LREAL",
        Value::Byte(_) => "BYTE",
        Value::Word(_) => "WORD",
        Value::DWord(_) => "DWORD",
        Value::LWord(_) => "LWORD",
        Value::String(_) => "STRING",
        Value::Array(_) => "ARRAY",
        _ => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_cases() -> Vec<(AdsDataTypeDescriptor, Value, Vec<u8>)> {
        vec![
            (
                AdsDataTypeDescriptor::scalar("BOOL", IecDataType::Bool),
                Value::Bool(true),
                vec![1],
            ),
            (
                AdsDataTypeDescriptor::scalar("SINT", IecDataType::Sint),
                Value::SInt(-5),
                (-5i8).to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("INT", IecDataType::Int),
                Value::Int(-1234),
                (-1234i16).to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("DINT", IecDataType::Dint),
                Value::DInt(-123_456),
                (-123_456i32).to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("LINT", IecDataType::Lint),
                Value::LInt(-123_456_789),
                (-123_456_789i64).to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("USINT", IecDataType::Usint),
                Value::USInt(7),
                vec![7],
            ),
            (
                AdsDataTypeDescriptor::scalar("UINT", IecDataType::Uint),
                Value::UInt(1234),
                1234u16.to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("UDINT", IecDataType::Udint),
                Value::UDInt(123_456),
                123_456u32.to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("ULINT", IecDataType::Ulint),
                Value::ULInt(123_456_789),
                123_456_789u64.to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                Value::Real(12.5),
                12.5f32.to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal),
                Value::LReal(12.5),
                12.5f64.to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("BYTE", IecDataType::Byte),
                Value::Byte(0x12),
                vec![0x12],
            ),
            (
                AdsDataTypeDescriptor::scalar("WORD", IecDataType::Word),
                Value::Word(0x1234),
                0x1234u16.to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("DWORD", IecDataType::Dword),
                Value::DWord(0x1234_5678),
                0x1234_5678u32.to_le_bytes().to_vec(),
            ),
            (
                AdsDataTypeDescriptor::scalar("LWORD", IecDataType::Lword),
                Value::LWord(0x1234_5678_9abc_def0),
                0x1234_5678_9abc_def0u64.to_le_bytes().to_vec(),
            ),
        ]
    }

    #[test]
    fn scalar_mapping_round_trips_every_supported_type() {
        for (descriptor, value, bytes) in scalar_cases() {
            assert_eq!(descriptor.byte_len(), Ok(bytes.len()));
            assert_eq!(value_from_ads_bytes(&descriptor, &bytes), Ok(value.clone()));
            assert_eq!(ads_bytes_from_value(&descriptor, &value), Ok(bytes));
        }
    }

    #[test]
    fn string_mapping_uses_declared_capacity_and_terminator() {
        let descriptor = AdsDataTypeDescriptor::string("STRING(8)", 8);
        let mut bytes = b"Pump".to_vec();
        bytes.resize(9, 0);

        assert_eq!(
            value_from_ads_bytes(&descriptor, &bytes),
            Ok(Value::String("Pump".into()))
        );
        assert_eq!(
            ads_bytes_from_value(&descriptor, &Value::String("Pump".into())),
            Ok(bytes)
        );
    }

    #[test]
    fn scalar_array_mapping_round_trips_dimensions() {
        let descriptor = AdsDataTypeDescriptor::scalar("INT", IecDataType::Int)
            .with_dimensions(vec![ArrayDimension { lower: 1, upper: 3 }]);
        let bytes = [1i16, 2, 3]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();
        let value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(1), Value::Int(2), Value::Int(3)],
            vec![(1, 3)],
        )));

        assert_eq!(value_from_ads_bytes(&descriptor, &bytes), Ok(value.clone()));
        assert_eq!(ads_bytes_from_value(&descriptor, &value), Ok(bytes));
    }

    #[test]
    fn rejects_shape_type_and_length_mismatches() {
        let descriptor = AdsDataTypeDescriptor::scalar("INT", IecDataType::Int)
            .with_dimensions(vec![ArrayDimension { lower: 1, upper: 2 }]);

        assert!(matches!(
            value_from_ads_bytes(&descriptor, &[0]),
            Err(AdsMappingError::ByteLengthMismatch { .. })
        ));
        assert!(matches!(
            ads_bytes_from_value(&descriptor, &Value::Int(1)),
            Err(AdsMappingError::ValueTypeMismatch { .. })
        ));
        let wrong_shape = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(1), Value::Int(2)],
            vec![(0, 1)],
        )));
        assert!(matches!(
            ads_bytes_from_value(&descriptor, &wrong_shape),
            Err(AdsMappingError::ArrayShapeMismatch { .. })
        ));
    }
}
