use super::*;

pub(super) enum ControlStream {
    Tcp(std::net::TcpStream),
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
}

impl Read for ControlStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ControlStream::Tcp(stream) => stream.read(buf),
            #[cfg(unix)]
            ControlStream::Unix(stream) => stream.read(buf),
        }
    }
}

impl Write for ControlStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            ControlStream::Tcp(stream) => stream.write(buf),
            #[cfg(unix)]
            ControlStream::Unix(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            ControlStream::Tcp(stream) => stream.flush(),
            #[cfg(unix)]
            ControlStream::Unix(stream) => stream.flush(),
        }
    }
}

pub(super) struct ControlClient {
    pub(super) token: Option<String>,
    pub(super) reader: io::BufReader<ControlStream>,
}

impl ControlClient {
    pub(super) fn connect(
        endpoint: ControlEndpoint,
        token: Option<String>,
    ) -> anyhow::Result<Self> {
        let stream = match &endpoint {
            ControlEndpoint::Tcp(addr) => ControlStream::Tcp(std::net::TcpStream::connect(addr)?),
            #[cfg(unix)]
            ControlEndpoint::Unix(path) => {
                ControlStream::Unix(std::os::unix::net::UnixStream::connect(path)?)
            }
        };
        Ok(Self {
            token,
            reader: io::BufReader::new(stream),
        })
    }

    pub(super) fn request(
        &mut self,
        mut payload: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        if let Some(token) = self.token.as_deref() {
            payload["auth"] = json!(token);
        }
        let line = serde_json::to_string(&payload)?;
        {
            let stream = self.reader.get_mut();
            stream.write_all(line.as_bytes())?;
            stream.write_all(b"\n")?;
            stream.flush()?;
        }
        let mut response = String::new();
        self.reader.read_line(&mut response)?;
        Ok(serde_json::from_str(&response)?)
    }
}

pub(super) fn resolve_endpoint(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
) -> anyhow::Result<(ControlEndpoint, Option<String>, Option<PathBuf>)> {
    let mut auth = token.or_else(|| std::env::var("TRUST_CTL_TOKEN").ok());
    if let Some(endpoint) = endpoint {
        return Ok((ControlEndpoint::parse(&endpoint)?, auth, bundle));
    }
    let bundle_path = detect_bundle_path(bundle).map_err(anyhow::Error::from)?;
    let bundle = RuntimeBundle::load(bundle_path.clone())?;
    if auth.is_none() {
        auth = bundle
            .runtime
            .control_auth_token
            .as_ref()
            .map(|value| value.to_string());
    }
    Ok((
        ControlEndpoint::parse(bundle.runtime.control_endpoint.as_str())?,
        auth,
        Some(bundle_path),
    ))
}

pub(super) fn load_console_config(root: &Path) -> ConsoleConfig {
    let path = root.join("runtime.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return ConsoleConfig::default(),
    };
    let value: toml::Value = match text.parse() {
        Ok(value) => value,
        Err(_) => return ConsoleConfig::default(),
    };
    let console = match value.get("console") {
        Some(console) => console,
        None => return ConsoleConfig::default(),
    };
    let layout = console
        .get("layout")
        .and_then(|value| value.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry.as_str())
                .filter_map(PanelKind::parse)
                .collect::<Vec<_>>()
        });
    let refresh_ms = console
        .get("refresh_ms")
        .and_then(|value| value.as_integer())
        .and_then(|value| u64::try_from(value).ok());
    ConsoleConfig { layout, refresh_ms }
}

pub(super) fn fetch_data(client: &mut ControlClient) -> anyhow::Result<UiData> {
    let status = client.request(json!({"id": 1, "type": "status"}))?;
    let tasks = client.request(json!({"id": 2, "type": "tasks.stats"}))?;
    let io = client.request(json!({"id": 3, "type": "io.list"}))?;
    let events =
        client.request(json!({"id": 4, "type": "events.tail", "params": { "limit": 20 }}))?;
    let settings = client.request(json!({"id": 5, "type": "config.get"}))?;
    Ok(UiData {
        status: parse_status(&status),
        tasks: parse_tasks(&tasks),
        io: parse_io(&io),
        events: parse_events(&events),
        settings: parse_settings(&settings),
    })
}
