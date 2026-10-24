/// The device the runtime cases run on (specification 44 §14.2): its control link, the target its
/// cases are compiled for and the toolchain that compiles them.
struct DeviceRun {
    link: DeviceLink,
    target: Target,
    toolchain: Toolchain,
}

impl DeviceRun {
    /// The device on `port`, a serial port or `tcp://<host>:<port>` of a bridge to one, whose cases
    /// are compiled for `target` as `trust-runtime build --target` names it, prepared for stepped
    /// scans ([`DeviceLink::prepare_steps`]).
    fn open(port: &str, target: &str) -> anyhow::Result<Self> {
        let target = crate::build::native_target(target)?;
        let toolchain = Toolchain::find()
            .map_err(|error| anyhow!("the native backend needs its toolchain: {error}"))?;
        let mut link = DeviceLink::open(port)?;
        let work = std::env::temp_dir().join(format!(
            "trust-conformance-device-{}-idle",
            std::process::id()
        ));
        let prepared = link.prepare_steps(target, &toolchain, &work);
        let _ = fs::remove_dir_all(&work);
        prepared.map_err(|error| anyhow!("prepare the device for stepped scans: {error}"))?;
        Ok(Self {
            link,
            target,
            toolchain,
        })
    }
}

/// The runtime case `case` run on the device (specification 44 §14.2): compiled into a program image
/// for the device and installed, then stepped cycle by cycle with `cycle.step` (specification 11
/// §6.8.1), each step with the restart, time, inputs and watches that the native runner takes for
/// that cycle, so that its trace compares with the same expected artifact.
fn execute_runtime_case_device(
    case: &CaseDefinition,
    sources: &[String],
    device: &mut DeviceRun,
) -> anyhow::Result<CaseArtifact> {
    let cycles = case.manifest.cycles;
    if cycles == 0 {
        bail!("runtime case '{}' must declare cycles > 0", case.id);
    }
    validate_series_lengths(case, cycles)?;
    let container = device_container(case, sources, device)?;
    device.link.install(&container)?;

    let mut now = Duration::ZERO;
    let mut trace = Vec::with_capacity(cycles as usize);
    for cycle_idx in 0..(cycles as usize) {
        let cycle_number = u32::try_from(cycle_idx + 1).unwrap_or(u32::MAX);
        let mut modes = Vec::new();
        for restart in case
            .manifest
            .restarts
            .iter()
            .filter(|entry| entry.before_cycle == cycle_number)
        {
            modes.push(parse_restart_mode(&restart.mode)?);
        }
        // The first step restarts the installed program cold, which loads it, as the native runner
        // starts its program cold; a warm restart after it changes nothing. Before a later cycle the
        // restarts the manifest names act as one: cold when one of them is.
        let restart = if cycle_number == 1 || modes.contains(&RestartMode::Cold) {
            Some("cold")
        } else if modes.is_empty() {
            None
        } else {
            Some("warm")
        };

        if !case.manifest.advance_ms.is_empty() {
            let advance = case.manifest.advance_ms[cycle_idx];
            let advance = Duration::checked_from_millis(advance).with_context(|| {
                format!(
                    "case '{}' advance_ms[{cycle_idx}] {advance} is outside the millisecond time range",
                    case.id
                )
            })?;
            now = Duration::from_nanos(
                now.as_nanos()
                    .checked_add(advance.as_nanos())
                    .ok_or_else(|| anyhow!("the case's time leaves the nanosecond range"))?,
            );
        }

        let mut inputs = serde_json::Map::new();
        for (name, series) in &case.manifest.input_series {
            let raw = &series[cycle_idx];
            if should_skip_step_value(raw) {
                continue;
            }
            parse_typed_value(raw)
                .with_context(|| format!("parse input series value for '{name}'"))?;
            inputs.insert(name.clone(), json!(raw));
        }
        let mut direct_inputs = serde_json::Map::new();
        for (address, series) in &case.manifest.direct_input_series {
            let raw = &series[cycle_idx];
            if should_skip_step_value(raw) {
                continue;
            }
            parse_typed_value(raw)
                .with_context(|| format!("parse direct input value for '{address}'"))?;
            direct_inputs.insert(address.clone(), json!(raw));
        }
        let mut params = json!({
            "time_ns": now.as_nanos(),
            "inputs": inputs,
            "direct_inputs": direct_inputs,
            "watch": case.manifest.watch_globals,
            "watch_direct": case.manifest.watch_direct,
        });
        if let Some(mode) = restart {
            params["restart"] = json!(mode);
        }
        let stepped = device
            .link
            .call("cycle.step", Some(params), device_link::REPLY_WAIT)
            .map_err(|error| anyhow!("step cycle {cycle_number}: {error:#}"))?;

        let read = |member: &str,
                    names: &[String]|
         -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
            names
                .iter()
                .map(|name| {
                    let value = stepped
                        .get(member)
                        .and_then(|values| values.get(name))
                        .ok_or_else(|| anyhow!("the step read no value of '{name}'"))?;
                    Ok((name.clone(), normalized(value)))
                })
                .collect()
        };
        let globals = read("values", &case.manifest.watch_globals)?;
        let direct = read("direct", &case.manifest.watch_direct)?;
        let errors = stepped
            .get("errors")
            .cloned()
            .ok_or_else(|| anyhow!("the step named no errors"))?;

        trace.push(json!({
            "cycle": cycle_number,
            "runtime_time_nanos": now.as_nanos(),
            "globals": globals,
            "direct": direct,
            "errors": errors
        }));
    }

    Ok(CaseArtifact {
        payload: json!({
            "version": 1,
            "case_id": case.id,
            "category": case.category,
            "kind": "runtime",
            "description": case.manifest.description,
            "cycles": cycles,
            "trace": trace
        }),
        cycles: Some(u64::from(cycles)),
    })
}

/// The container of the runtime case `case` for the device: its sources compiled into a program
/// image for the device's target, for stepped scans ([`device_link::stepping_container`]).
fn device_container(
    case: &CaseDefinition,
    sources: &[String],
    device: &DeviceRun,
) -> anyhow::Result<Vec<u8>> {
    let work = std::env::temp_dir().join(format!(
        "trust-conformance-device-{}-{}",
        std::process::id(),
        case.id
    ));
    let files: Vec<SourceFile> = sources
        .iter()
        .map(|text| SourceFile::new(text.as_str()))
        .collect();
    let container = native::build(
        &files,
        &native::Options {
            target: device.target,
            hooks: true,
        },
        &device.toolchain,
        &work.join("native"),
    )
    .map_err(|error| anyhow!("the case does not compile for the device: {error}"))
    .and_then(|mut built| {
        // Signed with the user's own key, as every image a tool installs (specification 44 §5.11).
        native::user_key()
            .and_then(|key| native::sign(&mut built.image, &key))
            .map_err(|error| anyhow!("{error}"))?;
        device_link::stepping_container(&built.image, &work).map_err(|error| anyhow!("{error}"))
    });
    let _ = fs::remove_dir_all(&work);
    container
}

/// `value`, a value as a device encodes it, as this computer encodes it: the number of a `REAL` or
/// an `LREAL` is written again from the value it holds. A device's JSON writes a `REAL` through its
/// double, where this computer's writes the shortest text of the `REAL` itself, and the two texts
/// would not compare equal.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the number of a REAL is a REAL widened to a double, which narrows back exactly"
)]
fn normalized(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let kind = object.get("type").and_then(serde_json::Value::as_str);
            let members = object
                .iter()
                .map(|(key, member)| {
                    let member = match (kind, key.as_str(), member.as_f64()) {
                        (Some("REAL"), "value", Some(number)) => json!(number as f32),
                        (Some("LREAL"), "value", Some(number)) => json!(number),
                        _ => normalized(member),
                    };
                    (key.clone(), member)
                })
                .collect();
            serde_json::Value::Object(members)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(normalized).collect())
        }
        other => other.clone(),
    }
}
