/// The statements one test may run in a compiled program before it ends with the work limit
/// (specification 44 §8.4), the compiled counterpart of the bytecode runner's timeout.
const NATIVE_TEST_STATEMENTS: u32 = 100_000_000;

/// Runs `tests` of `discovered` compiled into one program image for this computer (specification
/// 44 §14.2): each test after a cold restart, its program's body once, as the bytecode runner runs
/// it, with the toolchain at `TRUST_TOOLCHAIN`.
fn execute_native_tests(
    project_root: &Path,
    discovered: &[DiscoveredTest],
    tests: &[DiscoveredTest],
    timeout: u64,
) -> anyhow::Result<Vec<ExecutedTest>> {
    use trust_runtime::native::{build_with_tests, load, Device, Options, Target, Toolchain};
    use trust_runtime::value::Duration;

    let sources = collect_project_source_files(project_root, None)?;
    let programs: Vec<SmolStr> = discovered
        .iter()
        .filter(|case| matches!(case.kind, TestKind::Program))
        .map(|case| case.name.clone())
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
        let result = match case.kind {
            TestKind::FunctionBlock => Err(RuntimeError::ControlError(
                "a TEST_FUNCTION_BLOCK is not compiled yet (specification 44 NX-09)".into(),
            )),
            TestKind::Program => {
                let instance = built
                    .instances
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case(case.name.as_str()))
                    .and_then(|index| u32::try_from(index).ok());
                match instance {
                    Some(instance) => program
                        .cold_start(Duration::ZERO)
                        .and_then(|()| program.run_instance(instance, Duration::ZERO)),
                    None => Err(RuntimeError::UndefinedProgram(case.name.clone())),
                }
            }
        };
        results.push(executed(case, result, timeout, started));
    }
    Ok(results)
}
