fn print_cancel_message() {
    println!(
        "{}",
        style::warning("Setup cancelled. Resume any time with: trust-runtime setup")
    );
}

fn print_setup_complete(bundle_path: &Path) {
    println!("{}", style::success("✓ Setup complete!"));
    println!("Start the PLC with:");
    println!("  trust-runtime --project {}", bundle_path.display());
    println!("Open http://localhost:8080 to monitor.");
}

fn normalize_bind(bind: String) -> anyhow::Result<String> {
    let trimmed = bind.trim();
    if trimmed.is_empty() {
        anyhow::bail!("bind address must not be empty");
    }
    Ok(trimmed.to_string())
}

fn is_loopback_bind(bind: &str) -> bool {
    if bind.eq_ignore_ascii_case("localhost") || bind == "127.0.0.1" || bind == "::1" {
        return true;
    }
    bind.parse::<IpAddr>()
        .map(|addr| addr.is_loopback())
        .unwrap_or(false)
}

fn setup_port_from_prompt_value(value: u64) -> anyhow::Result<u16> {
    u16::try_from(value).map_err(|_| anyhow::anyhow!("setup port must be <= 65535"))
}

fn default_bundle_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("project")
}

#[derive(Debug, Clone)]
pub(crate) struct SetupDefaults {
    pub resource_name: SmolStr,
    pub cycle_ms: u64,
    pub driver: String,
}

impl SetupDefaults {
    pub fn from_bundle(root: &Path) -> Self {
        let resource_name = wizard::default_resource_name(root);
        let cycle_ms = 100;
        let driver = if trust_runtime::setup::is_raspberry_pi_hint() {
            "gpio"
        } else {
            "loopback"
        };
        Self {
            resource_name,
            cycle_ms,
            driver: driver.to_string(),
        }
    }
}
