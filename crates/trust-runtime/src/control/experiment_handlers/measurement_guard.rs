//! The measurement guard of an experiment (runtime protocol annex, E5.3, E5.4 and E5.6): the loop's measured output and
//! the band it must stay in. The runtime reads it after every execution of the experiment's task and stops the experiment,
//! restoring its target, once the measurement leaves the band.

use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use super::values::{numeric_value, snapshot_value};
use crate::io::{validate_process_image_address, IoAddress, IoSnapshot};
use crate::runtime::{Located, ProgramRead};

type Refusal = (String, &'static str);

/// The guard a start request names (E5.3): the measured signal, by process-image address or by variable path, and its
/// band.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MeasurementGuardParams {
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    symbol: Option<String>,
    minimum: f64,
    maximum: f64,
}

/// The measurement guard of an experiment: its signal, its band, and the value it read last.
#[derive(Clone, Debug)]
pub(super) struct MeasurementGuard {
    signal: GuardSignal,
    minimum: f64,
    maximum: f64,
    /// The value read at the latest execution of the experiment's task; after a stop, the value that stopped it.
    value: Option<f64>,
}

#[derive(Clone, Debug)]
enum GuardSignal {
    Address {
        address: IoAddress,
        text: String,
    },
    /// A variable, at the place its path resolved to at the start; a reloaded program that moved it reads as missing.
    Symbol {
        path: String,
        at: Located,
    },
}

/// What one reading of the guard found.
pub(super) enum GuardReading {
    Inside,
    Outside,
    /// The signal has no finite numeric value in the scan.
    Unreadable,
}

impl MeasurementGuard {
    /// The guard of a start request, checked against the scan the start reads: the band is finite and ordered, the signal
    /// is an address of the process image or a variable with a numeric value, and the measurement lies within the band.
    pub(super) fn start(
        params: MeasurementGuardParams,
        snapshot: &IoSnapshot,
        program: Option<&dyn ProgramRead>,
    ) -> Result<Self, Refusal> {
        if !(params.minimum.is_finite()
            && params.maximum.is_finite()
            && params.minimum < params.maximum)
        {
            return Err((
                "the measurement guard's band must be finite, with its minimum below its maximum"
                    .into(),
                "invalid_experiment",
            ));
        }
        let signal = match (params.address, params.symbol) {
            (Some(text), None) => {
                let address = IoAddress::parse(&text)
                    .map_err(|error| (error.to_string(), "invalid_address"))?;
                validate_process_image_address(&address)
                    .map_err(|error| (error.to_string(), "invalid_address"))?;
                GuardSignal::Address { address, text }
            }
            (None, Some(path)) => {
                let resolved = program
                    .ok_or_else(|| "no runtime scan has been captured yet".to_string())
                    .and_then(|program| {
                        crate::debug::resolve_control_parameter_path(program, &path)
                    })
                    .map_err(|reason| {
                        (
                            format!("measurement guard {path}: {reason}"),
                            "invalid_target",
                        )
                    })?;
                GuardSignal::Symbol {
                    path,
                    at: resolved.at,
                }
            }
            _ => {
                return Err((
                    "a measurement guard reads one signal: give either `address` or `symbol`"
                        .into(),
                    "invalid_target",
                ))
            }
        };
        let mut guard = Self {
            signal,
            minimum: params.minimum,
            maximum: params.maximum,
            value: None,
        };
        match guard.read(snapshot, program) {
            GuardReading::Inside => Ok(guard),
            GuardReading::Unreadable => Err((
                format!(
                    "the measurement guard's signal {} has no numeric value in the current scan",
                    guard.name()
                ),
                "invalid_target",
            )),
            GuardReading::Outside => Err((
                format!(
                    "the measurement {} reads {}, outside its band of {} to {}",
                    guard.name(),
                    guard.value.unwrap_or(f64::NAN),
                    guard.minimum,
                    guard.maximum
                ),
                "outside_limits",
            )),
        }
    }

    /// Reads the measurement in a scan (`io`, and `program` as the scan left it) and keeps the value it read.
    pub(super) fn read(
        &mut self,
        io: &IoSnapshot,
        program: Option<&dyn ProgramRead>,
    ) -> GuardReading {
        let value = match &self.signal {
            GuardSignal::Address { address, .. } => {
                snapshot_value(io, address).and_then(numeric_value)
            }
            GuardSignal::Symbol { path, at } => program
                .and_then(|program| {
                    crate::debug::resolve_control_parameter_path(program, path).ok()
                })
                .filter(|resolved| resolved.at == *at)
                .and_then(|resolved| numeric_value(&resolved.value)),
        }
        .filter(|value| value.is_finite());
        self.value = value;
        match value {
            None => GuardReading::Unreadable,
            Some(value) if value < self.minimum || value > self.maximum => GuardReading::Outside,
            Some(_) => GuardReading::Inside,
        }
    }

    /// The guard as the status reports it (E5.6): its signal as the request named it, its band, and the value it read last.
    pub(super) fn status(&self) -> JsonValue {
        let key = match &self.signal {
            GuardSignal::Address { .. } => "address",
            GuardSignal::Symbol { .. } => "symbol",
        };
        json!({
            key: self.name(),
            "minimum": self.minimum,
            "maximum": self.maximum,
            "value": self.value,
        })
    }

    fn name(&self) -> &str {
        match &self.signal {
            GuardSignal::Address { text, .. } => text,
            GuardSignal::Symbol { path, .. } => path,
        }
    }
}
