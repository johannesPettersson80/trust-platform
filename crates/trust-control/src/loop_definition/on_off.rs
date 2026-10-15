//! The switching (ON/OFF) controller of a loop as the loop file declares it (specification 35, C3 and §17.1; ported from
//! `onOffController.ts`): the hysteresis and the two minimum times, the band convention, the two output levels, the threshold-equality
//! policy and the retained states an Auto entry initializes. [`OnOffController::check_binding`] and [`LoopDefinition::check_on_off`] hold
//! the rules that need the loop's parameter binding and design (`decodeOnOffController` and `validateControlLoopDefinition`).

use std::collections::BTreeSet;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Map, Value};

use super::{
    Design, InitialSource, InitialValue, InitializedState, LoopChannels, LoopDefinition,
    ParameterBinding, StateType,
};
use crate::commissioning::initial_state::ControllerInitialization;
use crate::model::simulation::ActuatorKind;

/// The largest minimum ON or OFF time the setup accepts, in seconds. The source is the switching generator's bound on both times
/// (`loopSetup/onOffLoopGenerator.ts`, the two settings with a maximum of 3600), restated in the numerics annex N10.
pub const MINIMUM_TIME_MAXIMUM_SECONDS: f64 = 3600.0;

/// The largest value of the dwell timer state, in seconds. The source is the switching generator's bound on the timer state
/// (`loopSetup/onOffLoopGenerator.ts`, `maximum: 1e6`), restated in the numerics annex N10.
pub const DWELL_TIMER_MAXIMUM_SECONDS: f64 = 1_000_000.0;

/// The band convention of a switching controller's hysteresis (`bandConvention`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandConvention {
    /// The declared value is the half-width of the band around the setpoint.
    HalfBand,
    /// The declared value is the full width of the band.
    FullBand,
}

impl BandConvention {
    /// The convention a file names, or `None` for any other word.
    fn of(word: &str) -> Option<Self> {
        match word {
            "half-band" => Some(Self::HalfBand),
            "full-band" => Some(Self::FullBand),
            _ => None,
        }
    }

    /// The word the loop file writes.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::HalfBand => "half-band",
            Self::FullBand => "full-band",
        }
    }

    /// The half-width of the band for a declared value: the value itself for a half-band, half of it for a full band.
    #[must_use]
    pub fn half_width(self, declared: f64) -> f64 {
        match self {
            Self::HalfBand => declared,
            Self::FullBand => declared / 2.0,
        }
    }

    /// The declared value of a half-width: the inverse of [`Self::half_width`] (`halfBand * (full-band ? 2 : 1)`).
    #[must_use]
    pub fn declared_for(self, half_width: f64) -> f64 {
        match self {
            Self::HalfBand => half_width,
            Self::FullBand => half_width * 2.0,
        }
    }
}

/// Where the requested state changes when the measurement reaches a threshold (C3, D-10): at or beyond the threshold, the default, or
/// only strictly beyond it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThresholdEquality {
    /// The requested state changes at or beyond the threshold.
    #[default]
    AtOrBeyond,
    /// The requested state changes only strictly beyond the threshold.
    Beyond,
}

impl ThresholdEquality {
    /// The policy a file names, or `None` for any other word.
    fn of(word: &str) -> Option<Self> {
        match word {
            "at-or-beyond" => Some(Self::AtOrBeyond),
            "beyond" => Some(Self::Beyond),
            _ => None,
        }
    }

    /// The word the loop file writes.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::AtOrBeyond => "at-or-beyond",
            Self::Beyond => "beyond",
        }
    }
}

/// The switching controller of a loop (`controller.onOff`; C3): the three settings that Tune designs and Apply writes, the band
/// convention, the two output levels, the threshold policy, and the retained states an Auto entry initializes.
#[derive(Debug, Clone, PartialEq)]
pub struct OnOffController {
    /// The hysteresis setting's symbol; its unit is the measurement's unit.
    pub hysteresis_symbol: String,
    /// The minimum ON time's symbol, in seconds.
    pub minimum_on_time_symbol: String,
    /// The minimum OFF time's symbol, in seconds.
    pub minimum_off_time_symbol: String,
    /// Whether the hysteresis is the half-width or the full width of the band.
    pub band_convention: BandConvention,
    /// The lower output level, which is OFF.
    pub low_output: f64,
    /// The upper output level, which is ON.
    pub high_output: f64,
    /// The measurement's noise half-band, in the measurement's unit, zero or more; absent when none was declared.
    pub noise_band: Option<f64>,
    /// Where the requested state changes at a threshold (D-10); absent in the file, it is at or beyond the threshold.
    pub threshold_equality: ThresholdEquality,
    /// The retained states an Auto entry initializes: the output latch and, when the program counts them, the dwell timer.
    pub initialization: ControllerInitialization,
}

impl OnOffController {
    /// Reads a `controller.onOff` block: every rule of `decodeOnOffController` that needs no parameter binding. These are the band
    /// convention, the output levels in order, the noise band, the threshold policy, the three distinct symbols and the declared states.
    ///
    /// # Errors
    /// The block breaks one of these rules; the text names the field.
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("controller.onOff must be an object")?;
        let convention = BandConvention::of(text_field(object, "bandConvention")?)
            .ok_or("controller.onOff.bandConvention must be half-band or full-band")?;
        let low_output = finite_field(object, "lowOutput")?;
        let high_output = finite_field(object, "highOutput")?;
        if low_output >= high_output {
            return Err("controller.onOff.lowOutput must be below highOutput".to_owned());
        }
        let noise_band = match object.get("noiseBand") {
            None | Some(Value::Null) => None,
            Some(_) => {
                let band = finite_field(object, "noiseBand")?;
                if band < 0.0 {
                    return Err("controller.onOff.noiseBand must not be negative".to_owned());
                }
                Some(band)
            }
        };
        let threshold_equality = match object.get("thresholdEquality") {
            None | Some(Value::Null) => ThresholdEquality::default(),
            Some(Value::String(word)) => ThresholdEquality::of(word)
                .ok_or("controller.onOff.thresholdEquality must be at-or-beyond or beyond")?,
            Some(_) => return Err("controller.onOff.thresholdEquality must be text".to_owned()),
        };
        let symbols = [
            text_field(object, "hysteresisSymbol")?,
            text_field(object, "minimumOnTimeSymbol")?,
            text_field(object, "minimumOffTimeSymbol")?,
        ];
        let distinct: BTreeSet<&str> = symbols.iter().copied().collect();
        if distinct.len() != symbols.len() {
            return Err("controller.onOff's three symbols must be distinct".to_owned());
        }
        let initialization = initialization(
            object
                .get("initialization")
                .ok_or("controller.onOff.initialization must be declared")?,
        )?;
        Ok(Self {
            hysteresis_symbol: symbols[0].to_owned(),
            minimum_on_time_symbol: symbols[1].to_owned(),
            minimum_off_time_symbol: symbols[2].to_owned(),
            band_convention: convention,
            low_output,
            high_output,
            noise_band,
            threshold_equality,
            initialization,
        })
    }

    /// The block as the loop file writes it. The noise band and the threshold policy are written only when they differ from their
    /// absent value, so a file that declares neither keeps its shape.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut block = Map::new();
        block.insert(
            "hysteresisSymbol".to_owned(),
            Value::from(self.hysteresis_symbol.as_str()),
        );
        block.insert(
            "minimumOnTimeSymbol".to_owned(),
            Value::from(self.minimum_on_time_symbol.as_str()),
        );
        block.insert(
            "minimumOffTimeSymbol".to_owned(),
            Value::from(self.minimum_off_time_symbol.as_str()),
        );
        block.insert(
            "bandConvention".to_owned(),
            Value::from(self.band_convention.word()),
        );
        block.insert("lowOutput".to_owned(), Value::from(self.low_output));
        block.insert("highOutput".to_owned(), Value::from(self.high_output));
        if let Some(band) = self.noise_band {
            block.insert("noiseBand".to_owned(), Value::from(band));
        }
        if self.threshold_equality != ThresholdEquality::default() {
            block.insert(
                "thresholdEquality".to_owned(),
                Value::from(self.threshold_equality.word()),
            );
        }
        block.insert(
            "initialization".to_owned(),
            json!({
                "kind": "declared-state-v1",
                "states": serde_json::to_value(self.initialization.states()).unwrap_or_default(),
            }),
        );
        Value::Object(block)
    }

    /// The output latch: the BOOL state whose initial value follows the operating command (`model-input-is`), which an Auto entry starts at
    /// the output's applied level (C3). `None` when the block declares no such state.
    #[must_use]
    pub fn latch_symbol(&self) -> Option<&str> {
        self.initialization
            .states()
            .iter()
            .find(|state| {
                state.iec_type == StateType::Bool
                    && matches!(
                        state.initial,
                        InitialValue::From(InitialSource::ModelInputIs { .. })
                    )
            })
            .map(|state| state.symbol.as_str())
    }

    /// Checks the block against the loop's parameter binding (`decodeOnOffController`): the three symbols are bound parameters, and the
    /// two minimum times are in seconds.
    ///
    /// # Errors
    /// A symbol is not a bound parameter, or a minimum time is not in seconds.
    pub fn check_binding(&self, binding: &ParameterBinding) -> Result<(), String> {
        let parameter = |symbol: &str| {
            binding
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
        };
        for symbol in [
            self.hysteresis_symbol.as_str(),
            self.minimum_on_time_symbol.as_str(),
            self.minimum_off_time_symbol.as_str(),
        ] {
            if parameter(symbol).is_none() {
                return Err(format!(
                    "controller.onOff's symbol {symbol} is not a bound parameter"
                ));
            }
        }
        for symbol in [
            self.minimum_on_time_symbol.as_str(),
            self.minimum_off_time_symbol.as_str(),
        ] {
            if parameter(symbol).is_some_and(|found| found.unit.as_str() != "s") {
                return Err(format!(
                    "controller.onOff's minimum time {symbol} must be in seconds"
                ));
            }
        }
        Ok(())
    }
}

impl Serialize for OnOffController {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OnOffController {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Self::from_value(&value).map_err(D::Error::custom)
    }
}

impl LoopDefinition {
    /// The rules of a switching controller that need the loop's design, its law and its parameter binding
    /// (`validateControlLoopDefinition`): the project designs it, no other law is declared, and the block fits the binding.
    ///
    /// # Errors
    /// The loop declares a switching block with another design or another law, has no parameter binding, or its block does not fit
    /// the binding.
    pub(super) fn check_on_off(&self) -> Result<(), String> {
        let Some(block) = &self.controller.on_off else {
            return Ok(());
        };
        if self.controller.design != Some(Design::Project)
            || self.controller.law.is_some()
            || self.controller.state_feedback.is_some()
            || self.controller.project.is_some()
        {
            return Err(
                "controller.onOff needs the project design, and no other controller law".to_owned(),
            );
        }
        let Some(binding) = &self.parameter_binding else {
            return Err("controller.onOff needs the loop's parameter binding".to_owned());
        };
        block.check_binding(binding)
    }

    /// Whether the loop's channels have the shape of a switching controller (C3): one input, the manipulated variable, whose actuator is
    /// ON/OFF, and one output, the process variable, whose setpoint (when it has one) is the loop's setpoint. Only a loop with a
    /// switching block has that shape, and the gate serves it.
    #[must_use]
    pub(crate) fn is_on_off_channels(&self) -> bool {
        self.controller.on_off.is_some() && self.channels.as_ref().is_some_and(on_off_shape)
    }
}

/// Whether a loop file that does not decode has the shape of a switching controller's channels, read from its JSON as
/// [`LoopDefinition::is_on_off_channels`] reads a decoded loop. Channels that do not decode have no such shape.
#[must_use]
pub(crate) fn on_off_channels_raw(raw: &Value) -> bool {
    let Some(channels) = raw.get("channels").filter(|channels| !channels.is_null()) else {
        return false;
    };
    let has_block = raw
        .pointer("/controller/onOff")
        .is_some_and(|block| !block.is_null());
    match crate::json::decode_value::<LoopChannels>(channels.clone()) {
        Ok(channels) => has_block && on_off_shape(&channels),
        Err(_) => false,
    }
}

/// The shape a switching controller's channels take: one ON/OFF input on the manipulated variable, and one output on the process
/// variable, with the loop's setpoint as its setpoint when it names one.
fn on_off_shape(channels: &LoopChannels) -> bool {
    if channels.inputs.len() != 1 || channels.outputs.len() != 1 {
        return false;
    }
    let input = &channels.inputs[0];
    let output = &channels.outputs[0];
    input.signal_id == "manipulatedVariable"
        && input.actuator.kind == ActuatorKind::OnOff
        && output.signal_id == "processVariable"
        && output
            .setpoint_signal_id
            .as_deref()
            .is_none_or(|id| id == "setpoint")
}

/// The declared states of a switching controller: the `declared-state-v1` list, checked as a controller's states are (C0).
fn initialization(value: &Value) -> Result<ControllerInitialization, String> {
    let object = value
        .as_object()
        .ok_or("controller.onOff.initialization must be an object")?;
    if object.get("kind").and_then(Value::as_str) != Some("declared-state-v1") {
        return Err("controller.onOff.initialization.kind must be declared-state-v1".to_owned());
    }
    let states_value = object
        .get("states")
        .ok_or("controller.onOff.initialization.states must be a list")?;
    let states: Vec<InitializedState> = crate::json::decode_value(states_value.clone())
        .map_err(|error| format!("controller.onOff.initialization.states: {error}"))?;
    ControllerInitialization::new(states).map_err(|failure| failure.message)
}

/// A text field of the block that is not blank.
fn text_field<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| format!("controller.onOff.{name} must be text"))
}

/// A finite number field of the block.
fn finite_field(object: &Map<String, Value>, name: &str) -> Result<f64, String> {
    object
        .get(name)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("controller.onOff.{name} must be a finite number"))
}

#[cfg(test)]
#[path = "on_off_tests.rs"]
mod tests;
