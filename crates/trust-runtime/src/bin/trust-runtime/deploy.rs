//! Bundle deployment, versioning, and rollback.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use indicatif::{ProgressBar, ProgressStyle};
use trust_runtime::config::{IoConfig, RuntimeBundle, RuntimeConfig};
use trust_runtime::io::{IoAddress, IoDriverRegistry};
use trust_runtime::watchdog::WatchdogPolicy;

use crate::style;

pub struct DeployResult {
    pub current_bundle: PathBuf,
}

pub fn run_deploy(
    bundle: PathBuf,
    root: Option<PathBuf>,
    label: Option<String>,
) -> anyhow::Result<DeployResult> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner().template("{spinner} {msg}")?);
    spinner.enable_steady_tick(std::time::Duration::from_millis(120));
    spinner.set_message("Deploying project...");
    let source_bundle = RuntimeBundle::load(&bundle)?;
    let root = root.unwrap_or(std::env::current_dir()?);
    let bundles_dir = root.join("bundles");
    let deployments_dir = root.join("deployments");
    fs::create_dir_all(&bundles_dir)?;
    fs::create_dir_all(&deployments_dir)?;

    let bundle_name = label.unwrap_or_else(default_bundle_label);
    let dest = bundles_dir.join(&bundle_name);
    if dest.exists() {
        anyhow::bail!("deployment already exists: {}", dest.display());
    }

    copy_bundle(&source_bundle.root, &dest)?;
    let dest_bundle = RuntimeBundle::load(&dest)?;
    validate_bundle(&dest_bundle)?;

    let current_link = root.join("current");
    let previous_link = root.join("previous");
    let current_target = read_link_target(&current_link);
    let previous_target = read_link_target(&previous_link);

    let previous_bundle = current_target
        .as_ref()
        .and_then(|path| RuntimeBundle::load(path).ok());
    let summary = BundleChangeSummary::new(previous_bundle.as_ref(), &dest_bundle);

    summary.print();
    write_summary(&deployments_dir, &bundle_name, &summary)?;

    update_symlink(&current_link, &dest)?;
    if let Some(old_current) = current_target {
        update_symlink(&previous_link, &old_current)?;
    }

    prune_bundles(
        &bundles_dir,
        &bundle_targets(&dest, previous_target.as_ref()),
    )?;

    spinner.finish_and_clear();
    println!(
        "{}",
        style::success(format!(
            "Deployed project {} -> {}",
            bundle_name,
            dest.display()
        ))
    );
    println!("Current project version: {}", current_link.display());
    Ok(DeployResult {
        current_bundle: current_link,
    })
}

pub fn run_rollback(root: Option<PathBuf>) -> anyhow::Result<()> {
    let root = root.unwrap_or(std::env::current_dir()?);
    let current_link = root.join("current");
    let previous_link = root.join("previous");
    let current_target = read_link_target(&current_link)
        .ok_or_else(|| anyhow::anyhow!("no current project link at {}", current_link.display()))?;
    let previous_target = read_link_target(&previous_link).ok_or_else(|| {
        anyhow::anyhow!(
            "no previous project link at {} (nothing to rollback)",
            previous_link.display()
        )
    })?;

    update_symlink(&current_link, &previous_target)?;
    update_symlink(&previous_link, &current_target)?;

    println!(
        "{}",
        style::success(format!(
            "Rolled back to project {}",
            previous_target.display()
        ))
    );
    println!("Current project version: {}", current_link.display());
    Ok(())
}

fn validate_bundle(bundle: &RuntimeBundle) -> anyhow::Result<()> {
    let registry = IoDriverRegistry::default_registry();
    registry
        .validate(&bundle.io.driver, &bundle.io.params)
        .map_err(anyhow::Error::from)?;
    let mut runtime = trust_runtime::Runtime::new();
    runtime.apply_bytecode_bytes(&bundle.bytecode, Some(&bundle.runtime.resource_name))?;
    Ok(())
}

fn copy_bundle(source: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest)?;
    copy_file(source.join("runtime.toml"), dest.join("runtime.toml"))?;
    if source.join("io.toml").is_file() {
        copy_file(source.join("io.toml"), dest.join("io.toml"))?;
    }
    if source.join("simulation.toml").is_file() {
        copy_file(source.join("simulation.toml"), dest.join("simulation.toml"))?;
    }
    copy_file(source.join("program.stbc"), dest.join("program.stbc"))?;

    let sources = source.join("sources");
    if sources.is_dir() {
        copy_dir(&sources, &dest.join("sources"))?;
    }
    Ok(())
}

fn copy_file(source: PathBuf, dest: PathBuf) -> anyhow::Result<()> {
    if !source.is_file() {
        anyhow::bail!("missing file {}", source.display());
    }
    fs::copy(&source, &dest)?;
    Ok(())
}

fn copy_dir(source: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let target = dest.join(file_name);
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn default_bundle_label() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("project-{secs}")
}

fn read_link_target(path: &Path) -> Option<PathBuf> {
    fs::read_link(path).ok()
}

fn update_symlink(link: &Path, target: &Path) -> anyhow::Result<()> {
    if link.exists() {
        fs::remove_file(link)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)?;
    }
    Ok(())
}

fn prune_bundles(bundles_dir: &Path, keep: &[PathBuf]) -> anyhow::Result<()> {
    if !bundles_dir.is_dir() {
        return Ok(());
    }
    let keep_set = keep
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect::<HashSet<_>>();
    for entry in fs::read_dir(bundles_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let path = entry.path();
        let canonical = path.canonicalize().unwrap_or(path.clone());
        if keep_set.contains(&canonical) {
            continue;
        }
        fs::remove_dir_all(&path)?;
    }
    Ok(())
}

fn bundle_targets(current: &Path, previous: Option<&PathBuf>) -> Vec<PathBuf> {
    let mut targets = vec![current.to_path_buf()];
    if let Some(previous) = previous {
        targets.push(previous.clone());
    }
    targets
}

struct SourceDiff {
    added: Vec<String>,
    removed: Vec<String>,
    modified: Vec<String>,
}

impl SourceDiff {
    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

struct BundleChangeSummary {
    previous_path: Option<PathBuf>,
    runtime_changes: Vec<String>,
    io_changes: Vec<String>,
    bytecode_changed: bool,
    source_diff: SourceDiff,
}

impl BundleChangeSummary {
    fn new(previous: Option<&RuntimeBundle>, next: &RuntimeBundle) -> Self {
        let runtime_changes = diff_runtime(previous.map(|b| &b.runtime), &next.runtime);
        let io_changes = diff_io(previous.map(|b| &b.io), &next.io);
        let bytecode_changed = previous
            .map(|b| b.bytecode != next.bytecode)
            .unwrap_or(true);
        let source_diff = diff_sources(previous.map(|b| b.root.as_path()), next.root.as_path());
        Self {
            previous_path: previous.map(|b| b.root.clone()),
            runtime_changes,
            io_changes,
            bytecode_changed,
            source_diff,
        }
    }

    fn print(&self) {
        println!("Deployment summary:");
        if let Some(path) = &self.previous_path {
            println!("previous project version: {}", path.display());
        } else {
            println!("previous project version: none");
        }
        if self.runtime_changes.is_empty() {
            println!("runtime.toml: unchanged");
        } else {
            println!("runtime.toml changes:");
            for change in &self.runtime_changes {
                println!("  - {change}");
            }
        }
        if self.io_changes.is_empty() {
            println!("io.toml: unchanged");
        } else {
            println!("io.toml changes:");
            for change in &self.io_changes {
                println!("  - {change}");
            }
        }
        if self.bytecode_changed {
            println!("program.stbc: updated");
        } else {
            println!("program.stbc: unchanged");
        }
        if self.source_diff.is_empty() {
            println!("sources: unchanged");
        } else {
            if !self.source_diff.added.is_empty() {
                println!("sources added: {}", self.source_diff.added.join(", "));
            }
            if !self.source_diff.removed.is_empty() {
                println!("sources removed: {}", self.source_diff.removed.join(", "));
            }
            if !self.source_diff.modified.is_empty() {
                println!("sources modified: {}", self.source_diff.modified.join(", "));
            }
        }
    }

    fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Deployment summary".to_string());
        if let Some(path) = &self.previous_path {
            lines.push(format!("previous project version: {}", path.display()));
        } else {
            lines.push("previous project version: none".to_string());
        }
        if self.runtime_changes.is_empty() {
            lines.push("runtime.toml: unchanged".to_string());
        } else {
            lines.push("runtime.toml changes:".to_string());
            for change in &self.runtime_changes {
                lines.push(format!("  - {change}"));
            }
        }
        if self.io_changes.is_empty() {
            lines.push("io.toml: unchanged".to_string());
        } else {
            lines.push("io.toml changes:".to_string());
            for change in &self.io_changes {
                lines.push(format!("  - {change}"));
            }
        }
        lines.push(format!(
            "program.stbc: {}",
            if self.bytecode_changed {
                "updated"
            } else {
                "unchanged"
            }
        ));
        if self.source_diff.is_empty() {
            lines.push("sources: unchanged".to_string());
        } else {
            if !self.source_diff.added.is_empty() {
                lines.push(format!(
                    "sources added: {}",
                    self.source_diff.added.join(", ")
                ));
            }
            if !self.source_diff.removed.is_empty() {
                lines.push(format!(
                    "sources removed: {}",
                    self.source_diff.removed.join(", ")
                ));
            }
            if !self.source_diff.modified.is_empty() {
                lines.push(format!(
                    "sources modified: {}",
                    self.source_diff.modified.join(", ")
                ));
            }
        }
        lines.join("\n")
    }
}

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
        diff_field(&mut changes, "driver", &prev.driver, &next.driver);
        if prev.params != next.params {
            changes.push("params: updated".to_string());
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
            (Some(prev_bytes), Some(next_bytes)) => {
                if prev_bytes != next_bytes {
                    modified.push(key);
                }
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

fn safe_state_changed(
    prev: &trust_runtime::io::IoSafeState,
    next: &trust_runtime::io::IoSafeState,
) -> bool {
    if prev.outputs.len() != next.outputs.len() {
        return true;
    }
    let mut prev_set = BTreeSet::new();
    for (address, value) in &prev.outputs {
        prev_set.insert((format_address(address), format!("{value:?}")));
    }
    let mut next_set = BTreeSet::new();
    for (address, value) in &next.outputs {
        next_set.insert((format_address(address), format!("{value:?}")));
    }
    prev_set != next_set
}

fn format_address(address: &IoAddress) -> String {
    let area = match address.area {
        trust_runtime::memory::IoArea::Input => "I",
        trust_runtime::memory::IoArea::Output => "Q",
        trust_runtime::memory::IoArea::Memory => "M",
    };
    let size = match address.size {
        trust_runtime::io::IoSize::Bit => "X",
        trust_runtime::io::IoSize::Byte => "B",
        trust_runtime::io::IoSize::Word => "W",
        trust_runtime::io::IoSize::DWord => "D",
        trust_runtime::io::IoSize::LWord => "L",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    if address.size == trust_runtime::io::IoSize::Bit {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}

fn collect_sources(root: &Path) -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let sources_root = root.join("sources");
    if !sources_root.is_dir() {
        return Ok(BTreeMap::new());
    }
    let mut map = BTreeMap::new();
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    for pattern in patterns {
        for entry in glob::glob(&format!("{}/{}", sources_root.display(), pattern))? {
            let path = entry?;
            if !path.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(&sources_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            if map.contains_key(&relative) {
                continue;
            }
            let bytes = fs::read(&path)?;
            map.insert(relative, bytes);
        }
    }
    Ok(map)
}

fn diff_field<T: std::fmt::Display + PartialEq>(
    changes: &mut Vec<String>,
    name: &str,
    prev: &T,
    next: &T,
) {
    if prev != next {
        changes.push(format!("{name}: {prev} -> {next}"));
    }
}

fn token_state<T>(token: Option<&T>) -> &'static str {
    if token.is_some() {
        "set"
    } else {
        "unset"
    }
}

fn path_state(path: Option<&PathBuf>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "none".to_string())
}
