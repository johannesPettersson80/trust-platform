const SPARKPLUG_NAMESPACE: &str = "spBv1.0";
const SPARKPLUG_SPEC_VERSION: &str = "3.0.0";
const SPARKPLUG_DATATYPE_INT16: u32 = 2;
const SPARKPLUG_DATATYPE_INT32: u32 = 3;
const SPARKPLUG_DATATYPE_UINT16: u32 = 6;
const SPARKPLUG_DATATYPE_UINT32: u32 = 7;
const SPARKPLUG_DATATYPE_UINT64: u32 = 8;
const SPARKPLUG_DATATYPE_FLOAT: u32 = 9;
const SPARKPLUG_DATATYPE_BOOLEAN: u32 = 11;

#[derive(Debug, Clone, Deserialize)]
struct SparkplugToml {
    enabled: Option<bool>,
    namespace: Option<String>,
    spec_version: Option<String>,
    group_id: Option<String>,
    edge_node_id: Option<String>,
    birth_death_seq: Option<u64>,
}

#[derive(Debug, Clone)]
struct SparkplugConfig {
    namespace: SmolStr,
    spec_version: SmolStr,
    group_id: SmolStr,
    edge_node_id: SmolStr,
    birth_death_seq: u64,
}

impl SparkplugConfig {
    fn from_toml(value: Option<SparkplugToml>) -> Result<Option<Self>, RuntimeError> {
        let Some(value) = value else {
            return Ok(None);
        };
        if value.enabled == Some(false) {
            return Ok(None);
        }
        let namespace = value
            .namespace
            .as_deref()
            .unwrap_or(SPARKPLUG_NAMESPACE)
            .trim();
        if namespace != SPARKPLUG_NAMESPACE {
            return Err(RuntimeError::InvalidConfig(
                format!(
                    "mqtt sparkplug.namespace must be {SPARKPLUG_NAMESPACE:?}; got {namespace:?}"
                )
                .into(),
            ));
        }
        let spec_version = value
            .spec_version
            .as_deref()
            .unwrap_or(SPARKPLUG_SPEC_VERSION)
            .trim();
        if spec_version != SPARKPLUG_SPEC_VERSION {
            return Err(RuntimeError::InvalidConfig(
                format!(
                    "mqtt sparkplug.spec_version must be {SPARKPLUG_SPEC_VERSION:?}; got {spec_version:?}"
                )
                .into(),
            ));
        }
        Ok(Some(Self {
            namespace: SmolStr::new(namespace),
            spec_version: SmolStr::new(spec_version),
            group_id: parse_sparkplug_topic_token(value.group_id.as_deref(), "group_id")?,
            edge_node_id: parse_sparkplug_topic_token(
                value.edge_node_id.as_deref(),
                "edge_node_id",
            )?,
            birth_death_seq: value.birth_death_seq.unwrap_or(0),
        }))
    }

    fn nbirth_topic(&self) -> String {
        format!(
            "{}/{}/NBIRTH/{}",
            self.namespace, self.group_id, self.edge_node_id
        )
    }

    fn ndata_topic(&self) -> String {
        format!(
            "{}/{}/NDATA/{}",
            self.namespace, self.group_id, self.edge_node_id
        )
    }

    fn ndeath_topic(&self) -> String {
        format!(
            "{}/{}/NDEATH/{}",
            self.namespace, self.group_id, self.edge_node_id
        )
    }
}

fn parse_sparkplug_topic_token(
    value: Option<&str>,
    field: &str,
) -> Result<SmolStr, RuntimeError> {
    let value = value.ok_or_else(|| {
        RuntimeError::InvalidConfig(format!("mqtt sparkplug.{field} is required").into())
    })?;
    let value = value.trim();
    if value.is_empty()
        || value.contains('/')
        || value.contains('+')
        || value.contains('#')
        || value.chars().any(char::is_control)
    {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "mqtt sparkplug.{field} must be a non-empty MQTT topic token without '/', '+', '#', or control characters"
            )
            .into(),
        ));
    }
    Ok(SmolStr::new(value))
}

fn validate_sparkplug_profile(
    config: Option<&SparkplugConfig>,
    input_points: &[MqttInputPoint],
    output_points: &[MqttOutputPoint],
) -> Result<(), RuntimeError> {
    let Some(config) = config else {
        return Ok(());
    };
    if !input_points.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "mqtt sparkplug profile currently supports outbound output_points only".into(),
        ));
    }
    if output_points.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "mqtt sparkplug profile requires at least one output_point metric".into(),
        ));
    }
    if config.spec_version.as_str() != SPARKPLUG_SPEC_VERSION {
        return Err(RuntimeError::InvalidConfig(
            "mqtt sparkplug profile only supports Sparkplug 3.0.0".into(),
        ));
    }
    Ok(())
}

fn sparkplug_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn encode_sparkplug_nbirth(
    config: &SparkplugConfig,
    points: &[MqttOutputPoint],
    sequence: u64,
) -> Vec<u8> {
    let timestamp = sparkplug_timestamp_ms();
    encode_sparkplug_nbirth_at(config, points, sequence, timestamp)
}

fn encode_sparkplug_nbirth_at(
    config: &SparkplugConfig,
    points: &[MqttOutputPoint],
    sequence: u64,
    timestamp: u64,
) -> Vec<u8> {
    let mut metrics = Vec::with_capacity(points.len() + 1);
    metrics.push(SparkplugMetric::with_value(
        "bdSeq",
        SPARKPLUG_DATATYPE_UINT64,
        timestamp,
        SparkplugMetricValue::Long(config.birth_death_seq),
    ));
    for point in points {
        metrics.push(SparkplugMetric::definition(point, timestamp));
    }
    encode_payload(timestamp, sequence, &metrics)
}

fn encode_sparkplug_ndeath(config: &SparkplugConfig) -> Vec<u8> {
    encode_sparkplug_ndeath_at(config, sparkplug_timestamp_ms())
}

fn encode_sparkplug_ndeath_at(config: &SparkplugConfig, timestamp: u64) -> Vec<u8> {
    let metrics = [SparkplugMetric::with_value(
        "bdSeq",
        SPARKPLUG_DATATYPE_UINT64,
        timestamp,
        SparkplugMetricValue::Long(config.birth_death_seq),
    )];
    encode_payload(timestamp, 0, &metrics)
}

fn encode_sparkplug_ndata(
    points: &[MqttOutputPoint],
    image: &[u8],
    sequence: u64,
) -> Result<Vec<u8>, RuntimeError> {
    let timestamp = sparkplug_timestamp_ms();
    encode_sparkplug_ndata_at(points, image, sequence, timestamp)
}

fn encode_sparkplug_ndata_at(
    points: &[MqttOutputPoint],
    image: &[u8],
    sequence: u64,
    timestamp: u64,
) -> Result<Vec<u8>, RuntimeError> {
    let metrics = points
        .iter()
        .map(|point| SparkplugMetric::from_output_point(point, image, timestamp))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(encode_payload(timestamp, sequence, &metrics))
}

#[derive(Debug, Clone)]
struct SparkplugMetric {
    name: SmolStr,
    datatype: u32,
    timestamp: u64,
    value: Option<SparkplugMetricValue>,
}

impl SparkplugMetric {
    fn definition(point: &MqttOutputPoint, timestamp: u64) -> Self {
        Self {
            name: point.metric_name.clone(),
            datatype: sparkplug_datatype(point.data_type),
            timestamp,
            value: None,
        }
    }

    fn with_value(
        name: &str,
        datatype: u32,
        timestamp: u64,
        value: SparkplugMetricValue,
    ) -> Self {
        Self {
            name: SmolStr::new(name),
            datatype,
            timestamp,
            value: Some(value),
        }
    }

    fn from_output_point(
        point: &MqttOutputPoint,
        image: &[u8],
        timestamp: u64,
    ) -> Result<Self, RuntimeError> {
        let value = match point.data_type {
            MqttPointType::Bool => SparkplugMetricValue::Boolean(read_image_bool(
                image,
                point.image_offset,
                point.image_bit,
            )?),
            MqttPointType::U16 => SparkplugMetricValue::Int(
                checked_u16(scaled_sparkplug_raw(point, image)?)?.into(),
            ),
            MqttPointType::I16 => SparkplugMetricValue::Int(
                checked_i16(scaled_sparkplug_raw(point, image)?)? as i32 as u32,
            ),
            MqttPointType::U32 => SparkplugMetricValue::Int(checked_u32(
                scaled_sparkplug_raw(point, image)?,
            )?),
            MqttPointType::I32 => SparkplugMetricValue::Int(
                checked_i32(scaled_sparkplug_raw(point, image)?)? as u32,
            ),
            MqttPointType::F32 => SparkplugMetricValue::Float(checked_f32(
                scaled_sparkplug_raw(point, image)?,
            )?),
        };
        Ok(Self {
            name: point.metric_name.clone(),
            datatype: sparkplug_datatype(point.data_type),
            timestamp,
            value: Some(value),
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum SparkplugMetricValue {
    Int(u32),
    Long(u64),
    Float(f32),
    Boolean(bool),
}

fn scaled_sparkplug_raw(point: &MqttOutputPoint, image: &[u8]) -> Result<f64, RuntimeError> {
    let engineering = read_image_numeric(image, point.image_offset, point.data_type)?;
    Ok((engineering - point.offset) / point.scale)
}

fn sparkplug_datatype(data_type: MqttPointType) -> u32 {
    match data_type {
        MqttPointType::Bool => SPARKPLUG_DATATYPE_BOOLEAN,
        MqttPointType::U16 => SPARKPLUG_DATATYPE_UINT16,
        MqttPointType::I16 => SPARKPLUG_DATATYPE_INT16,
        MqttPointType::U32 => SPARKPLUG_DATATYPE_UINT32,
        MqttPointType::I32 => SPARKPLUG_DATATYPE_INT32,
        MqttPointType::F32 => SPARKPLUG_DATATYPE_FLOAT,
    }
}

fn encode_payload(timestamp: u64, sequence: u64, metrics: &[SparkplugMetric]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_varint_field(&mut out, 1, timestamp);
    for metric in metrics {
        let mut metric_bytes = Vec::new();
        encode_string_field(&mut metric_bytes, 1, metric.name.as_str());
        encode_varint_field(&mut metric_bytes, 3, metric.timestamp);
        encode_varint_field(&mut metric_bytes, 4, u64::from(metric.datatype));
        if let Some(value) = metric.value {
            match value {
                SparkplugMetricValue::Int(value) => {
                    encode_varint_field(&mut metric_bytes, 10, u64::from(value));
                }
                SparkplugMetricValue::Long(value) => {
                    encode_varint_field(&mut metric_bytes, 11, value);
                }
                SparkplugMetricValue::Float(value) => {
                    encode_fixed32_field(&mut metric_bytes, 12, value.to_bits());
                }
                SparkplugMetricValue::Boolean(value) => {
                    encode_varint_field(&mut metric_bytes, 14, if value { 1 } else { 0 });
                }
            }
        }
        encode_bytes_field(&mut out, 2, &metric_bytes);
    }
    encode_varint_field(&mut out, 3, sequence);
    out
}

fn encode_varint_field(out: &mut Vec<u8>, field_number: u32, value: u64) {
    encode_varint(out, u64::from(field_number << 3));
    encode_varint(out, value);
}

fn encode_string_field(out: &mut Vec<u8>, field_number: u32, value: &str) {
    encode_bytes_field(out, field_number, value.as_bytes());
}

fn encode_bytes_field(out: &mut Vec<u8>, field_number: u32, value: &[u8]) {
    encode_varint(out, u64::from((field_number << 3) | 2));
    encode_varint(out, value.len() as u64);
    out.extend_from_slice(value);
}

fn encode_fixed32_field(out: &mut Vec<u8>, field_number: u32, value: u32) {
    encode_varint(out, u64::from((field_number << 3) | 5));
    out.extend_from_slice(&value.to_le_bytes());
}

fn encode_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}
