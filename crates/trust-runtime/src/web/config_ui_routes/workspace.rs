use super::*;

pub(super) fn load_workspace_model(
    bundle_root: &Option<PathBuf>,
) -> Result<WorkspaceModel, RuntimeError> {
    let root = default_bundle_root(bundle_root);
    let mut runtime_roots = Vec::<PathBuf>::new();
    if root.join("runtime.toml").is_file() {
        runtime_roots.push(root.clone());
    }
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let candidate = entry.path();
            if candidate.join("runtime.toml").is_file() {
                runtime_roots.push(candidate);
            }
        }
    }
    runtime_roots.sort();
    runtime_roots.dedup();
    if runtime_roots.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "no runtime.toml found in '{}' or direct subdirectories",
                root.display()
            )
            .into(),
        ));
    }

    let mut runtimes = Vec::new();
    let mut seen_ids = BTreeSet::new();
    for runtime_root in runtime_roots {
        let runtime = RuntimeConfig::load(runtime_root.join("runtime.toml"))?;
        let runtime_id = runtime.resource_name.to_string();
        if !seen_ids.insert(runtime_id.clone()) {
            return Err(RuntimeError::InvalidConfig(
                format!("duplicate runtime.resource.name '{runtime_id}' in workspace").into(),
            ));
        }
        runtimes.push(WorkspaceRuntime {
            runtime_id,
            root: runtime_root,
            runtime,
        });
    }
    runtimes.sort_by(|left, right| left.runtime_id.cmp(&right.runtime_id));

    Ok(WorkspaceModel { root, runtimes })
}

pub(super) fn resolve_runtime_target<'a>(
    workspace: &'a WorkspaceModel,
    requested_runtime_id: Option<&str>,
    control_state: &Arc<ControlState>,
) -> Result<&'a WorkspaceRuntime, RuntimeError> {
    if let Some(requested) = requested_runtime_id
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return resolve_runtime_by_id(workspace, requested);
    }

    let connected_via =
        crate::control::runtime_resource_name_port(control_state.as_ref()).to_string();
    if !connected_via.is_empty() {
        if let Ok(runtime) = resolve_runtime_by_id(workspace, connected_via.as_str()) {
            return Ok(runtime);
        }
    }

    workspace
        .runtimes
        .first()
        .ok_or_else(|| RuntimeError::InvalidConfig("workspace has no runtimes".into()))
}

pub(super) fn resolve_runtime_by_id<'a>(
    workspace: &'a WorkspaceModel,
    runtime_id: &str,
) -> Result<&'a WorkspaceRuntime, RuntimeError> {
    workspace
        .runtimes
        .iter()
        .find(|runtime| runtime.runtime_id == runtime_id)
        .ok_or_else(|| {
            RuntimeError::InvalidConfig(format!("unknown runtime_id '{runtime_id}'").into())
        })
}

pub(super) fn normalize_runtime_id(raw: &str) -> Result<String, RuntimeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig("runtime_id is required".into()));
    }
    if trimmed
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime_id may only contain [a-zA-Z0-9-_]".into(),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub(super) fn normalize_host_group(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                        ch.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .trim_matches('-')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

pub(super) fn render_new_runtime_toml(runtime_id: &str, host_group: Option<&str>) -> String {
    let normalized_group = host_group
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default-host");
    format!(
        r#"[bundle]
version = 1

[resource]
name = "{runtime_id}"
cycle_interval_ms = 20

[runtime.control]
endpoint = "unix:///tmp/{runtime_id}.sock"
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"

[runtime.web]
enabled = true
listen = "127.0.0.1:0"
auth = "local"
tls = false

[runtime.discovery]
enabled = true
service_name = "{runtime_id}"
advertise = true
interfaces = ["lo"]
host_group = "{normalized_group}"

[runtime.mesh]
enabled = true
role = "peer"
listen = "127.0.0.1:0"
connect = []
tls = false
publish = []
subscribe = {{}}

[runtime.cloud]
profile = "dev"

[runtime.cloud.wan]
allow_write = []

[runtime.cloud.links]
transports = []
"#
    )
}

pub(super) fn create_workspace_runtime(
    workspace: &WorkspaceModel,
    runtime_id: &str,
    host_group: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let runtime_id = normalize_runtime_id(runtime_id)?;
    if workspace
        .runtimes
        .iter()
        .any(|runtime| runtime.runtime_id == runtime_id)
    {
        return Err(RuntimeError::InvalidConfig(
            format!("runtime '{runtime_id}' already exists").into(),
        ));
    }

    let runtime_root = workspace.root.join(runtime_id.as_str());
    if runtime_root.exists() {
        return Err(RuntimeError::InvalidConfig(
            format!("runtime folder '{}' already exists", runtime_root.display()).into(),
        ));
    }

    let normalized_group = normalize_host_group(host_group);
    let runtime_text = render_new_runtime_toml(runtime_id.as_str(), normalized_group.as_deref());
    crate::config::validate_runtime_toml_text(runtime_text.as_str())?;

    let io_template = crate::bundle_template::build_io_config_auto("simulated")
        .map_err(|error| RuntimeError::InvalidConfig(error.to_string().into()))?;
    let io_text = crate::bundle_template::render_io_toml(&io_template);
    crate::config::validate_io_toml_text(io_text.as_str())?;

    let main_st_text = "PROGRAM Main\nVAR\n  x : INT := 0;\nEND_VAR\nEND_PROGRAM\n";

    atomic_write_text(
        runtime_root.join("runtime.toml").as_path(),
        runtime_text.as_str(),
    )?;
    atomic_write_text(runtime_root.join("io.toml").as_path(), io_text.as_str())?;
    atomic_write_text(runtime_root.join("src/main.st").as_path(), main_st_text)?;

    Ok(json!({
        "ok": true,
        "runtime_id": runtime_id,
        "runtime_root": runtime_root.display().to_string(),
        "host_group": normalized_group,
        "runtime_revision": text_revision(runtime_text.as_str()),
        "io_revision": text_revision(io_text.as_str()),
        "st_revision": text_revision(main_st_text),
        "message": "runtime created",
    }))
}

pub(super) fn delete_workspace_runtime(
    workspace: &WorkspaceModel,
    runtime_id: &str,
) -> Result<serde_json::Value, RuntimeError> {
    if workspace.runtimes.len() <= 1 {
        return Err(RuntimeError::InvalidConfig(
            "cannot delete the last runtime in workspace".into(),
        ));
    }
    let runtime_id = normalize_runtime_id(runtime_id)?;
    let runtime = resolve_runtime_by_id(workspace, runtime_id.as_str())?;
    if runtime.root == workspace.root {
        return Err(RuntimeError::InvalidConfig(
            "refusing to delete workspace root runtime; move project to multi-runtime layout first"
                .into(),
        ));
    }
    if !runtime.root.starts_with(&workspace.root) {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "refusing to delete runtime outside workspace root: '{}'",
                runtime.root.display()
            )
            .into(),
        ));
    }
    fs::remove_dir_all(&runtime.root).map_err(|error| {
        RuntimeError::InvalidConfig(
            format!(
                "failed to delete runtime '{}': {error}",
                runtime.root.display()
            )
            .into(),
        )
    })?;
    Ok(json!({
        "ok": true,
        "runtime_id": runtime_id,
        "message": "runtime deleted",
    }))
}

pub(super) fn text_revision(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

pub(super) fn atomic_write_text(path: &Path, text: &str) -> Result<(), RuntimeError> {
    let parent = path.parent().ok_or_else(|| {
        RuntimeError::InvalidConfig(
            format!("invalid destination '{}': missing parent", path.display()).into(),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        RuntimeError::InvalidConfig(
            format!("failed to create directory '{}': {error}", parent.display()).into(),
        )
    })?;
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config"),
        std::process::id(),
        now_ns()
    ));
    fs::write(&temp_path, text).map_err(|error| {
        RuntimeError::InvalidConfig(
            format!(
                "failed to write temp file '{}': {error}",
                temp_path.display()
            )
            .into(),
        )
    })?;
    fs::rename(&temp_path, path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        RuntimeError::InvalidConfig(
            format!("failed to replace '{}': {error}", path.display()).into(),
        )
    })
}

pub(super) fn write_config_file(
    path: &Path,
    text: &str,
    expected_revision: Option<&str>,
    validator: fn(&str) -> Result<(), RuntimeError>,
) -> Result<String, RuntimeError> {
    let current_text = fs::read_to_string(path).unwrap_or_default();
    let current_revision = text_revision(current_text.as_str());
    if let Some(expected) = expected_revision
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if expected != current_revision {
            return Err(RuntimeError::ControlError(
                format!("conflict: {current_revision}").into(),
            ));
        }
    }
    validator(text)?;
    atomic_write_text(path, text)?;
    Ok(text_revision(text))
}

pub(super) fn normalize_st_relative_path(path: &str) -> Result<PathBuf, RuntimeError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig("path is required".into()));
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        return Err(RuntimeError::InvalidConfig(
            "path must be relative to src/".into(),
        ));
    }
    for component in candidate.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(RuntimeError::InvalidConfig(
                    "path must stay under src/".into(),
                ));
            }
            _ => {}
        }
    }
    let extension = candidate
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "st" {
        return Err(RuntimeError::InvalidConfig(
            "path must reference a .st file".into(),
        ));
    }
    Ok(candidate)
}

pub(super) fn load_project_io_config_response(
    runtime_root: &Path,
) -> Result<IoConfigResponse, RuntimeError> {
    let io_path = runtime_root.join("io.toml");
    if io_path.is_file() {
        let config = IoConfig::load(&io_path)?;
        return Ok(io_config_to_response(config, "project", false));
    }
    Ok(IoConfigResponse {
        driver: "loopback".to_string(),
        params: json!({}),
        drivers: Vec::new(),
        safe_state: Vec::new(),
        supported_drivers: IoDriverRegistry::default_registry().canonical_driver_names(),
        source: "default".to_string(),
        use_system_io: false,
    })
}

pub(super) fn config_project_state(
    workspace: WorkspaceModel,
) -> Result<serde_json::Value, RuntimeError> {
    let mut hasher = Sha256::new();
    let mut runtimes = Vec::new();

    for runtime in workspace.runtimes {
        let runtime_toml_path = runtime.root.join("runtime.toml");
        let io_toml_path = runtime.root.join("io.toml");
        let runtime_text = fs::read_to_string(&runtime_toml_path).map_err(|error| {
            RuntimeError::InvalidConfig(format!("failed to read runtime.toml: {error}").into())
        })?;
        let io_text = fs::read_to_string(&io_toml_path).unwrap_or_default();
        let st_files = list_sources(runtime.root.as_path());

        hasher.update(runtime.runtime_id.as_bytes());
        hasher.update(runtime_text.as_bytes());
        hasher.update(io_text.as_bytes());
        for file in &st_files {
            hasher.update(file.as_bytes());
        }

        runtimes.push(json!({
            "runtime_id": runtime.runtime_id,
            "project_path": runtime.root.display().to_string(),
            "host_group": runtime.runtime.discovery.host_group.map(|v| v.to_string()),
            "runtime_revision": text_revision(runtime_text.as_str()),
            "io_revision": text_revision(io_text.as_str()),
            "st_files": st_files,
        }));
    }

    Ok(json!({
        "ok": true,
        "api_version": RUNTIME_CLOUD_API_VERSION,
        "mode": "config",
        "project_root": workspace.root.display().to_string(),
        "revision": format!("{:x}", hasher.finalize()),
        "runtimes": runtimes,
    }))
}

pub(super) fn validate_st_sources(
    runtime_root: &Path,
    override_path: Option<&str>,
    override_text: Option<&str>,
) -> Result<Vec<serde_json::Value>, RuntimeError> {
    let mut source_map = BTreeMap::<String, String>::new();
    for file in list_sources(runtime_root) {
        let text = read_source_file(runtime_root, file.as_str())?;
        source_map.insert(file, text);
    }

    if let Some(path) = override_path {
        let normalized = normalize_st_relative_path(path)?;
        let text = if let Some(override_text) = override_text {
            override_text.to_string()
        } else {
            read_source_file(runtime_root, normalized.to_string_lossy().as_ref())?
        };
        source_map.insert(normalized.to_string_lossy().to_string(), text);
    }

    if source_map.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "no ST sources found under src/".into(),
        ));
    }

    let sources = source_map
        .iter()
        .map(|(path, text)| HarnessSourceFile::with_path(path.clone(), text.clone()))
        .collect::<Vec<_>>();

    match CompileSession::from_sources(sources).build_bytecode_module() {
        Ok(_) => Ok(Vec::new()),
        Err(error) => Err(RuntimeError::InvalidConfig(
            format!("ST validation failed: {error}").into(),
        )),
    }
}
