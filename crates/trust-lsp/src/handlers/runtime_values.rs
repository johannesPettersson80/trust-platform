use rustc_hash::FxHashMap;
use serde::Deserialize;
use serde_json::Value;
use smol_str::SmolStr;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;
use tracing::{debug, warn};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

const CONTROL_IO_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeInlineValues {
    pub(crate) locals: FxHashMap<SmolStr, String>,
    pub(crate) globals: FxHashMap<SmolStr, String>,
    pub(crate) retain: FxHashMap<SmolStr, String>,
}

#[derive(Debug)]
pub(crate) enum ControlEndpoint {
    Tcp(String),
    #[cfg(unix)]
    Unix(std::path::PathBuf),
}

impl ControlEndpoint {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        if let Some(rest) = text.strip_prefix("tcp://") {
            return Some(Self::Tcp(rest.to_string()));
        }
        #[cfg(unix)]
        if let Some(rest) = text.strip_prefix("unix://") {
            return Some(Self::Unix(std::path::PathBuf::from(rest)));
        }
        None
    }
}

#[derive(Debug)]
struct ControlClient {
    seq: u64,
    reader: BufReader<ControlStream>,
    auth: Option<String>,
}

impl ControlClient {
    fn connect(endpoint: ControlEndpoint, auth: Option<&str>) -> Option<Self> {
        let stream = match endpoint {
            ControlEndpoint::Tcp(addr) => ControlStream::Tcp(connect_tcp(&addr)?),
            #[cfg(unix)]
            ControlEndpoint::Unix(path) => {
                let stream = UnixStream::connect(path).ok()?;
                configure_unix_stream_timeout(&stream);
                ControlStream::Unix(stream)
            }
        };
        Some(Self {
            seq: 1,
            reader: BufReader::new(stream),
            auth: auth.map(|value| value.to_string()),
        })
    }

    fn next_id(&mut self) -> u64 {
        let id = self.seq;
        self.seq = self.seq.saturating_add(1);
        id
    }

    fn request(&mut self, kind: &str, params: Option<Value>) -> Option<Value> {
        debug!("inlineValue control request kind={}", kind);
        let mut payload = serde_json::Map::new();
        payload.insert("id".to_string(), Value::from(self.next_id()));
        payload.insert("type".to_string(), Value::from(kind));
        if let Some(params) = params {
            payload.insert("params".to_string(), params);
        }
        if let Some(auth) = &self.auth {
            payload.insert("auth".to_string(), Value::from(auth.clone()));
        }
        let line = serde_json::to_string(&Value::Object(payload)).ok()?;
        {
            let stream = self.reader.get_mut();
            if stream.write_all(line.as_bytes()).is_err() {
                warn!("inlineValue control request kind={} write failed", kind);
                return None;
            }
            if stream.write_all(b"\n").is_err() {
                warn!(
                    "inlineValue control request kind={} write newline failed",
                    kind
                );
                return None;
            }
            if stream.flush().is_err() {
                warn!("inlineValue control request kind={} flush failed", kind);
                return None;
            }
        }
        let mut response = String::new();
        match self.reader.read_line(&mut response) {
            Ok(0) => {
                warn!("inlineValue control request kind={} empty response", kind);
                return None;
            }
            Ok(_) => {}
            Err(err) => {
                warn!(
                    "inlineValue control request kind={} read failed: {err}",
                    kind
                );
                return None;
            }
        }
        let response: ControlResponse = match serde_json::from_str(&response) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    "inlineValue control request kind={} invalid response: {err}",
                    kind
                );
                return None;
            }
        };
        if !response.ok {
            warn!("inlineValue control request kind={} failed", kind);
            return None;
        }
        response.result
    }
}

fn connect_tcp(addr: &str) -> Option<TcpStream> {
    let socket_addr = resolve_socket_addr(addr)?;
    let stream = TcpStream::connect_timeout(&socket_addr, CONTROL_IO_TIMEOUT).ok()?;
    configure_tcp_stream_timeout(&stream);
    Some(stream)
}

fn resolve_socket_addr(addr: &str) -> Option<SocketAddr> {
    addr.parse::<SocketAddr>()
        .ok()
        .or_else(|| addr.to_socket_addrs().ok()?.next())
}

fn configure_tcp_stream_timeout(stream: &TcpStream) {
    if let Err(err) = stream.set_read_timeout(Some(CONTROL_IO_TIMEOUT)) {
        warn!("inlineValue control set TCP read timeout failed: {err}");
    }
    if let Err(err) = stream.set_write_timeout(Some(CONTROL_IO_TIMEOUT)) {
        warn!("inlineValue control set TCP write timeout failed: {err}");
    }
}

#[cfg(unix)]
fn configure_unix_stream_timeout(stream: &UnixStream) {
    if let Err(err) = stream.set_read_timeout(Some(CONTROL_IO_TIMEOUT)) {
        warn!("inlineValue control set Unix read timeout failed: {err}");
    }
    if let Err(err) = stream.set_write_timeout(Some(CONTROL_IO_TIMEOUT)) {
        warn!("inlineValue control set Unix write timeout failed: {err}");
    }
}

#[derive(Debug)]
enum ControlStream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl Read for ControlStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ControlStream::Tcp(stream) => stream.read(buf),
            #[cfg(unix)]
            ControlStream::Unix(stream) => stream.read(buf),
        }
    }
}

impl Write for ControlStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ControlStream::Tcp(stream) => stream.write(buf),
            #[cfg(unix)]
            ControlStream::Unix(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ControlStream::Tcp(stream) => stream.flush(),
            #[cfg(unix)]
            ControlStream::Unix(stream) => stream.flush(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ControlResponse {
    ok: bool,
    result: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugScope {
    name: String,
    variables_reference: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugVariable {
    name: String,
    value: String,
}

pub(crate) fn fetch_runtime_inline_values(
    endpoint: &str,
    auth: Option<&str>,
    frame_id: u32,
    owner_hints: &[SmolStr],
) -> Option<RuntimeInlineValues> {
    let endpoint = match ControlEndpoint::parse(endpoint) {
        Some(endpoint) => endpoint,
        None => {
            warn!("inlineValue control endpoint parse failed: {}", endpoint);
            return None;
        }
    };
    let mut client = match ControlClient::connect(endpoint, auth) {
        Some(client) => client,
        None => {
            warn!("inlineValue control connect failed");
            return None;
        }
    };
    debug!(
        "inlineValue control connected frame_id={} owner_hints={}",
        frame_id,
        owner_hints.len()
    );
    let scopes_value = client.request(
        "debug.scopes",
        Some(serde_json::json!({ "frame_id": frame_id })),
    )?;
    let scopes = scopes_value
        .get("scopes")
        .and_then(|value| serde_json::from_value::<Vec<DebugScope>>(value.clone()).ok())
        .unwrap_or_default();
    debug!(
        "inlineValue control scopes={:?}",
        scopes
            .iter()
            .map(|scope| scope.name.as_str())
            .collect::<Vec<_>>()
    );

    let mut locals_ref = None;
    let mut globals_ref = None;
    let mut retain_ref = None;
    let mut instances_ref = None;
    for scope in scopes {
        match scope.name.to_ascii_lowercase().as_str() {
            "locals" => locals_ref = Some(scope.variables_reference),
            "globals" => globals_ref = Some(scope.variables_reference),
            "retain" => retain_ref = Some(scope.variables_reference),
            "instances" => instances_ref = Some(scope.variables_reference),
            _ => {}
        }
    }

    let mut locals = locals_ref
        .and_then(|reference| fetch_variables(&mut client, reference))
        .unwrap_or_default();
    let globals = globals_ref
        .and_then(|reference| fetch_variables(&mut client, reference))
        .unwrap_or_default();
    let retain = retain_ref
        .and_then(|reference| fetch_variables(&mut client, reference))
        .unwrap_or_default();
    debug!(
        "inlineValue control values locals={} globals={} retain={}",
        locals.len(),
        globals.len(),
        retain.len()
    );

    if let Some(reference) = instances_ref {
        if let Some(instance_vars) = fetch_instance_variables(&mut client, reference, owner_hints) {
            for (name, value) in instance_vars {
                locals.entry(name).or_insert(value);
            }
        }
    }
    debug!(
        "inlineValue merged locals count={} (after instances)",
        locals.len()
    );

    Some(RuntimeInlineValues {
        locals,
        globals,
        retain,
    })
}

fn fetch_instance_variables(
    client: &mut ControlClient,
    reference: u32,
    owner_hints: &[SmolStr],
) -> Option<FxHashMap<SmolStr, String>> {
    let instances_value = client.request(
        "debug.variables",
        Some(serde_json::json!({ "variables_reference": reference })),
    )?;
    let instances = instances_value
        .get("variables")
        .and_then(|value| serde_json::from_value::<Vec<DebugVariableRef>>(value.clone()).ok())
        .unwrap_or_default();

    if instances.is_empty() {
        debug!("inlineValue instances scope empty");
        return None;
    }
    debug!(
        "inlineValue instances candidates={:?}",
        instances
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>()
    );

    let mut chosen = None;
    if !owner_hints.is_empty() {
        for hint in owner_hints {
            let needle = format!("{}#", hint);
            if let Some(instance) = instances.iter().find(|entry| {
                entry
                    .name
                    .to_ascii_lowercase()
                    .starts_with(&needle.to_ascii_lowercase())
            }) {
                chosen = Some(instance.variables_reference);
                debug!("inlineValue instance match exact prefix hint={}", hint);
                break;
            }
        }
    }

    if chosen.is_none() && !owner_hints.is_empty() {
        for hint in owner_hints {
            if let Some(instance) = instances.iter().find(|entry| {
                entry
                    .name
                    .split('#')
                    .next()
                    .map(|type_name| type_name.eq_ignore_ascii_case(hint.as_str()))
                    .unwrap_or(false)
            }) {
                chosen = Some(instance.variables_reference);
                debug!("inlineValue instance match type name hint={}", hint);
                break;
            }
        }
    }

    if chosen.is_none() && !owner_hints.is_empty() {
        let mut candidates = Vec::new();
        for hint in owner_hints {
            let hint_base = hint.rsplit('.').next().unwrap_or(hint.as_str());
            for instance in &instances {
                let type_name = instance.name.split('#').next().unwrap_or("");
                let instance_base = type_name.rsplit('.').next().unwrap_or(type_name);
                if instance_base.eq_ignore_ascii_case(hint_base)
                    && !candidates.contains(&instance.variables_reference)
                {
                    candidates.push(instance.variables_reference);
                }
            }
        }
        if candidates.len() == 1 {
            chosen = Some(candidates[0]);
            debug!(
                "inlineValue instance match base-name candidate_ref={}",
                candidates[0]
            );
        }
    }

    if chosen.is_none() && instances.len() == 1 {
        chosen = Some(instances[0].variables_reference);
        debug!("inlineValue instance match single candidate");
    }

    let reference = chosen?;
    fetch_variables(client, reference)
}

fn fetch_variables(
    client: &mut ControlClient,
    reference: u32,
) -> Option<FxHashMap<SmolStr, String>> {
    let vars_value = client.request(
        "debug.variables",
        Some(serde_json::json!({ "variables_reference": reference })),
    )?;
    let variables = vars_value
        .get("variables")
        .and_then(|value| serde_json::from_value::<Vec<DebugVariable>>(value.clone()).ok())
        .unwrap_or_default();
    let mut out = FxHashMap::default();
    for variable in variables {
        if variable.name.trim().is_empty() {
            continue;
        }
        out.entry(SmolStr::new(variable.name))
            .or_insert(variable.value);
    }
    Some(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugVariableRef {
    name: String,
    variables_reference: u32,
}
