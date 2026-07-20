//! IO state + write handling.
//! - handle_io_state/handle_io_write/handle_io_force/handle_io_release: DAP custom requests
//! - build_io_state/update_io_cache_for_write: cached IO events
//! - resolve_io_address*: map labels/addresses to IoAddress
//! - io_type_id/parse_io_value: typing helpers

use std::borrow::Cow;

use serde_json::Value;
use smol_str::SmolStr;

use trust_hir::TypeId;
use trust_runtime::io::{IoAddress, IoSize, IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use trust_runtime::memory::IoArea;
use trust_runtime::value::Value as RuntimeValue;

use crate::protocol::{
    IoReleaseArguments, IoStateEntry, IoStateEventBody, IoWriteArguments, Request,
};

use super::variables::{format_value, type_id_for_value, value_type_name};
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
        if let Some(remote) = self.remote_session.as_mut() {
            match remote.io_state() {
                Ok(body) => {
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
        let body = self
            .capture_io_state_from_runtime()
            .unwrap_or_else(|| self.build_io_state());
        if suppress_io_state_event_until_next_scan(&request, body.scan) {
            return DispatchOutcome {
                responses: vec![self.ok_response::<Value>(&request, None)],
                events: Vec::new(),
                should_exit: false,
                stop_gate: None,
            };
        }
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

        let runtime_handle = self.session.runtime_handle();
        let mut runtime = match runtime_handle.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "runtime busy")],
                    ..DispatchOutcome::default()
                };
            }
        };
        let address = match resolve_io_address(&runtime, &args.address) {
            Ok(address) => address,
            Err(err) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &err)],
                    ..DispatchOutcome::default()
                };
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
        let value_type = value_type_for_io_address(&runtime, &address);
        let value = match parse_io_value_for_type(&address, value_type, &args.value) {
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
        if let Err(err) = runtime.io_mut().write(&address, value.clone()) {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, &format!("{err}"))],
                ..DispatchOutcome::default()
            };
        }
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            events: Vec::new(),
            should_exit: false,
            stop_gate: None,
        }
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

        if self.remote_session.is_some() {
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

        let runtime_handle = self.session.runtime_handle();
        let mut runtime = match runtime_handle.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "runtime busy")],
                    ..DispatchOutcome::default()
                };
            }
        };

        let address = match resolve_io_address(&runtime, &args.address) {
            Ok(address) => address,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message)],
                    ..DispatchOutcome::default()
                }
            }
        };
        let value_type = value_type_for_io_address(&runtime, &address);
        let value = match parse_io_value_for_type(&address, value_type, &args.value) {
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
            .force_io(address.clone(), value.clone());
        self.set_io_forced(&address, true);
        if let Err(err) = runtime.io_mut().write(&address, value.clone()) {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, &err.to_string())],
                ..DispatchOutcome::default()
            };
        }
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            ..DispatchOutcome::default()
        }
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

        if self.remote_session.is_some() {
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

        let runtime_handle = self.session.runtime_handle();
        let runtime = match runtime_handle.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "runtime busy")],
                    ..DispatchOutcome::default()
                };
            }
        };

        let address = match resolve_io_address(&runtime, &args.address) {
            Ok(address) => address,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message)],
                    ..DispatchOutcome::default()
                }
            }
        };
        self.session.debug_control().release_io(&address);
        self.set_io_forced(&address, false);
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            ..DispatchOutcome::default()
        }
    }

    pub(super) fn build_io_state(&self) -> IoStateEventBody {
        if let Ok(cache) = self.last_io_state.lock() {
            if let Some(state) = cache.clone() {
                return state;
            }
        }
        if let Ok(runtime) = self.session.runtime_handle().try_lock() {
            let mut state = io_state_from_snapshot(self.runtime_io_snapshot(&runtime));
            self.apply_forced_flags(&mut state);
            append_ads_live_values(&mut state, runtime.ads_live_values());
            return state;
        }
        IoStateEventBody {
            scan: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            memory: Vec::new(),
        }
    }

    pub(super) fn capture_io_state_from_runtime(&self) -> Option<IoStateEventBody> {
        let runtime_handle = self.session.runtime_handle();
        let runtime = runtime_handle.try_lock().ok()?;
        let mut body = io_state_from_snapshot(self.runtime_io_snapshot(&runtime));
        self.apply_forced_flags(&mut body);
        append_ads_live_values(&mut body, runtime.ads_live_values());
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(body.clone());
        }
        Some(body)
    }

    pub(super) fn update_io_cache_from_runtime(
        &self,
        runtime: &trust_runtime::Runtime,
    ) -> IoStateEventBody {
        let mut body = io_state_from_snapshot(self.runtime_io_snapshot(runtime));
        self.apply_forced_flags(&mut body);
        append_ads_live_values(&mut body, runtime.ads_live_values());
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(body.clone());
        }
        body
    }

    fn runtime_io_snapshot(&self, runtime: &trust_runtime::Runtime) -> IoSnapshot {
        let mut snapshot = runtime.io().snapshot();
        snapshot.scan = Some(runtime.cycle_counter());
        snapshot
    }

    pub(super) fn emit_io_state_event_from_runtime(&self, events: &mut Vec<Value>) {
        if let Some(body) = self.capture_io_state_from_runtime() {
            events.push(self.event("stIoState", Some(body)));
        }
    }

    fn resolve_remote_io_address(&mut self, name: &str) -> Result<IoAddress, String> {
        if let Ok(address) = IoAddress::parse(name) {
            return Ok(address);
        }
        if let Ok(cache) = self.last_io_state.lock() {
            if let Some(state) = cache.as_ref() {
                if let Ok(address) = resolve_io_address_from_state(state, name) {
                    return Ok(address);
                }
            }
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

    pub(super) fn update_io_cache_for_write(
        &self,
        address: IoAddress,
        value: RuntimeValue,
    ) -> IoStateEventBody {
        let mut state = if let Ok(cache) = self.last_io_state.lock() {
            cache.clone().unwrap_or(IoStateEventBody {
                scan: None,
                inputs: Vec::new(),
                outputs: Vec::new(),
                memory: Vec::new(),
            })
        } else {
            IoStateEventBody {
                scan: None,
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
                source: None,
                value_type: type_id_for_value(&value)
                    .and_then(trust_runtime::io::io_value_type_name)
                    .map(str::to_string),
                value: format_value(&value),
                forced: false,
                writable: None,
            });
        }
        self.apply_forced_flags(&mut state);
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(state.clone());
        }
        state
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

pub(super) fn io_type_id(address: &IoAddress) -> TypeId {
    match address.size {
        IoSize::Bit => TypeId::BOOL,
        IoSize::Byte => TypeId::BYTE,
        IoSize::Word => TypeId::WORD,
        IoSize::DWord => TypeId::DWORD,
        IoSize::LWord => TypeId::LWORD,
        IoSize::Bytes(_) => TypeId::STRING,
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

fn value_type_for_io_address(
    runtime: &trust_runtime::Runtime,
    address: &IoAddress,
) -> Option<TypeId> {
    runtime
        .io()
        .bindings()
        .iter()
        .find(|binding| binding.address == *address)
        .and_then(|binding| binding.value_type)
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
        IoSize::Bytes(len) => {
            let text = trimmed.trim_matches('\'');
            if text.len() > len as usize {
                return Err(format!("STRING value exceeds {len} bytes"));
            }
            Ok(RuntimeValue::String(SmolStr::new(text)))
        }
    }
}

fn parse_io_value_for_type(
    address: &IoAddress,
    value_type: Option<TypeId>,
    raw: &str,
) -> Result<RuntimeValue, String> {
    match value_type {
        Some(TypeId::REAL) => {
            if !matches!(address.size, IoSize::DWord) {
                return Err("REAL I/O values require a DWORD address".to_string());
            }
            let value = raw
                .trim()
                .parse::<f32>()
                .map_err(|_| "REAL inputs accept decimal values such as 1.5".to_string())?;
            if !value.is_finite() {
                return Err("REAL I/O values must be finite".to_string());
            }
            Ok(RuntimeValue::DWord(value.to_bits()))
        }
        Some(TypeId::LREAL) => {
            if !matches!(address.size, IoSize::LWord) {
                return Err("LREAL I/O values require an LWORD address".to_string());
            }
            let value = raw
                .trim()
                .parse::<f64>()
                .map_err(|_| "LREAL inputs accept decimal values such as 1.5".to_string())?;
            if !value.is_finite() {
                return Err("LREAL I/O values must be finite".to_string());
            }
            Ok(RuntimeValue::LWord(value.to_bits()))
        }
        Some(TypeId::TIME) => {
            if !matches!(address.size, IoSize::DWord) {
                return Err("TIME I/O values require a DWORD address".to_string());
            }
            let millis = parse_time_millis(raw)?;
            let millis = u32::try_from(millis)
                .map_err(|_| "TIME value out of range for DWORD milliseconds".to_string())?;
            Ok(RuntimeValue::DWord(millis))
        }
        Some(TypeId::STRING) => match address.size {
            IoSize::Bytes(len) => {
                let text = raw.trim().trim_matches('\'');
                if text.len() > len as usize {
                    return Err(format!("STRING value exceeds {len} bytes"));
                }
                Ok(RuntimeValue::String(SmolStr::new(text)))
            }
            _ => Err("STRING I/O values require a byte-span address".to_string()),
        },
        _ => parse_io_value(address, raw),
    }
}

fn parse_time_millis(raw: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("TIME value cannot be empty".to_string());
    }
    if let Ok(value) = parse_numeric(trimmed) {
        return Ok(value);
    }
    let upper = trimmed.to_ascii_uppercase();
    let value = upper.strip_prefix("T#").ok_or_else(|| {
        "TIME inputs accept milliseconds or IEC literals such as T#250ms".to_string()
    })?;
    let split_at = value
        .find(|ch: char| ch.is_ascii_alphabetic())
        .ok_or_else(|| "TIME literal requires a unit such as ms or s".to_string())?;
    let (number, unit) = value.split_at(split_at);
    let number = number
        .parse::<u64>()
        .map_err(|_| "invalid TIME literal number".to_string())?;
    match unit {
        "MS" => Ok(number),
        "S" => number
            .checked_mul(1_000)
            .ok_or_else(|| "TIME literal is too large".to_string()),
        "US" => Ok(number / 1_000),
        "NS" => Ok(number / 1_000_000),
        _ => Err("TIME literal unit must be ns, us, ms, or s".to_string()),
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
    fn convert(entries: Vec<IoSnapshotEntry>, forced: &[IoAddress]) -> Vec<IoStateEntry> {
        entries
            .into_iter()
            .map(|entry| {
                let name = entry.name.map(|name| name.to_string());
                let address = format_io_address(&entry.address);
                let value_type = entry
                    .value_type
                    .and_then(trust_runtime::io::io_value_type_name)
                    .map(str::to_string)
                    .or_else(|| match &entry.value {
                        IoSnapshotValue::Value(value) => value_type_name(value),
                        IoSnapshotValue::Error(_) | IoSnapshotValue::Unresolved => None,
                    });
                let value = match entry.value {
                    IoSnapshotValue::Value(value) => format_value(&value),
                    IoSnapshotValue::Error(err) => format!("error: {err}"),
                    IoSnapshotValue::Unresolved => "<unresolved>".to_string(),
                };
                IoStateEntry {
                    name,
                    address,
                    source: entry.source.map(|source| source.to_string()),
                    value_type,
                    value,
                    forced: forced.iter().any(|forced| forced == &entry.address),
                    writable: None,
                }
            })
            .collect()
    }

    let forced = snapshot.forced;
    IoStateEventBody {
        scan: snapshot.scan,
        inputs: convert(snapshot.inputs, &forced),
        outputs: convert(snapshot.outputs, &forced),
        memory: convert(snapshot.memory, &forced),
    }
}

pub(super) fn append_ads_live_values(
    state: &mut IoStateEventBody,
    values: Vec<trust_runtime::ads::AdsLiveValue>,
) {
    for value in values {
        let entry = IoStateEntry {
            name: Some(value.point_name.clone()),
            address: format!("global:{}", value.point_name),
            source: Some(format!(
                "ADS {} · port {}",
                value.connection_name, value.ams_port
            )),
            value_type: value_type_name(&value.value),
            value: format_value(&value.value),
            forced: value.forced,
            writable: Some(value.writable),
        };
        if value.input {
            state.inputs.push(entry);
        } else {
            state.outputs.push(entry);
        }
    }
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
