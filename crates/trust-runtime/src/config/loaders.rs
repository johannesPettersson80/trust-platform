impl RuntimeBundle {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let root = root.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(RuntimeError::InvalidBundle(
                format!("project folder not found: {}", root.display()).into(),
            ));
        }
        let runtime_path = root.join("runtime.toml");
        let io_path = root.join("io.toml");
        let simulation_path = root.join("simulation.toml");
        let program_path = root.join("program.stbc");

        if !runtime_path.is_file() {
            return Err(RuntimeError::InvalidBundle(
                format!(
                    "missing runtime.toml at {} (run `trust-runtime` to auto-create a project folder)",
                    runtime_path.display()
                )
                .into(),
            ));
        }
        if !program_path.is_file() {
            return Err(RuntimeError::InvalidBundle(
                format!(
                    "missing program.stbc at {} (run `trust-runtime` to auto-create a project folder)",
                    program_path.display()
                )
                .into(),
            ));
        }

        let runtime = RuntimeConfig::load(&runtime_path)?;
        let io = if io_path.is_file() {
            IoConfig::load(&io_path)?
        } else if let Some(system_io) = load_system_io_config()? {
            system_io
        } else {
            return Err(RuntimeError::InvalidBundle(
                format!(
                    "missing io.toml at {} and no system io config at {} (run `trust-runtime setup` or `trust-runtime`)",
                    io_path.display(),
                    system_io_config_path().display()
                )
                .into(),
            ));
        };
        let ads = load_bundle_ads_config(&root, &runtime)?;
        let bytecode = std::fs::read(&program_path).map_err(|err| {
            RuntimeError::InvalidBundle(format!("failed to read program.stbc: {err}").into())
        })?;
        let simulation = SimulationConfig::load_optional(&simulation_path)?;

        Ok(Self {
            root,
            runtime,
            io,
            ads: ads.as_ref().map(|loaded| loaded.config.clone()),
            ads_config_hash: ads.map(|loaded| loaded.config_hash),
            simulation,
            bytecode,
        })
    }
}

impl RuntimeConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|err| RuntimeError::InvalidConfig(format!("runtime.toml: {err}").into()))?;
        parser::parse_runtime_toml_from_text(&text, "runtime.toml")
    }
}

impl IoConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|err| RuntimeError::InvalidConfig(format!("io.toml: {err}").into()))?;
        parser::parse_io_toml_from_text(&text, "io.toml")
    }
}

#[must_use]
pub fn system_io_config_path() -> PathBuf {
    PathBuf::from(SYSTEM_IO_CONFIG_PATH)
}

pub fn load_system_io_config() -> Result<Option<IoConfig>, RuntimeError> {
    let path = system_io_config_path();
    if !path.is_file() {
        return Ok(None);
    }
    IoConfig::load(path).map(Some)
}

struct LoadedAdsConfig {
    config: crate::ads::AdsClientConfig,
    config_hash: String,
}

fn load_bundle_ads_config(
    root: &Path,
    runtime: &RuntimeConfig,
) -> Result<Option<LoadedAdsConfig>, RuntimeError> {
    if !runtime.ads.enabled {
        return Ok(None);
    }
    let ads_path = if runtime.ads.config_path.is_relative() {
        root.join(&runtime.ads.config_path)
    } else {
        runtime.ads.config_path.clone()
    };
    if !ads_path.is_file() {
        return Err(RuntimeError::InvalidBundle(
            format!(
                "runtime.ads.enabled=true but ADS config is missing at {}",
                ads_path.display()
            )
            .into(),
        ));
    }
    let text = std::fs::read_to_string(&ads_path).map_err(|err| {
        RuntimeError::InvalidConfig(format!("{}: {err}", ads_path.display()).into())
    })?;
    let config_hash = crate::ads::diagnostics::sha256_evidence_hash(text.as_bytes());
    let config = crate::ads::parse_ads_toml(&text)?;
    Ok(Some(LoadedAdsConfig {
        config,
        config_hash,
    }))
}

pub fn validate_runtime_toml_text(text: &str) -> Result<(), RuntimeError> {
    parser::parse_runtime_toml_from_text(text, "runtime.toml").map(|_| ())
}

pub fn validate_io_toml_text(text: &str) -> Result<(), RuntimeError> {
    parser::parse_io_toml_from_text(text, "io.toml").map(|_| ())
}
