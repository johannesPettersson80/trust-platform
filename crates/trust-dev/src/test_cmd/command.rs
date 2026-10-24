/// What runs the tests: the bytecode engine, the kernel on this computer, or a device's kernel over
/// its control link (specification 44 §14.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestRunner {
    /// The bytecode engine.
    Bytecode,
    /// The project compiled for this computer and run by the kernel; with `fallback`, the bytecode
    /// engine runs the tests of a project with a construct the generator does not compile yet
    /// (specification 44 NX-09), as `trust-runtime build` builds it.
    Native {
        /// Whether the bytecode engine stands in for a construct not compiled yet.
        fallback: bool,
    },
    /// The project compiled for the device's target and run on the device at `port`.
    Device {
        /// A serial port, or `tcp://<host>:<port>` of a bridge to one.
        port: String,
        /// The device's target, as `trust-runtime build --target` names it.
        target: String,
    },
}

impl TestRunner {
    /// The runner of `backend`, on the device that `device` names with its target when it names one.
    /// Without a backend the tests run compiled when `toolchain`, the toolchain of specification 44
    /// §4.6, is installed, as `trust-runtime build` builds (NX-15), and on the bytecode engine
    /// otherwise.
    ///
    /// # Errors
    ///
    /// A device with the bytecode backend: a device runs compiled tests.
    pub fn of(
        backend: Option<TestBackend>,
        device: Option<(String, String)>,
        toolchain: bool,
    ) -> anyhow::Result<Self> {
        let fallback = backend.is_none();
        let backend = backend.unwrap_or(if toolchain || device.is_some() {
            TestBackend::Native
        } else {
            TestBackend::Bytecode
        });
        Ok(match (backend, device) {
            (TestBackend::Bytecode, None) => Self::Bytecode,
            (TestBackend::Native, None) => Self::Native { fallback },
            (TestBackend::Native, Some((port, target))) => Self::Device { port, target },
            (TestBackend::Bytecode, Some(_)) => {
                anyhow::bail!("a device runs compiled tests: use --execution-backend native")
            }
        })
    }
}

pub fn run_test(
    project: Option<PathBuf>,
    filter: Option<String>,
    list: bool,
    timeout: u64,
    output: TestOutput,
    ci: bool,
    runner: TestRunner,
) -> anyhow::Result<()> {
    let output = effective_output(output, ci);
    let project_root = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    let sources_root = resolve_sources_root(&project_root, None)?;

    let sources = load_sources(&sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let discovered = discover_tests(&sources);
    let mut tests = discovered.clone();
    let discovered_total = tests.len();
    if let Some(filter) = filter.as_deref() {
        let needle = filter.to_ascii_lowercase();
        tests.retain(|case| case.name.as_str().to_ascii_lowercase().contains(&needle));
    }

    if list {
        let rendered =
            render_list_output(&project_root, &tests, discovered_total, filter.as_deref());
        print!("{rendered}");
        return Ok(());
    }

    if tests.is_empty() && discovered_total > 0 {
        let rendered = render_output(
            output,
            &project_root,
            &[],
            TestSummary::default(),
            discovered_total,
            filter.as_deref(),
            0,
        )?;
        print!("{rendered}");
        return Ok(());
    }

    let mut runtime = match runner {
        TestRunner::Bytecode => Some(compile_test_runtime(&project_root, &discovered)?),
        TestRunner::Native { .. } | TestRunner::Device { .. } => None,
    };
    if tests.is_empty() {
        let rendered = render_output(
            output,
            &project_root,
            &[],
            TestSummary::default(),
            discovered_total,
            filter.as_deref(),
            0,
        )?;
        print!("{rendered}");
        return Ok(());
    }

    let test_timeout = if timeout == 0 {
        None
    } else {
        Some(StdDuration::from_secs(timeout))
    };
    let total_started = Instant::now();
    let results = match runtime.as_mut() {
        Some(runtime) => execute_tests_in_runtime(runtime, &tests, timeout, test_timeout),
        None => match &runner {
            TestRunner::Device { port, target } => execute_native_tests_on_device(
                &project_root,
                &discovered,
                &tests,
                timeout,
                port,
                target,
            )?,
            TestRunner::Native { fallback } => {
                match execute_native_tests(&project_root, &discovered, &tests, timeout) {
                    Err(error) if *fallback && error.is::<NotCompiledYet>() => {
                        eprintln!("note: {error}: running the tests on the bytecode engine");
                        let mut runtime = compile_test_runtime(&project_root, &discovered)?;
                        execute_tests_in_runtime(&mut runtime, &tests, timeout, test_timeout)
                    }
                    results => results?,
                }
            }
            TestRunner::Bytecode => unreachable!("the bytecode engine runs with a runtime"),
        },
    };
    let total_duration_ms = elapsed_ms(total_started.elapsed());

    let summary = summarize_results(&results);
    let rendered = render_output(
        output,
        &project_root,
        &results,
        summary,
        discovered_total,
        filter.as_deref(),
        total_duration_ms,
    )?;
    print!("{rendered}");

    if summary.has_failures() {
        anyhow::bail!("{} ST test(s) failed", summary.failed + summary.errors);
    }

    Ok(())
}

/// Runs `tests` on the bytecode engine's `runtime`, each after a cold restart.
fn execute_tests_in_runtime(
    runtime: &mut Runtime,
    tests: &[DiscoveredTest],
    timeout: u64,
    test_timeout: Option<StdDuration>,
) -> Vec<ExecutedTest> {
    let mut results = Vec::with_capacity(tests.len());
    for case in tests {
        let case_started = Instant::now();
        let result = execute_test_case_in_runtime(runtime, case, test_timeout);
        results.push(executed(case, result, timeout, case_started));
    }
    results
}

/// The outcome of `case` from its run's `result`: passed, failed with an assertion's message, or an
/// error, a timeout of `timeout` seconds among them.
fn executed(
    case: &DiscoveredTest,
    result: Result<(), RuntimeError>,
    timeout: u64,
    started: Instant,
) -> ExecutedTest {
    let (outcome, message) = match result {
        Ok(()) => (TestOutcome::Passed, None),
        Err(RuntimeError::AssertionFailed(message)) => {
            (TestOutcome::Failed, Some(message.to_string()))
        }
        Err(RuntimeError::ExecutionTimeout) => (TestOutcome::Error, Some(timeout_message(timeout))),
        Err(err) => (TestOutcome::Error, Some(err.to_string())),
    };
    ExecutedTest {
        case: case.clone(),
        outcome,
        message,
        duration_ms: elapsed_ms(started.elapsed()),
    }
}
