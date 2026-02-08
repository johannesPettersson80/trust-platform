//! IO state + write handling.
//! - handle_io_state/handle_io_write: DAP custom requests
//! - build_io_state/update_io_cache_for_write: cached IO events
//! - resolve_io_address*: map labels/addresses to IoAddress
//! - io_type_id/parse_io_value: typing helpers

use std::borrow::Cow;

use serde_json::Value;

use trust_hir::TypeId;
use trust_runtime::io::{IoAddress, IoSize, IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use trust_runtime::memory::IoArea;
use trust_runtime::value::Value as RuntimeValue;

use crate::protocol::{IoStateEntry, IoStateEventBody, IoWriteArguments, Request};

use super::variables::format_value;
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
        if let Ok(mut cache) = self.last_io_state.lock() {
            if let Some(state) = cache.as_mut() {
                self.apply_forced_flags(state);
            }
        }
    }

    fn apply_forced_flags(&self, state: &mut IoStateEventBody) {
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
        if let Some(remote) = self.remote_session.as_mut() {
            match remote.io_state() {
                Ok(body) => {
                    let event = self.event("stIoState", Some(body));
                    return DispatchOutcome {
                        responses: vec![self.ok_response::<Value>(&request, None)],
                        events: vec![event],
                        ..DispatchOutcome::default()
                    };
                }
                Err(err) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &err.to_string())],
                        ..DispatchOutcome::default()
                    };
                }
            }
        }
        let body = self.build_io_state();
        let event = self.event("stIoState", Some(body));
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            events: vec![event],
            should_exit: false,
            stop_gate: None,
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

        if let Some(remote) = self.remote_session.as_mut() {
            let response = remote.io_write(&args.address, &args.value);
            return match response {
                Ok(()) => DispatchOutcome {
                    responses: vec![self.ok_response::<Value>(&request, None)],
                    ..DispatchOutcome::default()
                },
                Err(err) => DispatchOutcome {
                    responses: vec![self.error_response(&request, &err.to_string())],
                    ..DispatchOutcome::default()
                },
            };
        }

        let address = match IoAddress::parse(&args.address) {
            Ok(address) => address,
            Err(err) => {
                return DispatchOutcome {
                    responses: vec![
                        self.error_response(&request, &format!("invalid I/O address: {err}"))
                    ],
                    ..DispatchOutcome::default()
                }
            }
        };

        if address.area != IoArea::Input {
            return DispatchOutcome {
                responses: vec![
                    self.error_response(&request, "only input addresses can be written")
                ],
                ..DispatchOutcome::default()
            };
        }

        let value = match parse_io_value(&address, &args.value) {
            Ok(value) => value,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message)],
                    ..DispatchOutcome::default()
                };
            }
        };

        self.session
            .debug_control()
            .enqueue_io_write(address.clone(), value.clone());
        if let Ok(mut runtime) = self.session.runtime_handle().try_lock() {
            if let Err(err) = runtime.io_mut().write(&address, value.clone()) {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &format!("{err}"))],
                    ..DispatchOutcome::default()
                };
            }
        }
        let body = self.update_io_cache_for_write(address, value);
        let event = self.event("stIoState", Some(body));
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            events: vec![event],
            should_exit: false,
            stop_gate: None,
        }
    }

    pub(super) fn build_io_state(&self) -> IoStateEventBody {
        if let Ok(cache) = self.last_io_state.lock() {
            if let Some(state) = cache.clone() {
                return state;
            }
        }
        if let Ok(runtime) = self.session.runtime_handle().lock() {
            let mut state = io_state_from_snapshot(runtime.io().snapshot());
            self.apply_forced_flags(&mut state);
            return state;
        }
        IoStateEventBody {
            inputs: Vec::new(),
            outputs: Vec::new(),
            memory: Vec::new(),
        }
    }

    pub(super) fn capture_io_state_from_runtime(&self) -> Option<IoStateEventBody> {
        let runtime_handle = self.session.runtime_handle();
        let runtime = runtime_handle.lock().ok()?;
        let mut body = io_state_from_snapshot(runtime.io().snapshot());
        self.apply_forced_flags(&mut body);
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(body.clone());
        }
        Some(body)
    }

    pub(super) fn update_io_cache_from_runtime(
        &self,
        runtime: &trust_runtime::Runtime,
    ) -> IoStateEventBody {
        let mut body = io_state_from_snapshot(runtime.io().snapshot());
        self.apply_forced_flags(&mut body);
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(body.clone());
        }
        body
    }

    pub(super) fn emit_io_state_event_from_runtime(&self, events: &mut Vec<Value>) {
        if let Some(body) = self.capture_io_state_from_runtime() {
            events.push(self.event("stIoState", Some(body)));
        }
    }

    pub(super) fn update_io_cache_for_write(
        &self,
        address: IoAddress,
        value: RuntimeValue,
    ) -> IoStateEventBody {
        let mut state = if let Ok(cache) = self.last_io_state.lock() {
            cache.clone().unwrap_or(IoStateEventBody {
                inputs: Vec::new(),
                outputs: Vec::new(),
                memory: Vec::new(),
            })
        } else {
            IoStateEventBody {
                inputs: Vec::new(),
                outputs: Vec::new(),
                memory: Vec::new(),
            }
        };
        let address_str = format_io_address(&address);
        let entries = match address.area {
            IoArea::Input => &mut state.inputs,
            IoArea::Output => &mut state.outputs,
            IoArea::Memory => &mut state.memory,
        };
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.address == address_str)
        {
            entry.value = format_value(&value);
        } else {
            entries.push(IoStateEntry {
                name: None,
                address: address_str,
                value: format_value(&value),
                forced: false,
            });
        }
        self.apply_forced_flags(&mut state);
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(state.clone());
        }
        state
    }
}
pub(super) fn io_type_id(address: &IoAddress) -> TypeId {
    match address.size {
        IoSize::Bit => TypeId::BOOL,
        IoSize::Byte => TypeId::BYTE,
        IoSize::Word => TypeId::WORD,
        IoSize::DWord => TypeId::DWORD,
        IoSize::LWord => TypeId::LWORD,
    }
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
    if let Ok(address) = IoAddress::parse(name) {
        return Ok(address);
    }
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
        0 => Err("unknown I/O entry".to_string()),
        _ => Err("ambiguous I/O entry name".to_string()),
    }
}

pub(super) fn resolve_io_address(
    runtime: &trust_runtime::Runtime,
    name: &str,
) -> Result<IoAddress, String> {
    let snapshot = runtime.io().snapshot();
    let entries = snapshot
        .inputs
        .iter()
        .chain(snapshot.outputs.iter())
        .chain(snapshot.memory.iter())
        .map(|entry| IoEntryLookup {
            label: entry.name.as_deref(),
            address: Cow::Owned(format_io_address(&entry.address)),
            parsed: Some(entry.address.clone()),
        });
    find_io_address_in_entries(name, entries)
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

fn parse_io_value(address: &IoAddress, raw: &str) -> Result<RuntimeValue, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("I/O value cannot be empty".to_string());
    }

    match address.size {
        IoSize::Bit => {
            if let Some(flag) = parse_bool(trimmed) {
                return Ok(RuntimeValue::Bool(flag));
            }
            let numeric = parse_numeric(trimmed)?;
            if numeric <= 1 {
                return Ok(RuntimeValue::Bool(numeric == 1));
            }
            Err("bit inputs accept TRUE/FALSE or 0/1".to_string())
        }
        IoSize::Byte => {
            let numeric = parse_numeric(trimmed)?;
            let value = u8::try_from(numeric)
                .map_err(|_| "BYTE value out of range (0..255)".to_string())?;
            Ok(RuntimeValue::Byte(value))
        }
        IoSize::Word => {
            let numeric = parse_numeric(trimmed)?;
            let value = u16::try_from(numeric)
                .map_err(|_| "WORD value out of range (0..65535)".to_string())?;
            Ok(RuntimeValue::Word(value))
        }
        IoSize::DWord => {
            let numeric = parse_numeric(trimmed)?;
            let value = u32::try_from(numeric)
                .map_err(|_| "DWORD value out of range (0..4294967295)".to_string())?;
            Ok(RuntimeValue::DWord(value))
        }
        IoSize::LWord => {
            let numeric = parse_numeric(trimmed)?;
            Ok(RuntimeValue::LWord(numeric))
        }
    }
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "TRUE" => Some(true),
        "FALSE" => Some(false),
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

fn parse_numeric(raw: &str) -> Result<u64, String> {
    let cleaned = raw.trim().replace('_', "");
    if let Some(rest) = cleaned.strip_prefix("2#") {
        return u64::from_str_radix(rest, 2).map_err(|_| "invalid base-2 literal".to_string());
    }
    if let Some(rest) = cleaned.strip_prefix("8#") {
        return u64::from_str_radix(rest, 8).map_err(|_| "invalid base-8 literal".to_string());
    }
    if let Some(rest) = cleaned.strip_prefix("16#") {
        return u64::from_str_radix(rest, 16).map_err(|_| "invalid base-16 literal".to_string());
    }
    if let Some(rest) = cleaned.strip_prefix("0x") {
        return u64::from_str_radix(rest, 16).map_err(|_| "invalid hex literal".to_string());
    }
    cleaned
        .parse::<u64>()
        .map_err(|_| "invalid numeric literal".to_string())
}

pub(super) fn io_state_from_snapshot(snapshot: IoSnapshot) -> IoStateEventBody {
    fn convert(entries: Vec<IoSnapshotEntry>) -> Vec<IoStateEntry> {
        entries
            .into_iter()
            .map(|entry| {
                let name = entry.name.map(|name| name.to_string());
                let address = format_io_address(&entry.address);
                let value = match entry.value {
                    IoSnapshotValue::Value(value) => format_value(&value),
                    IoSnapshotValue::Error(err) => format!("error: {err}"),
                    IoSnapshotValue::Unresolved => "<unresolved>".to_string(),
                };
                IoStateEntry {
                    name,
                    address,
                    value,
                    forced: false,
                }
            })
            .collect()
    }

    IoStateEventBody {
        inputs: convert(snapshot.inputs),
        outputs: convert(snapshot.outputs),
        memory: convert(snapshot.memory),
    }
}

fn format_io_address(address: &IoAddress) -> String {
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
