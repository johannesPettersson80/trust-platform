/// The runtime case `case` compiled into a program image for this computer and run by the kernel
/// (specification 44 §14.2): each cycle's restarts, time, inputs, scan and watches, in the expected
/// artifact's form.
fn execute_runtime_case(case: &CaseDefinition, sources: &[String]) -> anyhow::Result<CaseArtifact> {
    let cycles = case.manifest.cycles;
    if cycles == 0 {
        bail!("runtime case '{}' must declare cycles > 0", case.id);
    }
    validate_series_lengths(case, cycles)?;
    let toolchain = Toolchain::find()
        .map_err(|error| anyhow!("the native backend needs its toolchain: {error}"))?;
    let target =
        Target::this_cpu().ok_or_else(|| anyhow!("this computer's CPU has no native target"))?;
    let files: Vec<SourceFile> = sources
        .iter()
        .map(|text| SourceFile::new(text.as_str()))
        .collect();
    let work = std::env::temp_dir().join(format!(
        "trust-conformance-native-{}-{}",
        std::process::id(),
        case.id
    ));
    let built = native::build(
        &files,
        &native::Options {
            target,
            hooks: true,
        },
        &toolchain,
        &work,
    );
    let _ = fs::remove_dir_all(&work);
    let built = built.map_err(|error| anyhow!("the case does not compile natively: {error}"))?;
    let device = native::this_computer()
        .ok_or_else(|| anyhow!("this computer's CPU runs no program image"))?;
    let mut program = native::load(&built.image, &device).map_err(|error| anyhow!("{error}"))?;
    program
        .cold_start(Duration::ZERO)
        .map_err(|error| anyhow!("the cold start failed: {error}"))?;

    let mut now = Duration::ZERO;
    let mut trace = Vec::with_capacity(cycles as usize);
    for cycle_idx in 0..(cycles as usize) {
        let cycle_number = u32::try_from(cycle_idx + 1).unwrap_or(u32::MAX);
        for restart in case
            .manifest
            .restarts
            .iter()
            .filter(|entry| entry.before_cycle == cycle_number)
        {
            let restarted = match parse_restart_mode(&restart.mode)? {
                RestartMode::Cold => program.cold_start(now),
                RestartMode::Warm => program.warm_start(now),
            };
            restarted
                .map_err(|err| anyhow!("restart before cycle {cycle_number} failed: {err}"))?;
        }

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

        for (name, series) in &case.manifest.input_series {
            let raw = &series[cycle_idx];
            if should_skip_step_value(raw) {
                continue;
            }
            let value = parse_typed_value(raw)
                .with_context(|| format!("parse input series value for '{name}'"))?;
            let symbol = native_symbol(&program.image(), name)?;
            program
                .write_symbol(symbol, &value)
                .map_err(|error| anyhow!("set input '{name}': {error}"))?;
        }

        for (address, series) in &case.manifest.direct_input_series {
            let raw = &series[cycle_idx];
            if should_skip_step_value(raw) {
                continue;
            }
            let value = parse_typed_value(raw)
                .with_context(|| format!("parse direct input value for '{address}'"))?;
            write_native_image(&mut program, address, value)
                .with_context(|| format!("set direct input '{address}'"))?;
        }

        let scanned = program.scan(now, &mut native::NoHooks);

        let mut globals = BTreeMap::new();
        for name in &case.manifest.watch_globals {
            let symbol = native_symbol(&program.image(), name)?;
            let value = program
                .read_symbol(symbol)
                .map_err(|error| anyhow!("watch global '{name}': {error}"))?;
            globals.insert(name.clone(), encode_value(&value));
        }

        let mut direct = BTreeMap::new();
        for address in &case.manifest.watch_direct {
            let value = read_native_image(&program, address)
                .with_context(|| format!("read direct output '{address}'"))?;
            direct.insert(address.clone(), encode_value(&value));
        }

        let errors = scanned
            .err()
            .map(|error| error.to_string())
            .into_iter()
            .collect::<Vec<_>>();

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

/// The symbol a case's name reads or writes in a compiled program, by the harness's rules
/// ([`Image::find_variable`](trust_runtime_core::native::image::Image::find_variable)).
fn native_symbol(
    image: &trust_runtime_core::native::image::Image<'_>,
    name: &str,
) -> anyhow::Result<u32> {
    image
        .find_variable(name)
        .map_err(|error| anyhow!("'{name}' {error}"))
}

/// The value at process-image `address` of the kernel's program.
fn read_native_image(
    program: &native::NativeProgram<'_, native::HostCode>,
    address: &str,
) -> anyhow::Result<Value> {
    let address = IoAddress::parse(address).map_err(|error| anyhow!("{error}"))?;
    program
        .read_address(&address)
        .map_err(|error| anyhow!("{error}"))
}

/// Writes `value` at process-image `address` of the kernel's program.
fn write_native_image(
    program: &mut native::NativeProgram<'_, native::HostCode>,
    address: &str,
    value: Value,
) -> anyhow::Result<()> {
    let address = IoAddress::parse(address).map_err(|error| anyhow!("{error}"))?;
    program
        .write_address(&address, value)
        .map_err(|error| anyhow!("{error}"))
}
