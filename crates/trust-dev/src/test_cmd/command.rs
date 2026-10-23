pub fn run_test(
    project: Option<PathBuf>,
    filter: Option<String>,
    list: bool,
    timeout: u64,
    output: TestOutput,
    ci: bool,
    backend: TestBackend,
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

    let mut runtime = match backend {
        TestBackend::Bytecode => Some(compile_test_runtime(&project_root, &discovered)?),
        TestBackend::Native => None,
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
        Some(runtime) => {
            let mut results = Vec::with_capacity(tests.len());
            for case in &tests {
                let case_started = Instant::now();
                let result = execute_test_case_in_runtime(runtime, case, test_timeout);
                results.push(executed(case, result, timeout, case_started));
            }
            results
        }
        None => execute_native_tests(&project_root, &discovered, &tests, timeout)?,
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
