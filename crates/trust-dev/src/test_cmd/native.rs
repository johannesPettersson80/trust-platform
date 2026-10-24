/// The statements one test may run in a compiled program before it ends with the work limit
/// (specification 44 §8.4), the compiled counterpart of the bytecode runner's timeout.
const NATIVE_TEST_STATEMENTS: u32 = 100_000_000;

/// The name of the test program that runs `TEST_FUNCTION_BLOCK` `name` in a compiled image.
fn function_block_runner(name: &str) -> SmolStr {
    SmolStr::new(format!("TRUST_TEST_FB_{name}"))
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
/// (specification 44 §13.2, §14.2).
fn build_test_image(
    project_root: &Path,
    discovered: &[DiscoveredTest],
    target: trust_runtime::native::Target,
    toolchain: &trust_runtime::native::Toolchain,
) -> anyhow::Result<trust_runtime::native::Build> {
    use trust_runtime::harness::SourceFile;
    use trust_runtime::native::{build_with_tests, debug_hooks, Options};

    let hooks = debug_hooks(project_root)?;
    let mut sources = collect_project_source_files(project_root, None)?;
    let runners = function_block_runners(discovered);
    if !runners.is_empty() {
        sources.push(SourceFile::with_path(
            "trust-test-function-blocks.st",
            runners,
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
    let built = build_with_tests(
        &sources,
        &programs,
        &Options { target, hooks },
        toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    built.map_err(|error| match error {
        trust_runtime::native::NativeError::Unsupported { .. } => {
            anyhow::Error::new(NotCompiledYet(error.to_string()))
        }
        error => anyhow::anyhow!("the project does not compile: {error}"),
    })
}

/// A construct of the project that the generator does not compile yet (specification 44 NX-09).
#[derive(Debug)]
struct NotCompiledYet(String);

impl std::fmt::Display for NotCompiledYet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "the project does not compile: {}", self.0)
    }
}

impl std::error::Error for NotCompiledYet {}

/// Runs `tests` of `discovered` compiled into one program image for this computer (specification
/// 44 §14.2): each test after a cold restart, its program's body once, or one call of a test
/// function block's body through a test program that holds an instance of it, as the bytecode
/// runner runs them, with the toolchain at `TRUST_TOOLCHAIN`.
fn execute_native_tests(
    project_root: &Path,
    discovered: &[DiscoveredTest],
    tests: &[DiscoveredTest],
    timeout: u64,
) -> anyhow::Result<Vec<ExecutedTest>> {
    use trust_runtime::native::{load, this_computer, Device, Target, Toolchain};
    use trust_runtime::value::Duration;

    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("TRUST_TOOLCHAIN names no toolchain: {error}"))?;
    let host = Target::this_cpu().context("this computer's CPU has no native target")?;
    let built = build_test_image(project_root, discovered, host, &toolchain)?;
    let device = Device {
        max_statements: NATIVE_TEST_STATEMENTS,
        ..this_computer().context("this computer's CPU runs no program image")?
    };
    let mut program = load(&built.image, &device)
        .map_err(|error| anyhow::anyhow!("the program image does not load: {error}"))?;
    let mut results = Vec::with_capacity(tests.len());
    for case in tests {
        let started = Instant::now();
        let name = test_program(case);
        let instance = built
            .instances
            .iter()
            .position(|instance| instance.eq_ignore_ascii_case(name.as_str()))
            .and_then(|index| u32::try_from(index).ok());
        let result = match instance {
            Some(instance) => program
                .cold_start(Duration::ZERO)
                .and_then(|()| program.run_instance(instance, Duration::ZERO)),
            None => Err(RuntimeError::UndefinedProgram(case.name.clone())),
        };
        results.push(executed(case, result, timeout, started));
    }
    Ok(results)
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
) -> anyhow::Result<Vec<ExecutedTest>> {
    use trust_runtime::device_link::{self, DeviceLink};
    use trust_runtime::native::{target_named, Toolchain};

    let target =
        target_named(target).ok_or_else(|| anyhow::anyhow!("unknown target '{target}'"))?;
    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("TRUST_TOOLCHAIN names no toolchain: {error}"))?;
    let mut built = build_test_image(project_root, discovered, target, &toolchain)?;
    // The device admits what a key it trusts signed: the project's (specification 44 §5.11).
    trust_runtime::native::sign(
        &mut built.image,
        &trust_runtime::native::project_key(project_root)?,
    )?;
    let work = std::env::temp_dir().join(format!("trust-dev-device-{}", std::process::id()));
    let mut link = DeviceLink::open(port)?;
    let prepared = link
        .prepare_steps(target, &toolchain, &work.join("idle"))
        .and_then(|()| device_link::stepping_container(&built.image, &work.join("tests")))
        .and_then(|container| link.install(&container));
    let _ = std::fs::remove_dir_all(&work);
    prepared.map_err(|error| anyhow::anyhow!("install the tests on the device: {error}"))?;
    let mut results = Vec::with_capacity(tests.len());
    for case in tests {
        let started = Instant::now();
        let stepped = link.call(
            "cycle.step",
            Some(json!({"time_ns": 0, "restart": "cold", "run": test_program(case).as_str()})),
            device_link::REPLY_WAIT,
        );
        let error = match &stepped {
            Ok(result) => result["errors"]
                .get(0)
                .map(|error| error.as_str().unwrap_or_default().to_string()),
            Err(error) => Some(error.to_string()),
        };
        results.push(executed_on_device(case, error.as_deref(), timeout, started));
    }
    Ok(results)
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
    }
}
