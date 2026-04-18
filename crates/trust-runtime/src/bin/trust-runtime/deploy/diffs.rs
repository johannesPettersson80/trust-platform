fn write_summary(dir: &Path, name: &str, summary: &BundleChangeSummary) -> anyhow::Result<()> {
    let path = dir.join(format!("{name}.txt"));
    fs::write(&path, summary.render())?;
    fs::write(dir.join("last.txt"), summary.render())?;
    Ok(())
}

fn diff_runtime(previous: Option<&RuntimeConfig>, next: &RuntimeConfig) -> Vec<String> {
    let mut changes = Vec::new();
    if let Some(prev) = previous {
        diff_field(
            &mut changes,
            "resource",
            &prev.resource_name,
            &next.resource_name,
        );
        diff_field(
            &mut changes,
            "cycle_interval_ms",
            &prev.cycle_interval.as_millis(),
            &next.cycle_interval.as_millis(),
        );
        diff_field(&mut changes, "log_level", &prev.log_level, &next.log_level);
        diff_field(
            &mut changes,
            "control_endpoint",
            &prev.control_endpoint,
            &next.control_endpoint,
        );
        if prev.control_auth_token.is_some() != next.control_auth_token.is_some() {
            changes.push(format!(
                "control_auth_token: {} -> {}",
                token_state(prev.control_auth_token.as_ref()),
                token_state(next.control_auth_token.as_ref())
            ));
        }
        if prev.control_debug_enabled != next.control_debug_enabled {
            changes.push(format!(
                "control_debug_enabled: {} -> {}",
                prev.control_debug_enabled, next.control_debug_enabled
            ));
        }
        diff_retain(&mut changes, prev, next);
        diff_watchdog(&mut changes, &prev.watchdog, &next.watchdog);
        if prev.fault_policy != next.fault_policy {
            changes.push(format!(
                "fault_policy: {:?} -> {:?}",
                prev.fault_policy, next.fault_policy
            ));
        }
    } else {
        changes.push("new project version (no previous runtime.toml)".to_string());
    }
    changes
}

fn diff_retain(changes: &mut Vec<String>, prev: &RuntimeConfig, next: &RuntimeConfig) {
    if prev.retain_mode != next.retain_mode {
        changes.push(format!(
            "retain_mode: {:?} -> {:?}",
            prev.retain_mode, next.retain_mode
        ));
    }
    if prev.retain_path != next.retain_path {
        changes.push(format!(
            "retain_path: {} -> {}",
            path_state(prev.retain_path.as_ref()),
            path_state(next.retain_path.as_ref())
        ));
    }
    if prev.retain_save_interval != next.retain_save_interval {
        changes.push(format!(
            "retain_save_interval_ms: {} -> {}",
            prev.retain_save_interval.as_millis(),
            next.retain_save_interval.as_millis()
        ));
    }
}

fn diff_watchdog(changes: &mut Vec<String>, prev: &WatchdogPolicy, next: &WatchdogPolicy) {
    if prev.enabled != next.enabled {
        changes.push(format!(
            "watchdog.enabled: {} -> {}",
            prev.enabled, next.enabled
        ));
    }
    if prev.timeout != next.timeout {
        changes.push(format!(
            "watchdog.timeout_ms: {} -> {}",
            prev.timeout.as_millis(),
            next.timeout.as_millis()
        ));
    }
    if prev.action != next.action {
        changes.push(format!(
            "watchdog.action: {:?} -> {:?}",
            prev.action, next.action
        ));
    }
}

fn diff_io(previous: Option<&IoConfig>, next: &IoConfig) -> Vec<String> {
    let mut changes = Vec::new();
    if let Some(prev) = previous {
        if prev.drivers != next.drivers {
            changes.push("drivers: updated".to_string());
        }
        if safe_state_changed(&prev.safe_state, &next.safe_state) {
            changes.push("safe_state: updated".to_string());
        }
    } else {
        changes.push("new project version (no previous io.toml)".to_string());
    }
    changes
}

fn diff_sources(previous_root: Option<&Path>, next_root: &Path) -> SourceDiff {
    let prev = previous_root.and_then(|root| collect_sources(root).ok());
    let next = collect_sources(next_root).unwrap_or_default();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let prev = prev.unwrap_or_default();

    let mut keys = BTreeSet::new();
    keys.extend(prev.keys().cloned());
    keys.extend(next.keys().cloned());
    for key in keys {
        match (prev.get(&key), next.get(&key)) {
            (None, Some(_)) => added.push(key),
            (Some(_), None) => removed.push(key),
            (Some(prev_bytes), Some(next_bytes)) if prev_bytes != next_bytes => {
                modified.push(key);
            }
            _ => {}
        }
    }

    SourceDiff {
        added,
        removed,
        modified,
    }
}
