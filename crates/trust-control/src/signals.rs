//! Signal reads (runtime protocol annex, E3): the bindings Control Studio asks the runtime to
//! inspect, the scan it answers with, and the freshness of successive readings (specification 35,
//! A3.1 and A3.4).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::Failure;
use crate::limits::RECORDING_CHANNEL_LIMIT;
use crate::loop_definition::{LoopDefinition, ModeWhenTrue};

/// A signal Control Studio reads: its identifier in the request, its IEC type and exactly one
/// selector, an address or a symbol (E2.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalBinding {
    /// The identifier, unique in the request.
    pub id: String,
    /// The direct address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The variable's path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// The upper-case IEC type name.
    pub iec_type: String,
}

impl SignalBinding {
    fn addressed(id: &str, address: Option<&str>, symbol: &str, iec_type: &str) -> Self {
        Self {
            id: id.to_string(),
            address: address.map(str::to_string),
            symbol: address.is_none().then(|| symbol.to_string()),
            iec_type: iec_type.to_string(),
        }
    }

    fn is_bool(&self) -> bool {
        self.iec_type.eq_ignore_ascii_case("BOOL")
    }
}

/// The quality of one value of a scan (E3, rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// A valid value that is not forced.
    Good,
    /// Not a number or BOOL, not finite, out of the safe integer range, or of the wrong type.
    Bad,
    /// A valid value that is forced.
    Forced,
    /// The symbol or address does not resolve.
    Missing,
}

/// The request of `control.signals.inspect` (E3), checked before it is sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct InspectionRequest {
    /// The loop revision the watch belongs to.
    pub loop_revision: String,
    /// The program revision the answer must have, once one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_program_revision: Option<String>,
    /// The signals, 1 to 32.
    pub bindings: Vec<SignalBinding>,
}

impl InspectionRequest {
    /// A request for `bindings` of the loop revision `loop_revision`.
    ///
    /// # Errors
    /// The revisions are empty, or the bindings are not 1 to 32 with unique identifiers and one
    /// selector each.
    pub fn new(
        loop_revision: &str,
        expected_program_revision: Option<&str>,
        bindings: Vec<SignalBinding>,
    ) -> Result<Self, Failure> {
        let invalid =
            |what: &str| Failure::invalid_request(&format!("The signal inspection {what}."));
        if loop_revision.is_empty()
            || loop_revision.len() > 256
            || expected_program_revision.is_some_and(str::is_empty)
        {
            return Err(invalid("needs a loop revision of 1 to 256 bytes"));
        }
        check_bindings(&bindings).map_err(|reason| invalid(&reason))?;
        Ok(Self {
            loop_revision: loop_revision.to_string(),
            expected_program_revision: expected_program_revision.map(str::to_string),
            bindings,
        })
    }

    /// Reads the runtime's answer: the echo of the request's identity, and one value, forced flag
    /// and quality per binding.
    ///
    /// # Errors
    /// The answer does not match the request or the format of E3.
    pub fn decode(&self, response: &Value) -> Result<Inspection, Failure> {
        let invalid = || Failure::invalid_response("control.signals.inspect");
        let answer: InspectionAnswer =
            serde_json::from_value(response.clone()).map_err(|_| invalid())?;
        if answer.loop_revision != self.loop_revision
            || answer.program_revision.is_empty()
            || self
                .expected_program_revision
                .as_ref()
                .is_some_and(|expected| *expected != answer.program_revision)
            || answer.bindings != self.bindings
        {
            return Err(invalid());
        }
        if !answer.available {
            // A pending answer authorizes nothing. While no scan is captured the runtime may omit its
            // reason, so the reason of E3 stands in for it.
            let reason = answer
                .reason
                .filter(|reason| !reason.is_empty())
                .unwrap_or_else(|| WAITING_FOR_SCAN.to_string());
            return Ok(Inspection::Pending {
                program_revision: answer.program_revision,
                reason,
            });
        }
        let count = self.bindings.len();
        let (
            Some(scan),
            Some(runtime_time_ns),
            Some(captured_at_ms),
            Some(values),
            Some(forced),
            Some(quality),
        ) = (
            answer.scan,
            answer
                .runtime_time_ns
                .and_then(|time| u64::try_from(time).ok()),
            answer.captured_at_ms,
            answer.values,
            answer.forced,
            answer.quality,
        )
        else {
            return Err(invalid());
        };
        if values.len() != count
            || forced.len() != count
            || quality.len() != count
            || values.iter().flatten().any(|value| !value.is_finite())
            || [scan, runtime_time_ns, captured_at_ms]
                .iter()
                .any(|number| *number > MAX_SAFE_INTEGER)
        {
            return Err(invalid());
        }
        Ok(Inspection::Available(InspectionSample {
            program_revision: answer.program_revision,
            loop_revision: answer.loop_revision,
            bindings: answer.bindings,
            scan,
            runtime_time_ns,
            captured_at_ms,
            values,
            forced,
            quality,
        }))
    }
}

/// The largest integer JSON carries exactly: 2⁵³ − 1 (E0.3).
const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// The reason of a pending inspection (E3).
const WAITING_FOR_SCAN: &str = "Waiting for the next coherent runtime scan.";

#[derive(Deserialize)]
struct InspectionAnswer {
    program_revision: String,
    loop_revision: String,
    bindings: Vec<SignalBinding>,
    available: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    scan: Option<u64>,
    /// Negative while the runtime has no scan time yet.
    #[serde(default)]
    runtime_time_ns: Option<i64>,
    #[serde(default)]
    captured_at_ms: Option<u64>,
    #[serde(default)]
    values: Option<Vec<Option<f64>>>,
    #[serde(default)]
    forced: Option<Vec<bool>>,
    #[serde(default)]
    quality: Option<Vec<Quality>>,
}

/// The runtime's answer to an inspection.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Inspection {
    /// No complete scan yet.
    #[serde(rename_all = "camelCase")]
    Pending {
        /// The current program revision.
        program_revision: String,
        /// Why no scan is available.
        reason: String,
    },
    /// One scan.
    Available(InspectionSample),
}

/// One scan of the inspected signals; all values come from that scan (E3, rule 2).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionSample {
    /// The program revision.
    pub program_revision: String,
    /// The loop revision.
    pub loop_revision: String,
    /// The signals, in request order.
    pub bindings: Vec<SignalBinding>,
    /// The resource cycle count.
    pub scan: u64,
    /// The runtime time of the scan.
    pub runtime_time_ns: u64,
    /// The runtime's wall-clock time of capture, for display only.
    pub captured_at_ms: u64,
    /// One value per binding; null when bad or missing. A BOOL is 0 or 1.
    pub values: Vec<Option<f64>>,
    /// One forced flag per binding.
    pub forced: Vec<bool>,
    /// One quality per binding.
    pub quality: Vec<Quality>,
}

impl InspectionSample {
    /// The value of binding `id` when it can be used: good, not forced, finite, and 0 or 1 for a
    /// BOOL.
    #[must_use]
    // A BOOL is reported as exactly 0 or 1 (E0.3).
    #[allow(clippy::float_cmp)]
    pub fn usable(&self, id: &str) -> Option<f64> {
        let index = self.bindings.iter().position(|binding| binding.id == id)?;
        let value = self.values[index].filter(|value| value.is_finite())?;
        if self.quality[index] != Quality::Good || self.forced[index] {
            return None;
        }
        (!self.bindings[index].is_bool() || value == 0.0 || value == 1.0).then_some(value)
    }

    /// The loop's mode, from the mode signal's usable value; `None` when it cannot be read.
    #[must_use]
    // A usable BOOL is exactly 0 or 1.
    #[allow(clippy::float_cmp)]
    pub fn controller_mode(
        &self,
        loop_definition: &LoopDefinition,
        mode_id: &str,
    ) -> Option<ModeWhenTrue> {
        let when_true = loop_definition
            .controller
            .mode_when_true
            .unwrap_or(ModeWhenTrue::Auto);
        let value = self.usable(mode_id)?;
        Some(match (value == 1.0, when_true) {
            (true, mode) => mode,
            (false, ModeWhenTrue::Auto) => ModeWhenTrue::Manual,
            (false, ModeWhenTrue::Manual) => ModeWhenTrue::Auto,
        })
    }
}

fn check_bindings(bindings: &[SignalBinding]) -> Result<(), String> {
    let limit = usize::try_from(RECORDING_CHANNEL_LIMIT).unwrap_or(usize::MAX);
    if bindings.is_empty() || bindings.len() > limit {
        return Err(format!("selects 1 to {limit} signals"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for binding in bindings {
        let selector = (
            binding
                .address
                .as_deref()
                .filter(|address| !address.is_empty()),
            binding
                .symbol
                .as_deref()
                .filter(|symbol| !symbol.is_empty()),
        );
        if binding.id.is_empty()
            || binding.id.len() > 128
            || binding.iec_type.is_empty()
            || !ids.insert(binding.id.as_str())
            || matches!(selector, (Some(_), Some(_)) | (None, None))
            || binding
                .symbol
                .as_ref()
                .is_some_and(|symbol| symbol.len() > 256)
        {
            return Err(format!(
                "binding {} needs a unique identifier, a type and one selector",
                binding.id
            ));
        }
    }
    Ok(())
}

/// The signals one inspection watch reads for a loop: its signals, then its mode variables as
/// BOOLs when no signal already reads them. Every reader of a loop shares this exact selection.
#[must_use]
pub fn loop_inspection_bindings(loop_definition: &LoopDefinition) -> Vec<SignalBinding> {
    let signals = &loop_definition.signals;
    let mut bindings: Vec<SignalBinding> = signals
        .setpoint
        .iter()
        .map(|signal| ("setpoint", signal))
        .chain([
            ("processVariable", &signals.process_variable),
            ("manipulatedVariable", &signals.manipulated_variable),
        ])
        .chain(signals.actuator.iter().map(|signal| ("actuator", signal)))
        .map(|(id, signal)| {
            SignalBinding::addressed(
                id,
                signal.address.as_deref(),
                &signal.symbol,
                &signal.iec_type,
            )
        })
        .collect();
    bindings.extend(signals.additional.iter().flatten().map(|signal| {
        SignalBinding::addressed(
            &signal.id,
            signal.address.as_deref(),
            &signal.symbol,
            &signal.iec_type,
        )
    }));
    let modes = [
        (
            "controllerMode",
            loop_definition.controller.mode_signal.as_deref(),
        ),
        (
            "acquisitionMode",
            loop_definition
                .parameter_binding
                .as_ref()
                .map(|binding| binding.mode.symbol.as_str()),
        ),
    ];
    for (name, symbol) in modes {
        let Some(symbol) = symbol else { continue };
        if loop_inspection_mode_id(loop_definition, &bindings, symbol).is_some() {
            continue;
        }
        let mut id = name.to_string();
        while bindings.iter().any(|binding| binding.id == id) {
            id.push('_');
        }
        let address = controller_mode_address(loop_definition, symbol);
        bindings.push(SignalBinding::addressed(
            &id,
            address.as_deref(),
            symbol,
            "BOOL",
        ));
    }
    bindings
}

/// The binding that reads the mode variable `symbol`: a BOOL with its address, or with its symbol.
#[must_use]
pub fn loop_inspection_mode_id<'a>(
    loop_definition: &LoopDefinition,
    bindings: &'a [SignalBinding],
    symbol: &str,
) -> Option<&'a str> {
    let address = controller_mode_address(loop_definition, symbol);
    bindings
        .iter()
        .find(|binding| {
            binding.is_bool()
                && match &address {
                    Some(address) => binding.address.as_deref() == Some(address.as_str()),
                    None => binding
                        .symbol
                        .as_deref()
                        .is_some_and(|bound| bound.eq_ignore_ascii_case(symbol)),
                }
        })
        .map(|binding| binding.id.as_str())
}

/// The address of a mode variable: that of a further BOOL signal that names it, or the mode
/// signal itself when it is a bit address such as `%MX0.0`.
#[must_use]
pub fn controller_mode_address(loop_definition: &LoopDefinition, mode: &str) -> Option<String> {
    let auxiliary = loop_definition
        .signals
        .additional
        .iter()
        .flatten()
        .find(|signal| {
            signal.iec_type.eq_ignore_ascii_case("BOOL")
                && (signal.symbol.eq_ignore_ascii_case(mode)
                    || signal
                        .address
                        .as_deref()
                        .is_some_and(|address| address.eq_ignore_ascii_case(mode)))
        });
    if let Some(signal) = auxiliary {
        return signal.address.clone();
    }
    is_bit_address(mode).then(|| mode.to_ascii_uppercase())
}

/// `%IX`, `%QX` or `%MX` with a byte and a bit number.
fn is_bit_address(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    let Some(rest) = upper.strip_prefix('%') else {
        return false;
    };
    let mut chars = rest.chars();
    if !matches!(chars.next(), Some('I' | 'Q' | 'M')) || chars.next() != Some('X') {
        return false;
    }
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|digit| digit.is_ascii_digit());
    chars
        .as_str()
        .split_once('.')
        .is_some_and(|(byte, bit)| digits(byte) && digits(bit))
}

/// Process time and receipt supervision for one operation that reads the runtime (A3.1, A3.4).
/// Process time runs from the runtime times of the readings, never from the host's clock. The
/// connection-loss rule compares receipts on the receiver's own clock: a newer runtime value must
/// arrive within the connection-loss timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadingClock {
    first_ns: Option<u64>,
    newest_ns: Option<u64>,
    newest_receipt_ms: u64,
}

impl ReadingClock {
    /// A clock started at the receiver's time `now_ms`.
    #[must_use]
    pub fn new(now_ms: u64) -> Self {
        Self {
            first_ns: None,
            newest_ns: None,
            newest_receipt_ms: now_ms,
        }
    }

    /// Records a reading of runtime time `runtime_time_ns`, received at `now_ms`, and returns the
    /// process time from the first reading to the newest one, in ms.
    // Process time differences stay far below 2^53 ns, so the conversion is exact.
    #[allow(clippy::cast_precision_loss)]
    pub fn observe(&mut self, runtime_time_ns: u64, now_ms: u64) -> f64 {
        let first = *self.first_ns.get_or_insert(runtime_time_ns);
        if self.newest_ns.is_none_or(|newest| runtime_time_ns > newest) {
            self.newest_ns = Some(runtime_time_ns);
            self.newest_receipt_ms = now_ms;
        }
        (self.newest_ns.unwrap_or(runtime_time_ns) - first) as f64 / 1e6
    }

    /// Whether a newer runtime value arrived within `timeout_ms` before `now_ms`.
    ///
    /// # Errors
    /// No newer value arrived: "The runtime stopped sending values."
    pub fn ensure_fresh(&self, now_ms: u64, timeout_ms: u64) -> Result<(), Failure> {
        if now_ms.saturating_sub(self.newest_receipt_ms) > timeout_ms {
            return Err(Failure::stopped_sending());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod tests;
