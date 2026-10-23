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
    use trust_runtime::harness::SourceFile;
    use trust_runtime::native::{build_with_tests, load, Device, Options, Target, Toolchain};
    use trust_runtime::value::Duration;

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
        .map(|case| match case.kind {
            TestKind::Program => case.name.clone(),
            TestKind::FunctionBlock => function_block_runner(case.name.as_str()),
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let toolchain = Toolchain::from_env()
        .map_err(|error| anyhow::anyhow!("TRUST_TOOLCHAIN names no toolchain: {error}"))?;
    let host = Target::this_cpu().context("this computer's CPU has no native target")?;
    let work = std::env::temp_dir().join(format!("trust-dev-test-{}", std::process::id()));
    let built = build_with_tests(
        &sources,
        &programs,
        &Options {
            target: host,
            hooks: false,
        },
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    let built = built.map_err(|error| anyhow::anyhow!("the project does not compile: {error}"))?;
    let device = Device {
        target: host,
        program_memory: 1 << 30,
        task_stack: 1 << 24,
        max_statements: NATIVE_TEST_STATEMENTS,
    };
    let mut program = load(&built.image, &device)
        .map_err(|error| anyhow::anyhow!("the program image does not load: {error}"))?;
    let mut results = Vec::with_capacity(tests.len());
    for case in tests {
        let started = Instant::now();
        let name = match case.kind {
            TestKind::Program => case.name.clone(),
            TestKind::FunctionBlock => function_block_runner(case.name.as_str()),
        };
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
