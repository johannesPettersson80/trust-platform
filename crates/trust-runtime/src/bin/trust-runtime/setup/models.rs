const DEFAULT_SETUP_PORT: u16 = 8080;
const DEFAULT_REMOTE_TOKEN_TTL_MINUTES: u64 = 15;
const MAX_REMOTE_TOKEN_TTL_MINUTES: u64 = 24 * 60;

#[derive(Debug, Clone)]
pub struct SetupCommandOptions {
    pub mode: Option<SetupModeArg>,
    pub access: Option<SetupAccessArg>,
    pub project: Option<PathBuf>,
    pub bind: Option<String>,
    pub port: Option<u16>,
    pub token_ttl_minutes: Option<u64>,
    pub dry_run: bool,
    pub driver: Option<String>,
    pub backend: Option<String>,
    pub path: Option<PathBuf>,
    pub force: bool,
}

#[derive(Debug, Clone)]
struct BrowserSetupProfile {
    bind: String,
    port: u16,
    token_required: bool,
    token_ttl_minutes: u64,
}

impl BrowserSetupProfile {
    fn build(
        access: SetupAccessArg,
        bind_override: Option<String>,
        port: u16,
        token_ttl_minutes: Option<u64>,
    ) -> anyhow::Result<Self> {
        let default_bind = match access {
            SetupAccessArg::Local => "127.0.0.1",
            SetupAccessArg::Remote => "0.0.0.0",
        };
        let bind = normalize_bind(bind_override.unwrap_or_else(|| default_bind.to_string()))?;
        match access {
            SetupAccessArg::Local => {
                if !is_loopback_bind(&bind) {
                    anyhow::bail!(
                        "local browser setup must use a loopback bind (127.0.0.1, ::1, localhost)"
                    );
                }
                if token_ttl_minutes.unwrap_or(0) > 0 {
                    anyhow::bail!(
                        "local browser setup must not set token TTL (tokens are remote-only)"
                    );
                }
                Ok(Self {
                    bind,
                    port,
                    token_required: false,
                    token_ttl_minutes: 0,
                })
            }
            SetupAccessArg::Remote => {
                if is_loopback_bind(&bind) {
                    anyhow::bail!(
                        "remote browser setup must not use a loopback bind; use 0.0.0.0 or a LAN address"
                    );
                }
                let ttl = token_ttl_minutes.unwrap_or(DEFAULT_REMOTE_TOKEN_TTL_MINUTES);
                if ttl == 0 {
                    anyhow::bail!("remote browser setup requires token_ttl_minutes > 0");
                }
                if ttl > MAX_REMOTE_TOKEN_TTL_MINUTES {
                    anyhow::bail!(
                        "token_ttl_minutes exceeds max allowed value ({MAX_REMOTE_TOKEN_TTL_MINUTES})"
                    );
                }
                Ok(Self {
                    bind,
                    port,
                    token_required: true,
                    token_ttl_minutes: ttl,
                })
            }
        }
    }
}
