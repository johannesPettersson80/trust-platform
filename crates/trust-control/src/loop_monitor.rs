//! The live monitor of one loop (specification 35, B2 and A3.4): one coherent runtime scan projected into the sample the page
//! shows, and the rules that decide whether that sample is current. The engine reads the runtime and passes the scan here. This
//! module decides availability, freshness, the missing signals, the quality, the controller mode and the saturation.
//!
//! Times are passed in, never read from a clock: the engine stamps each scan on its own clock, and the runtime's clock is never
//! compared with it (A3.4).

use schemars::JsonSchema;
use serde::Serialize;

use crate::loop_definition::{
    AuxiliaryRole, AuxiliarySignal, LoopDefinition, LoopSignal, ModeWhenTrue,
};
use crate::signals::{InspectionSample, Quality};

/// The reason of an unavailable sample while the runtime is not running (B2).
pub const NOT_PRODUCING_SCANS: &str = "Runtime is not producing scans";
/// The reason of an unavailable sample whose scan has not advanced for longer than the stale threshold (A3.4).
pub const STALE_SCAN: &str = "Runtime scan is stale";
/// The reason of an unavailable sample for a loop that declares no output limits, which its sample and saturation need.
pub const NO_OUTPUT_LIMITS: &str =
    "The loop's output limits are missing: limits.outputMinimum and limits.outputMaximum must be set.";
/// The reason of the pending sample a call answers before the first read of its observation has come back (B2).
pub const AWAITING_SCAN: &str = "Waiting for the first runtime scan of the loop.";

/// Where the observed values come from (A2.4): a simulated plant, or any other target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub enum MonitorSource {
    /// Simulation is enabled and every attached I/O is simulated.
    Simulation,
    /// Any other target.
    Physical,
}

/// The quality of a monitored value: a valid value, not forced or forced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MonitorQuality {
    /// A valid value that is not forced.
    Good,
    /// A valid value that is forced.
    Forced,
}

/// The controller's mode, as its mode signal reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub enum ControllerMode {
    /// Automatic control.
    Auto,
    /// Manual control.
    Manual,
    /// The mode signal does not give a usable value.
    Unavailable,
}

/// Whether the manipulated variable sits at an output limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Saturation {
    /// At or below the smallest output.
    Minimum,
    /// At or above the largest output.
    Maximum,
    /// Between the limits.
    #[serde(rename = "none")]
    NotSaturated,
}

/// One value of a loop's signal as the page reads it: its symbol, its address when it has one, its unit and its quality.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSignal {
    /// The variable's path.
    pub symbol: String,
    /// The variable's address; absent for a program variable without one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The value, in the signal's unit.
    pub value: f64,
    /// The signal's unit; empty means dimensionless.
    pub unit: String,
    /// Whether the value is forced.
    pub quality: MonitorQuality,
    /// Whether the plots draw the value: a forced value is a gap in its own trace (B9.6).
    pub plotted: bool,
}

/// A further signal of the loop, with its channel, its role and a label for display.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonitorAuxiliarySignal {
    /// The signal's identifier in the loop.
    pub id: String,
    /// The channel it belongs to.
    pub channel: String,
    /// Its role in the channel.
    pub role: AuxiliaryRole,
    /// The identifier, humanized for display; the symbol stays available beside it.
    pub display_name: String,
    /// The variable's path.
    pub symbol: String,
    /// The variable's address; absent for a program variable without one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The value, in the signal's unit.
    pub value: f64,
    /// The signal's unit; empty means dimensionless.
    pub unit: String,
    /// Whether the value is forced.
    pub quality: MonitorQuality,
    /// Whether the plots draw the value: a forced value is a gap in its own trace (B9.6).
    pub plotted: bool,
}

/// The controller of a sample: its instance, its revision and its mode.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonitorController {
    /// The controller's instance path.
    pub instance: String,
    /// The controller's content revision.
    pub revision: String,
    /// The mode the mode signal gives.
    pub mode: ControllerMode,
}

/// The smallest and the largest output of the loop.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputLimits {
    /// The smallest output.
    pub minimum: f64,
    /// The largest output.
    pub maximum: f64,
}

/// The rates the output may change at, when the loop declares them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RateLimits {
    /// The largest rise of the output, per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_per_second: Option<f64>,
    /// The largest fall of the output, per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down_per_second: Option<f64>,
}

/// One coherent scan of the loop, as the page shows it (B2).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(inline)]
pub struct AvailableSample {
    /// The debug session the runtime is attached to; a new session starts a new observation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_session_id: Option<String>,
    /// Where the values come from.
    pub source: MonitorSource,
    /// The loop's identifier.
    pub loop_id: String,
    /// The loop's content revision.
    pub loop_revision: String,
    /// The controller and its mode.
    pub controller: MonitorController,
    /// The runtime's resource cycle count of the scan.
    pub scan: u64,
    /// The engine's clock when the scan was received, in milliseconds.
    pub observed_at_ms: u64,
    /// The setpoint, when the loop has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setpoint: Option<MonitorSignal>,
    /// The measurement.
    pub process_variable: MonitorSignal,
    /// The controller's output.
    pub manipulated_variable: MonitorSignal,
    /// The further signals of the loop; absent when the loop has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<Vec<MonitorAuxiliarySignal>>,
    /// The output limits.
    pub output_limits: OutputLimits,
    /// The rate limits the loop declares.
    pub rate_limits: RateLimits,
    /// Whether the output sits at a limit.
    pub saturation: Saturation,
    /// Whether a runtime recording of the loop is running.
    pub recording: bool,
}

/// A loop whose sample is unavailable, with the reason the page shows.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(inline)]
pub struct UnavailableSample {
    /// Where the values would come from.
    pub source: MonitorSource,
    /// The loop's identifier.
    pub loop_id: String,
    /// Why the sample is unavailable, as a sentence.
    pub reason: String,
}

/// The sample of a loop: a coherent scan, or the reason there is none (B2, A3.4).
#[allow(clippy::large_enum_variant, clippy::module_name_repetitions)] // One sample per answer, built once and serialized at once; the name is the page's.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "availability", rename_all = "camelCase")]
pub enum LoopMonitorSample {
    /// The loop's values from one coherent scan.
    Available(AvailableSample),
    /// The loop's values are not current; the reason says why.
    Unavailable(UnavailableSample),
}

/// The values one coherent scan gives a loop: the scan number, the controller's mode and each signal the scan has a good or
/// forced value for. A signal the scan does not have is `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct CoherentReadings {
    /// The runtime's scan count.
    pub scan: u64,
    /// The controller's mode.
    pub controller_mode: ControllerMode,
    /// The setpoint, when the loop has one and the scan has it.
    pub setpoint: Option<MonitorSignal>,
    /// The measurement.
    pub process_variable: Option<MonitorSignal>,
    /// The controller's output.
    pub manipulated_variable: Option<MonitorSignal>,
    /// One entry per further signal of the loop, in the loop's order.
    pub additional: Vec<Option<MonitorAuxiliarySignal>>,
}

/// The observation of one loop: the last scan, when it was first seen, and the runtime-recording flag. It keeps no
/// runtime state of its own; the engine resets it when the runtime is replaced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoopMonitor {
    last_scan: Option<u64>,
    last_new_scan_ms: Option<u64>,
    runtime_recording: bool,
}

impl LoopMonitor {
    /// Sets whether a runtime recording of the loop is running; each later sample says so.
    pub fn set_runtime_recording(&mut self, active: bool) {
        self.runtime_recording = active;
    }

    /// A confirmed runtime replacement resets freshness and keeps the recording flag.
    pub fn reset_runtime_observation(&mut self) {
        self.last_scan = None;
        self.last_new_scan_ms = None;
    }

    /// Projects one scan of the loop into its sample. `readings` is `None` while the runtime is not running. The scan is stale
    /// once it has not advanced for more than `stale_after_ms` of the engine's clock, which is `observed_at_ms` (A3.4).
    #[must_use]
    pub fn observe(
        &mut self,
        loop_definition: &LoopDefinition,
        source: MonitorSource,
        readings: Option<CoherentReadings>,
        observed_at_ms: u64,
        stale_after_ms: u64,
    ) -> LoopMonitorSample {
        let Some(readings) = readings else {
            self.reset_runtime_observation();
            return unavailable_sample(source, &loop_definition.id, NOT_PRODUCING_SCANS);
        };
        if let Some(reason) = self.freshness(readings.scan, observed_at_ms, stale_after_ms) {
            return unavailable_sample(source, &loop_definition.id, &reason);
        }
        match self.available(loop_definition, source, readings, observed_at_ms) {
            Ok(sample) => LoopMonitorSample::Available(sample),
            Err(reason) => unavailable_sample(source, &loop_definition.id, &reason),
        }
    }

    /// Records a new scan, or refuses a scan that went backwards or has not advanced for too long (A3.4). Returns the reason
    /// the scan is refused.
    fn freshness(&mut self, scan: u64, observed_at_ms: u64, stale_after_ms: u64) -> Option<String> {
        if let Some(last) = self.last_scan {
            if scan < last {
                return Some(format!(
                    "Runtime scan moved backwards from {last} to {scan}"
                ));
            }
        }
        if self.last_scan != Some(scan) {
            self.last_scan = Some(scan);
            self.last_new_scan_ms = Some(observed_at_ms);
            return None;
        }
        let stale = self
            .last_new_scan_ms
            .is_some_and(|since| observed_at_ms.saturating_sub(since) > stale_after_ms);
        stale.then(|| STALE_SCAN.to_string())
    }

    /// The sample of a fresh scan, or the reason a signal or limit it needs is missing.
    fn available(
        &self,
        loop_definition: &LoopDefinition,
        source: MonitorSource,
        readings: CoherentReadings,
        observed_at_ms: u64,
    ) -> Result<AvailableSample, String> {
        let signals = &loop_definition.signals;
        let scan = readings.scan;
        let setpoint = match (&signals.setpoint, readings.setpoint) {
            (Some(binding), None) => return Err(missing_reason("Setpoint", binding, scan)),
            (_, reading) => reading,
        };
        let process_variable = readings
            .process_variable
            .ok_or_else(|| missing_reason("Process variable", &signals.process_variable, scan))?;
        let manipulated_variable = readings.manipulated_variable.ok_or_else(|| {
            missing_reason("Manipulated variable", &signals.manipulated_variable, scan)
        })?;
        let mut additional = Vec::new();
        let mut slots = readings.additional.into_iter();
        for binding in signals.additional.iter().flatten() {
            let reading = slots
                .next()
                .flatten()
                .ok_or_else(|| missing_reason(&binding.symbol, binding, scan))?;
            additional.push(reading);
        }
        let limits = &loop_definition.limits;
        let (Some(minimum), Some(maximum)) = (limits.output_minimum, limits.output_maximum) else {
            return Err(NO_OUTPUT_LIMITS.to_string());
        };
        let output_saturation = saturation(manipulated_variable.value, minimum, maximum);
        Ok(AvailableSample {
            runtime_session_id: None,
            source,
            loop_id: loop_definition.id.clone(),
            loop_revision: loop_definition.revision.clone(),
            controller: MonitorController {
                instance: loop_definition.controller.instance.clone(),
                revision: loop_definition.controller.revision.clone(),
                mode: readings.controller_mode,
            },
            scan,
            observed_at_ms,
            setpoint,
            process_variable,
            manipulated_variable,
            additional: (!additional.is_empty()).then_some(additional),
            output_limits: OutputLimits { minimum, maximum },
            rate_limits: RateLimits {
                up_per_second: limits.rate_up_per_s,
                down_per_second: limits.rate_down_per_s,
            },
            saturation: output_saturation,
            recording: self.runtime_recording,
        })
    }
}

/// The readings of one coherent scan for the loop. A signal reads only with a good or forced value; a bad or missing value
/// is not a reading, so the sample names it as missing rather than promoting it (B2).
#[must_use]
pub fn coherent_readings(
    loop_definition: &LoopDefinition,
    sample: &InspectionSample,
    mode_id: Option<&str>,
) -> CoherentReadings {
    let signals = &loop_definition.signals;
    CoherentReadings {
        scan: sample.scan,
        controller_mode: controller_mode(loop_definition, sample, mode_id),
        setpoint: signals
            .setpoint
            .as_ref()
            .and_then(|binding| reading(sample, "setpoint", binding)),
        process_variable: reading(sample, "processVariable", &signals.process_variable),
        manipulated_variable: reading(sample, "manipulatedVariable", &signals.manipulated_variable),
        additional: signals
            .additional
            .iter()
            .flatten()
            .map(|binding| {
                reading(sample, &binding.id, binding).map(|signal| MonitorAuxiliarySignal {
                    id: binding.id.clone(),
                    channel: binding.channel.clone(),
                    role: binding.role,
                    display_name: humanize_signal_identifier(&binding.id),
                    symbol: signal.symbol,
                    address: signal.address,
                    value: signal.value,
                    unit: signal.unit,
                    quality: signal.quality,
                    plotted: signal.plotted,
                })
            })
            .collect(),
    }
}

/// Turns a kebab- or snake-case identifier into a label, such as "door-open-indication" into "Door open indication", so a
/// learner reads the loop's signals in the lesson's own words (B2). An identifier with no word is returned as it is.
#[must_use]
pub fn humanize_signal_identifier(id: &str) -> String {
    let mut words = id.split(['-', '_']).filter(|word| !word.is_empty());
    let Some(first) = words.next() else {
        return id.to_string();
    };
    std::iter::once(capitalize(first))
        .chain(words.map(str::to_lowercase))
        .collect::<Vec<_>>()
        .join(" ")
}

/// An unavailable sample of the loop, with the reason the page shows (B2).
#[must_use]
pub fn unavailable_sample(source: MonitorSource, loop_id: &str, reason: &str) -> LoopMonitorSample {
    LoopMonitorSample::Unavailable(UnavailableSample {
        source,
        loop_id: loop_id.to_string(),
        reason: reason.to_string(),
    })
}

/// The sample a loop has before its observation holds a read: the target is not known yet, so it is physical (A2.4).
#[must_use]
pub fn pending_sample(loop_id: &str) -> LoopMonitorSample {
    unavailable_sample(MonitorSource::Physical, loop_id, AWAITING_SCAN)
}

/// The signal of the loop that a reading names: its symbol, its address when it has one, and its unit.
trait LoopSignalView {
    fn symbol(&self) -> &str;
    fn address(&self) -> Option<&str>;
    fn unit(&self) -> &str;
}

impl LoopSignalView for LoopSignal {
    fn symbol(&self) -> &str {
        &self.symbol
    }
    fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }
    fn unit(&self) -> &str {
        &self.unit
    }
}

impl LoopSignalView for AuxiliarySignal {
    fn symbol(&self) -> &str {
        &self.symbol
    }
    fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }
    fn unit(&self) -> &str {
        &self.unit
    }
}

/// The reading of the signal that the scan's binding `id` gives, when the value is there and good or forced.
fn reading(
    sample: &InspectionSample,
    id: &str,
    signal: &impl LoopSignalView,
) -> Option<MonitorSignal> {
    let index = sample
        .bindings
        .iter()
        .position(|binding| binding.id == id)?;
    let value = sample.values[index]?;
    if !matches!(sample.quality[index], Quality::Good | Quality::Forced) {
        return None;
    }
    let forced = sample.forced[index] || sample.quality[index] == Quality::Forced;
    Some(MonitorSignal {
        symbol: signal.symbol().to_string(),
        address: signal.address().map(str::to_string),
        value,
        unit: signal.unit().to_string(),
        quality: if forced {
            MonitorQuality::Forced
        } else {
            MonitorQuality::Good
        },
        plotted: !forced,
    })
}

/// The controller's mode, from the mode signal's usable value; `Unavailable` when the signal gives none (B2).
fn controller_mode(
    loop_definition: &LoopDefinition,
    sample: &InspectionSample,
    mode_id: Option<&str>,
) -> ControllerMode {
    match mode_id.and_then(|id| sample.controller_mode(loop_definition, id)) {
        Some(ModeWhenTrue::Auto) => ControllerMode::Auto,
        Some(ModeWhenTrue::Manual) => ControllerMode::Manual,
        None => ControllerMode::Unavailable,
    }
}

/// Whether the value is at or beyond an output limit (B2).
fn saturation(value: f64, minimum: f64, maximum: f64) -> Saturation {
    if value <= minimum {
        Saturation::Minimum
    } else if value >= maximum {
        Saturation::Maximum
    } else {
        Saturation::NotSaturated
    }
}

fn missing_reason(label: &str, signal: &impl LoopSignalView, scan: u64) -> String {
    format!(
        "{label} {} is missing from runtime scan {scan}",
        signal.address().unwrap_or_else(|| signal.symbol())
    )
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first
        .to_uppercase()
        .chain(chars.flat_map(char::to_lowercase))
        .collect()
}

#[cfg(test)]
#[path = "loop_monitor_tests.rs"]
mod tests;
