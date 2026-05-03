fn evaluate_alerts(
    rules: &[CompiledAlertRule],
    latest_numeric: &HashMap<String, f64>,
    timestamp_ms: u128,
    trackers: &mut HashMap<SmolStr, AlertTracker>,
) -> Vec<(HistorianAlertEvent, Option<HookTarget>)> {
    let mut events = Vec::new();
    for rule in rules {
        let value = latest_numeric.get(rule.variable.as_str()).copied();
        let breached = value.is_some_and(|v| threshold_breached(v, rule.above, rule.below));
        let tracker = trackers.entry(rule.name.clone()).or_default();

        if breached {
            tracker.consecutive = tracker.consecutive.saturating_add(1);
            if !tracker.active && tracker.consecutive >= rule.debounce_samples {
                tracker.active = true;
                events.push((
                    HistorianAlertEvent {
                        timestamp_ms,
                        rule: rule.name.to_string(),
                        variable: rule.variable.to_string(),
                        state: AlertState::Triggered,
                        value,
                        threshold: rule_threshold_text(rule.above, rule.below),
                    },
                    rule.hook.clone(),
                ));
            }
        } else {
            tracker.consecutive = 0;
            if tracker.active {
                tracker.active = false;
                events.push((
                    HistorianAlertEvent {
                        timestamp_ms,
                        rule: rule.name.to_string(),
                        variable: rule.variable.to_string(),
                        state: AlertState::Cleared,
                        value,
                        threshold: rule_threshold_text(rule.above, rule.below),
                    },
                    rule.hook.clone(),
                ));
            }
        }
    }
    events
}

fn threshold_breached(value: f64, above: Option<f64>, below: Option<f64>) -> bool {
    above.is_some_and(|limit| value > limit) || below.is_some_and(|limit| value < limit)
}

fn rule_threshold_text(above: Option<f64>, below: Option<f64>) -> String {
    match (above, below) {
        (Some(_), Some(_)) => "outside_band".to_string(),
        (Some(_), None) => "above".to_string(),
        (None, Some(_)) => "below".to_string(),
        (None, None) => "threshold".to_string(),
    }
}

fn dispatch_hook(target: &HookTarget, event: &HistorianAlertEvent) {
    match target {
        HookTarget::Log => {
            warn!(
                "historian alert {} for {} is {:?}",
                event.rule, event.variable, event.state
            );
        }
        HookTarget::File(path) => {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                if let Ok(line) = serde_json::to_string(event) {
                    let _ = file.write_all(line.as_bytes());
                    let _ = file.write_all(b"\n");
                }
            }
        }
        HookTarget::Webhook(url) => {
            let payload = match serde_json::to_string(event) {
                Ok(payload) => payload,
                Err(_) => return,
            };
            let config = ureq::Agent::config_builder()
                .timeout_connect(Some(Duration::from_millis(500)))
                .timeout_recv_response(Some(Duration::from_millis(800)))
                .build();
            let agent: ureq::Agent = config.into();
            if let Err(err) = agent
                .post(url.as_str())
                .header("Content-Type", "application/json")
                .send(payload.as_str())
            {
                warn!(
                    "historian webhook delivery failed for '{}': {err}",
                    event.rule
                );
            }
        }
    }
}

fn resolve_path(path: &Path, bundle_root: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match bundle_root {
        Some(root) => root.join(path),
        None => path.to_path_buf(),
    }
}

fn unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
