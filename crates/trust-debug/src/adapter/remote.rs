//! Remote control client helpers for attach sessions.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
#[cfg(unix)]
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::protocol::{
    AttachArguments, Breakpoint, BreakpointLocation, BreakpointLocationsResponseBody,
    EvaluateResponseBody, IoStateEntry, IoStateEventBody, Scope, Source, StackFrame, Variable,
};

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
}

#[derive(Debug, Clone)]
pub struct RemoteStop {
    pub reason: String,
    pub thread_id: Option<u32>,
    pub file_id: Option<u32>,
    pub breakpoint_generation: Option<u64>,
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

    pub fn debug_stops(&mut self) -> RemoteResult<Vec<RemoteStop>> {
        let payload = self.request("debug.stops", None)?;
        let stops = payload
            .get("stops")
            .and_then(|value| value.as_array())
            .map(|stops| stops.iter().filter_map(parse_stop).collect())
            .unwrap_or_default();
        Ok(stops)
    }

    pub fn set_breakpoints(
        &mut self,
        source: &str,
        lines: Vec<u32>,
    ) -> RemoteResult<(Vec<Breakpoint>, Option<u32>, Option<u64>)> {
        let params = json!({
            "source": source,
            "lines": lines,
        });
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
            "source": source,
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
            "source": source,
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

    pub fn variables(&mut self, variables_reference: u32) -> RemoteResult<Vec<Variable>> {
        let payload = self.request(
            "debug.variables",
            Some(json!({ "variables_reference": variables_reference })),
        )?;
        let vars = payload
            .get("variables")
            .cloned()
            .unwrap_or_else(|| json!([]));
        serde_json::from_value::<Vec<Variable>>(vars).map_err(|err| err.to_string())
    }

    pub fn evaluate(
        &mut self,
        expression: &str,
        frame_id: Option<u32>,
    ) -> RemoteResult<EvaluateResponseBody> {
        let params = json!({
            "expression": expression,
            "frame_id": frame_id,
        });
        let payload = self.request("debug.evaluate", Some(params))?;
        serde_json::from_value::<EvaluateResponseBody>(payload).map_err(|err| err.to_string())
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
                            Some(IoStateEntry {
                                name,
                                address,
                                value: value_str,
                                forced,
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
            inputs,
            outputs,
            memory,
        })
    }

    pub fn io_write(&mut self, address: &str, value: &str) -> RemoteResult<()> {
        let params = json!({
            "address": address,
            "value": value,
        });
        let _ = self.request("io.write", Some(params))?;
        Ok(())
    }

    fn request(&mut self, kind: &str, params: Option<Value>) -> RemoteResult<Value> {
        let mut payload = json!({
            "id": self.client.next_id(),
            "type": kind,
            "params": params,
        });
        if let Some(token) = self.token.as_deref() {
            payload["auth"] = json!(token);
        }
        let response = self.client.request(payload)?;
        if !response.ok {
            let message = response
                .error
                .unwrap_or_else(|| "request failed".to_string());
            return Err(message);
        }
        Ok(response.result.unwrap_or_else(|| json!({})))
    }
}

pub fn attach_from_args(
    args: &AttachArguments,
) -> RemoteResult<(RemoteSession, Option<RemoteDebugState>)> {
    let endpoint = args
        .additional
        .get("endpoint")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "attach requires endpoint".to_string())?;
    let token = args
        .additional
        .get("authToken")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let endpoint = RemoteEndpoint::parse(endpoint)?;
    let mut session = RemoteSession::connect(endpoint, token)?;
    let state = session.debug_state().ok();
    let _ = session.debug_stops();
    Ok((session, state))
}

#[derive(Debug)]
struct ControlClient {
    seq: u64,
    reader: BufReader<ControlStream>,
}

impl ControlClient {
    fn connect(endpoint: RemoteEndpoint, _token: Option<String>) -> RemoteResult<Self> {
        let stream = match endpoint {
            RemoteEndpoint::Tcp(addr) => {
                ControlStream::Tcp(TcpStream::connect(addr).map_err(|err| err.to_string())?)
            }
            #[cfg(unix)]
            RemoteEndpoint::Unix(path) => ControlStream::Unix(
                std::os::unix::net::UnixStream::connect(path).map_err(|err| err.to_string())?,
            ),
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
