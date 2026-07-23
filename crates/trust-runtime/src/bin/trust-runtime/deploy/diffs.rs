fn validate_summary_slots(dir: &Path, name: &str) -> anyhow::Result<()> {
    for path in [dir.join(format!("{name}.txt")), dir.join("last.txt")] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
            Ok(_) => anyhow::bail!(
                "deployment summary '{}' exists and is not a regular file",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect deployment summary '{}'", path.display()));
            }
        }
    }
    Ok(())
}

fn publish_summary_file(staged: &Path, target: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        fs::rename(staged, target)
            .with_context(|| format!("publish deployment summary '{}'", target.display()))
    }
    #[cfg(windows)]
    {
        let backup = scratch_path(target, "old");
        let had_target = target.exists();
        if had_target {
            fs::rename(target, &backup)
                .with_context(|| format!("backup deployment summary '{}'", target.display()))?;
        }
        if let Err(error) = fs::rename(staged, target) {
            let restore = if had_target {
                fs::rename(&backup, target)
            } else {
                Ok(())
            };
            if let Err(restore_error) = restore {
                return Err(error).context(format!(
                    "publish deployment summary '{}' and restore its backup: {restore_error}",
                    target.display()
                ));
            }
            return Err(error)
                .with_context(|| format!("publish deployment summary '{}'", target.display()));
        }
        if had_target {
            fs::remove_file(&backup).with_context(|| {
                format!("remove deployment summary backup '{}'", backup.display())
            })?;
        }
        Ok(())
    }
}

fn restore_summary_file(path: &Path, previous: Option<&[u8]>) -> anyhow::Result<()> {
    let Some(previous) = previous else {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("remove deployment summary '{}'", path.display())),
        };
    };
    let staged = scratch_path(path, "restore");
    fs::write(&staged, previous)
        .with_context(|| format!("stage restored summary '{}'", path.display()))?;
    publish_summary_file(&staged, path)
}

fn write_summary(dir: &Path, name: &str, summary: &BundleChangeSummary) -> anyhow::Result<()> {
    validate_summary_slots(dir, name)?;
    let labelled = dir.join(format!("{name}.txt"));
    let last = dir.join("last.txt");
    let previous_labelled = read_optional_summary(&labelled)?;
    let labelled_staged = scratch_path(&labelled, "new");
    let last_staged = scratch_path(&last, "new");
    let rendered = summary.render();
    fs::write(&labelled_staged, &rendered)
        .with_context(|| format!("stage deployment summary '{}'", labelled_staged.display()))?;
    if let Err(error) = fs::write(&last_staged, rendered) {
        let _ = fs::remove_file(&labelled_staged);
        return Err(error)
            .with_context(|| format!("stage deployment summary '{}'", last_staged.display()));
    }
    if let Err(error) = publish_summary_file(&labelled_staged, &labelled) {
        let _ = fs::remove_file(&labelled_staged);
        let _ = fs::remove_file(&last_staged);
        return Err(error);
    }
    if let Err(error) = publish_summary_file(&last_staged, &last) {
        let restore = restore_summary_file(&labelled, previous_labelled.as_deref());
        let _ = fs::remove_file(&last_staged);
        if let Err(restore_error) = restore {
            return Err(error.context(format!(
                "also failed to restore deployment summary '{}': {restore_error:#}",
                labelled.display(),
            )));
        }
        return Err(error);
    }
    Ok(())
}

fn read_optional_summary(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("read deployment summary '{}'", path.display()))
        }
    }
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

fn diff_sources(previous_root: Option<&Path>, next_root: &Path) -> anyhow::Result<SourceDiff> {
    let prev = previous_root.map(collect_sources).transpose()?;
    let next = collect_sources(next_root)?;
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

    Ok(SourceDiff {
        added,
        removed,
        modified,
    })
}
