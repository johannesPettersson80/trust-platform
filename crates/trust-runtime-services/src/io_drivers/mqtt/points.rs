//! The typed points of the `mqtt` driver (specification 32 §6 and §7; specification 11, typed
//! protocol point-map integration contract): their types and payload formats, and the
//! `input_points` and `output_points` lists of the `params` table, each point's topic and storage
//! once in its list.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use smol_str::SmolStr;

use crate::io_drivers::{quoted, Once, ParamsError, Table};

/// What a topic must be (specification 32 §7), as a refusal states it.
pub(super) const EXACT_TOPIC: &str = "an exact MQTT topic: not empty, at most 65535 bytes, \
                                      without the wildcards + and # and without control characters";

/// The scalar types, as a refusal lists them.
const TYPES: &str = "bool, u16, i16, u32, i32 or f32, or an alias: boolean, uint16, word, int16, \
                     int, uint32, dword, int32, dint, float or real";

/// The payload formats, as a refusal lists them.
const FORMATS: &str = "text, json, binary_le or binary_be, or an alias: string, little_endian, \
                       le, big_endian or be";

/// The rule a refusal of a topic used twice in one list states.
const TOPIC_ONCE: &str = "each point of a list has a topic of its own";

/// The rule a refusal of overlapping storage in one list states.
const STORAGE_ONCE: &str = "the points of a list use storage of their own";

/// The scalar type of a typed point (specification 32 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointType {
    /// `bool` or `boolean`: one bit of the image.
    Bool,
    /// `u16`, `uint16` or `word`.
    U16,
    /// `i16`, `int16` or `int`.
    I16,
    /// `u32`, `uint32` or `dword`.
    U32,
    /// `i32`, `int32` or `dint`.
    I32,
    /// `f32`, `float` or `real`.
    F32,
}

impl PointType {
    /// The type that the word `alias` names, trimmed and in lower case.
    fn from_alias(alias: &str) -> Option<Self> {
        Some(match alias {
            "bool" | "boolean" => Self::Bool,
            "u16" | "uint16" | "word" => Self::U16,
            "i16" | "int16" | "int" => Self::I16,
            "u32" | "uint32" | "dword" => Self::U32,
            "i32" | "int32" | "dint" => Self::I32,
            "f32" | "float" | "real" => Self::F32,
            _ => return None,
        })
    }

    /// The type's name, as `data_type` writes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::U16 => "u16",
            Self::I16 => "i16",
            Self::U32 => "u32",
            Self::I32 => "i32",
            Self::F32 => "f32",
        }
    }

    /// The bytes of the image that a value of the type uses: the byte of a bool's bit, 2 or 4.
    #[must_use]
    pub fn image_bytes(self) -> u32 {
        match self {
            Self::Bool => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
        }
    }

    /// The bits of the image that a value of the type takes: 1, 16 or 32.
    fn bits(self) -> u64 {
        match self {
            Self::Bool => 1,
            Self::U16 | Self::I16 => 16,
            Self::U32 | Self::I32 | Self::F32 => 32,
        }
    }
}

/// The format of a typed point's payload (specification 32 §7); `text` when `payload_format` is
/// absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadFormat {
    /// `text` or `string`: the value as text.
    Text,
    /// `json`: a JSON value.
    Json,
    /// `binary_le`, `little_endian` or `le`: the value's bytes, least significant first.
    BinaryLe,
    /// `binary_be`, `big_endian` or `be`: the value's bytes, most significant first.
    BinaryBe,
}

/// A typed input point: the payloads of `topic` into the input image.
#[derive(Debug, Clone, PartialEq)]
pub struct InputPoint {
    /// `topic`, trimmed: an exact topic.
    pub topic: SmolStr,
    /// `image_offset`: the byte of the input image where the value starts.
    pub image_offset: u32,
    /// `image_bit`, 0 to 7: the bit of a bool point; `None` for the other types.
    pub image_bit: Option<u8>,
    /// `data_type`.
    pub data_type: PointType,
    /// `payload_format`.
    pub payload_format: PayloadFormat,
    /// `scale`, finite and not 0: `engineering = raw × scale + offset`; 1 when absent.
    pub scale: f64,
    /// `offset`, finite; 0 when absent.
    pub offset: f64,
}

/// A typed output point: the value of the output image published to `topic`.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputPoint {
    /// `topic`, trimmed: an exact topic.
    pub topic: SmolStr,
    /// `metric_name`, trimmed: the point's Sparkplug B metric; the topic when absent.
    pub metric_name: SmolStr,
    /// `image_offset`: the byte of the output image where the value starts.
    pub image_offset: u32,
    /// `image_bit`, 0 to 7: the bit of a bool point; `None` for the other types.
    pub image_bit: Option<u8>,
    /// `data_type`.
    pub data_type: PointType,
    /// `payload_format`.
    pub payload_format: PayloadFormat,
    /// `scale`, finite and not 0: `raw = (engineering − offset) ÷ scale`; 1 when absent.
    pub scale: f64,
    /// `offset`, finite; 0 when absent.
    pub offset: f64,
}

/// The fields that input and output points share, read from one entry.
struct Shape {
    topic: SmolStr,
    image_offset: u32,
    image_bit: Option<u8>,
    data_type: PointType,
    payload_format: PayloadFormat,
    scale: f64,
    offset: f64,
}

/// The `input_points` list: each entry's keys checked, its topic and its storage once in the list.
pub(super) fn input_points(root: &Table<'_>) -> Result<Vec<InputPoint>, ParamsError> {
    let mut claims = Claims::new('I');
    let mut points = Vec::new();
    for entry in root.list("input_points")? {
        entry.only(
            &[
                "data_type",
                "image_bit",
                "image_offset",
                "offset",
                "payload_format",
                "scale",
                "topic",
            ],
            "an input point",
        )?;
        let shape = shape(&entry)?;
        claims.claim(&entry, &shape)?;
        points.push(InputPoint {
            topic: shape.topic,
            image_offset: shape.image_offset,
            image_bit: shape.image_bit,
            data_type: shape.data_type,
            payload_format: shape.payload_format,
            scale: shape.scale,
            offset: shape.offset,
        });
    }
    Ok(points)
}

/// The `output_points` list, read as [`input_points`] reads its list, with each point's
/// `metric_name`.
pub(super) fn output_points(root: &Table<'_>) -> Result<Vec<OutputPoint>, ParamsError> {
    let mut claims = Claims::new('Q');
    let mut points = Vec::new();
    for entry in root.list("output_points")? {
        entry.only(
            &[
                "data_type",
                "image_bit",
                "image_offset",
                "metric_name",
                "offset",
                "payload_format",
                "scale",
                "topic",
            ],
            "an output point",
        )?;
        let shape = shape(&entry)?;
        claims.claim(&entry, &shape)?;
        let metric_name = match entry.get("metric_name") {
            None => shape.topic.clone(),
            Some(value) => value
                .as_str()
                .map(str::trim)
                .filter(|name| !name.is_empty() && !name.chars().any(char::is_control))
                .map(SmolStr::new)
                .ok_or_else(|| {
                    entry.refuse(
                        "metric_name",
                        format!("must be a non-empty name without control characters, not {value}"),
                    )
                })?,
        };
        points.push(OutputPoint {
            topic: shape.topic,
            metric_name,
            image_offset: shape.image_offset,
            image_bit: shape.image_bit,
            data_type: shape.data_type,
            payload_format: shape.payload_format,
            scale: shape.scale,
            offset: shape.offset,
        });
    }
    Ok(points)
}

/// The topic `key` gives, trimmed, when the key is given: an exact topic (specification 32 §7).
pub(super) fn exact_topic(table: &Table<'_>, key: &str) -> Result<Option<SmolStr>, ParamsError> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    value
        .as_str()
        .map(str::trim)
        .filter(|topic| is_exact_topic(topic))
        .map(|topic| Some(SmolStr::new(topic)))
        .ok_or_else(|| table.refuse(key, format!("must be {EXACT_TOPIC}, not {value}")))
}

/// Whether `topic` is an exact topic: not empty, at most the 65,535 bytes of an MQTT string, and
/// without wildcards and control characters.
fn is_exact_topic(topic: &str) -> bool {
    !topic.is_empty()
        && topic.len() <= 65_535
        && !topic.contains(['+', '#'])
        && !topic.chars().any(char::is_control)
}

/// The fields of the point `entry` that input and output points share.
fn shape(entry: &Table<'_>) -> Result<Shape, ParamsError> {
    let topic = exact_topic(entry, "topic")?;
    let topic = entry.required("topic", EXACT_TOPIC, topic)?;
    let image_offset = entry.required_integer("image_offset", 0_u32, u32::MAX)?;
    let data_type = data_type(entry)?;
    let image_bit = entry.integer("image_bit", 0_u8, 7)?;
    match (data_type, image_bit) {
        (PointType::Bool, None) => {
            return Err(entry.refuse(
                "image_bit",
                "is required for a bool point, the bit from 0 to 7",
            ));
        }
        (PointType::Bool, Some(_)) | (_, None) => {}
        (other, Some(_)) => {
            return Err(entry.refuse(
                "image_bit",
                format!("is only for bool points, not for a {} point", other.name()),
            ));
        }
    }
    let payload_format = payload_format(entry)?;
    let (scale, offset) = scaling(entry, data_type)?;
    Ok(Shape {
        topic,
        image_offset,
        image_bit,
        data_type,
        payload_format,
        scale,
        offset,
    })
}

/// `data_type`: one of [`PointType`]'s words or aliases, trimmed and in any case; required.
fn data_type(entry: &Table<'_>) -> Result<PointType, ParamsError> {
    let Some(value) = entry.get("data_type") else {
        return Err(entry.refuse("data_type", format!("is required, {TYPES}")));
    };
    value
        .as_str()
        .and_then(|word| PointType::from_alias(&word.trim().to_ascii_lowercase()))
        .ok_or_else(|| entry.refuse("data_type", format!("must be {TYPES}, not {value}")))
}

/// `payload_format`: one of [`PayloadFormat`]'s words or aliases, trimmed, in any case and with
/// `-` read as `_`; `text` when absent.
fn payload_format(entry: &Table<'_>) -> Result<PayloadFormat, ParamsError> {
    let Some(value) = entry.get("payload_format") else {
        return Ok(PayloadFormat::Text);
    };
    let word = value
        .as_str()
        .map(|word| word.trim().to_ascii_lowercase().replace('-', "_"));
    let format = match word.as_deref() {
        Some("text" | "string") => PayloadFormat::Text,
        Some("json") => PayloadFormat::Json,
        Some("binary_le" | "little_endian" | "le") => PayloadFormat::BinaryLe,
        Some("binary_be" | "big_endian" | "be") => PayloadFormat::BinaryBe,
        _ => {
            return Err(entry.refuse("payload_format", format!("must be {FORMATS}, not {value}")));
        }
    };
    Ok(format)
}

/// `scale` and `offset`: a finite scale other than 0 and a finite offset, 1 and 0 when absent; a
/// bool point takes neither (specification 32 §7).
fn scaling(entry: &Table<'_>, data_type: PointType) -> Result<(f64, f64), ParamsError> {
    if data_type == PointType::Bool {
        for key in ["scale", "offset"] {
            if entry.get(key).is_some() {
                return Err(entry.refuse(key, "is not taken by a bool point"));
            }
        }
        return Ok((1.0, 0.0));
    }
    let scale = match entry.get("scale") {
        None => 1.0,
        Some(value) => value
            .as_f64()
            .filter(|scale| scale.is_finite() && *scale != 0.0)
            .ok_or_else(|| {
                entry.refuse(
                    "scale",
                    format!("must be a finite number other than 0, not {value}"),
                )
            })?,
    };
    let offset = match entry.get("offset") {
        None => 0.0,
        Some(value) => value
            .as_f64()
            .filter(|offset| offset.is_finite())
            .ok_or_else(|| {
                entry.refuse("offset", format!("must be a finite number, not {value}"))
            })?,
    };
    Ok((scale, offset))
}

/// The topics and the storage that the points of one list use, each with the key that uses it.
struct Claims {
    /// `I` for input points, `Q` for output points.
    area: char,
    topics: Once<SmolStr>,
    /// The bits of the image each point uses, with the point's key and its storage in words.
    storage: Vec<(Range<u64>, String, String)>,
}

impl Claims {
    /// Nothing used yet in the image `area`.
    fn new(area: char) -> Self {
        Self {
            area,
            topics: Once::new(TOPIC_ONCE),
            storage: Vec::new(),
        }
    }

    /// Records the topic and the storage of the point `entry`; refused when an earlier point of
    /// the list uses the topic or one of the bits (specification 32 §7).
    fn claim(&mut self, entry: &Table<'_>, shape: &Shape) -> Result<(), ParamsError> {
        self.topics.claim(
            shape.topic.clone(),
            entry.key("topic"),
            &quoted(&shape.topic),
        )?;
        let start = u64::from(shape.image_offset) * 8 + u64::from(shape.image_bit.unwrap_or(0));
        let bits = start..start + shape.data_type.bits();
        let text = storage_text(self.area, shape);
        if let Some((_, key, earlier)) = self
            .storage
            .iter()
            .find(|(used, _, _)| used.start < bits.end && bits.start < used.end)
        {
            return Err(entry.refuse(
                "image_offset",
                format!("{text} overlaps {earlier} of {key}; {STORAGE_ONCE}"),
            ));
        }
        self.storage.push((bits, entry.path.clone(), text));
        Ok(())
    }
}

/// A point's storage as an IEC 61131-3 address of the flat image: `%IX0.3` for a bool, `%IW2` for
/// a 16-bit value and `%ID4` for a 32-bit one, the number being the byte (specification 11 §6.5).
fn storage_text(area: char, shape: &Shape) -> String {
    let byte = shape.image_offset;
    match shape.data_type {
        PointType::Bool => format!("%{area}X{byte}.{}", shape.image_bit.unwrap_or(0)),
        PointType::U16 | PointType::I16 => format!("%{area}W{byte}"),
        PointType::U32 | PointType::I32 | PointType::F32 => format!("%{area}D{byte}"),
    }
}
