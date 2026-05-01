#[cfg(test)]
fn execute_test_case(
    session: &CompileSession,
    case: &DiscoveredTest,
    timeout: Option<StdDuration>,
) -> Result<(), RuntimeError> {
    let mut runtime = session
        .build_runtime()
        .map_err(|err| RuntimeError::ControlError(err.to_string().into()))?;
    execute_test_case_in_runtime(&mut runtime, case, timeout)
}

fn execute_test_case_in_runtime(
    runtime: &mut Runtime,
    case: &DiscoveredTest,
    timeout: Option<StdDuration>,
) -> Result<(), RuntimeError> {
    runtime.restart(trust_runtime::RestartMode::Cold)?;
    let deadline = timeout.and_then(|limit| Instant::now().checked_add(limit));
    runtime.set_execution_deadline(deadline);
    let result = match case.kind {
        TestKind::Program => execute_test_program(runtime, case.name.as_str()),
        TestKind::FunctionBlock => execute_test_function_block(runtime, case.name.as_str()),
    };
    runtime.set_execution_deadline(None);
    result
}

fn execute_test_program(runtime: &mut Runtime, name: &str) -> Result<(), RuntimeError> {
    let program = runtime
        .programs()
        .values()
        .find(|program| program.name.eq_ignore_ascii_case(name))
        .cloned()
        .ok_or_else(|| RuntimeError::UndefinedProgram(name.into()))?;
    runtime.execute_program(&program)
}

fn execute_test_function_block(runtime: &mut Runtime, name: &str) -> Result<(), RuntimeError> {
    runtime.execute_function_block_by_name(name)
}
