/// The statements one test may run in a compiled program before it ends with the work limit
/// (specification 44 §8.4), the compiled counterpart of the bytecode runner's timeout.
const NATIVE_TEST_STATEMENTS: u32 = 100_000_000;

/// The name of the test program that runs `TEST_FUNCTION_BLOCK` `name` in a compiled image: an
/// identifier (IEC 61131-3 §6.1.2) for every name, a qualified one included, whose dots become
/// underscores, followed by a hash of the whole name so that `A.B_C` and `A_B.C` stay apart.
fn function_block_runner(name: &str) -> SmolStr {
    // FNV-1a, 32 bits: stable across builds and platforms.
    let hash = name.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    let flat: String = name
        .split('.')
        .map(|part| part.trim_matches('_'))
        .collect::<Vec<_>>()
        .join("_");
    SmolStr::new(format!("TRUST_TEST_FB_{flat}_{hash:08x}"))
}

/// A test program for each `TEST_FUNCTION_BLOCK` of `discovered`: one instance of it, whose body it
/// calls once, as the bytecode runner runs a temporary instance of it.
fn function_block_runners(discovered: &[DiscoveredTest]) -> String {
    let mut source = String::new();
    for case in discovered
        .iter()
        .filter(|case| matches!(case.kind, TestKind::FunctionBlock))
    {
        let _ = write!(
            source,
            "TEST_PROGRAM {}\nVAR\n    Test : {};\nEND_VAR\nTest();\nEND_TEST_PROGRAM\n\n",
            function_block_runner(case.name.as_str()),
            case.name
        );
    }
    source
}

/// The name of the test program that runs `case`: its own, or for a `TEST_FUNCTION_BLOCK` the test
/// program that calls it once.
fn test_program(case: &DiscoveredTest) -> SmolStr {
    match case.kind {
        TestKind::Program => case.name.clone(),
        TestKind::FunctionBlock => function_block_runner(case.name.as_str()),
    }
}

/// The program image of the project at `project_root` with every test of `discovered`, each an
/// instance of its test program that no task runs, compiled for `target` as the project's
/// `[build] debug` chooses, a debug or a release image like the one the project deploys
/// (specification 44 §13.2, §14.2), with the conversion rules of its `vendor_profile`, as
/// `trust-runtime run` runs the project (docs/specs/07-standard-functions.md 2.10).
fn build_test_image(
    project_root: &Path,
    discovered: &[DiscoveredTest],
    target: trust_runtime::native::Target,
    toolchain: &trust_runtime::native::Toolchain,
) -> anyhow::Result<trust_runtime::native::Build> {
    use trust_runtime::harness::SourceFile;
    use trust_runtime::native::{build_with_conversions, conversion_profile, debug_hooks, Options};

    let hooks = debug_hooks(project_root)?;
    // A `vendor_profile` that names no conversion rules refuses the run before anything builds.
    let conversions = conversion_profile(project_root)?;
    // Each file by its path relative to the project folder, as `trust-runtime build` labels it
    // (specification 44 §5.3): a compile error and a device's statement name `src/tests.st`.
    let mut sources = trust_runtime::bundle_builder::collect_project_sources_labelled(project_root, &[])?;
    let runners = function_block_runners(discovered);
    if !runners.is_empty() {
        sources.push(SourceFile::with_path(
            "trust-test-function-blocks.st",
            runners,
        ));
    }
    // The mark `TEST_FINISHED()` sets, which the runner reads after each cycle of a multi-cycle test (specification 22).
    if discovered.iter().any(|case| case.cycles.is_some()) {
        sources.push(SourceFile::with_path(
            "trust-test-finished.st",
            format!(
                "VAR_GLOBAL\n    {} : BOOL;\nEND_VAR\n",
                trust_runtime::native::TEST_FINISHED_GLOBAL
            ),
        ));
    }
    let programs: Vec<SmolStr> = discovered
        .iter()
        .map(test_program)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    // A folder of this process and this project, so that builds of two projects running at once in
    // one process, as the tests of trust-dev do, never share one: a build removes its folder when
    // it ends.
    let project_name = project_root
        .file_name()
        .map_or_else(|| "project".into(), |name| name.to_string_lossy());
    let work = std::env::temp_dir().join(format!(
        "trust-dev-test-{}-{project_name}",
        std::process::id()
    ));
    let built = build_with_conversions(
        &sources,
        &programs,
        &Options { target, hooks },
        conversions,
        toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    built.map_err(|error| anyhow::anyhow!("the project does not compile: {error}"))
}

/// Compiles the project at `project_root` for this computer as a run of its tests compiles it, for a
/// project without tests: a project that does not compile fails the run however many tests it has.
fn check_project(project_root: &Path) -> anyhow::Result<()> {
    use trust_runtime::native::{Target, Toolchain};

    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("the tests need the toolchain: {error}"))?;
    let host = Target::this_cpu().context("this computer's CPU has no native target")?;
    build_test_image(project_root, &[], host, &toolchain).map(|_| ())
}

/// Runs `tests` of `discovered` compiled into one program image for this computer (specification
/// 44 §14.2): each test after a cold restart, its program's body once, or one call of a test
/// function block's body through a test program that holds an instance of it, with the toolchain of
/// §4.6.
fn execute_native_tests(
    project_root: &Path,
    discovered: &[DiscoveredTest],
    tests: &[DiscoveredTest],
    timeout: u64,
) -> anyhow::Result<Vec<ExecutedTest>> {
    use trust_runtime::native::{load, this_computer, Device, Target, Toolchain};
    use trust_runtime::value::Duration;

    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("the tests need the toolchain: {error}"))?;
    let host = Target::this_cpu().context("this computer's CPU has no native target")?;
    let built = build_test_image(project_root, discovered, host, &toolchain)?;
    let device = Device {
        max_statements: NATIVE_TEST_STATEMENTS,
        ..this_computer().context("this computer's CPU runs no program image")?
    };
    let mut program = load(&built.image, &device)
        .map_err(|error| anyhow::anyhow!("the program image does not load: {error}"))?;
    let mut results = Vec::with_capacity(tests.len());
    // The project's simulation, read once for the tests that run it (specification 22, "A plant in the loop").
    let mut simulation = None;
    for case in tests {
        let started = Instant::now();
        let name = test_program(case);
        let instance = built
            .instances
            .iter()
            .position(|instance| instance.eq_ignore_ascii_case(name.as_str()))
            .and_then(|index| u32::try_from(index).ok());
        if let (Some(_), Some(cycles)) = (instance, case.cycles)
            && cycles.simulation
        {
            let config = simulation.get_or_insert_with(|| project_simulation(project_root));
            results.push(match config {
                Ok(config) => {
                    cycles_simulated(&built.image, case, cycles, config, timeout, started)
                }
                Err(text) => error_result(case, text.clone(), started),
            });
            continue;
        }
        if let (Some(instance), Some(cycles)) = (instance, case.cycles) {
            results.push(cycles_here(&mut program, instance, case, cycles, timeout, started));
            continue;
        }
        let result = match instance {
            Some(instance) => program
                .cold_start(Duration::ZERO)
                .and_then(|()| program.run_instance(instance, Duration::ZERO)),
            None => Err(RuntimeError::UndefinedProgram(case.name.clone())),
        };
        let mut done = executed(case, result, timeout, started);
        // The statement that raised a failure or an error of this run (specification 22); a test
        // that ran nothing has none, whatever an earlier test's run left.
        if instance.is_some() {
            done.failed_at = trust_runtime::native::fault_at(&program);
            done.assertion = program
                .failed_assertion()
                .map(|assertion| trust_runtime::native::assertion_json(&assertion));
        }
        results.push(done);
    }
    Ok(results)
}

/// The project's simulation for the tests that run it (specification 22, "A plant in the loop"): its `simulation.toml`, run
/// whatever the file's `enabled` says, as the test asks for it; the text of the error that ends such a test otherwise.
fn project_simulation(
    project_root: &Path,
) -> Result<trust_runtime::simulation::SimulationConfig, String> {
    let path = project_root.join("simulation.toml");
    match trust_runtime::simulation::SimulationConfig::load_optional(&path) {
        Ok(Some(mut config)) => {
            config.enabled = true;
            Ok(config)
        }
        Ok(None) => Err(
            "the test runs the project's simulation ({attribute 'test_simulation'}), but the project has no simulation.toml"
                .to_string(),
        ),
        Err(error) => Err(format!("the project's simulation.toml does not load: {error}")),
    }
}

/// The result of the test `case` that the error `text` ended before it ran.
fn error_result(case: &DiscoveredTest, text: String, started: Instant) -> ExecutedTest {
    ExecutedTest {
        case: case.clone(),
        outcome: TestOutcome::Error,
        message: Some(text),
        duration_ms: elapsed_ms(started.elapsed()),
        cycles: None,
        failed_at: None,
        assertion: None,
    }
}

/// Runs the multi-cycle test `case` with the project's simulation `config` in lockstep (specification 22, "A plant in the
/// loop"): the test image loaded as a runtime of its own and restarted cold at time zero, and each cycle the simulation's work
/// before a scan, the test program's body by one step, then the simulation's work after it (specification 46 §2), as
/// `trust-runtime run --simulation` runs a scan.
fn cycles_simulated(
    image: &[u8],
    case: &DiscoveredTest,
    cycles: TestCycles,
    config: &trust_runtime::simulation::SimulationConfig,
    timeout: u64,
    started: Instant,
) -> ExecutedTest {
    use trust_runtime::native::TEST_FINISHED_GLOBAL;
    use trust_runtime::scheduler::StepRequest;
    use trust_runtime::simulation::SimulationController;
    use trust_runtime::value::{Duration, Value};
    use trust_runtime::{CompiledRuntime, RestartMode};
    let mut runtime = match CompiledRuntime::load(image.to_vec(), "RESOURCE") {
        Ok(runtime) => runtime,
        Err(error) => {
            return error_result(case, format!("the program image does not load: {error}"), started);
        }
    };
    let mut simulation = SimulationController::new(config.clone());
    let request = |now: Duration| StepRequest {
        time: now,
        restart: None,
        inputs: Vec::new(),
        direct_inputs: Vec::new(),
        watch: vec![SmolStr::new(TEST_FINISHED_GLOBAL)],
        watch_direct: Vec::new(),
        run: Some(test_program(case)),
        // The plants reach the program's located variables through its process image, as in a scan.
        io: true,
    };
    let (mut failed_at, mut assertion) = (None, None);
    let (outcome, message, run) = run_cycles(cycles, timeout, started, |n, time_ns| {
        let now = Duration::from_nanos(i64::try_from(time_ns).unwrap_or(i64::MAX));
        // The cold restart comes first, so that the plants' first feedback reaches the first cycle.
        if n == 1
            && let Err(error) = runtime.restart(RestartMode::Cold)
        {
            return CycleResult::Error(error.to_string());
        }
        if let Err(error) = simulation.apply_pre_cycle(now, &mut runtime) {
            return CycleResult::Error(error.to_string());
        }
        let stepped = match runtime.step(&request(now), Duration::ZERO) {
            Ok(stepped) => stepped,
            Err(refusal) => {
                return CycleResult::Error(format!("the step was refused: {refusal:?}"));
            }
        };
        if let Some(error) = stepped.error {
            failed_at = stepped.error_at;
            assertion = stepped
                .assertion
                .map(|failed| trust_runtime::native::assertion_json(&failed));
            return match error {
                RuntimeError::AssertionFailed(message) => CycleResult::Failed(message.to_string()),
                error => CycleResult::Error(error.to_string()),
            };
        }
        if let Err(error) = simulation.apply_post_cycle(now, &runtime) {
            return CycleResult::Error(error.to_string());
        }
        CycleResult::Ran(
            stepped
                .values
                .iter()
                .any(|(name, value)| name == TEST_FINISHED_GLOBAL && *value == Value::Bool(true)),
        )
    });
    ExecutedTest {
        case: case.clone(),
        outcome,
        message,
        duration_ms: elapsed_ms(started.elapsed()),
        cycles: Some(run),
        failed_at,
        assertion,
    }
}

/// Runs the multi-cycle test `case`, test program `instance` of `program`, on this computer (specification 22, "Multi-cycle
/// tests"): a cold restart at time zero, then its body once per cycle at the program's time, with the finished mark read after
/// each cycle.
fn cycles_here(
    program: &mut trust_runtime::native::NativeProgram<'_, trust_runtime::native::HostCode>,
    instance: u32,
    case: &DiscoveredTest,
    cycles: TestCycles,
    timeout: u64,
    started: Instant,
) -> ExecutedTest {
    use trust_runtime::value::{Duration, Value};
    let (mut failed_at, mut assertion) = (None, None);
    let (outcome, message, run) = run_cycles(cycles, timeout, started, |n, time_ns| {
        let now = Duration::from_nanos(i64::try_from(time_ns).unwrap_or(i64::MAX));
        let ran = if n == 1 {
            program
                .cold_start(now)
                .and_then(|()| program.run_instance(instance, now))
        } else {
            program.run_instance(instance, now)
        };
        match ran {
            Ok(()) => CycleResult::Ran(matches!(
                program.value(trust_runtime::native::TEST_FINISHED_GLOBAL),
                Some(Value::Bool(true))
            )),
            Err(error) => {
                failed_at = trust_runtime::native::fault_at(program);
                assertion = program
                    .failed_assertion()
                    .map(|failed| trust_runtime::native::assertion_json(&failed));
                match error {
                    RuntimeError::AssertionFailed(message) => CycleResult::Failed(message.to_string()),
                    error => CycleResult::Error(error.to_string()),
                }
            }
        }
    });
    ExecutedTest {
        case: case.clone(),
        outcome,
        message,
        duration_ms: elapsed_ms(started.elapsed()),
        cycles: Some(run),
        failed_at,
        assertion,
    }
}

/// Runs `tests` of `discovered` on the device at `port` (specification 44 §14.2): the same image,
/// compiled for the device's `target`, installed once, and each test by one `cycle.step` that
/// restarts the program cold and runs the test program's body (`run`, specification 11 §6.8.1).
/// The device's limits are its own, its statement budget among them.
fn execute_native_tests_on_device(
    project_root: &Path,
    discovered: &[DiscoveredTest],
    tests: &[DiscoveredTest],
    timeout: u64,
    port: &str,
    target: &str,
    token: Option<&str>,
) -> anyhow::Result<Vec<ExecutedTest>> {
    use trust_runtime::device_link::{self, DeviceLink};
    use trust_runtime::native::{target_named, Toolchain};

    let target =
        target_named(target).ok_or_else(|| anyhow::anyhow!("unknown target '{target}'"))?;
    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("the tests need the toolchain: {error}"))?;
    let mut built = build_test_image(project_root, discovered, target, &toolchain)?;
    // The device admits what a key it trusts signed: the project's (specification 44 §5.11).
    trust_runtime::native::sign(
        &mut built.image,
        &trust_runtime::native::project_key(project_root)?,
    )?;
    let work = std::env::temp_dir().join(format!("trust-dev-device-{}", std::process::id()));
    // A device whose settings set a control token refuses a request without it (specification 34
    // PORT-06): the caller's token, else the project's, which the device was installed with.
    let token = trust_runtime::control::endpoint::device_token(Some(project_root), token)?;
    let mut link = DeviceLink::open(port, token.as_deref())?;
    // Which update the device and these tools need (specification 34 TARGET-05).
    if let Some(advice) = link.version_advice() {
        eprintln!("warning: {advice}");
    }
    let prepared = link
        .prepare_steps(target, &toolchain, &work.join("idle"))
        .and_then(|()| device_link::stepping_container(&built.image, &work.join("tests")))
        .and_then(|container| link.install(&container));
    let _ = std::fs::remove_dir_all(&work);
    prepared.map_err(|error| anyhow::anyhow!("install the tests on the device: {error}"))?;
    let mut results = Vec::with_capacity(tests.len());
    // The stepped scans keep one logical time across the tests (specification 11 §6.8.1), so each
    // test starts where the previous test's last step was; a test's own time, which its timers
    // measure and its result reports, starts there (specification 22, "Multi-cycle tests").
    let mut stepped_ns = 0_u64;
    // The project's simulation and a copy of the tests built for this computer, which its plants run against, made once for
    // the tests that run it (specification 22, "A plant in the loop").
    let mut simulated: Option<Result<(trust_runtime::simulation::SimulationConfig, Vec<u8>), String>> =
        None;
    for case in tests {
        let started = Instant::now();
        if let Some(cycles) = case.cycles.filter(|cycles| cycles.simulation) {
            let setup = simulated.get_or_insert_with(|| {
                let config = project_simulation(project_root)?;
                let host = trust_runtime::native::Target::this_cpu()
                    .ok_or_else(|| "this computer's CPU has no native target".to_string())?;
                let copy = build_test_image(project_root, discovered, host, &toolchain)
                    .map_err(|error| error.to_string())?;
                Ok((config, copy.image))
            });
            let executed = match setup {
                Ok((config, copy)) => cycles_simulated_on_device(
                    &mut link, copy, case, cycles, config, timeout, started, stepped_ns,
                ),
                Err(text) => error_result(case, text.clone(), started),
            };
            if let Some(run) = executed.cycles {
                stepped_ns = stepped_ns.saturating_add(run.time_ns);
            }
            results.push(executed);
            continue;
        }
        if let Some(cycles) = case.cycles {
            let executed = cycles_on_device(&mut link, case, cycles, timeout, started, stepped_ns);
            if let Some(run) = executed.cycles {
                stepped_ns = stepped_ns.saturating_add(run.time_ns);
            }
            results.push(executed);
            continue;
        }
        let stepped = link.call(
            "cycle.step",
            Some(json!({"time_ns": stepped_ns, "restart": "cold", "run": test_program(case).as_str()})),
            device_link::REPLY_WAIT,
        );
        let error = match &stepped {
            Ok(result) => result["errors"]
                .get(0)
                .map(|error| error.as_str().unwrap_or_default().to_string()),
            Err(error) => Some(error.to_string()),
        };
        let mut done = executed_on_device(case, error.as_deref(), timeout, started);
        done.failed_at = stepped.as_ref().ok().and_then(step_error_at);
        done.assertion = stepped
            .as_ref()
            .ok()
            .and_then(|result| result.get("assertion").cloned());
        results.push(done);
    }
    Ok(results)
}

/// Runs the multi-cycle test `case` with the project's simulation `config` on the device behind `link` (specification 22, "A
/// plant in the loop"): the plants run on this computer, against a copy of the tests built for it (`copy`) that holds the
/// values the simulation reaches and runs no scan. Each cycle the simulation's work before a scan writes into the copy, the
/// step carries every value the simulation writes to the device (`inputs`, `direct_inputs`) and watches every value the
/// simulation reaches, the reply sets those values in the copy, and the simulation's work after the scan reads them there.
/// So the program on the device takes the plants' feedback, and the plants its commands, at the moments a run on this
/// computer has them, and a value the simulation leaves alone goes back to the device as the device last had it. The device
/// restarts cold at the first step, before the step's values are written (specification 11 §6.8.1).
#[allow(clippy::too_many_arguments)] // One test's run: its link, its plants' copy and its own time are separate inputs.
fn cycles_simulated_on_device(
    link: &mut trust_runtime::device_link::DeviceLink,
    copy: &[u8],
    case: &DiscoveredTest,
    cycles: TestCycles,
    config: &trust_runtime::simulation::SimulationConfig,
    timeout: u64,
    started: Instant,
    start_ns: u64,
) -> ExecutedTest {
    use trust_runtime::device_link;
    use trust_runtime::native::TEST_FINISHED_GLOBAL;
    use trust_runtime::simulation::{ProgramSignal, SimulationController};
    use trust_runtime::value::Duration;
    use trust_runtime::{CompiledRuntime, RestartMode};
    let mut copy = match CompiledRuntime::load(copy.to_vec(), "RESOURCE") {
        Ok(copy) => copy,
        Err(error) => {
            return error_result(case, format!("the program image does not load: {error}"), started);
        }
    };
    let mut simulation = SimulationController::new(config.clone());
    let signals = config.program_signals();
    let mut watch = vec![json!(TEST_FINISHED_GLOBAL)];
    let mut watch_direct = Vec::new();
    for signal in signals.written.iter().chain(signals.read.iter()) {
        let (list, name) = match signal {
            ProgramSignal::Variable(path) => (&mut watch, path.clone()),
            ProgramSignal::Address(address) => (&mut watch_direct, address.to_string()),
        };
        if !list.contains(&json!(name)) {
            list.push(json!(name));
        }
    }
    let (mut failed_at, mut assertion) = (None, None);
    let (outcome, message, run) = run_cycles(cycles, timeout, started, |n, time_ns| {
        let now = Duration::from_nanos(i64::try_from(time_ns).unwrap_or(i64::MAX));
        if n == 1
            && let Err(error) = copy.restart(RestartMode::Cold)
        {
            return CycleResult::Error(error.to_string());
        }
        if let Err(error) = simulation.apply_pre_cycle(now, &mut copy) {
            return CycleResult::Error(error.to_string());
        }
        let (inputs, direct_inputs) = match written_values(&copy, &signals.written) {
            Ok(values) => values,
            Err(error) => return CycleResult::Error(error),
        };
        let mut params = json!({
            "time_ns": start_ns.saturating_add(time_ns),
            "run": test_program(case).as_str(),
            "inputs": inputs,
            "direct_inputs": direct_inputs,
            "watch": watch,
            "watch_direct": watch_direct,
            "io": true,
        });
        if n == 1 {
            params["restart"] = json!("cold");
        }
        let result = match link.call("cycle.step", Some(params), device_link::REPLY_WAIT) {
            Ok(result) => result,
            Err(error) => return CycleResult::Error(error.to_string()),
        };
        if let Some(error) = result["errors"].get(0) {
            failed_at = step_error_at(&result);
            assertion = result.get("assertion").cloned();
            let text = error.as_str().unwrap_or_default();
            return match text.strip_prefix("assertion failed: ") {
                Some(message) => CycleResult::Failed(message.to_string()),
                None => CycleResult::Error(text.to_string()),
            };
        }
        if let Err(error) = take_values(&mut copy, &result) {
            return CycleResult::Error(error);
        }
        if let Err(error) = simulation.apply_post_cycle(now, &copy) {
            return CycleResult::Error(error.to_string());
        }
        CycleResult::Ran(result["values"][TEST_FINISHED_GLOBAL]["value"] == json!(true))
    });
    ExecutedTest {
        case: case.clone(),
        outcome,
        message,
        duration_ms: elapsed_ms(started.elapsed()),
        cycles: Some(run),
        failed_at,
        assertion,
    }
}

/// A step's typed value texts by the variable or the address each writes (specification 11 §6.8.1).
type ValueTexts = serde_json::Map<String, serde_json::Value>;

/// The values the simulation wrote into `copy` for the program's signals `written`, as a step's `inputs` and
/// `direct_inputs`: each as a typed value text (specification 11 §6.8.1).
fn written_values(
    copy: &trust_runtime::CompiledRuntime,
    written: &[trust_runtime::simulation::ProgramSignal],
) -> Result<(ValueTexts, ValueTexts), String> {
    use trust_runtime::device_link::typed_value_text;
    use trust_runtime::simulation::ProgramSignal;
    let (mut inputs, mut direct_inputs) = (ValueTexts::new(), ValueTexts::new());
    for signal in written {
        match signal {
            ProgramSignal::Variable(path) => {
                let value = copy
                    .with_program_view(|program| {
                        program
                            .locate(path)
                            .map_err(|error| error.to_string())
                            .and_then(|at| program.read_at(&at).map_err(|error| error.to_string()))
                    })
                    .map_err(|error| format!("the plants' {path}: {error}"))?;
                let text = typed_value_text(&value).map_err(|error| format!("the plants' {path}: {error}"))?;
                inputs.insert(path.clone(), json!(text));
            }
            ProgramSignal::Address(address) => {
                let value = copy
                    .read_io(address)
                    .map_err(|error| format!("the plants' {address}: {error}"))?;
                let text =
                    typed_value_text(&value).map_err(|error| format!("the plants' {address}: {error}"))?;
                direct_inputs.insert(address.to_string(), json!(text));
            }
        }
    }
    Ok((inputs, direct_inputs))
}

/// Sets in `copy` the values that the step's reply `result` read back, its `values` but the finished mark and its `direct`,
/// through the writes the plants use themselves.
fn take_values(copy: &mut trust_runtime::CompiledRuntime, result: &serde_json::Value) -> Result<(), String> {
    use trust_runtime::device_link::decode_value;
    use trust_runtime::native::TEST_FINISHED_GLOBAL;
    let empty = serde_json::Map::new();
    for (path, encoded) in result["values"].as_object().unwrap_or(&empty) {
        if path == TEST_FINISHED_GLOBAL {
            continue;
        }
        let value = decode_value(encoded).map_err(|error| format!("the device's {path}: {error}"))?;
        copy.with_program(|program| {
            program
                .locate(path)
                .map_err(|error| error.to_string())
                .and_then(|at| program.write_at(&at, &value).map_err(|error| error.to_string()))
        })
        .map_err(|error| format!("the device's {path}: {error}"))?;
    }
    for (text, encoded) in result["direct"].as_object().unwrap_or(&empty) {
        let address = trust_runtime::io::IoAddress::parse(text)
            .map_err(|error| format!("the device's {text}: {error}"))?;
        let value = decode_value(encoded).map_err(|error| format!("the device's {text}: {error}"))?;
        copy.write_io(&address, value)
            .map_err(|error| format!("the device's {text}: {error}"))?;
    }
    Ok(())
}

/// The outcome of `case` from the error text a device's step ended with, by the rules of
/// [`executed`]: none passes, an assertion's text (`assertion failed: <message>`) fails with its
/// message, and every other error is the test's error.
fn executed_on_device(
    case: &DiscoveredTest,
    error: Option<&str>,
    timeout: u64,
    started: Instant,
) -> ExecutedTest {
    let (outcome, message) = match error {
        None => (TestOutcome::Passed, None),
        Some(text) => match text.strip_prefix("assertion failed: ") {
            Some(message) => (TestOutcome::Failed, Some(message.to_string())),
            None if text == RuntimeError::ExecutionTimeout.to_string() => {
                (TestOutcome::Error, Some(timeout_message(timeout)))
            }
            None => (TestOutcome::Error, Some(text.to_string())),
        },
    };
    ExecutedTest {
        case: case.clone(),
        outcome,
        message,
        duration_ms: elapsed_ms(started.elapsed()),
        cycles: None,
        failed_at: None,
        assertion: None,
    }
}

/// Runs the multi-cycle test `case` on the device behind `link` (specification 22, "Multi-cycle tests"): one `cycle.step` per
/// cycle, the first restarting the program cold at the stepped time `start_ns`, where the previous test left it, and each later
/// one a cycle on, each reading the finished mark back (specification 11 §6.8.1). The cycles' times the result reports are the
/// test's own, from its start.
fn cycles_on_device(
    link: &mut trust_runtime::device_link::DeviceLink,
    case: &DiscoveredTest,
    cycles: TestCycles,
    timeout: u64,
    started: Instant,
    start_ns: u64,
) -> ExecutedTest {
    use trust_runtime::device_link;
    use trust_runtime::native::TEST_FINISHED_GLOBAL;
    let (mut failed_at, mut assertion) = (None, None);
    let (outcome, message, run) = run_cycles(cycles, timeout, started, |n, time_ns| {
        let mut params = json!({
            "time_ns": start_ns.saturating_add(time_ns),
            "run": test_program(case).as_str(),
            "watch": [TEST_FINISHED_GLOBAL],
        });
        if n == 1 {
            params["restart"] = json!("cold");
        }
        match link.call("cycle.step", Some(params), device_link::REPLY_WAIT) {
            Ok(result) => match result["errors"].get(0) {
                Some(error) => {
                    failed_at = step_error_at(&result);
                    assertion = result.get("assertion").cloned();
                    let text = error.as_str().unwrap_or_default();
                    match text.strip_prefix("assertion failed: ") {
                        Some(message) => CycleResult::Failed(message.to_string()),
                        None => CycleResult::Error(text.to_string()),
                    }
                }
                None => CycleResult::Ran(
                    result["values"][TEST_FINISHED_GLOBAL]["value"] == json!(true),
                ),
            },
            Err(error) => CycleResult::Error(error.to_string()),
        }
    });
    ExecutedTest {
        case: case.clone(),
        outcome,
        message,
        duration_ms: elapsed_ms(started.elapsed()),
        cycles: Some(run),
        failed_at,
        assertion,
    }
}

/// The statement a device's step names in `error_at` as the one that raised its error (specification 11 §6.8.1).
fn step_error_at(result: &serde_json::Value) -> Option<trust_runtime::native::StatementAt> {
    let at = result.get("error_at")?;
    Some(trust_runtime::native::StatementAt {
        file: at["file"].as_str()?.into(),
        line: u32::try_from(at["line"].as_u64()?).ok()?,
    })
}
