//! I/O requests of the debug adapter, answered by the connected runtime.
//! - handle_io_state/write/force/release: stIo* requests
//! - forced flags: the addresses this session forced, marked in the I/O state
//! - address helpers: resolve an entry's name or address, format an address

use std::borrow::Cow;

use serde_json::Value;

use trust_runtime::io::{IoAddress, IoSize};
use trust_runtime::memory::IoArea;

use crate::protocol::{IoReleaseArguments, IoStateEventBody, IoWriteArguments, Request};

use super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(super) fn set_io_forced(&self, address: &IoAddress, forced: bool) {
        let key = format_io_address(address);
        if let Ok(mut entries) = self.forced_io_addresses.lock() {
            if forced {
                entries.insert(key);
            } else {
                entries.remove(&key);
            }
        }
        if let Ok(mut cache) = self.last_io_state.lock()
            && let Some(state) = cache.as_mut()
        {
            self.apply_forced_flags(state);
        }
    }

    pub(super) fn apply_forced_flags(&self, state: &mut IoStateEventBody) {
        let forced = if let Ok(entries) = self.forced_io_addresses.lock() {
            entries.clone()
        } else {
            Default::default()
        };
        for entry in state
            .inputs
            .iter_mut()
            .chain(state.outputs.iter_mut())
            .chain(state.memory.iter_mut())
        {
            entry.forced = forced.contains(entry.address.as_str());
        }
    }

    pub(super) fn handle_io_state(&mut self, request: Request<Value>) -> DispatchOutcome {
        let Some(remote) = self.remote_session.as_mut() else {
            return self.not_connected(&request);
        };
        let body = match remote.io_state() {
            Ok(body) => body,
            Err(err) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &err)],
                    ..DispatchOutcome::default()
                };
            }
        };
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(body.clone());
        }
        if suppress_io_state_event_until_next_scan(&request, body.scan) {
            return DispatchOutcome {
                responses: vec![self.ok_response::<Value>(&request, None)],
                ..DispatchOutcome::default()
            };
        }
        let event = self.event("stIoState", Some(body));
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            events: vec![event],
            ..DispatchOutcome::default()
        }
    }

    pub(super) fn handle_io_write(&mut self, request: Request<Value>) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<IoWriteArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid stIoWrite args")],
                ..DispatchOutcome::default()
            };
        };
        let Some(remote) = self.remote_session.as_mut() else {
            return self.not_connected(&request);
        };
        let response = remote.io_write(&args.address, &args.value);
        self.io_answer(&request, response)
    }

    pub(super) fn handle_io_force(&mut self, request: Request<Value>) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<IoWriteArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid stIoForce args")],
                ..DispatchOutcome::default()
            };
        };
        if self.remote_session.is_none() {
            return self.not_connected(&request);
        }
        let address = match self.resolve_remote_io_address(&args.address) {
            Ok(address) => address,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message)],
                    ..DispatchOutcome::default()
                };
            }
        };
        let address_text = format_io_address(&address);
        let response = self
            .remote_session
            .as_mut()
            .expect("remote session checked")
            .io_force(&address_text, &args.value);
        self.io_answer(&request, response)
    }

    pub(super) fn handle_io_release(&mut self, request: Request<Value>) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<IoReleaseArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid stIoRelease args")],
                ..DispatchOutcome::default()
            };
        };
        if self.remote_session.is_none() {
            return self.not_connected(&request);
        }
        let address = match self.resolve_remote_io_address(&args.address) {
            Ok(address) => address,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message)],
                    ..DispatchOutcome::default()
                };
            }
        };
        let address_text = format_io_address(&address);
        let response = self
            .remote_session
            .as_mut()
            .expect("remote session checked")
            .io_unforce(&address_text);
        self.io_answer(&request, response)
    }

    /// The answer to an I/O request from the runtime's `response`.
    fn io_answer(&self, request: &Request<Value>, response: Result<(), String>) -> DispatchOutcome {
        match response {
            Ok(()) => DispatchOutcome {
                responses: vec![self.ok_response::<Value>(request, None)],
                ..DispatchOutcome::default()
            },
            Err(err) => DispatchOutcome {
                responses: vec![self.error_response(request, &err)],
                ..DispatchOutcome::default()
            },
        }
    }

    pub(super) fn resolve_remote_io_address(&mut self, name: &str) -> Result<IoAddress, String> {
        if let Ok(address) = IoAddress::parse(name) {
            return Ok(address);
        }
        if let Ok(cache) = self.last_io_state.lock()
            && let Some(state) = cache.as_ref()
            && let Ok(address) = resolve_io_address_from_state(state, name)
        {
            return Ok(address);
        }
        let Some(remote) = self.remote_session.as_mut() else {
            return Err("attach session is not connected".to_string());
        };
        let state = remote.io_state().map_err(|err| err.to_string())?;
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(state.clone());
        }
        resolve_io_address_from_state(&state, name)
    }
}

fn suppress_io_state_event_until_next_scan(request: &Request<Value>, scan: Option<u64>) -> bool {
    let Some(after_scan) = request
        .arguments
        .as_ref()
        .and_then(|arguments| arguments.get("afterScan"))
        .and_then(|value| value.as_u64())
    else {
        return false;
    };
    scan.is_some_and(|scan| scan <= after_scan)
}

struct IoEntryLookup<'a> {
    label: Option<&'a str>,
    address: Cow<'a, str>,
    parsed: Option<IoAddress>,
}

fn find_io_address_in_entries<'a, I>(name: &str, entries: I) -> Result<IoAddress, String>
where
    I: IntoIterator<Item = IoEntryLookup<'a>>,
{
    let mut matches = Vec::new();
    for entry in entries {
        if entry.label == Some(name) || entry.address == name {
            if let Some(address) = entry.parsed {
                matches.push(address);
            } else if let Ok(address) = IoAddress::parse(entry.address.as_ref()) {
                matches.push(address);
            }
        }
    }
    match matches.len() {
        1 => Ok(matches[0].clone()),
        0 => IoAddress::parse(name).map_err(|_| "unknown I/O entry".to_string()),
        _ => Err("ambiguous I/O entry name".to_string()),
    }
}

pub(super) fn resolve_io_address_from_state(
    state: &IoStateEventBody,
    name: &str,
) -> Result<IoAddress, String> {
    let entries = state
        .inputs
        .iter()
        .chain(state.outputs.iter())
        .chain(state.memory.iter())
        .map(|entry| IoEntryLookup {
            label: entry.name.as_deref(),
            address: Cow::Borrowed(entry.address.as_str()),
            parsed: None,
        });
    find_io_address_in_entries(name, entries)
}

pub(super) fn format_io_address(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => "I",
        IoArea::Output => "Q",
        IoArea::Memory => "M",
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
        IoSize::Bytes(_) => "B",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    let mut path = address
        .path
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(".");
    if matches!(address.size, IoSize::Bit) {
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(&address.bit.to_string());
    }
    format!("%{area}{size}{path}")
}
