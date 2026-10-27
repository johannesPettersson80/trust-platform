//! Remote control client helpers for attach sessions.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::protocol::{
    AttachArguments, Breakpoint, BreakpointLocation, BreakpointLocationsResponseBody,
    EvaluateResponseBody, IoStateEntry, IoStateEventBody, Scope, Source, StackFrame, Variable,
    VariablesArguments,
};
use trust_runtime::control::endpoint::{
    control_token_from_env, resolve_control_target, ControlTarget, ControlTargetError,
};
use trust_runtime::control::ControlEndpoint;

pub(in crate::adapter) mod writes;

type RemoteResult<T> = std::result::Result<T, String>;

#[derive(Debug, Clone)]
pub enum RemoteEndpoint {
    Tcp(SocketAddr),
    #[cfg(unix)]
    Unix(PathBuf),
}

impl RemoteEndpoint {
    pub fn parse(text: &str) -> RemoteResult<Self> {
        if let Some(rest) = text.strip_prefix("tcp://") {
            let addr = rest.parse::<SocketAddr>().map_err(|err| err.to_string())?;
            return Ok(Self::Tcp(addr));
        }
        #[cfg(unix)]
        if let Some(rest) = text.strip_prefix("unix://") {
            return Ok(Self::Unix(PathBuf::from(rest)));
        }
        Err(format!("unsupported endpoint '{text}'"))
    }
}

#[derive(Debug, Deserialize)]
struct ControlResponse {
    #[allow(dead_code)]
    id: u64,
    ok: bool,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<String>,
    /// The stable code of a refusal that has one (specification 11).
    #[serde(default)]
    error_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RemoteStop {
    pub reason: String,
    pub thread_id: Option<u32>,
    pub file_id: Option<u32>,
    pub breakpoint_generation: Option<u64>,
}

/// A logpoint's message that `debug.stops` returned (specification 11 §6.13.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLog {
    pub text: String,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RemoteDebugState {
    pub paused: bool,
    pub last_stop: Option<RemoteStop>,
}

#[derive(Debug)]
pub struct RemoteSession {
    endpoint: RemoteEndpoint,
    token: Option<String>,
    client: ControlClient,
}

impl RemoteSession {
    pub fn connect(endpoint: RemoteEndpoint, token: Option<String>) -> RemoteResult<Self> {
        let client = ControlClient::connect(endpoint.clone(), token.clone())?;
        Ok(Self {
            endpoint,
            token,
            client,
        })
    }

    /// Connects to a resolved target. Nothing listening at the endpoint file's endpoint or at the
    /// platform default means that the runtime is not running (spec 11, section 6.8.0a).
    fn connect_target(target: &ControlTarget) -> RemoteResult<Self> {
        let endpoint = to_remote_endpoint(target.endpoint.clone());
        let client =
            ControlClient::open(endpoint.clone()).map_err(|error| target.connect_error(&error))?;
        Ok(Self {
            endpoint,
            token: target.auth_token.clone(),
            client,
        })
    }

    pub fn endpoint(&self) -> &RemoteEndpoint {
        &self.endpoint
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn debug_state(&mut self) -> RemoteResult<RemoteDebugState> {
        let payload = self.request("debug.state", None)?;
        let paused = payload
            .get("paused")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let last_stop = payload.get("last_stop").and_then(parse_stop);
        Ok(RemoteDebugState { paused, last_stop })
    }

    /// The stops and the logpoint messages since the last call (specification 11 §6.13.2,
    /// §6.13.3).
    pub fn debug_stops(&mut self) -> RemoteResult<(Vec<RemoteStop>, Vec<RemoteLog>)> {
        let payload = self.request("debug.stops", None)?;
        let stops = payload
            .get("stops")
            .and_then(|value| value.as_array())
            .map(|stops| stops.iter().filter_map(parse_stop).collect())
            .unwrap_or_default();
        let logs = payload
            .get("logs")
            .and_then(|value| value.as_array())
            .map(|logs| logs.iter().filter_map(parse_log).collect())
            .unwrap_or_default();
        Ok((stops, logs))
    }

    /// `breakpoints.set` of the zero-based `lines` of `source`, with each line's `options` when
    /// any line has some (specification 11 §6.13.2).
    pub fn set_breakpoints(
        &mut self,
        source: &str,
        lines: Vec<u32>,
        options: Option<Vec<Value>>,
    ) -> RemoteResult<(Vec<Breakpoint>, Option<u32>, Option<u64>)> {
        let mut params = json!({
            "source": resolved_source(source),
            "lines": lines,
        });
        if let Some(options) = options {
            params["options"] = Value::Array(options);
        }
        let payload = self.request("breakpoints.set", Some(params))?;
        let resolved = payload
            .get("resolved")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let breakpoints = resolved
            .into_iter()
            .filter_map(|entry| {
                let line = entry.get("line")?.as_u64()? as u32;
                let column = entry
                    .get("column")
                    .and_then(|value| value.as_u64())
                    .map(|v| v as u32);
                Some(Breakpoint {
                    id: None,
                    verified: true,
                    message: None,
                    source: Some(Source {
                        name: None,
                        path: Some(source.to_string()),
                        source_reference: None,
                    }),
                    line: Some(line),
                    column,
                    end_line: None,
                    end_column: None,
                })
            })
            .collect::<Vec<_>>();
        let file_id = payload
            .get("file_id")
            .and_then(|value| value.as_u64())
            .map(|v| v as u32);
        let generation = payload.get("generation").and_then(|value| value.as_u64());
        Ok((breakpoints, file_id, generation))
    }

    pub fn clear_breakpoints(&mut self, source: &str) -> RemoteResult<()> {
        let params = json!({
            "source": resolved_source(source),
            "lines": [],
        });
        let _payload = self.request("breakpoints.clear", Some(params))?;
        Ok(())
    }

    pub fn breakpoint_locations(
        &mut self,
        source: &str,
        line: u32,
        column: Option<u32>,
        end_line: Option<u32>,
        end_column: Option<u32>,
    ) -> RemoteResult<BreakpointLocationsResponseBody> {
        let params = json!({
            "source": resolved_source(source),
            "line": line,
            "column": column,
            "end_line": end_line,
            "end_column": end_column,
        });
        let payload = self.request("debug.breakpoint_locations", Some(params))?;
        let breakpoints = payload
            .get("breakpoints")
            .and_then(|value| value.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| {
                        let line = entry.get("line")?.as_u64()? as u32;
                        let column = entry
                            .get("column")
                            .and_then(|value| value.as_u64())
                            .map(|v| v as u32);
                        Some(BreakpointLocation {
                            line,
                            column,
                            end_line: None,
                            end_column: None,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(BreakpointLocationsResponseBody { breakpoints })
    }

    pub fn stack_trace(&mut self) -> RemoteResult<Vec<StackFrame>> {
        let payload = self.request("debug.stack", None)?;
        let frames = payload
            .get("stack_frames")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let stack_frames =
            serde_json::from_value::<Vec<StackFrame>>(frames).map_err(|err| err.to_string())?;
        Ok(stack_frames)
    }

    pub fn scopes(&mut self, frame_id: u32) -> RemoteResult<Vec<Scope>> {
        let payload = self.request("debug.scopes", Some(json!({ "frame_id": frame_id })))?;
        let scopes = payload.get("scopes").cloned().unwrap_or_else(|| json!([]));
        serde_json::from_value::<Vec<Scope>>(scopes).map_err(|err| err.to_string())
    }

    /// The children of `args.variables_reference`. The client's `filter`, `start` and `count` go
    /// to the runtime as they are, and an argument the client left out is left out: the runtime
    /// cuts the page, and its entries carry their `indexedVariables` and `namedVariables`
    /// (specification 13, "Variables / Evaluate").
    pub fn variables(&mut self, args: &VariablesArguments) -> RemoteResult<Vec<Variable>> {
        let mut params = json!({ "variables_reference": args.variables_reference });
        if let Some(filter) = args.filter {
            params["filter"] = json!(filter);
        }
        if let Some(start) = args.start {
            params["start"] = json!(start);
        }
        if let Some(count) = args.count {
            params["count"] = json!(count);
        }
        let payload = self.request("debug.variables", Some(params))?;
        let vars = payload
            .get("variables")
            .cloned()
            .unwrap_or_else(|| json!([]));
        serde_json::from_value::<Vec<Variable>>(vars).map_err(|err| err.to_string())
    }

    /// `debug.evaluate` of the compiled expression `expr`, version 1 (specification 11 §6.13.2).
    pub fn evaluate_compiled(
        &mut self,
        expr: &[u8],
        frame_id: Option<u32>,
    ) -> RemoteResult<EvaluateResponseBody> {
        let mut params = compiled_expression(expr);
        params["frame_id"] = json!(frame_id);
        let payload = self.request("debug.evaluate", Some(params))?;
        evaluate_body(payload)
    }

    /// `bytecode.reload` with the program `image`: the runtime changes its running program online
    /// (specification 44 §12).
    pub fn reload_program(&mut self, image: &[u8]) -> RemoteResult<()> {
        use base64::Engine as _;
        let params = json!({ "bytes": base64::engine::general_purpose::STANDARD.encode(image) });
        let _ = self.request("bytecode.reload", Some(params))?;
        Ok(())
    }

    pub fn pause(&mut self) -> RemoteResult<()> {
        let _ = self.request("pause", None)?;
        Ok(())
    }

    pub fn resume(&mut self) -> RemoteResult<()> {
        let _ = self.request("resume", None)?;
        Ok(())
    }

    pub fn step_in(&mut self) -> RemoteResult<()> {
        let _ = self.request("step_in", None)?;
        Ok(())
    }

    pub fn step_over(&mut self) -> RemoteResult<()> {
        let _ = self.request("step_over", None)?;
        Ok(())
    }

    pub fn step_out(&mut self) -> RemoteResult<()> {
        let _ = self.request("step_out", None)?;
        Ok(())
    }

    pub fn io_state(&mut self) -> RemoteResult<IoStateEventBody> {
        let payload = self.request("io.read", None)?;
        let snapshot = payload.get("snapshot").cloned().unwrap_or(Value::Null);
        let scan = snapshot.get("scan").and_then(|scan| scan.as_u64());
        let parse_entries = |value: &Value| -> Vec<IoStateEntry> {
            value
                .as_array()
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| {
                            let address = entry.get("address")?.as_str()?.to_string();
                            let name = entry
                                .get("name")
                                .and_then(|name| name.as_str())
                                .map(|name| name.to_string());
                            let source = entry
                                .get("source")
                                .and_then(|source| source.as_str())
                                .map(|source| source.to_string());
                            let value_type = entry
                                .get("valueType")
                                .or_else(|| entry.get("value_type"))
                                .or_else(|| entry.get("type"))
                                .and_then(|value_type| value_type.as_str())
                                .map(|value_type| value_type.to_string());
                            let value = entry.get("value")?;
                            let value_str = if let Some(text) = value.as_str() {
                                text.to_string()
                            } else {
                                value.to_string()
                            };
                            let forced = entry
                                .get("forced")
                                .and_then(|flag| flag.as_bool())
                                .unwrap_or(false);
                            let writable = entry.get("writable").and_then(|flag| flag.as_bool());
                            Some(IoStateEntry {
                                name,
                                address,
                                source,
                                value_type,
                                value: value_str,
                                forced,
                                writable,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let inputs = snapshot
            .get("inputs")
            .map(parse_entries)
            .unwrap_or_default();
        let outputs = snapshot
            .get("outputs")
            .map(parse_entries)
            .unwrap_or_default();
        let memory = snapshot
            .get("memory")
            .map(parse_entries)
            .unwrap_or_default();
        Ok(IoStateEventBody {
            scan,
            inputs,
            outputs,
            memory,
        })
    }

    /// `io.write` of `value` at `address`, sent again while the runtime is busy
    /// (`write_request`).
    pub fn io_write(&mut self, address: &str, value: &str) -> RemoteResult<()> {
        let params = json!({
            "address": address,
            "value": value,
        });
        let _ = self.write_request("io.write", &params)?;
        Ok(())
    }

    /// `io.force` of `value` at `address`, sent again while the runtime is busy
    /// (`write_request`).
    pub fn io_force(&mut self, address: &str, value: &str) -> RemoteResult<()> {
        let params = json!({
            "address": address,
            "value": value,
        });
        let _ = self.write_request("io.force", &params)?;
        Ok(())
    }

    /// `io.unforce` of `address`, sent again while the runtime is busy
    /// (`write_request`).
    pub fn io_unforce(&mut self, address: &str) -> RemoteResult<()> {
        let params = json!({
            "address": address,
        });
        let _ = self.write_request("io.unforce", &params)?;
        Ok(())
    }

    fn request(&mut self, kind: &str, params: Option<Value>) -> RemoteResult<Value> {
        let response = self.exchange(kind, params)?;
        answer(response)
    }

    /// Sends the request `kind` with `params` and reads the runtime's response.
    fn exchange(&mut self, kind: &str, params: Option<Value>) -> RemoteResult<ControlResponse> {
        let mut payload = json!({
            "id": self.client.next_id(),
            "type": kind,
            "params": params,
        });
        if let Some(token) = self.token.as_deref() {
            payload["auth"] = json!(token);
        }
        self.client.request(payload)
    }
}

/// The result of the runtime's `response`, or the text of its refusal.
fn answer(response: ControlResponse) -> RemoteResult<Value> {
    if !response.ok {
        let message = response
            .error
            .unwrap_or_else(|| "request failed".to_string());
        return Err(message);
    }
    Ok(response.result.unwrap_or_else(|| json!({})))
}

/// A compiled expression, version 1, as a control request carries it: `expr_version` and the base64
/// of its bytes (specification 11 §6.13.2).
pub fn compiled_expression(bytes: &[u8]) -> Value {
    use base64::Engine as _;
    json!({
        "expr_version": 1,
        "expr": base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

/// A `debug.evaluate` answer as the control protocol carries it, in snake case (specification 11
/// §6.13.2).
#[derive(Debug, Deserialize)]
struct EvaluateReply {
    result: String,
    #[serde(default, rename = "type")]
    type_name: Option<String>,
    #[serde(default)]
    variables_reference: u32,
}

/// The DAP body of the `debug.evaluate` answer `payload`.
fn evaluate_body(payload: Value) -> RemoteResult<EvaluateResponseBody> {
    let reply = serde_json::from_value::<EvaluateReply>(payload).map_err(|err| err.to_string())?;
    Ok(EvaluateResponseBody {
        result: reply.result,
        r#type: reply.type_name,
        variables_reference: reply.variables_reference,
        named_variables: None,
        indexed_variables: None,
    })
}

/// Attaches to a running runtime (spec 13 attach arguments; spec 11, sections 6.8.0a and 6.8.1).
/// The explicit `endpoint` argument is used as it is. Without one, the project folder named by
/// `runtimeRoot` resolves the endpoint: the endpoint file, then the configured endpoint, then the
/// platform default. The token is the explicit `authToken`, else `TRUST_CTL_TOKEN`, else the
/// project's token.
pub fn attach_from_args(
    args: &AttachArguments,
) -> RemoteResult<(RemoteSession, Option<RemoteDebugState>)> {
    attach_with_env_token(args, control_token_from_env())
}

/// [`attach_from_args`] with the `TRUST_CTL_TOKEN` value given, so that tests do not depend on the
/// environment.
pub(super) fn attach_with_env_token(
    args: &AttachArguments,
    env_token: Option<String>,
) -> RemoteResult<(RemoteSession, Option<RemoteDebugState>)> {
    let target = resolve_attach_target(args, env_token)?;
    let mut session = RemoteSession::connect_target(&target)?;
    let state = session.debug_state().ok();
    let _ = session.debug_stops();
    Ok((session, state))
}

/// The endpoint and token that an attach request names (spec 13).
pub(super) fn resolve_attach_target(
    args: &AttachArguments,
    env_token: Option<String>,
) -> RemoteResult<ControlTarget> {
    let explicit = args
        .additional
        .get("endpoint")
        .and_then(|value| value.as_str())
        .map(RemoteEndpoint::parse)
        .transpose()?
        .map(to_control_endpoint);
    let project = args
        .additional
        .get("runtimeRoot")
        .and_then(|value| value.as_str())
        .map(PathBuf::from);
    let token = args
        .additional
        .get("authToken")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or(env_token);
    resolve_control_target(project.as_deref(), explicit, token.as_deref()).map_err(|error| {
        match error {
            ControlTargetError::NoTarget => "attach requires endpoint or runtimeRoot".to_string(),
            other => other.to_string(),
        }
    })
}

fn to_control_endpoint(endpoint: RemoteEndpoint) -> ControlEndpoint {
    match endpoint {
        RemoteEndpoint::Tcp(addr) => ControlEndpoint::Tcp(addr),
        #[cfg(unix)]
        RemoteEndpoint::Unix(path) => ControlEndpoint::Unix(path),
    }
}

fn to_remote_endpoint(endpoint: ControlEndpoint) -> RemoteEndpoint {
    match endpoint {
        ControlEndpoint::Tcp(addr) => RemoteEndpoint::Tcp(addr),
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => RemoteEndpoint::Unix(path),
    }
}

#[derive(Debug)]
struct ControlClient {
    seq: u64,
    reader: BufReader<ControlStream>,
}

impl ControlClient {
    fn connect(endpoint: RemoteEndpoint, _token: Option<String>) -> RemoteResult<Self> {
        Self::open(endpoint).map_err(|err| err.to_string())
    }

    /// Opens the connection, keeping the operating-system error so that a caller can tell a
    /// runtime that is not running from another failure.
    fn open(endpoint: RemoteEndpoint) -> io::Result<Self> {
        let stream = match endpoint {
            RemoteEndpoint::Tcp(addr) => ControlStream::Tcp(TcpStream::connect(addr)?),
            #[cfg(unix)]
            RemoteEndpoint::Unix(path) => {
                ControlStream::Unix(std::os::unix::net::UnixStream::connect(path)?)
            }
        };
        Ok(Self {
            seq: 1,
            reader: BufReader::new(stream),
        })
    }

    fn next_id(&mut self) -> u64 {
        let id = self.seq;
        self.seq = self.seq.saturating_add(1);
        id
    }

    fn request(&mut self, payload: Value) -> RemoteResult<ControlResponse> {
        let line = serde_json::to_string(&payload).map_err(|err| err.to_string())?;
        {
            let stream = self.reader.get_mut();
            stream
                .write_all(line.as_bytes())
                .map_err(|err| err.to_string())?;
            stream.write_all(b"\n").map_err(|err| err.to_string())?;
            stream.flush().map_err(|err| err.to_string())?;
        }
        let mut response = String::new();
        self.reader
            .read_line(&mut response)
            .map_err(|err| err.to_string())?;
        let response: ControlResponse =
            serde_json::from_str(&response).map_err(|err| err.to_string())?;
        Ok(response)
    }
}

#[derive(Debug)]
enum ControlStream {
    Tcp(TcpStream),
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

fn parse_log(value: &Value) -> Option<RemoteLog> {
    let number = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|number| u32::try_from(number).ok())
    };
    Some(RemoteLog {
        text: value.get("text")?.as_str()?.to_string(),
        path: value
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string),
        line: number("line"),
        column: number("column"),
    })
}

fn parse_stop(value: &Value) -> Option<RemoteStop> {
    let reason = value.get("reason")?.as_str()?.to_string();
    let thread_id = value
        .get("thread_id")
        .and_then(|value| value.as_u64())
        .map(|value| value as u32);
    let file_id = value
        .get("file_id")
        .and_then(|value| value.as_u64())
        .map(|value| value as u32);
    let breakpoint_generation = value
        .get("breakpoint_generation")
        .and_then(|value| value.as_u64());
    Some(RemoteStop {
        reason,
        thread_id,
        file_id,
        breakpoint_generation,
    })
}

/// `source` as the runtime's build named it: through the symbolic links of this computer, as the
/// build reads the project's files (`trust_runtime::runtime_sources`), so that a project opened
/// through a link finds its sources (specification 13, "Breakpoint evaluation, source resolution,
/// and hook contract"). A path that does not resolve is sent as given.
/// The client's path of a file the runtime names by `path` (specification 13, "Stack Trace and
/// Navigation"): a label, the path relative to the project root that a compiled program's file table
/// holds, under `root`, the folder the program was built from; a path that is absolute already, or one
/// with no root to resolve it against, stays as it is.
pub(super) fn client_source_path(root: Option<&std::path::Path>, path: String) -> String {
    let Some(root) = root.filter(|_| std::path::Path::new(&path).is_relative()) else {
        return path;
    };
    let mut file = root.to_path_buf();
    file.extend(path.split('/'));
    file.to_string_lossy().into_owned()
}

fn resolved_source(source: &str) -> String {
    trust_runtime::runtime_sources::canonicalize_lossy(std::path::Path::new(source))
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod client_source_path_tests {
    use std::path::Path;

    use super::client_source_path;

    /// Specification 13, "Stack Trace and Navigation": a label, relative to the project root with `/`
    /// separators, becomes the file under the root; an absolute path, and a label with no root to
    /// resolve it against, stay as they are. Catches frames that keep the label, which the editor
    /// cannot open, and paths that the adapter would move although the runtime named the file.
    #[test]
    fn a_label_resolves_under_the_project_root_and_a_path_stays() {
        let root = std::env::temp_dir().join("project");
        assert_eq!(
            client_source_path(Some(&root), String::from("src/lib/Main.st")),
            root.join("src")
                .join("lib")
                .join("Main.st")
                .to_string_lossy()
        );
        let absolute = root
            .join("src")
            .join("Main.st")
            .to_string_lossy()
            .into_owned();
        assert_eq!(client_source_path(Some(&root), absolute.clone()), absolute);
        assert_eq!(
            client_source_path(None::<&Path>, String::from("src/Main.st")),
            "src/Main.st"
        );
    }
}
