//! Tutorial 23 (observability: historian and Prometheus metrics) as its README writes it.

use super::*;

/// The historian's samples in `project`'s `history/historian.jsonl`, as (time, variable, value),
/// one JSON object a line. A running historian appends a sample a line at a time, so a line it has
/// only half written is passed over; with `complete`, for a file read after the runtime stopped,
/// every line must be a whole sample.
fn historian_samples(
    project: &std::path::Path,
    complete: bool,
) -> Vec<(u64, String, serde_json::Value)> {
    let Ok(text) = std::fs::read_to_string(project.join("history").join("historian.jsonl")) else {
        return Vec::new();
    };
    let parse = |line: &str| {
        let sample: serde_json::Value = serde_json::from_str(line).ok()?;
        Some((
            sample["timestamp_ms"].as_u64()?,
            sample["variable"].as_str()?.to_string(),
            sample["value"]
                .as_object()
                .map(|_| sample["value"].clone())?,
        ))
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let sample = parse(line);
            assert!(
                sample.is_some() || !complete,
                "a history line that is not a whole sample: {line}"
            );
            sample
        })
        .collect()
}

/// The rows of the captures in `samples` whose every allowlisted variable is written: a capture
/// that a running historian is still appending holds fewer.
fn whole_capture_rows(samples: &[(u64, String, serde_json::Value)]) -> usize {
    let mut per_time: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
    for (time, _, _) in samples {
        *per_time.entry(*time).or_default() += 1;
    }
    per_time.values().filter(|rows| **rows == 2).sum()
}

/// The value of the Prometheus metric `name` in `body`.
fn metric(body: &str, name: &str) -> f64 {
    body.lines()
        .find_map(|line| line.strip_prefix(&format!("{name} ")))
        .and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("step 7: no {name} in\n{body}"))
}

#[test]
fn tutorial_23_records_its_allowlist_and_serves_metrics_as_its_readme_writes_it() {
    // examples/tutorials/23_observability_historian_prometheus/README.md on tutorial 13's project:
    // step 2 puts the README's sections into runtime.toml (the web interface on, the historian in
    // allowlist mode for `P1.StartCmd` and `P1.LampOut`, a sample a second, history/historian.jsonl
    // and the Prometheus path /metrics); steps 3 and 4 build and run; step 5 toggles `%IX0.0` with
    // `ctl io-write`; step 6 reads the history file; step 7 the Prometheus endpoint. Specification
    // 11, the runtime historian's recording and export contract: allowlist mode records exactly the
    // configured names, a program's variable by its full path, and the Prometheus renderer gives
    // the runtime's uptime and fault totals with the historian's sample total. The program copies
    // `StartCmd` to `LampOut`, so each sample holds the two equal. The copy moves the web interface
    // to its own loopback port. Catches an allowlist that names nothing the historian records, a
    // history file holding other variables, and an endpoint without the historian's totals.
    let readme = std::fs::read_to_string(
        repo_root().join("examples/tutorials/23_observability_historian_prometheus/README.md"),
    )
    .expect("tutorial 23's README");
    // The block stands in a numbered step, indented as the step's text is.
    let configuration: String = readme
        .split("```toml\n")
        .nth(1)
        .and_then(|block| block.split("```").next())
        .expect("the README's configuration, its first TOML block")
        .lines()
        .map(|line| format!("{}\n", line.trim_start()))
        .collect();
    let mut files = here_documents(
        &std::fs::read_to_string(
            repo_root().join("examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md"),
        )
        .expect("tutorial 13's README"),
    );
    for (file, text) in &mut files {
        if file == "runtime.toml" {
            *text = with_sections(text, &configuration);
        }
    }

    let mut runtime = ExampleRuntime::written("tutorial-23", files).web().start();
    for value in [true, false] {
        set(&runtime, "%IX0.0", value);
        runtime.wait_until(
            STEP,
            &format!("step 6: a sample of P1.StartCmd = {value}"),
            || {
                historian_samples(runtime.project(), false)
                    .iter()
                    .any(|(_, variable, sampled)| {
                        variable == "P1.StartCmd" && sampled["value"] == value
                    })
            },
        );
    }

    // The sample total counts persisted rows, and it follows each capture's rows: compared with the
    // rows of the whole captures read just before the scrape, it is at least those and at most one
    // capture, two rows, more. A scrape between a capture's rows and its count is taken again.
    let scrape = || {
        let mut response = ureq::get(&format!("{}/metrics", runtime.web_base()))
            .call()
            .expect("step 7: GET /metrics");
        response.body_mut().read_to_string().expect("the metrics")
    };
    let deadline = std::time::Instant::now() + STEP;
    let body = loop {
        let rows = whole_capture_rows(&historian_samples(runtime.project(), false));
        let body = scrape();
        let total = metric(&body, "trust_runtime_historian_samples_total");
        if rows >= 4 && total >= rows as f64 && total <= rows as f64 + 2.0 {
            break body;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "step 7: the sample total {total} against {rows} rows of whole captures\n{body}"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(metric(&body, "trust_runtime_uptime_ms") > 0.0, "{body}");
    assert_eq!(metric(&body, "trust_runtime_faults_total"), 0.0, "{body}");

    // The historian appends a capture a line at a time, so its file is read whole once the runtime
    // has stopped, with every capture complete.
    runtime.ctl_ok(&["shutdown", "--exit"]);
    runtime.wait_for_exit(STEP);
    let samples = historian_samples(runtime.project(), true);
    let variables: std::collections::BTreeSet<&str> = samples
        .iter()
        .map(|(_, variable, _)| variable.as_str())
        .collect();
    assert_eq!(
        variables,
        std::collections::BTreeSet::from(["P1.LampOut", "P1.StartCmd"]),
        "step 6: the history holds the allowlist, and only it"
    );
    let mut by_time: std::collections::BTreeMap<u64, Vec<(&str, &serde_json::Value)>> =
        std::collections::BTreeMap::new();
    for (time, variable, value) in &samples {
        by_time
            .entry(*time)
            .or_default()
            .push((variable.as_str(), value));
    }
    // Each capture records the current snapshot of every allowlisted name at one time.
    for (time, sample) in &by_time {
        let mut names: Vec<&str> = sample.iter().map(|(variable, _)| *variable).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            ["P1.LampOut", "P1.StartCmd"],
            "the capture at {time} ms holds each allowlisted variable once"
        );
        let of = |name: &str| {
            sample
                .iter()
                .find(|(variable, _)| *variable == name)
                .map(|(_, value)| *value)
                .expect("checked above")
        };
        let (start, lamp) = (of("P1.StartCmd"), of("P1.LampOut"));
        assert_eq!(
            start, lamp,
            "the capture at {time} ms: LampOut copies StartCmd"
        );
        assert_eq!(
            start["kind"], "bool",
            "the capture at {time} ms keeps the BOOL's type"
        );
    }
}
