#[must_use]
pub fn render_prometheus(
    runtime: &RuntimeMetricsSnapshot,
    historian: Option<HistorianPrometheusSnapshot>,
) -> String {
    let mut body = String::new();
    body.push_str("# HELP trust_runtime_uptime_ms Runtime uptime in milliseconds.\n");
    body.push_str("# TYPE trust_runtime_uptime_ms gauge\n");
    let _ = writeln!(body, "trust_runtime_uptime_ms {}", runtime.uptime_ms);

    body.push_str("# HELP trust_runtime_execution_backend_info Active runtime execution backend label.\n");
    body.push_str("# TYPE trust_runtime_execution_backend_info gauge\n");
    let _ = writeln!(
        body,
        "trust_runtime_execution_backend_info{{backend=\"{}\"}} 1",
        escape_label(runtime.execution_backend.as_str())
    );

    body.push_str("# HELP trust_runtime_faults_total Runtime fault count.\n");
    body.push_str("# TYPE trust_runtime_faults_total counter\n");
    let _ = writeln!(body, "trust_runtime_faults_total {}", runtime.faults);

    body.push_str("# HELP trust_runtime_overruns_total Runtime cycle overrun count.\n");
    body.push_str("# TYPE trust_runtime_overruns_total counter\n");
    let _ = writeln!(body, "trust_runtime_overruns_total {}", runtime.overruns);

    body.push_str("# HELP trust_runtime_cycle_last_ms Last cycle duration in milliseconds.\n");
    body.push_str("# TYPE trust_runtime_cycle_last_ms gauge\n");
    let _ = writeln!(
        body,
        "trust_runtime_cycle_last_ms {:.6}",
        runtime.cycle.last_ms
    );

    body.push_str("# HELP trust_runtime_cycle_avg_ms Average cycle duration in milliseconds.\n");
    body.push_str("# TYPE trust_runtime_cycle_avg_ms gauge\n");
    let _ = writeln!(
        body,
        "trust_runtime_cycle_avg_ms {:.6}",
        runtime.cycle.avg_ms
    );

    body.push_str(
        "# HELP trust_runtime_cycle_p50_ms Recent cycle duration p50 in milliseconds.\n",
    );
    body.push_str("# TYPE trust_runtime_cycle_p50_ms gauge\n");
    let _ = writeln!(
        body,
        "trust_runtime_cycle_p50_ms {:.6}",
        runtime.cycle_percentiles.p50_ms
    );

    body.push_str(
        "# HELP trust_runtime_cycle_p95_ms Recent cycle duration p95 in milliseconds.\n",
    );
    body.push_str("# TYPE trust_runtime_cycle_p95_ms gauge\n");
    let _ = writeln!(
        body,
        "trust_runtime_cycle_p95_ms {:.6}",
        runtime.cycle_percentiles.p95_ms
    );

    body.push_str(
        "# HELP trust_runtime_cycle_p99_ms Recent cycle duration p99 in milliseconds.\n",
    );
    body.push_str("# TYPE trust_runtime_cycle_p99_ms gauge\n");
    let _ = writeln!(
        body,
        "trust_runtime_cycle_p99_ms {:.6}",
        runtime.cycle_percentiles.p99_ms
    );

    body.push_str(
        "# HELP trust_runtime_cycle_window_samples Recent cycle sample window size.\n",
    );
    body.push_str("# TYPE trust_runtime_cycle_window_samples gauge\n");
    let _ = writeln!(
        body,
        "trust_runtime_cycle_window_samples {}",
        runtime.cycle_percentiles.window_samples
    );

    body.push_str("# HELP trust_runtime_task_last_ms Last task duration in milliseconds.\n");
    body.push_str("# TYPE trust_runtime_task_last_ms gauge\n");
    for task in &runtime.tasks {
        let _ = writeln!(
            body,
            "trust_runtime_task_last_ms{{task=\"{}\"}} {:.6}",
            escape_label(task.name.as_str()),
            task.last_ms
        );
    }

    body.push_str("# HELP trust_runtime_task_overruns_total Task overrun count.\n");
    body.push_str("# TYPE trust_runtime_task_overruns_total counter\n");
    for task in &runtime.tasks {
        let _ = writeln!(
            body,
            "trust_runtime_task_overruns_total{{task=\"{}\"}} {}",
            escape_label(task.name.as_str()),
            task.overruns
        );
    }

    if let Some(historian) = historian {
        body.push_str(
            "# HELP trust_runtime_historian_samples_total Persisted historian samples.\n",
        );
        body.push_str("# TYPE trust_runtime_historian_samples_total counter\n");
        let _ = writeln!(
            body,
            "trust_runtime_historian_samples_total {}",
            historian.samples_total
        );

        body.push_str(
            "# HELP trust_runtime_historian_series_total Distinct historian variables tracked.\n",
        );
        body.push_str("# TYPE trust_runtime_historian_series_total gauge\n");
        let _ = writeln!(
            body,
            "trust_runtime_historian_series_total {}",
            historian.series_total
        );

        body.push_str("# HELP trust_runtime_historian_alerts_total Historian alert transitions.\n");
        body.push_str("# TYPE trust_runtime_historian_alerts_total counter\n");
        let _ = writeln!(
            body,
            "trust_runtime_historian_alerts_total {}",
            historian.alerts_total
        );
    }

    body
}

fn escape_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
