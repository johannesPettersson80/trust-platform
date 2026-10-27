fn compile_patterns(patterns: &[SmolStr]) -> Result<Vec<Pattern>, RuntimeError> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern.as_str()).map_err(|err| {
                RuntimeError::InvalidConfig(
                    format!("runtime.observability.include invalid pattern '{pattern}': {err}")
                        .into(),
                )
            })
        })
        .collect()
}

fn compile_alert_rules(
    rules: &[AlertRule],
    bundle_root: Option<&Path>,
) -> Result<Vec<CompiledAlertRule>, RuntimeError> {
    rules
        .iter()
        .map(|rule| {
            if rule.name.trim().is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.observability.alerts[].name must not be empty".into(),
                ));
            }
            if rule.variable.trim().is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.observability.alerts[].variable must not be empty".into(),
                ));
            }
            if rule.above.is_none() && rule.below.is_none() {
                return Err(RuntimeError::InvalidConfig(
                    format!(
                        "runtime.observability.alert '{}': set above and/or below threshold",
                        rule.name
                    )
                    .into(),
                ));
            }
            if rule.debounce_samples == 0 {
                return Err(RuntimeError::InvalidConfig(
                    format!(
                        "runtime.observability.alert '{}': debounce_samples must be >= 1",
                        rule.name
                    )
                    .into(),
                ));
            }

            let hook = rule.hook.as_deref().map(|value| {
                if value.eq_ignore_ascii_case("log") {
                    HookTarget::Log
                } else if value.starts_with("http://") || value.starts_with("https://") {
                    HookTarget::Webhook(SmolStr::new(value))
                } else {
                    HookTarget::File(resolve_path(Path::new(value), bundle_root))
                }
            });

            Ok(CompiledAlertRule {
                name: rule.name.clone(),
                variable: rule.variable.clone(),
                above: rule.above,
                below: rule.below,
                debounce_samples: rule.debounce_samples,
                hook,
            })
        })
        .collect()
}

fn load_existing_samples(
    path: &Path,
    max_entries: usize,
    inner: &mut HistorianInner,
) -> Result<(), RuntimeError> {
    if !path.is_file() {
        return Ok(());
    }
    let file = std::fs::File::open(path).map_err(|err| {
        RuntimeError::ControlError(format!("historian open failed: {err}").into())
    })?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => continue,
        };
        let Ok(sample) = serde_json::from_str::<HistorianSample>(&line) else {
            continue;
        };
        inner.tracked_variables.insert(sample.variable.clone());
        inner.samples.push_back(sample);
        while inner.samples.len() > max_entries {
            let _ = inner.samples.pop_front();
        }
        inner.samples_total = inner.samples_total.saturating_add(1);
    }
    Ok(())
}

fn collect_snapshot_samples(
    snapshot: &DebugSnapshot,
    config: &HistorianConfig,
    patterns: &[Pattern],
    timestamp_ms: u128,
) -> Vec<HistorianSample> {
    // A compiled program's variables are read from its state through its symbol table
    // (specification 44 §8.8, §13.3).
    if let Some(state) = snapshot.compiled.as_deref() {
        return compiled_samples(
            &state.variables(),
            &|symbol| state.value_of(symbol),
            &|symbol| state.members(symbol),
            state.now(),
            config,
            patterns,
            timestamp_ms,
        );
    }
    let mut samples = Vec::<HistorianSample>::new();
    let mut values = Vec::<(String, HistorianValue)>::new();
    for (name, value) in snapshot.storage.globals() {
        flatten_value(
            name.as_str(),
            value,
            Some(&snapshot.storage),
            config,
            patterns,
            &mut values,
        );
    }
    for (name, value) in snapshot.storage.retain() {
        let path = format!("retain.{name}");
        flatten_value(
            path.as_str(),
            value,
            Some(&snapshot.storage),
            config,
            patterns,
            &mut values,
        );
    }
    samples.reserve(values.len());
    for (variable, value) in values {
        samples.push(HistorianSample {
            timestamp_ms,
            source_time_ns: snapshot.now.as_nanos(),
            variable,
            value,
        });
    }
    samples
}

/// The samples of a compiled program at its logical time `now`, flattened as an interpreted
/// program's are: each variable of its program instances and each global by its declared full name
/// (`Vat.RegState`), a structure's members and an array's elements under their paths, and an
/// instance of a function block or a class, which holds no value, by its members (`Vat.Timer.Q`).
/// `value_of` reads a variable's value from its symbol and `members_of` lists an instance's
/// members. A compiled program has no retain bank: its retained variables are recorded by their
/// names, as an interpreted program's are.
fn compiled_samples(
    variables: &[crate::CompiledVariable],
    value_of: &dyn Fn(u32) -> Option<Value>,
    members_of: &dyn Fn(u32) -> Vec<crate::CompiledVariable>,
    now: crate::value::Duration,
    config: &HistorianConfig,
    patterns: &[Pattern],
    timestamp_ms: u128,
) -> Vec<HistorianSample> {
    let mut values = Vec::<(String, HistorianValue)>::new();
    for variable in variables {
        flatten_compiled(
            variable,
            value_of,
            members_of,
            config,
            patterns,
            &mut values,
        );
    }
    values
        .into_iter()
        .map(|(variable, value)| HistorianSample {
            timestamp_ms,
            source_time_ns: now.as_nanos(),
            variable,
            value,
        })
        .collect()
}

/// Records the compiled program's `variable` at its full name, an instance by its members.
fn flatten_compiled(
    variable: &crate::CompiledVariable,
    value_of: &dyn Fn(u32) -> Option<Value>,
    members_of: &dyn Fn(u32) -> Vec<crate::CompiledVariable>,
    config: &HistorianConfig,
    patterns: &[Pattern],
    out: &mut Vec<(String, HistorianValue)>,
) {
    if variable.instance {
        for member in members_of(variable.symbol) {
            flatten_compiled(&member, value_of, members_of, config, patterns, out);
        }
    } else if let Some(value) = value_of(variable.symbol) {
        flatten_value(variable.name.as_str(), &value, None, config, patterns, out);
    }
}

/// Records `value` at `path`, or each of its members and elements under their own paths.
/// `storage` holds the instances an interpreted program's values name; a compiled program's values
/// name none.
fn flatten_value(
    path: &str,
    value: &Value,
    storage: Option<&crate::memory::VariableStorage>,
    config: &HistorianConfig,
    patterns: &[Pattern],
    out: &mut Vec<(String, HistorianValue)>,
) {
    if let Some(hist_value) = to_historian_value(value) {
        if should_record(path, config, patterns) {
            out.push((path.to_string(), hist_value));
        }
        return;
    }
    match value {
        Value::Struct(value) => {
            for (field, field_value) in value.fields() {
                let nested = format!("{path}.{field}");
                flatten_value(nested.as_str(), field_value, storage, config, patterns, out);
            }
        }
        Value::Array(value) => {
            for (idx, element) in value.elements().iter().enumerate() {
                let nested = format!("{path}[{idx}]");
                flatten_value(nested.as_str(), element, storage, config, patterns, out);
            }
        }
        Value::Instance(instance_id) => {
            if let Some(instance) = storage.and_then(|storage| storage.get_instance(*instance_id)) {
                for (field, field_value) in &instance.variables {
                    let nested = format!("{path}.{field}");
                    flatten_value(nested.as_str(), field_value, storage, config, patterns, out);
                }
            }
        }
        _ => {}
    }
}

fn should_record(path: &str, config: &HistorianConfig, patterns: &[Pattern]) -> bool {
    match config.mode {
        RecordingMode::All => true,
        RecordingMode::Allowlist => patterns.iter().any(|pattern| pattern.matches(path)),
    }
}

fn to_historian_value(value: &Value) -> Option<HistorianValue> {
    match value {
        Value::Bool(value) => Some(HistorianValue::Bool(*value)),
        Value::SInt(value) => Some(HistorianValue::Integer((*value).into())),
        Value::Int(value) => Some(HistorianValue::Integer((*value).into())),
        Value::DInt(value) => Some(HistorianValue::Integer((*value).into())),
        Value::LInt(value) => Some(HistorianValue::Integer(*value)),
        Value::USInt(value) => Some(HistorianValue::Unsigned((*value).into())),
        Value::UInt(value) => Some(HistorianValue::Unsigned((*value).into())),
        Value::UDInt(value) => Some(HistorianValue::Unsigned((*value).into())),
        Value::ULInt(value) => Some(HistorianValue::Unsigned(*value)),
        Value::Real(value) => Some(HistorianValue::Float(f64::from(*value))),
        Value::LReal(value) => Some(HistorianValue::Float(*value)),
        Value::Byte(value) => Some(HistorianValue::Unsigned((*value).into())),
        Value::Word(value) => Some(HistorianValue::Unsigned((*value).into())),
        Value::DWord(value) => Some(HistorianValue::Unsigned((*value).into())),
        Value::LWord(value) => Some(HistorianValue::Unsigned(*value)),
        Value::Time(value) | Value::LTime(value) => {
            Some(HistorianValue::Integer(value.as_millis()))
        }
        Value::Date(value) => Some(HistorianValue::Integer(value.ticks())),
        Value::LDate(value) => Some(HistorianValue::Integer(value.nanos())),
        Value::Tod(value) => Some(HistorianValue::Integer(value.ticks())),
        Value::LTod(value) => Some(HistorianValue::Integer(value.nanos())),
        Value::Dt(value) => Some(HistorianValue::Integer(value.ticks())),
        Value::Ldt(value) => Some(HistorianValue::Integer(value.nanos())),
        Value::String(value) => Some(HistorianValue::String(value.to_string())),
        Value::WString(value) => Some(HistorianValue::String(value.clone())),
        Value::Char(value) => Some(HistorianValue::String(char::from(*value).to_string())),
        Value::WChar(value) => {
            char::from_u32(u32::from(*value)).map(|ch| HistorianValue::String(ch.to_string()))
        }
        Value::Enum(value) => Some(HistorianValue::String(value.variant_name().to_string())),
        _ => None,
    }
}

fn append_samples(path: &Path, samples: &[HistorianSample]) -> Result<(), RuntimeError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            RuntimeError::ControlError(format!("historian write failed: {err}").into())
        })?;
    for sample in samples {
        let line = serde_json::to_string(sample).map_err(|err| {
            RuntimeError::ControlError(format!("historian serialization failed: {err}").into())
        })?;
        file.write_all(line.as_bytes()).map_err(|err| {
            RuntimeError::ControlError(format!("historian write failed: {err}").into())
        })?;
        file.write_all(b"\n").map_err(|err| {
            RuntimeError::ControlError(format!("historian write failed: {err}").into())
        })?;
    }
    Ok(())
}
