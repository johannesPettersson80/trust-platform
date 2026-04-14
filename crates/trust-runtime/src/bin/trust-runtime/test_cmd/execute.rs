fn execute_test_case(
    session: &CompileSession,
    case: &DiscoveredTest,
    timeout: Option<StdDuration>,
) -> Result<(), RuntimeError> {
    let mut runtime = session
        .build_runtime()
        .map_err(|err| RuntimeError::ControlError(err.to_string().into()))?;
    let deadline = timeout.and_then(|limit| Instant::now().checked_add(limit));
    runtime.set_execution_deadline(deadline);
    let result = match case.kind {
        TestKind::Program => execute_test_program(&mut runtime, case.name.as_str()),
        TestKind::FunctionBlock => execute_test_function_block(&mut runtime, case.name.as_str()),
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
