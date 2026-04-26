fn validate_series_lengths(case: &CaseDefinition, cycles: u32) -> anyhow::Result<()> {
    let expected_len = usize::try_from(cycles).unwrap_or(usize::MAX);
    if !case.manifest.advance_ms.is_empty() && case.manifest.advance_ms.len() != expected_len {
        bail!(
            "case '{}' advance_ms length {} must equal cycles {}",
            case.id,
            case.manifest.advance_ms.len(),
            cycles
        );
    }
    for (name, series) in &case.manifest.input_series {
        if series.len() != expected_len {
            bail!(
                "case '{}' input series '{}' length {} must equal cycles {}",
                case.id,
                name,
                series.len(),
                cycles
            );
        }
    }
    for (address, series) in &case.manifest.direct_input_series {
        if series.len() != expected_len {
            bail!(
                "case '{}' direct input series '{}' length {} must equal cycles {}",
                case.id,
                address,
                series.len(),
                cycles
            );
        }
    }
    for restart in &case.manifest.restarts {
        if restart.before_cycle == 0 || restart.before_cycle > cycles {
            bail!(
                "case '{}' restart before_cycle {} must be within 1..={}",
                case.id,
                restart.before_cycle,
                cycles
            );
        }
    }
    Ok(())
}

fn parse_restart_mode(mode: &str) -> anyhow::Result<RestartMode> {
    match mode.to_ascii_lowercase().as_str() {
        "cold" => Ok(RestartMode::Cold),
        "warm" => Ok(RestartMode::Warm),
        _ => bail!("unsupported restart mode '{mode}', expected warm|cold"),
    }
}

fn should_skip_step_value(value: &str) -> bool {
    value.eq_ignore_ascii_case("skip") || value == "_"
}

fn parse_typed_value(raw: &str) -> anyhow::Result<Value> {
    let (kind, payload) = raw
        .split_once(':')
        .ok_or_else(|| anyhow!("typed value must be KIND:VALUE, got '{raw}'"))?;
    let payload = payload.trim();
    let normalized = kind.trim().to_ascii_uppercase();
    let number = |input: &str| -> anyhow::Result<String> { Ok(input.trim().replace('_', "")) };
    Ok(match normalized.as_str() {
        "BOOL" => Value::Bool(parse_bool(payload)?),
        "SINT" => Value::SInt(number(payload)?.parse::<i8>().context("parse SINT")?),
        "INT" => Value::Int(number(payload)?.parse::<i16>().context("parse INT")?),
        "DINT" => Value::DInt(number(payload)?.parse::<i32>().context("parse DINT")?),
        "LINT" => Value::LInt(number(payload)?.parse::<i64>().context("parse LINT")?),
        "USINT" => Value::USInt(number(payload)?.parse::<u8>().context("parse USINT")?),
        "UINT" => Value::UInt(number(payload)?.parse::<u16>().context("parse UINT")?),
        "UDINT" => Value::UDInt(number(payload)?.parse::<u32>().context("parse UDINT")?),
        "ULINT" => Value::ULInt(number(payload)?.parse::<u64>().context("parse ULINT")?),
        "BYTE" => Value::Byte(number(payload)?.parse::<u8>().context("parse BYTE")?),
        "WORD" => Value::Word(number(payload)?.parse::<u16>().context("parse WORD")?),
        "DWORD" => Value::DWord(number(payload)?.parse::<u32>().context("parse DWORD")?),
        "LWORD" => Value::LWord(number(payload)?.parse::<u64>().context("parse LWORD")?),
        "REAL" => Value::Real(number(payload)?.parse::<f32>().context("parse REAL")?),
        "LREAL" => Value::LReal(number(payload)?.parse::<f64>().context("parse LREAL")?),
        "TIME" => Value::Time(parse_duration(payload)?),
        "LTIME" => Value::LTime(parse_duration(payload)?),
        "STRING" => Value::String(payload.to_string().into()),
        _ => bail!("unsupported typed value kind '{normalized}'"),
    })
}

fn parse_bool(raw: &str) -> anyhow::Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => bail!("invalid BOOL literal '{raw}'"),
    }
}

fn parse_duration(raw: &str) -> anyhow::Result<Duration> {
    let text = raw.trim().to_ascii_lowercase().replace('_', "");
    if let Some(millis) = text.strip_suffix("ms") {
        let value = millis.parse::<i64>().context("parse TIME milliseconds")?;
        return Ok(Duration::from_millis(value));
    }
    if let Some(nanos) = text.strip_suffix("ns") {
        let value = nanos.parse::<i64>().context("parse TIME nanoseconds")?;
        return Ok(Duration::from_nanos(value));
    }
    if let Some(seconds) = text.strip_suffix('s') {
        let value = seconds.parse::<i64>().context("parse TIME seconds")?;
        return Ok(Duration::from_secs(value));
    }
    let value = text.parse::<i64>().context("parse TIME as milliseconds")?;
    Ok(Duration::from_millis(value))
}

fn encode_value(value: &Value) -> serde_json::Value {
    match value {
        Value::Bool(v) => json!({"type": "BOOL", "value": v}),
        Value::SInt(v) => json!({"type": "SINT", "value": v}),
        Value::Int(v) => json!({"type": "INT", "value": v}),
        Value::DInt(v) => json!({"type": "DINT", "value": v}),
        Value::LInt(v) => json!({"type": "LINT", "value": v}),
        Value::USInt(v) => json!({"type": "USINT", "value": v}),
        Value::UInt(v) => json!({"type": "UINT", "value": v}),
        Value::UDInt(v) => json!({"type": "UDINT", "value": v}),
        Value::ULInt(v) => json!({"type": "ULINT", "value": v}),
        Value::Real(v) => json!({"type": "REAL", "value": v}),
        Value::LReal(v) => json!({"type": "LREAL", "value": v}),
        Value::Byte(v) => json!({"type": "BYTE", "value": v}),
        Value::Word(v) => json!({"type": "WORD", "value": v}),
        Value::DWord(v) => json!({"type": "DWORD", "value": v}),
        Value::LWord(v) => json!({"type": "LWORD", "value": v}),
        Value::Time(v) => json!({"type": "TIME", "nanos": v.as_nanos()}),
        Value::LTime(v) => json!({"type": "LTIME", "nanos": v.as_nanos()}),
        Value::Date(v) => json!({"type": "DATE", "ticks": v.ticks()}),
        Value::LDate(v) => json!({"type": "LDATE", "nanos": v.nanos()}),
        Value::Tod(v) => json!({"type": "TOD", "ticks": v.ticks()}),
        Value::LTod(v) => json!({"type": "LTOD", "nanos": v.nanos()}),
        Value::Dt(v) => json!({"type": "DT", "ticks": v.ticks()}),
        Value::Ldt(v) => json!({"type": "LDT", "nanos": v.nanos()}),
        Value::String(v) => json!({"type": "STRING", "value": v.to_string()}),
        Value::WString(v) => json!({"type": "WSTRING", "value": v}),
        Value::Char(v) => json!({"type": "CHAR", "value": v}),
        Value::WChar(v) => json!({"type": "WCHAR", "value": v}),
        Value::Array(array) => json!({
            "type": "ARRAY",
            "dimensions": array.dimensions(),
            "elements": array.elements().iter().map(encode_value).collect::<Vec<_>>()
        }),
        Value::Struct(value) => {
            let mut fields = BTreeMap::new();
            for (name, field_value) in value.fields() {
                fields.insert(name.to_string(), encode_value(field_value));
            }
            json!({
                "type": "STRUCT",
                "type_name": value.type_name().to_string(),
                "fields": fields
            })
        }
        Value::Enum(value) => json!({
            "type": "ENUM",
            "type_name": value.type_name().to_string(),
            "variant": value.variant_name().to_string(),
            "numeric": value.numeric_value()
        }),
        Value::Reference(reference) => json!({
            "type": "REFERENCE",
            "value": reference.as_ref().map(|entry| format!("{entry:?}"))
        }),
        Value::Instance(id) => json!({"type": "INSTANCE", "value": id.0}),
        Value::Null => json!({"type": "NULL"}),
    }
}

fn read_json_value(path: &Path) -> anyhow::Result<serde_json::Value> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read json file '{}'", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse json file '{}'", path.display()))
}

fn write_json_pretty(path: &Path, value: &serde_json::Value) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create directory '{}'", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).context("serialize json payload")?;
    fs::write(path, format!("{text}\n")).with_context(|| format!("write '{}'", path.display()))
}

fn reason(code: &str, message: &str, details: Option<String>) -> SummaryReason {
    SummaryReason {
        code: code.to_string(),
        message: message.to_string(),
        details,
    }
}
