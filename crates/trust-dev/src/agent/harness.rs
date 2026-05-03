use super::*;

impl AgentServer {
    pub(super) fn harness_load(
        &mut self,
        params: HarnessLoadParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let source_texts = self.collect_harness_sources(&params)?;
        let summary = self.harness.load_sources(&source_texts)?;
        Ok(json!({
            "source_count": source_texts.len(),
            "cycle_count": summary.cycle_count,
            "elapsed_ms": summary.elapsed_ms,
        }))
    }

    pub(super) fn harness_reload(
        &mut self,
        params: HarnessLoadParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let source_texts = self.collect_harness_sources(&params)?;
        let summary = self.harness.reload_sources(&source_texts)?;
        Ok(json!({
            "source_count": source_texts.len(),
            "cycle_count": summary.cycle_count,
            "elapsed_ms": summary.elapsed_ms,
        }))
    }

    pub(super) fn harness_cycle(
        &mut self,
        params: HarnessCycleParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let snapshot =
            self.harness
                .cycle(params.count, params.dt_ms.unwrap_or(0), &params.watch)?;
        Ok(json!({
            "cycle_count": snapshot.cycle_count,
            "elapsed_ms": snapshot.elapsed_ms,
            "values": encode_watch_snapshot(&snapshot.values),
        }))
    }

    pub(super) fn harness_execute(
        &self,
        params: HarnessExecuteParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let source_texts = self.collect_harness_sources(&params.load)?;
        let source_count = source_texts.len();
        let mut harness = HarnessAutomation::new();
        match harness.load_sources(&source_texts) {
            Ok(_) => {}
            Err(HarnessAutomationError::NotLoaded | HarnessAutomationError::InvalidArgument(_)) => {
                return Err(HarnessAutomationError::InvalidArgument(
                    "Harness fixture session failed to initialize due to invalid inputs."
                        .to_string(),
                )
                .into())
            }
            Err(error) => {
                return serde_json::to_value(build_harness_execute_result(
                    source_count,
                    0,
                    0,
                    0,
                    empty_watch_snapshot(),
                    vec![harness_execute_failure(None, None, error, None)],
                ))
                .map_err(anyhow::Error::from)
                .map_err(AgentCommandError::from_anyhow);
            }
        }

        let mut steps_run = 0;
        for (step_index, step) in params.steps.iter().enumerate() {
            if let Err(error) = apply_harness_execute_step(&mut harness, step) {
                return match error {
                    HarnessAutomationError::NotLoaded
                    | HarnessAutomationError::InvalidArgument(_) => Err(error.into()),
                    other => {
                        let snapshot = harness
                            .snapshot(&params.watch)
                            .map(|summary| encode_harness_watch_snapshot(&summary))
                            .unwrap_or_else(|_| empty_watch_snapshot());
                        serde_json::to_value(build_harness_execute_result(
                            source_count,
                            steps_run,
                            0,
                            params.assertions.len(),
                            snapshot,
                            vec![harness_execute_failure(
                                Some(step_index),
                                Some(step),
                                other,
                                Some(&mut harness),
                            )],
                        ))
                        .map_err(anyhow::Error::from)
                        .map_err(AgentCommandError::from_anyhow)
                    }
                };
            }
            steps_run += 1;
        }

        let mut failures = Vec::new();
        let mut assertions_passed = 0;
        for assertion in &params.assertions {
            if let Some(failure) = evaluate_harness_assertion(&mut harness, assertion)? {
                failures.push(failure);
            } else {
                assertions_passed += 1;
            }
        }

        let final_snapshot = harness
            .snapshot(&params.watch)
            .map(|summary| encode_harness_watch_snapshot(&summary))
            .map_err(AgentCommandError::from)?;

        serde_json::to_value(build_harness_execute_result(
            source_count,
            steps_run,
            assertions_passed,
            params.assertions.len(),
            final_snapshot,
            failures,
        ))
        .map_err(anyhow::Error::from)
        .map_err(AgentCommandError::from_anyhow)
    }

    pub(super) fn harness_set_input(
        &mut self,
        params: HarnessSetInputParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let value = decode_json_value(&params.value)?;
        self.harness.set_input(&params.name, value)?;
        Ok(json!({
            "name": params.name,
            "status": "ok",
        }))
    }

    pub(super) fn harness_get_output(
        &mut self,
        params: HarnessGetOutputParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let snapshot = self.harness.get_output(&params.name)?;
        Ok(json!({
            "name": snapshot.name,
            "value": encode_json_value(&snapshot.value),
        }))
    }

    pub(super) fn harness_advance_time(
        &mut self,
        params: HarnessAdvanceTimeParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let summary = self.harness.advance_time(params.duration_ms)?;
        Ok(json!({
            "cycle_count": summary.cycle_count,
            "elapsed_ms": summary.elapsed_ms,
        }))
    }

    pub(super) fn harness_run_until(
        &mut self,
        params: HarnessRunUntilParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let expected = decode_json_value(&params.equals)?;
        let summary = self.harness.run_until(
            &params.name,
            expected,
            params.dt_ms.unwrap_or(0),
            params.max_cycles.unwrap_or(10_000),
            &params.watch,
        )?;
        Ok(json!({
            "name": summary.name,
            "cycles_ran": summary.cycles_ran,
            "cycle_count": summary.cycle_count,
            "elapsed_ms": summary.elapsed_ms,
            "matched_value": encode_json_value(&summary.matched_value),
            "values": encode_watch_snapshot(&summary.values),
        }))
    }

    pub(super) fn resolve_project_root(
        &self,
        project: Option<&str>,
    ) -> Result<PathBuf, AgentCommandError> {
        match project {
            Some(path) => {
                let relative_path = normalize_workspace_path(path)?;
                let full_path = self.workspace_root.join(relative_path);
                if !full_path.is_dir() {
                    return Err(AgentCommandError::invalid_params(format!(
                        "Project path '{}' is not a directory inside the workspace root.",
                        full_path.display()
                    )));
                }
                Ok(full_path)
            }
            None => Ok(self.workspace_root.clone()),
        }
    }

    pub(super) fn resolve_project_subpath(
        &self,
        project_root: &Path,
        subpath: &str,
    ) -> Result<PathBuf, AgentCommandError> {
        let relative_path = normalize_workspace_path(subpath)?;
        Ok(project_root.join(relative_path))
    }

    pub(super) fn collect_harness_sources(
        &self,
        params: &HarnessLoadParams,
    ) -> Result<Vec<String>, AgentCommandError> {
        if let Some(inline_sources) = params.inline_sources.as_ref() {
            if inline_sources.is_empty() {
                return Err(AgentCommandError::invalid_params(
                    "inline_sources must not be empty when provided.",
                ));
            }
            return Ok(inline_sources
                .iter()
                .map(|source| source.text.clone())
                .collect());
        }

        if let Some(files) = params.files.as_ref() {
            if files.is_empty() {
                return Err(AgentCommandError::invalid_params(
                    "files must not be empty when provided.",
                ));
            }
            let mut sources = Vec::with_capacity(files.len());
            for file in files {
                let relative_path = normalize_workspace_path(file)?;
                let full_path = self.workspace_root.join(&relative_path);
                let text = fs::read_to_string(&full_path).map_err(|error| {
                    AgentCommandError::io(
                        format!("failed to read '{}': {error}", full_path.display()),
                        json!({
                            "path": relative_path.display().to_string(),
                        }),
                    )
                })?;
                sources.push(text);
            }
            return Ok(sources);
        }

        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let compile_sources = collect_project_source_files(&project_root, None)
            .map_err(AgentCommandError::from_anyhow)?;
        Ok(compile_sources
            .into_iter()
            .map(|source| source.text)
            .collect::<Vec<_>>())
    }
}

fn encode_watch_snapshot(
    values: &std::collections::BTreeMap<String, trust_runtime::harness::BoundaryEntry>,
) -> JsonValue {
    JsonValue::Object(
        values
            .iter()
            .map(|(name, entry)| (name.clone(), encode_boundary_entry(entry)))
            .collect::<serde_json::Map<String, JsonValue>>(),
    )
}

fn encode_boundary_entry(entry: &trust_runtime::harness::BoundaryEntry) -> JsonValue {
    if let Some(value) = entry.value.as_ref() {
        return json!({
            "status": "ok",
            "value": encode_json_value(value),
        });
    }
    let Some(error) = entry.error.as_ref() else {
        return json!({
            "status": "error",
            "code": "internal_failure",
            "message": "watch entry missing value and error",
        });
    };
    json!({
        "status": "error",
        "code": error.code(),
        "message": error.to_string(),
        "path": error.path(),
        "candidates": error.candidates().iter().map(|candidate| candidate.as_str()).collect::<Vec<_>>(),
    })
}

fn empty_watch_snapshot() -> JsonValue {
    json!({
        "cycleCount": 0,
        "elapsedMs": 0,
        "values": JsonValue::Object(serde_json::Map::new()),
    })
}

fn encode_harness_watch_snapshot(
    snapshot: &trust_runtime::harness::HarnessWatchSnapshot,
) -> JsonValue {
    json!({
        "cycleCount": snapshot.cycle_count,
        "elapsedMs": snapshot.elapsed_ms,
        "values": encode_watch_snapshot(&snapshot.values),
    })
}

fn build_harness_execute_result(
    source_count: usize,
    steps_run: usize,
    assertions_passed: usize,
    assertions_total: usize,
    watch_snapshot: JsonValue,
    failures: Vec<HarnessExecuteFailure>,
) -> HarnessExecuteResult {
    let assertion_failures = failures
        .iter()
        .filter(|failure| failure.kind == "assertion_failed")
        .count();
    HarnessExecuteResult {
        source_count,
        status: if failures.is_empty() { "pass" } else { "fail" },
        passed: failures.is_empty(),
        steps_run,
        assertions: HarnessExecuteAssertionSummary {
            total: assertions_total,
            evaluated: assertions_passed + assertion_failures,
            passed: assertions_passed,
            failed: assertion_failures,
        },
        watch_snapshot,
        failures,
    }
}

fn apply_harness_execute_step(
    harness: &mut HarnessAutomation,
    step: &HarnessExecuteStep,
) -> Result<(), HarnessAutomationError> {
    match step {
        HarnessExecuteStep::SetInput { name, value } => {
            harness.set_input(name, decode_json_value(value)?)?;
        }
        HarnessExecuteStep::SetAccess { name, value } => {
            harness.set_access(name, decode_json_value(value)?)?;
        }
        HarnessExecuteStep::BindDirect { name, address } => {
            harness.bind_direct(name, address)?;
        }
        HarnessExecuteStep::SetDirectInput { address, value } => {
            harness.set_direct_input(address, decode_json_value(value)?)?;
        }
        HarnessExecuteStep::AdvanceTime { duration_ms } => {
            harness.advance_time(*duration_ms)?;
        }
        HarnessExecuteStep::Cycle { count, dt_ms } => {
            harness.cycle(*count, dt_ms.unwrap_or(0), &[])?;
        }
        HarnessExecuteStep::RunUntil {
            name,
            equals,
            max_cycles,
            dt_ms,
        } => {
            harness.run_until(
                name,
                decode_json_value(equals)?,
                dt_ms.unwrap_or(0),
                max_cycles.unwrap_or(10_000),
                &[],
            )?;
        }
        HarnessExecuteStep::Restart { mode } => {
            harness.restart(parse_restart_mode(mode.as_deref().unwrap_or("cold"))?)?;
        }
    }
    Ok(())
}

fn parse_restart_mode(mode: &str) -> Result<RestartMode, HarnessAutomationError> {
    match mode.to_ascii_lowercase().as_str() {
        "cold" => Ok(RestartMode::Cold),
        "warm" => Ok(RestartMode::Warm),
        other => Err(HarnessAutomationError::InvalidArgument(format!(
            "unsupported restart mode '{other}'"
        ))),
    }
}

fn evaluate_harness_assertion(
    harness: &mut HarnessAutomation,
    assertion: &HarnessAssertion,
) -> Result<Option<HarnessExecuteFailure>, AgentCommandError> {
    let (actual, expected, mismatch_label) = match assertion {
        HarnessAssertion::OutputEquals { name, equals } => (
            harness.get_output(name)?.value,
            decode_json_value(equals)?,
            format!("output '{name}'"),
        ),
        HarnessAssertion::AccessEquals { name, equals } => (
            harness.get_access(name)?.value,
            decode_json_value(equals)?,
            format!("access '{name}'"),
        ),
        HarnessAssertion::DirectOutputEquals { address, equals } => (
            harness.get_direct_output(address)?.value,
            decode_json_value(equals)?,
            format!("direct output '{address}'"),
        ),
    };

    if actual == expected {
        return Ok(None);
    }

    Ok(Some(HarnessExecuteFailure {
        kind: "assertion_failed",
        step_index: None,
        step: None,
        assertion: Some(assertion.clone()),
        message: Some(format!(
            "{mismatch_label} did not match the expected value."
        )),
        expected: Some(encode_json_value(&expected)),
        actual: Some(encode_json_value(&actual)),
        errors: Vec::new(),
    }))
}

fn harness_execute_failure(
    step_index: Option<usize>,
    step: Option<&HarnessExecuteStep>,
    error: HarnessAutomationError,
    harness: Option<&mut HarnessAutomation>,
) -> HarnessExecuteFailure {
    match error {
        HarnessAutomationError::Compile(message) => HarnessExecuteFailure {
            kind: "compile_error",
            step_index,
            step: step.cloned(),
            assertion: None,
            message: Some(message),
            expected: None,
            actual: None,
            errors: Vec::new(),
        },
        HarnessAutomationError::Runtime(message) => HarnessExecuteFailure {
            kind: "runtime_error",
            step_index,
            step: step.cloned(),
            assertion: None,
            message: Some(message),
            expected: None,
            actual: None,
            errors: Vec::new(),
        },
        HarnessAutomationError::RuntimeCycle { message, errors } => HarnessExecuteFailure {
            kind: "runtime_cycle_error",
            step_index,
            step: step.cloned(),
            assertion: None,
            message: Some(message),
            expected: None,
            actual: None,
            errors,
        },
        HarnessAutomationError::Boundary(error) => HarnessExecuteFailure {
            kind: error.code(),
            step_index,
            step: step.cloned(),
            assertion: None,
            message: Some(error.to_string()),
            expected: None,
            actual: None,
            errors: error
                .candidates()
                .iter()
                .map(|candidate| format!("candidate: {candidate}"))
                .collect(),
        },
        HarnessAutomationError::RunUntilTimeout {
            name,
            max_cycles,
            expected,
        } => {
            let actual = harness
                .and_then(|loaded| loaded.get_output(&name).ok())
                .map(|snapshot| snapshot.value);
            HarnessExecuteFailure {
                kind: "run_until_timeout",
                step_index,
                step: step.cloned(),
                assertion: None,
                message: Some(format!(
                    "run_until exceeded {max_cycles} cycles before '{name}' matched the expected value."
                )),
                expected: Some(encode_json_value(&expected)),
                actual: actual.as_ref().map(encode_json_value),
                errors: Vec::new(),
            }
        }
        HarnessAutomationError::NotLoaded => HarnessExecuteFailure {
            kind: "not_loaded",
            step_index,
            step: step.cloned(),
            assertion: None,
            message: Some("Harness is not loaded. Call harness.load first.".to_string()),
            expected: None,
            actual: None,
            errors: Vec::new(),
        },
        HarnessAutomationError::InvalidArgument(message) => HarnessExecuteFailure {
            kind: "invalid_argument",
            step_index,
            step: step.cloned(),
            assertion: None,
            message: Some(message),
            expected: None,
            actual: None,
            errors: Vec::new(),
        },
    }
}
