//! Bundle template rendering helpers for setup and wizards.

use smol_str::SmolStr;

/// Template for an io.toml file.
#[derive(Debug, Clone)]
pub struct IoConfigTemplate {
    /// Driver configurations.
    pub drivers: Vec<IoDriverTemplate>,
    /// Optional safe state entries.
    pub safe_state: Vec<(String, String)>,
}

/// Single I/O driver template.
#[derive(Debug, Clone)]
pub struct IoDriverTemplate {
    /// Driver name.
    pub name: String,
    /// Driver parameters.
    pub params: toml::Value,
}

/// Build a default io.toml template for a driver.
pub fn build_io_config_auto(driver: &str) -> anyhow::Result<IoConfigTemplate> {
    if !matches!(
        driver,
        "loopback" | "gpio" | "modbus-tcp" | "simulated" | "mqtt"
    ) {
        anyhow::bail!("unknown driver '{driver}'");
    }
    let safe_state = vec![("%QX0.0".to_string(), "FALSE".to_string())];
    if driver.eq_ignore_ascii_case("gpio") {
        let mut params = toml::map::Map::new();
        params.insert("backend".into(), toml::Value::String("sysfs".to_string()));
        let inputs = toml::Value::Array(vec![toml::Value::Table(toml::map::Map::from_iter([
            ("address".into(), toml::Value::String("%IX0.0".to_string())),
            ("line".into(), toml::Value::Integer(17)),
        ]))]);
        let outputs = toml::Value::Array(vec![toml::Value::Table(toml::map::Map::from_iter([
            ("address".into(), toml::Value::String("%QX0.0".to_string())),
            ("line".into(), toml::Value::Integer(27)),
        ]))]);
        params.insert("inputs".into(), inputs);
        params.insert("outputs".into(), outputs);
        return Ok(IoConfigTemplate {
            drivers: vec![IoDriverTemplate {
                name: "gpio".to_string(),
                params: toml::Value::Table(params),
            }],
            safe_state,
        });
    }
    if driver.eq_ignore_ascii_case("modbus-tcp") {
        let mut params = toml::map::Map::new();
        params.insert(
            "address".into(),
            toml::Value::String("127.0.0.1:502".to_string()),
        );
        params.insert("unit_id".into(), toml::Value::Integer(1));
        params.insert("input_start".into(), toml::Value::Integer(0));
        params.insert("output_start".into(), toml::Value::Integer(0));
        params.insert("timeout_ms".into(), toml::Value::Integer(500));
        params.insert("on_error".into(), toml::Value::String("fault".to_string()));
        return Ok(IoConfigTemplate {
            drivers: vec![IoDriverTemplate {
                name: "modbus-tcp".to_string(),
                params: toml::Value::Table(params),
            }],
            safe_state,
        });
    }
    if driver.eq_ignore_ascii_case("simulated") {
        return Ok(IoConfigTemplate {
            drivers: vec![IoDriverTemplate {
                name: "simulated".to_string(),
                params: toml::Value::Table(toml::map::Map::new()),
            }],
            safe_state,
        });
    }
    if driver.eq_ignore_ascii_case("mqtt") {
        let mut params = toml::map::Map::new();
        params.insert(
            "broker".into(),
            toml::Value::String("127.0.0.1:1883".to_string()),
        );
        params.insert(
            "topic_in".into(),
            toml::Value::String("trust/io/in".to_string()),
        );
        params.insert(
            "topic_out".into(),
            toml::Value::String("trust/io/out".to_string()),
        );
        params.insert("reconnect_ms".into(), toml::Value::Integer(500));
        params.insert("keep_alive_s".into(), toml::Value::Integer(5));
        params.insert("allow_insecure_remote".into(), toml::Value::Boolean(false));
        return Ok(IoConfigTemplate {
            drivers: vec![IoDriverTemplate {
                name: "mqtt".to_string(),
                params: toml::Value::Table(params),
            }],
            safe_state,
        });
    }
    Ok(IoConfigTemplate {
        drivers: vec![IoDriverTemplate {
            name: "loopback".to_string(),
            params: toml::Value::Table(toml::map::Map::new()),
        }],
        safe_state,
    })
}

/// Render a default runtime.toml file.
#[must_use]
pub fn render_runtime_toml(resource_name: &SmolStr, cycle_ms: u64) -> String {
    format!(
        "[bundle]\nversion = 1\n\n[resource]\nname = \"{resource_name}\"\ncycle_interval_ms = {cycle_ms}\n\n[runtime.control]\nendpoint = \"unix:///tmp/trust-runtime.sock\"\nmode = \"production\"\ndebug_enabled = false\n\n[runtime.web]\nenabled = true\nlisten = \"0.0.0.0:8080\"\nauth = \"local\"\ntls = false\n\n[runtime.tls]\nmode = \"disabled\"\nrequire_remote = false\n\n[runtime.discovery]\nenabled = true\nservice_name = \"truST\"\nadvertise = true\ninterfaces = [\"eth0\", \"wlan0\"]\n\n[runtime.mesh]\nenabled = false\nlisten = \"0.0.0.0:5200\"\ntls = false\nauth_token = \"\"\npublish = []\n\n[runtime.opcua]\nenabled = false\nlisten = \"0.0.0.0:4840\"\nendpoint_path = \"/\"\nnamespace_uri = \"urn:trust:runtime\"\npublish_interval_ms = 250\nmax_nodes = 128\nexpose = []\nsecurity_policy = \"basic256sha256\"\nsecurity_mode = \"sign_and_encrypt\"\nallow_anonymous = false\n\n[runtime.observability]\nenabled = false\nsample_interval_ms = 1000\nmode = \"all\"\ninclude = []\nhistory_path = \"history/historian.jsonl\"\nmax_entries = 20000\nprometheus_enabled = true\nprometheus_path = \"/metrics\"\n\n[runtime.log]\nlevel = \"info\"\n\n[runtime.retain]\nmode = \"none\"\nsave_interval_ms = 1000\n\n[runtime.watchdog]\nenabled = false\ntimeout_ms = 5000\naction = \"halt\"\n\n[runtime.fault]\npolicy = \"halt\"\n"
    )
}

/// Render an io.toml file from a template.
#[must_use]
pub fn render_io_toml(config: &IoConfigTemplate) -> String {
    let mut root = toml::map::Map::new();
    let mut io = toml::map::Map::new();
    if config.drivers.len() == 1 {
        if let Some(driver) = config.drivers.first() {
            io.insert("driver".into(), toml::Value::String(driver.name.clone()));
            io.insert("params".into(), driver.params.clone());
        }
    } else {
        let drivers = config
            .drivers
            .iter()
            .map(|driver| {
                toml::Value::Table(toml::map::Map::from_iter([
                    ("name".into(), toml::Value::String(driver.name.clone())),
                    ("params".into(), driver.params.clone()),
                ]))
            })
            .collect::<Vec<_>>();
        io.insert("drivers".into(), toml::Value::Array(drivers));
    }
    if !config.safe_state.is_empty() {
        let entries = config
            .safe_state
            .iter()
            .map(|(address, value)| {
                toml::Value::Table(toml::map::Map::from_iter([
                    ("address".into(), toml::Value::String(address.clone())),
                    ("value".into(), toml::Value::String(value.clone())),
                ]))
            })
            .collect::<Vec<_>>();
        io.insert("safe_state".into(), toml::Value::Array(entries));
    }
    root.insert("io".into(), toml::Value::Table(io));
    toml::to_string(&toml::Value::Table(root)).unwrap_or_default()
}
