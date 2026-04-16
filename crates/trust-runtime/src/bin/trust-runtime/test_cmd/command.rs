pub fn run_test(
    project: Option<PathBuf>,
    filter: Option<String>,
    list: bool,
    timeout: u64,
    output: TestOutput,
    ci: bool,
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

    let mut tests = discover_tests(&sources);
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

    let compile_sources = collect_project_source_files(&project_root, None)?;
    let extra_program_instances = tests
        .iter()
        .filter(|case| matches!(case.kind, TestKind::Program))
        .map(|case| case.name.clone())
        .collect::<BTreeSet<_>>();
    let session = CompileSession::from_sources(compile_sources)
        .with_extra_program_instances(extra_program_instances);
    let mut runtime = session.build_runtime()?;
    let bytecode = session.build_bytecode_bytes()?;
    runtime
        .apply_bytecode_bytes(&bytecode, None)
        .context("failed to preload bytecode for ST test execution")?;

    let test_timeout = if timeout == 0 {
        None
    } else {
        Some(StdDuration::from_secs(timeout))
    };
    let total_started = Instant::now();
    let mut results = Vec::with_capacity(tests.len());
    for case in &tests {
        let case_started = Instant::now();
        let result = match execute_test_case_in_runtime(&mut runtime, case, test_timeout) {
            Ok(()) => ExecutedTest {
                case: case.clone(),
                outcome: TestOutcome::Passed,
                message: None,
                duration_ms: elapsed_ms(case_started.elapsed()),
            },
            Err(RuntimeError::AssertionFailed(message)) => ExecutedTest {
                case: case.clone(),
                outcome: TestOutcome::Failed,
                message: Some(message.to_string()),
                duration_ms: elapsed_ms(case_started.elapsed()),
            },
            Err(RuntimeError::ExecutionTimeout) => ExecutedTest {
                case: case.clone(),
                outcome: TestOutcome::Error,
                message: Some(timeout_message(timeout)),
                duration_ms: elapsed_ms(case_started.elapsed()),
            },
            Err(err) => ExecutedTest {
                case: case.clone(),
                outcome: TestOutcome::Error,
                message: Some(err.to_string()),
                duration_ms: elapsed_ms(case_started.elapsed()),
            },
        };
        results.push(result);
    }
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
