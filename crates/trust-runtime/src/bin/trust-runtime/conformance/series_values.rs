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
    for (index, advance) in case.manifest.advance_ms.iter().enumerate() {
        if Duration::checked_from_millis(*advance).is_none() {
            bail!(
                "case '{}' advance_ms[{}] {} is outside the millisecond time range",
                case.id,
                index,
                advance
            );
        }
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
        // The runtime currently has cold/warm execution primitives. The
        // conformance matrix keeps the industrial labels explicit and maps
        // them to their implemented retain behavior.
        "hot" | "fault" => Ok(RestartMode::Warm),
        "download" => Ok(RestartMode::Cold),
        _ => bail!("unsupported restart mode '{mode}', expected warm|cold|hot|fault|download"),
    }
}

fn should_skip_step_value(value: &str) -> bool {
    value.eq_ignore_ascii_case("skip") || value == "_"
}

/// The value of a typed value text, as the manifests write them (specification 11 §6.8.1): the
/// shared reading that a device's `cycle.step` reads them with too.
fn parse_typed_value(raw: &str) -> anyhow::Result<Value> {
    trust_runtime_services::cycle_step::parse_typed_value(raw).map_err(|reason| anyhow!(reason))
}

/// `value` as a conformance artifact writes it, the shared encoding a device's `cycle.step` replies
/// with too.
fn encode_value(value: &Value) -> serde_json::Value {
    trust_runtime_services::cycle_step::encode_value(value)
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
