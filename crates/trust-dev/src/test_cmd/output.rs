fn timeout_message(timeout_seconds: u64) -> String {
    if timeout_seconds == 1 {
        "test timed out after 1 second".to_string()
    } else {
        format!("test timed out after {timeout_seconds} seconds")
    }
}

fn elapsed_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn summarize_results(results: &[ExecutedTest]) -> TestSummary {
    let mut summary = TestSummary::default();
    for result in results {
        match result.outcome {
            TestOutcome::Passed => summary.passed += 1,
            TestOutcome::Failed => summary.failed += 1,
            TestOutcome::Error => summary.errors += 1,
        }
    }
    summary
}

fn render_output(
    output: TestOutput,
    project_root: &Path,
    results: &[ExecutedTest],
    summary: TestSummary,
    discovered_total: usize,
    filter: Option<&str>,
    total_duration_ms: u64,
) -> anyhow::Result<String> {
    match output {
        TestOutput::Human => Ok(render_human_output(
            project_root,
            results,
            summary,
            discovered_total,
            filter,
            total_duration_ms,
        )),
        TestOutput::Json => render_json_output(project_root, results, summary, total_duration_ms),
        TestOutput::Tap => Ok(render_tap_output(project_root, results)),
        TestOutput::Junit => Ok(render_junit_output(project_root, results, summary)),
    }
}

fn effective_output(output: TestOutput, ci: bool) -> TestOutput {
    if ci && matches!(output, TestOutput::Human) {
        TestOutput::Junit
    } else {
        output
    }
}

fn render_human_output(
    project_root: &Path,
    results: &[ExecutedTest],
    summary: TestSummary,
    discovered_total: usize,
    filter: Option<&str>,
    total_duration_ms: u64,
) -> String {
    let mut output = String::new();
    let mut failed_results = Vec::new();
    let _ = writeln!(
        output,
        "{}",
        style::accent(format!(
            "Running {} ST test(s) in {}",
            summary.total(),
            trust_lsp::check::native_path(project_root)
        ))
    );
    if results.is_empty() {
        render_no_tests_message(&mut output, filter, discovered_total);
    }
    for (idx, result) in results.iter().enumerate() {
        let prefix = format!("[{}/{}]", idx + 1, results.len());
        let test_id = format!("{}::{}", result.case.kind.label(), result.case.name);
        let display_path = display_path(project_root, &result.case.file);
        match result.outcome {
            TestOutcome::Passed => {
                let _ = writeln!(
                    output,
                    "{} {} {} ({}) [{}ms]",
                    style::success("PASS"),
                    prefix,
                    test_id,
                    display_path,
                    result.duration_ms
                );
            }
            TestOutcome::Failed => {
                let place = reported_place(project_root, result);
                let _ = writeln!(
                    output,
                    "{} {} {} {}:{} [{}ms]",
                    style::error("FAIL"),
                    prefix,
                    test_id,
                    place.file,
                    place.line,
                    result.duration_ms
                );
                let _ = writeln!(
                    output,
                    "    reason   : {}",
                    result.message.as_deref().unwrap_or("assertion failed")
                );
                if let Some(source_line) = place.source.as_deref() {
                    let _ = writeln!(output, "    source   : {source_line}");
                }
                failed_results.push(result);
            }
            TestOutcome::Error => {
                let place = reported_place(project_root, result);
                let _ = writeln!(
                    output,
                    "{} {} {} {}:{} [{}ms]",
                    style::error("ERROR"),
                    prefix,
                    test_id,
                    place.file,
                    place.line,
                    result.duration_ms
                );
                let _ = writeln!(
                    output,
                    "    reason   : {}",
                    result.message.as_deref().unwrap_or("runtime error")
                );
                if let Some(source_line) = place.source.as_deref() {
                    let _ = writeln!(output, "    source   : {source_line}");
                }
                failed_results.push(result);
            }
        }
    }
    if !failed_results.is_empty() {
        let _ = writeln!(output);
        let _ = writeln!(output, "{}", style::warning("Failure summary:"));
        for (idx, result) in failed_results.iter().enumerate() {
            let place = reported_place(project_root, result);
            let _ = writeln!(
                output,
                "  {}. {}::{} @ {}:{}",
                idx + 1,
                result.case.kind.label(),
                result.case.name,
                place.file,
                place.line
            );
            let _ = writeln!(
                output,
                "     {}",
                result.message.as_deref().unwrap_or(match result.outcome {
                    TestOutcome::Failed => "assertion failed",
                    TestOutcome::Error => "runtime error",
                    TestOutcome::Passed => "passed",
                })
            );
            if let Some(source_line) = place.source.as_deref() {
                let _ = writeln!(output, "     source: {source_line}");
            }
        }
    }
    let _ = writeln!(
        output,
        "{} passed, {} failed, {} errors ({}ms)",
        summary.passed, summary.failed, summary.errors, total_duration_ms
    );
    output
}

fn render_no_tests_message(output: &mut String, filter: Option<&str>, discovered_total: usize) {
    if let (Some(filter), total) = (filter, discovered_total)
        && total > 0
    {
        let _ = writeln!(
            output,
            "{}",
            style::warning(format!(
                "0 tests matched filter \"{filter}\" ({total} tests discovered, all filtered out)"
            ))
        );
        return;
    }
    let _ = writeln!(output, "{}", style::warning("No ST tests discovered."));
}

/// `file` as the output names it (specification 22): relative to the project folder, written as
/// the platform writes paths and without the verbatim prefix (`\\?\`) that canonicalization adds on
/// Windows, as `trust-lsp check` writes its paths; a file outside the project keeps its whole path.
/// A relative `--project` names the same folder as the canonical paths the sources are found by.
fn display_path(project_root: &Path, file: &Path) -> String {
    let root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    trust_lsp::check::native_path(file.strip_prefix(&root).unwrap_or(&file))
}

/// The project folder as the JSON report names it: its whole path, as [`display_path`] writes one.
fn display_project(project_root: &Path) -> String {
    let root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    trust_lsp::check::native_path(&root)
}

/// A place in the sources as the reports name it (specification 22): the file as [`display_path`]
/// writes it, the 1-based line and that line's text, trimmed, unless it is empty.
struct Place {
    file: String,
    line: usize,
    source: Option<String>,
}

/// The statement that raised `result`'s failure or error (specification 22), its file found by its
/// label, a path relative to the project folder or a whole one (specification 44 §5.9); `None`
/// when the runtime named no statement.
fn failure_place(project_root: &Path, result: &ExecutedTest) -> Option<Place> {
    let at = result.failed_at.as_ref()?;
    let file = project_root.join(at.file.as_str());
    let line = usize::try_from(at.line).ok()?;
    let source = std::fs::read_to_string(&file).ok().and_then(|text| {
        let text = text.lines().nth(line.checked_sub(1)?)?.trim();
        (!text.is_empty()).then(|| text.to_string())
    });
    Some(Place {
        file: display_path(project_root, &file),
        line,
        source,
    })
}

/// Where the human output and TAP report a failed or errored `result` (specification 22): at the
/// statement that raised its failure or error, else at the test's declaration.
fn reported_place(project_root: &Path, result: &ExecutedTest) -> Place {
    failure_place(project_root, result).unwrap_or_else(|| Place {
        file: display_path(project_root, &result.case.file),
        line: result.case.line,
        source: result.case.source_line.clone(),
    })
}

fn render_list_output(
    project_root: &Path,
    tests: &[DiscoveredTest],
    discovered_total: usize,
    filter: Option<&str>,
) -> String {
    let mut output = String::new();
    if tests.is_empty() {
        render_no_tests_message(&mut output, filter, discovered_total);
        return output;
    }
    for case in tests {
        let _ = writeln!(
            output,
            "{}::{} ({}:{})",
            case.kind.label(),
            case.name,
            display_path(project_root, &case.file),
            case.line
        );
    }
    let _ = writeln!(output, "{} test(s) listed", tests.len());
    output
}

fn render_json_output(
    project_root: &Path,
    results: &[ExecutedTest],
    summary: TestSummary,
    total_duration_ms: u64,
) -> anyhow::Result<String> {
    let tests = results
        .iter()
        .map(|result| {
            let mut test = json!({
                "name": result.case.name.as_str(),
                "kind": result.case.kind.label(),
                "status": result.outcome.as_str(),
                "file": display_path(project_root, &result.case.file),
                "line": result.case.line,
                "source": result.case.source_line.as_deref(),
                "message": result.message.as_deref(),
                "duration_ms": result.duration_ms,
            });
            // A test that failed at a statement names it beside its declaration (specification 22).
            if let Some(place) = failure_place(project_root, result) {
                test["failure"] = json!({
                    "file": place.file,
                    "line": place.line,
                    "source": place.source,
                });
            }
            // A failed assertion's values, typed (specification 22).
            if let Some(assertion) = &result.assertion {
                test["assertion"] = assertion.clone();
            }
            // A multi-cycle test reports the cycles it ran and the time of the last (specification 22).
            if let Some(run) = result.cycles {
                test["cycles"] = json!(run.cycles);
                test["simulated_time_ms"] = json!(run.time_ns / 1_000_000);
            }
            test
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "version": 1,
        "project": display_project(project_root),
        "summary": {
            "total": summary.total(),
            "passed": summary.passed,
            "failed": summary.failed,
            "errors": summary.errors,
            "duration_ms": total_duration_ms,
        },
        "tests": tests,
    });
    let mut text = serde_json::to_string_pretty(&payload)?;
    text.push('\n');
    Ok(text)
}

fn render_tap_output(project_root: &Path, results: &[ExecutedTest]) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "TAP version 13");
    let _ = writeln!(output, "1..{}", results.len());
    for (idx, result) in results.iter().enumerate() {
        let name = tap_escape(&format!(
            "{}::{}",
            result.case.kind.label(),
            result.case.name
        ));
        match result.outcome {
            TestOutcome::Passed => {
                let _ = writeln!(output, "ok {} - {}", idx + 1, name);
            }
            TestOutcome::Failed | TestOutcome::Error => {
                let _ = writeln!(output, "not ok {} - {}", idx + 1, name);
                let place = reported_place(project_root, result);
                let _ = writeln!(output, "# file: {}", place.file);
                let _ = writeln!(output, "# line: {}", place.line);
                if let Some(source_line) = place.source.as_deref() {
                    let _ = writeln!(output, "# source: {}", tap_escape(source_line));
                }
                if let Some(message) = &result.message {
                    for line in message.lines() {
                        let _ = writeln!(output, "# {}", tap_escape(line));
                    }
                }
            }
        }
    }
    output
}

fn render_junit_output(
    project_root: &Path,
    results: &[ExecutedTest],
    summary: TestSummary,
) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        output,
        "<testsuite name=\"trust-runtime\" tests=\"{}\" failures=\"{}\" errors=\"{}\" skipped=\"0\">",
        summary.total(),
        summary.failed,
        summary.errors
    );
    for result in results {
        let name = xml_escape(&format!(
            "{}::{}",
            result.case.kind.label(),
            result.case.name
        ));
        let file = xml_escape(&display_path(project_root, &result.case.file));
        let _ = writeln!(
            output,
            "  <testcase name=\"{}\" classname=\"st\" file=\"{}\" line=\"{}\">",
            name, file, result.case.line
        );
        match result.outcome {
            TestOutcome::Passed => {}
            TestOutcome::Failed => {
                let message_text = result.message.as_deref().unwrap_or("assertion failed");
                let message = xml_escape(message_text);
                let details = xml_escape(&junit_details(project_root, result, message_text));
                let _ = writeln!(
                    output,
                    "    <failure message=\"{}\">{}</failure>",
                    message, details
                );
            }
            TestOutcome::Error => {
                let message_text = result.message.as_deref().unwrap_or("runtime error");
                let message = xml_escape(message_text);
                let details = xml_escape(&junit_details(project_root, result, message_text));
                let _ = writeln!(
                    output,
                    "    <error message=\"{}\">{}</error>",
                    message, details
                );
            }
        }
        let _ = writeln!(output, "  </testcase>");
    }
    let _ = writeln!(output, "</testsuite>");
    output
}

/// The text of a JUnit failure or error (specification 22): its message, then the statement that
/// raised it as `at: <file>:<line>` with that statement's `source:`, or the source of the test's
/// declaration when the runtime named no statement.
fn junit_details(project_root: &Path, result: &ExecutedTest, message: &str) -> String {
    let mut details = String::from(message);
    let source = match failure_place(project_root, result) {
        Some(place) => {
            let _ = write!(details, "\nat: {}:{}", place.file, place.line);
            place.source
        }
        None => result.case.source_line.clone(),
    };
    if let Some(source) = source {
        let _ = write!(details, "\nsource: {source}");
    }
    details
}

fn tap_escape(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

fn xml_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
