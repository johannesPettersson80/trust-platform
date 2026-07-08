pub fn run_conformance(
    suite_root: Option<PathBuf>,
    output: Option<PathBuf>,
    update_expected: bool,
    filter: Option<String>,
) -> anyhow::Result<()> {
    let suite_root = resolve_suite_root(suite_root)?;
    let mut cases = discover_cases(&suite_root)?;
    if let Some(filter) = filter.as_deref() {
        let needle = filter.to_ascii_lowercase();
        cases.retain(|case| case.id.to_ascii_lowercase().contains(&needle));
    }
    if cases.is_empty() {
        bail!(
            "no conformance cases discovered under {}",
            suite_root.display()
        );
    }
    cases.sort_by(|left, right| left.id.cmp(&right.id));

    let reports_root = suite_root.join("reports");
    fs::create_dir_all(&reports_root)
        .with_context(|| format!("create reports directory '{}'", reports_root.display()))?;
    let actual_root = reports_root.join("actual");
    fs::create_dir_all(&actual_root)
        .with_context(|| format!("create actual report directory '{}'", actual_root.display()))?;

    let timestamp = now_utc_parts();
    let output_path = output.unwrap_or_else(|| {
        reports_root.join(format!("{}_trust-runtime_summary.json", timestamp.compact))
    });
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create summary output parent '{}'", parent.display()))?;
    }

    let mut results = Vec::with_capacity(cases.len());
    let mut passed = 0_usize;
    let mut failed = 0_usize;
    let mut errors = 0_usize;
    let mut skipped = 0_usize;

    for case in &cases {
        let started = Instant::now();
        let expected_ref = format!("expected/{}/{}.json", case.category, case.id);
        let expected_path = suite_root.join(&expected_ref);
        let actual_ref = format!("reports/actual/{}.json", case.id);
        let actual_path = suite_root.join(&actual_ref);

        let mut summary_result = SummaryCaseResult {
            case_id: case.id.clone(),
            category: case.category.clone(),
            status: CaseStatus::Error.as_str().to_string(),
            expected_ref,
            actual_ref: None,
            duration_ms: None,
            cycles: None,
            reason: None,
        };

        match execute_case(case) {
            Ok(artifact) => {
                summary_result.cycles = artifact.cycles;
                if update_expected {
                    if let Err(err) = write_json_pretty(&expected_path, &artifact.payload) {
                        summary_result.status = CaseStatus::Error.as_str().to_string();
                        summary_result.reason = Some(reason(
                            "expected_write_error",
                            "failed writing expected artifact",
                            Some(err.to_string()),
                        ));
                        errors += 1;
                    } else {
                        summary_result.status = CaseStatus::Passed.as_str().to_string();
                        passed += 1;
                    }
                } else if !expected_path.is_file() {
                    summary_result.status = CaseStatus::Error.as_str().to_string();
                    summary_result.actual_ref = Some(actual_ref.clone());
                    summary_result.reason = Some(reason(
                        "expected_missing",
                        "expected artifact is missing",
                        Some(expected_path.display().to_string()),
                    ));
                    let _ = write_json_pretty(&actual_path, &artifact.payload);
                    errors += 1;
                } else {
                    match read_json_value(&expected_path) {
                        Ok(expected) if expected == artifact.payload => {
                            summary_result.status = CaseStatus::Passed.as_str().to_string();
                            passed += 1;
                        }
                        Ok(_) => {
                            summary_result.status = CaseStatus::Failed.as_str().to_string();
                            summary_result.actual_ref = Some(actual_ref.clone());
                            summary_result.reason = Some(reason(
                                "expected_mismatch",
                                "actual artifact differs from expected",
                                None,
                            ));
                            let _ = write_json_pretty(&actual_path, &artifact.payload);
                            failed += 1;
                        }
                        Err(err) => {
                            summary_result.status = CaseStatus::Error.as_str().to_string();
                            summary_result.actual_ref = Some(actual_ref.clone());
                            summary_result.reason = Some(reason(
                                "expected_read_error",
                                "failed reading expected artifact",
                                Some(err.to_string()),
                            ));
                            let _ = write_json_pretty(&actual_path, &artifact.payload);
                            errors += 1;
                        }
                    }
                }
            }
            Err(err) => {
                summary_result.status = CaseStatus::Error.as_str().to_string();
                summary_result.reason = Some(reason(
                    "case_execution_error",
                    "case execution failed",
                    Some(err.to_string()),
                ));
                errors += 1;
            }
        }
        summary_result.duration_ms = Some(elapsed_ms(started.elapsed()));
        if summary_result.status == CaseStatus::Skipped.as_str() {
            skipped += 1;
        }
        results.push(summary_result);
    }

    let summary_contract = summary_contract_for_cases(&cases);
    let summary = SummaryOutput {
        version: summary_contract.version,
        profile: summary_contract.profile.to_string(),
        generated_at_utc: timestamp.rfc3339,
        ordering: "case_id_asc".to_string(),
        runtime: RuntimeSummaryMeta {
            name: "trust-runtime".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            target: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        },
        summary: SummaryTotals {
            total: results.len(),
            passed,
            failed,
            errors,
            skipped,
        },
        results,
    };

    let rendered =
        serde_json::to_string_pretty(&summary).context("serialize conformance summary")?;
    println!("{rendered}");
    fs::write(&output_path, format!("{rendered}\n")).with_context(|| {
        format!(
            "write conformance summary output '{}'",
            output_path.display()
        )
    })?;

    if summary.summary.failed > 0 || summary.summary.errors > 0 {
        bail!(
            "conformance failed: {} failed, {} errors",
            summary.summary.failed,
            summary.summary.errors
        );
    }
    Ok(())
}

fn summary_contract_for_cases(cases: &[CaseDefinition]) -> SummaryContract {
    if cases
        .iter()
        .all(|case| V1_CATEGORIES.contains(&case.category.as_str()))
    {
        SummaryContract::v1()
    } else {
        SummaryContract::v2()
    }
}
