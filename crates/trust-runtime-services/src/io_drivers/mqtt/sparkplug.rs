//! The Sparkplug B edge node profile of the `mqtt` driver (specification 32 §9; Eclipse Sparkplug
//! 3.0.0, namespace `spBv1.0`): its `sparkplug` table, its topics, the protobuf payloads of NBIRTH,
//! NDEATH and NDATA, written by hand for the fields they use, and the one command it takes from
//! NCMD, a rebirth request.
//!
//! A payload is a Sparkplug B `Payload` message: field 1 the timestamp, field 2 each metric and
//! field 3 the sequence number, which an NDEATH leaves out. A metric carries field 1 its name,
//! field 3 the timestamp, field 4 its data type and one value field: 10 `int_value` for the 16- and
//! 32-bit integers (a signed value as its two's complement in 32 bits), 11 `long_value`, 12
//! `float_value` and 14 `boolean_value`; or field 7 `is_null` for a metric of an NBIRTH whose value
//! is not known yet.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;
use smol_str::SmolStr;

use super::payload;
use super::points::{InputPoint, OutputPoint, PointType};
use crate::io_drivers::{quoted, ParamsError, Table};

/// The Sparkplug B namespace, the first level of every topic.
pub(super) const NAMESPACE: &str = "spBv1.0";

/// The Sparkplug B version of the profile.
const SPEC_VERSION: &str = "3.0.0";

/// The data type `Int16` of a metric.
const INT16: u32 = 2;
/// The data type `Int32`.
const INT32: u32 = 3;
/// The data type `Int64`, of `bdSeq`
/// (`tck-id-message-flow-edge-node-birth-publish-will-message-payload-bdSeq`).
const INT64: u32 = 4;
/// The data type `UInt16`.
const UINT16: u32 = 6;
/// The data type `UInt32`.
const UINT32: u32 = 7;
/// The data type `Float`.
const FLOAT: u32 = 9;
/// The data type `Boolean`.
const BOOLEAN: u32 = 11;

/// What `group_id` and `edge_node_id` must be.
const TOKEN: &str = "a non-empty MQTT topic level without /, +, # and control characters";

/// The metric of an NBIRTH, and of an NCMD, that asks the node for a new birth
/// (`tck-id-operational-behavior-data-commands-rebirth-name`).
const REBIRTH: &str = "Node Control/Rebirth";

/// The largest `bdSeq`: it counts the connections from 0 to 255 and then from 0 again.
const MAX_BD_SEQ: u64 = 255;

/// The `sparkplug` table when its profile is enabled (specification 32 §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparkplugParams {
    /// `group_id`, trimmed: the topic level of the node's group; required.
    pub group_id: SmolStr,
    /// `edge_node_id`, trimmed: the topic level of the node; required.
    pub edge_node_id: SmolStr,
    /// `birth_death_seq`, 0 to 255: the `bdSeq` of the first connection, which each new
    /// connection counts on by one, from 255 to 0; 0 when absent.
    pub birth_death_seq: u64,
}

impl SparkplugParams {
    /// `spBv1.0/<group_id>/NBIRTH/<edge_node_id>`.
    #[must_use]
    pub fn nbirth_topic(&self) -> String {
        self.topic("NBIRTH")
    }

    /// `spBv1.0/<group_id>/NDATA/<edge_node_id>`.
    #[must_use]
    pub fn ndata_topic(&self) -> String {
        self.topic("NDATA")
    }

    /// `spBv1.0/<group_id>/NDEATH/<edge_node_id>`, the topic of the will.
    #[must_use]
    pub fn ndeath_topic(&self) -> String {
        self.topic("NDEATH")
    }

    /// `spBv1.0/<group_id>/NCMD/<edge_node_id>`, the topic of the commands to the node.
    #[must_use]
    pub fn ncmd_topic(&self) -> String {
        self.topic("NCMD")
    }

    /// The node's topic of the message type `kind`.
    fn topic(&self, kind: &str) -> String {
        format!("{NAMESPACE}/{}/{kind}/{}", self.group_id, self.edge_node_id)
    }
}

/// The `sparkplug` table: `None` when it is absent or `enabled = false`. Every key is checked for
/// its type first, as trust-runtime's driver reads them; the values only when the profile is on.
pub(super) fn params(root: &Table<'_>) -> Result<Option<SparkplugParams>, ParamsError> {
    let Some(value) = root.get("sparkplug") else {
        return Ok(None);
    };
    let Value::Object(map) = value else {
        return Err(root.refuse("sparkplug", format!("must be a table, not {value}")));
    };
    let table = Table {
        path: root.key("sparkplug"),
        map,
    };
    table.only(
        &[
            "birth_death_seq",
            "edge_node_id",
            "enabled",
            "group_id",
            "namespace",
            "spec_version",
        ],
        "the sparkplug table",
    )?;
    let enabled = table.boolean("enabled")?;
    let namespace = string(&table, "namespace")?;
    let spec_version = string(&table, "spec_version")?;
    let group_id = string(&table, "group_id")?;
    let edge_node_id = string(&table, "edge_node_id")?;
    let birth_death_seq = table
        .integer("birth_death_seq", 0_u64, MAX_BD_SEQ)?
        .unwrap_or(0);
    if enabled == Some(false) {
        return Ok(None);
    }
    fixed(&table, "namespace", namespace, NAMESPACE)?;
    fixed(&table, "spec_version", spec_version, SPEC_VERSION)?;
    Ok(Some(SparkplugParams {
        group_id: token(&table, "group_id", group_id)?,
        edge_node_id: token(&table, "edge_node_id", edge_node_id)?,
        birth_death_seq,
    }))
}

/// Refuses input points and requires an output point with the profile, which publishes the output
/// points as the metrics of one node (specification 32 §9).
pub(super) fn check_profile(
    root: &Table<'_>,
    inputs: &[InputPoint],
    outputs: &[OutputPoint],
) -> Result<(), ParamsError> {
    if !inputs.is_empty() {
        return Err(root.refuse(
            "input_points",
            "is not taken with Sparkplug, whose profile publishes output points only",
        ));
    }
    if outputs.is_empty() {
        return Err(root.refuse(
            "output_points",
            "is required with Sparkplug, at least one output point for a metric",
        ));
    }
    Ok(())
}

/// The string `key` gives, when the key is given; any string.
fn string<'a>(table: &Table<'a>, key: &str) -> Result<Option<&'a str>, ParamsError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.as_str())),
        Some(other) => Err(table.refuse(key, format!("must be a string, not {other}"))),
    }
}

/// Refuses a `key` that, trimmed, is not `expected`, its only value; an absent key is `expected`.
fn fixed(
    table: &Table<'_>,
    key: &str,
    given: Option<&str>,
    expected: &str,
) -> Result<(), ParamsError> {
    let given = given.map_or(expected, str::trim);
    if given == expected {
        Ok(())
    } else {
        Err(table.refuse(
            key,
            format!("must be {}, not {}", quoted(expected), quoted(given)),
        ))
    }
}

/// The topic level `given` for `key`, trimmed; required.
fn token(table: &Table<'_>, key: &str, given: Option<&str>) -> Result<SmolStr, ParamsError> {
    let Some(given) = given else {
        return Err(table.refuse(key, format!("is required, {TOKEN}")));
    };
    let level = given.trim();
    if level.is_empty() || level.contains(['/', '+', '#']) || level.chars().any(char::is_control) {
        return Err(table.refuse(key, format!("must be {TOKEN}, not {}", quoted(given))));
    }
    Ok(SmolStr::new(level))
}

/// One metric of a payload.
struct Metric<'a> {
    name: &'a str,
    datatype: u32,
    /// `None` for a metric of an NBIRTH whose value is not known yet, written as `is_null`.
    value: Option<MetricValue>,
}

/// The value of a metric, in the field of its kind.
#[derive(Clone, Copy)]
enum MetricValue {
    Int(u32),
    Long(u64),
    Float(f32),
    Boolean(bool),
}

/// The NBIRTH payload at `seq` and `timestamp` of the session with `bd_seq`: the `bdSeq` metric,
/// the `Node Control/Rebirth` metric with false, then each output point's metric with its name,
/// data type and value in `image`, the newest output image, or `is_null` without an image or for a
/// value that does not convert.
pub(super) fn encode_nbirth(
    points: &[OutputPoint],
    image: Option<&[u8]>,
    bd_seq: u64,
    seq: u64,
    timestamp: u64,
) -> Vec<u8> {
    let mut metrics = Vec::with_capacity(points.len().saturating_add(2));
    metrics.push(bd_seq_metric(bd_seq));
    metrics.push(Metric {
        name: REBIRTH,
        datatype: BOOLEAN,
        value: Some(MetricValue::Boolean(false)),
    });
    metrics.extend(points.iter().map(|point| Metric {
        name: &point.metric_name,
        datatype: datatype(point.data_type),
        value: image.and_then(|image| metric_value(point, image).ok()),
    }));
    encode_payload(timestamp, Some(seq), &metrics)
}

/// The NDEATH payload at `timestamp` of the session with `bd_seq`, the will: the `bdSeq` metric
/// alone, without a sequence number (`tck-id-payloads-ndeath-seq`).
pub(super) fn encode_ndeath(bd_seq: u64, timestamp: u64) -> Vec<u8> {
    encode_payload(timestamp, None, &[bd_seq_metric(bd_seq)])
}

/// The NDATA payload of the output image `image` at `seq` and `timestamp`: each output point's
/// metric with its value, converted as its typed payload is (specification 32 §7).
pub(super) fn encode_ndata(
    points: &[OutputPoint],
    image: &[u8],
    seq: u64,
    timestamp: u64,
) -> Result<Vec<u8>, String> {
    let metrics = points
        .iter()
        .map(|point| {
            Ok(Metric {
                name: &point.metric_name,
                datatype: datatype(point.data_type),
                value: Some(metric_value(point, image)?),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(encode_payload(timestamp, Some(seq), &metrics))
}

/// The `bdSeq` metric: `Int64`, the session's `bd_seq`.
fn bd_seq_metric(bd_seq: u64) -> Metric<'static> {
    Metric {
        name: "bdSeq",
        datatype: INT64,
        value: Some(MetricValue::Long(bd_seq)),
    }
}

/// The value of the output point `point` in `image`: a bool's bit, or the raw value of a number,
/// its engineering value less its offset divided by its scale, converted to its type.
fn metric_value(point: &OutputPoint, image: &[u8]) -> Result<MetricValue, String> {
    let raw = || payload::scaled_raw(point, image);
    Ok(match point.data_type {
        PointType::Bool => MetricValue::Boolean(payload::read_bit(
            image,
            point.image_offset,
            point.image_bit,
        )?),
        PointType::U16 => MetricValue::Int(u32::from(payload::checked_u16(raw()?)?)),
        PointType::I16 => {
            MetricValue::Int(i32::from(payload::checked_i16(raw()?)?).cast_unsigned())
        }
        PointType::U32 => MetricValue::Int(payload::checked_u32(raw()?)?),
        PointType::I32 => MetricValue::Int(payload::checked_i32(raw()?)?.cast_unsigned()),
        PointType::F32 => MetricValue::Float(payload::checked_f32(raw()?)?),
    })
}

/// The data type of a metric of `data_type`.
fn datatype(data_type: PointType) -> u32 {
    match data_type {
        PointType::Bool => BOOLEAN,
        PointType::U16 => UINT16,
        PointType::I16 => INT16,
        PointType::U32 => UINT32,
        PointType::I32 => INT32,
        PointType::F32 => FLOAT,
    }
}

/// A `Payload` of `timestamp`, `metrics` and `seq` when there is one, each metric with
/// `timestamp` too.
fn encode_payload(timestamp: u64, seq: Option<u64>, metrics: &[Metric<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    varint_field(&mut out, 1, timestamp);
    for metric in metrics {
        let mut bytes = Vec::new();
        bytes_field(&mut bytes, 1, metric.name.as_bytes());
        varint_field(&mut bytes, 3, timestamp);
        varint_field(&mut bytes, 4, u64::from(metric.datatype));
        match metric.value {
            Some(MetricValue::Int(value)) => varint_field(&mut bytes, 10, u64::from(value)),
            Some(MetricValue::Long(value)) => varint_field(&mut bytes, 11, value),
            Some(MetricValue::Float(value)) => fixed32_field(&mut bytes, 12, value.to_bits()),
            Some(MetricValue::Boolean(value)) => varint_field(&mut bytes, 14, u64::from(value)),
            None => varint_field(&mut bytes, 7, 1),
        }
        bytes_field(&mut out, 2, &bytes);
    }
    if let Some(seq) = seq {
        varint_field(&mut out, 3, seq);
    }
    out
}

/// A varint field: the key with wire type 0, then the value.
fn varint_field(out: &mut Vec<u8>, field: u32, value: u64) {
    varint(out, u64::from(field << 3));
    varint(out, value);
}

/// A length-delimited field: the key with wire type 2, the length and the bytes.
fn bytes_field(out: &mut Vec<u8>, field: u32, value: &[u8]) {
    varint(out, u64::from((field << 3) | 2));
    varint(out, u64::try_from(value.len()).unwrap_or(u64::MAX));
    out.extend_from_slice(value);
}

/// A 32-bit field: the key with wire type 5, then the four bytes, least significant first.
fn fixed32_field(out: &mut Vec<u8>, field: u32, value: u32) {
    varint(out, u64::from((field << 3) | 5));
    out.extend_from_slice(&value.to_le_bytes());
}

/// `value` in base 128, seven bits a byte, least significant first, the top bit set on every byte
/// but the last.
fn varint(out: &mut Vec<u8>, value: u64) {
    let mut rest = value;
    while rest >= 0x80 {
        out.push(rest.to_le_bytes()[0] | 0x80);
        rest >>= 7;
    }
    out.push(rest.to_le_bytes()[0]);
}

/// Whether the NCMD payload `payload` asks the node for a rebirth: a metric named
/// `Node Control/Rebirth` with the Boolean value true. A payload that does not parse asks for
/// nothing.
pub(super) fn is_rebirth_request(payload: &[u8]) -> bool {
    for field in Fields(payload) {
        match field {
            Ok((2, Field::Bytes(metric))) if is_rebirth_metric(metric) => return true,
            Ok(_) => {}
            Err(Malformed) => return false,
        }
    }
    false
}

/// Whether the `Metric` message `metric` is `Node Control/Rebirth` with true.
fn is_rebirth_metric(metric: &[u8]) -> bool {
    let mut name = None;
    let mut value = None;
    for field in Fields(metric) {
        match field {
            Ok((1, Field::Bytes(bytes))) => name = Some(bytes),
            Ok((14, Field::Varint(flag))) => value = Some(flag != 0),
            Ok(_) => {}
            Err(Malformed) => return false,
        }
    }
    name == Some(REBIRTH.as_bytes()) && value == Some(true)
}

/// A protobuf message whose bytes end inside a field, or with a wire type a Sparkplug payload
/// does not use.
struct Malformed;

/// The value of a protobuf field: a varint, the bytes of a length-delimited field, or a fixed
/// 32- or 64-bit value, which the rebirth request does not read.
enum Field<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed,
}

/// The fields of a protobuf message, each with its number.
struct Fields<'a>(&'a [u8]);

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u64, Field<'a>), Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        let field = self.field();
        if field.is_err() {
            self.0 = &[];
        }
        Some(field)
    }
}

impl<'a> Fields<'a> {
    /// The next field: its key, the field number times 8 plus the wire type, then its value.
    fn field(&mut self) -> Result<(u64, Field<'a>), Malformed> {
        let key = self.varint()?;
        let value = match key & 7 {
            0 => Field::Varint(self.varint()?),
            1 => {
                self.take(8)?;
                Field::Fixed
            }
            2 => {
                let len = usize::try_from(self.varint()?).map_err(|_| Malformed)?;
                Field::Bytes(self.take(len)?)
            }
            5 => {
                self.take(4)?;
                Field::Fixed
            }
            _ => return Err(Malformed),
        };
        Ok((key >> 3, value))
    }

    /// A varint: seven bits a byte, least significant first, in at most ten bytes.
    fn varint(&mut self) -> Result<u64, Malformed> {
        let mut value = 0_u64;
        for shift in (0..64).step_by(7) {
            let (&byte, rest) = self.0.split_first().ok_or(Malformed)?;
            self.0 = rest;
            value |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(Malformed)
    }

    /// The next `len` bytes.
    fn take(&mut self, len: usize) -> Result<&'a [u8], Malformed> {
        if len > self.0.len() {
            return Err(Malformed);
        }
        let (taken, rest) = self.0.split_at(len);
        self.0 = rest;
        Ok(taken)
    }
}
