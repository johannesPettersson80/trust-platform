#[test]
fn register_executor_fb_omitted_input_uses_initializer_then_reuses_stored_value() {
    let source = r#"
            FUNCTION_BLOCK Adjust
            VAR_INPUT
                base : INT;
                inc : INT := INT#5;
            END_VAR
            VAR_OUTPUT
                result : INT;
            END_VAR
            result := base + inc;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Adjust;
                first : INT := INT#0;
                second : INT := INT#0;
                third : INT := INT#0;
            END_VAR

            fb(base := INT#3);
            first := fb.result;

            fb(base := INT#3, inc := INT#9);
            second := fb.result;

            fb(base := INT#3);
            third := fb.result;
            END_PROGRAM
        "#;

    let mut harness = TestHarness::from_source(source).expect("create harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("set backend");
    harness
        .runtime_mut()
        .restart(RestartMode::Cold)
        .expect("restart");
    harness.runtime_mut().set_vm_register_profile_enabled(true);
    harness.runtime_mut().reset_vm_register_profile();

    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "cycle errors: {:?}",
        result.errors
    );
    assert_eq!(harness.get_output("first"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(12)));
    assert_eq!(harness.get_output("third"), Some(Value::Int(12)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_executor_runs_multi_label_case_program_without_fallback() {
    let source = r#"
            VAR_GLOBAL
                g_selector : UINT := UINT#3;
                g_output : UINT := UINT#0;
            END_VAR

            PROGRAM Main
            CASE g_selector OF
                UINT#1:
                    g_output := UINT#10;
                UINT#2:
                    g_output := UINT#20;
                UINT#3:
                    g_output := UINT#30;
                ELSE
                    g_output := UINT#99;
            END_CASE;
            END_PROGRAM
        "#;

    let mut harness = TestHarness::from_source(source).expect("create harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("set backend");
    harness
        .runtime_mut()
        .restart(RestartMode::Cold)
        .expect("restart");
    harness.runtime_mut().set_vm_register_profile_enabled(true);
    harness.runtime_mut().reset_vm_register_profile();

    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "cycle errors: {:?}",
        result.errors
    );
    assert_eq!(harness.get_output("g_output"), Some(Value::UInt(30)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_executor_runs_case_branch_with_nested_if_without_fallback() {
    let source = r#"
            VAR_GLOBAL
                g_current_step : UINT := UINT#30;
                g_last_error : UINT := UINT#0;
                g_power_status : BOOL := TRUE;
            END_VAR

            PROGRAM Main
            CASE g_current_step OF
                UINT#10:
                    IF FALSE THEN
                        g_current_step := UINT#20;
                    END_IF;
                UINT#20:
                    IF FALSE THEN
                        g_current_step := UINT#30;
                    END_IF;
                UINT#30:
                    IF g_power_status THEN
                        g_current_step := UINT#40;
                    END_IF;
                ELSE
                    g_last_error := UINT#512;
                    g_current_step := UINT#900;
            END_CASE;

            IF g_last_error <> UINT#0 THEN
                g_current_step := UINT#900;
            END_IF;
            END_PROGRAM
        "#;

    let mut harness = TestHarness::from_source(source).expect("create harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("set backend");
    harness
        .runtime_mut()
        .restart(RestartMode::Cold)
        .expect("restart");
    harness.runtime_mut().set_vm_register_profile_enabled(true);
    harness.runtime_mut().reset_vm_register_profile();

    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "cycle errors: {:?}",
        result.errors
    );
    assert_eq!(harness.get_output("g_current_step"), Some(Value::UInt(40)));
    assert_eq!(harness.get_output("g_last_error"), Some(Value::UInt(0)));

    let profile = harness.runtime().vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

#[test]
fn register_executor_progresses_motion_demo_to_step_40_without_error_by_cycle_three() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/plcopen_motion_single_axis_demo");
    let runtime_config =
        RuntimeConfig::load(project.join("runtime.toml")).expect("load runtime config");
    let cycle_budget = runtime_config.cycle_interval;
    let compile_sources =
        collect_project_source_files(&project, None).expect("collect project sources");
    let session = CompileSession::from_sources(compile_sources);
    let mut runtime = session.build_runtime().expect("build runtime");
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("set backend");
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    for cycle in 0..3 {
        runtime.execute_cycle().unwrap_or_else(|err| {
            panic!("cycle {} failed: {err}", cycle + 1);
        });
        runtime.advance_time(cycle_budget);
    }

    assert_eq!(
        runtime.storage().get_global("g_motion_demo_current_step"),
        Some(&Value::UInt(40))
    );
    assert_eq!(
        runtime.storage().get_global("g_motion_demo_last_error"),
        Some(&Value::Word(0))
    );

    let profile = runtime.vm_register_profile_snapshot();
    assert!(profile.register_programs_executed >= 1);
    assert_eq!(
        profile.register_program_fallbacks, 0,
        "expected no register fallbacks, got {:?}",
        profile.fallback_reasons
    );
}

