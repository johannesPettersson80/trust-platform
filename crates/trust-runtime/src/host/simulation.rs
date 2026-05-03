//! Simulation configuration and scheduler hooks.

#![allow(missing_docs)]

use std::collections::VecDeque;
use std::path::Path;

use serde::Deserialize;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoAddress, IoSize};
use crate::memory::IoArea;
use crate::value::{Duration, Value};
use crate::Runtime;

#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub enabled: bool,
    pub seed: u64,
    pub time_scale: u32,
    pub couplings: Vec<SignalCouplingRule>,
    pub disturbances: Vec<SimulationDisturbance>,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            seed: 0,
            time_scale: 1,
            couplings: Vec::new(),
            disturbances: Vec::new(),
        }
    }
}

impl SimulationConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|err| {
            RuntimeError::InvalidConfig(
                format!(
                    "{}: failed to read simulation config: {err}",
                    path.display()
                )
                .into(),
            )
        })?;
        let raw: SimulationToml = toml::from_str(&text).map_err(|err| {
            RuntimeError::InvalidConfig(
                format!("{}: invalid simulation config: {err}", path.display()).into(),
            )
        })?;
        raw.into_config()
    }

    pub fn load_optional(path: impl AsRef<Path>) -> Result<Option<Self>, RuntimeError> {
        let path = path.as_ref();
        if !path.is_file() {
            return Ok(None);
        }
        Self::load(path).map(Some)
    }
}

#[derive(Debug, Clone)]
pub struct SignalCouplingRule {
    pub source: IoAddress,
    pub target: IoAddress,
    pub threshold: Option<f64>,
    pub delay: Duration,
    pub on_true: Option<Value>,
    pub on_false: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct SimulationDisturbance {
    pub at: Duration,
    pub kind: SimulationDisturbanceKind,
}

#[derive(Debug, Clone)]
pub enum SimulationDisturbanceKind {
    SetInput { target: IoAddress, value: Value },
    Fault { message: SmolStr },
}

#[derive(Debug, Clone)]
struct PendingEffect {
    due: Duration,
    sequence: u64,
    target: IoAddress,
    value: Value,
}

#[derive(Debug, Clone)]
pub struct SimulationController {
    config: SimulationConfig,
    disturbances: Vec<SimulationDisturbance>,
    disturbance_cursor: usize,
    pending_effects: VecDeque<PendingEffect>,
    next_sequence: u64,
    last_coupling_values: Vec<Option<Value>>,
}

impl SimulationController {
    pub fn new(config: SimulationConfig) -> Self {
        let last_coupling_values = vec![None; config.couplings.len()];
        Self {
            disturbances: config.disturbances.clone(),
            config,
            disturbance_cursor: 0,
            pending_effects: VecDeque::new(),
            next_sequence: 0,
            last_coupling_values,
        }
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    #[must_use]
    pub fn time_scale(&self) -> u32 {
        self.config.time_scale.max(1)
    }

    pub fn apply_pre_cycle(
        &mut self,
        now: Duration,
        runtime: &mut Runtime,
    ) -> Result<(), RuntimeError> {
        if !self.enabled() {
            return Ok(());
        }

        while self.disturbance_cursor < self.disturbances.len() {
            let disturbance = &self.disturbances[self.disturbance_cursor];
            if disturbance.at.as_nanos() > now.as_nanos() {
                break;
            }
            self.disturbance_cursor += 1;
            match &disturbance.kind {
                SimulationDisturbanceKind::SetInput { target, value } => {
                    if let Err(err) = runtime.io_mut().write(target, value.clone()) {
                        let msg = format!(
                            "simulation disturbance failed for {}: {err}",
                            format_io(target)
                        );
                        return Err(runtime.simulation_fault(msg));
                    }
                }
                SimulationDisturbanceKind::Fault { message } => {
                    return Err(runtime.simulation_fault(message.clone()));
                }
            }
        }

        while let Some(effect) = self.pending_effects.front() {
            if effect.due.as_nanos() > now.as_nanos() {
                break;
            }
            let effect = self
                .pending_effects
                .pop_front()
                .expect("front element must exist");
            if let Err(err) = runtime.io_mut().write(&effect.target, effect.value.clone()) {
                let msg = format!(
                    "simulation delayed effect failed for {}: {err}",
                    format_io(&effect.target)
                );
                return Err(runtime.simulation_fault(msg));
            }
        }

        Ok(())
    }

    pub fn apply_post_cycle(
        &mut self,
        now: Duration,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        if !self.enabled() {
            return Ok(());
        }

        for idx in 0..self.config.couplings.len() {
            let coupling = self.config.couplings[idx].clone();
            let source_value = runtime.io().read(&coupling.source)?;
            let next_value = derive_coupling_value(&coupling, &source_value)?;
            if self.last_coupling_values[idx].as_ref() == Some(&next_value) {
                continue;
            }
            self.last_coupling_values[idx] = Some(next_value.clone());
            let due =
                Duration::from_nanos(now.as_nanos().saturating_add(coupling.delay.as_nanos()));
            self.enqueue_effect(PendingEffect {
                due,
                sequence: self.next_sequence,
                target: coupling.target.clone(),
                value: next_value,
            });
            self.next_sequence = self.next_sequence.saturating_add(1);
        }
        Ok(())
    }

    fn enqueue_effect(&mut self, effect: PendingEffect) {
        self.pending_effects.push_back(effect);
        let mut effects = self.pending_effects.drain(..).collect::<Vec<_>>();
        effects.sort_by_key(|item| (item.due.as_nanos(), item.sequence));
        self.pending_effects = effects.into_iter().collect();
    }
}

#[derive(Debug, Deserialize)]
struct SimulationToml {
    simulation: Option<SimulationSection>,
    couplings: Option<Vec<CouplingSection>>,
    disturbances: Option<Vec<DisturbanceSection>>,
}

#[derive(Debug, Default, Deserialize)]
struct SimulationSection {
    enabled: Option<bool>,
    seed: Option<u64>,
    time_scale: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CouplingSection {
    source: String,
    target: String,
    threshold: Option<f64>,
    delay_ms: Option<u64>,
    on_true: Option<String>,
    on_false: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DisturbanceSection {
    at_ms: u64,
    kind: Option<String>,
    target: Option<String>,
    value: Option<String>,
    message: Option<String>,
}

impl SimulationToml {
    fn into_config(self) -> Result<SimulationConfig, RuntimeError> {
        let section = self.simulation.unwrap_or_default();
        let couplings = self
            .couplings
            .unwrap_or_default()
            .into_iter()
            .map(CouplingSection::into_rule)
            .collect::<Result<Vec<_>, _>>()?;
        let mut disturbances = self
            .disturbances
            .unwrap_or_default()
            .into_iter()
            .map(DisturbanceSection::into_disturbance)
            .collect::<Result<Vec<_>, _>>()?;
        disturbances.sort_by_key(|entry| entry.at.as_nanos());

        let has_rules = !couplings.is_empty() || !disturbances.is_empty();
        let enabled = section.enabled.unwrap_or(has_rules);
        let time_scale = section.time_scale.unwrap_or(1);
        if time_scale == 0 {
            return Err(RuntimeError::InvalidConfig(
                "simulation.time_scale must be >= 1".into(),
            ));
        }

        Ok(SimulationConfig {
            enabled,
            seed: section.seed.unwrap_or(0),
            time_scale,
            couplings,
            disturbances,
        })
    }
}

impl CouplingSection {
    fn into_rule(self) -> Result<SignalCouplingRule, RuntimeError> {
        let source = IoAddress::parse(self.source.as_str())?;
        let target = IoAddress::parse(self.target.as_str())?;
        if source.area != IoArea::Output {
            return Err(RuntimeError::InvalidConfig(
                format!("coupling source must be %Q*, got {}", self.source).into(),
            ));
        }
        if target.area != IoArea::Input {
            return Err(RuntimeError::InvalidConfig(
                format!("coupling target must be %I*, got {}", self.target).into(),
            ));
        }
        if self.threshold.is_none() && (self.on_true.is_some() || self.on_false.is_some()) {
            return Err(RuntimeError::InvalidConfig(
                "coupling on_true/on_false require threshold".into(),
            ));
        }
        let on_true = self
            .on_true
            .as_deref()
            .map(|text| parse_io_value(text, target.size))
            .transpose()?;
        let on_false = self
            .on_false
            .as_deref()
            .map(|text| parse_io_value(text, target.size))
            .transpose()?;
        Ok(SignalCouplingRule {
            source,
            target,
            threshold: self.threshold,
            delay: Duration::from_millis(self.delay_ms.unwrap_or(0) as i64),
            on_true,
            on_false,
        })
    }
}

impl DisturbanceSection {
    fn into_disturbance(self) -> Result<SimulationDisturbance, RuntimeError> {
        let at = Duration::from_millis(self.at_ms as i64);
        let kind_name = self.kind.unwrap_or_else(|| "set".to_string());
        if kind_name.eq_ignore_ascii_case("fault") {
            let message = self
                .message
                .unwrap_or_else(|| "simulated fault injection".to_string());
            return Ok(SimulationDisturbance {
                at,
                kind: SimulationDisturbanceKind::Fault {
                    message: SmolStr::new(message),
                },
            });
        }
        if !kind_name.eq_ignore_ascii_case("set") {
            return Err(RuntimeError::InvalidConfig(
                format!("unsupported disturbance kind '{kind_name}'").into(),
            ));
        }
        let target_text = self.target.ok_or_else(|| {
            RuntimeError::InvalidConfig("disturbance target required for kind=set".into())
        })?;
        let target = IoAddress::parse(target_text.as_str())?;
        if target.area != IoArea::Input {
            return Err(RuntimeError::InvalidConfig(
                format!("disturbance target must be %I*, got {target_text}").into(),
            ));
        }
        let value_text = self.value.ok_or_else(|| {
            RuntimeError::InvalidConfig("disturbance value required for kind=set".into())
        })?;
        let value = parse_io_value(value_text.as_str(), target.size)?;
        Ok(SimulationDisturbance {
            at,
            kind: SimulationDisturbanceKind::SetInput { target, value },
        })
    }
}

fn derive_coupling_value(rule: &SignalCouplingRule, source: &Value) -> Result<Value, RuntimeError> {
    if let Some(threshold) = rule.threshold {
        let number = value_to_f64(source).unwrap_or(0.0);
        if number >= threshold {
            return Ok(rule
                .on_true
                .clone()
                .unwrap_or_else(|| default_value_for_size(rule.target.size, true)));
        }
        return Ok(rule
            .on_false
            .clone()
            .unwrap_or_else(|| default_value_for_size(rule.target.size, false)));
    }
    coerce_to_size(source, rule.target.size)
}

fn coerce_to_size(source: &Value, target_size: IoSize) -> Result<Value, RuntimeError> {
    match target_size {
        IoSize::Bit => Ok(Value::Bool(value_to_bool(source))),
        IoSize::Byte => Ok(Value::Byte(value_to_u64(source)? as u8)),
        IoSize::Word => Ok(Value::Word(value_to_u64(source)? as u16)),
        IoSize::DWord => Ok(Value::DWord(value_to_u64(source)? as u32)),
        IoSize::LWord => Ok(Value::LWord(value_to_u64(source)?)),
    }
}

fn value_to_bool(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        _ => value_to_f64(value).unwrap_or(0.0) != 0.0,
    }
}

fn value_to_u64(value: &Value) -> Result<u64, RuntimeError> {
    let number = value_to_f64(value).ok_or(RuntimeError::TypeMismatch)?;
    if !number.is_finite() || number < 0.0 {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok(number as u64)
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        Value::SInt(v) => Some(*v as f64),
        Value::Int(v) => Some(*v as f64),
        Value::DInt(v) => Some(*v as f64),
        Value::LInt(v) => Some(*v as f64),
        Value::USInt(v) => Some(*v as f64),
        Value::UInt(v) => Some(*v as f64),
        Value::UDInt(v) => Some(*v as f64),
        Value::ULInt(v) => Some(*v as f64),
        Value::Real(v) => Some(*v as f64),
        Value::LReal(v) => Some(*v),
        Value::Byte(v) => Some(*v as f64),
        Value::Word(v) => Some(*v as f64),
        Value::DWord(v) => Some(*v as f64),
        Value::LWord(v) => Some(*v as f64),
        _ => None,
    }
}

fn parse_io_value(text: &str, size: IoSize) -> Result<Value, RuntimeError> {
    let trimmed = text.trim();
    let upper = trimmed.to_ascii_uppercase();
    match size {
        IoSize::Bit => match upper.as_str() {
            "TRUE" | "1" => Ok(Value::Bool(true)),
            "FALSE" | "0" => Ok(Value::Bool(false)),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid BOOL simulation value '{trimmed}'").into(),
            )),
        },
        IoSize::Byte => Ok(Value::Byte(parse_u64(trimmed)? as u8)),
        IoSize::Word => Ok(Value::Word(parse_u64(trimmed)? as u16)),
        IoSize::DWord => Ok(Value::DWord(parse_u64(trimmed)? as u32)),
        IoSize::LWord => Ok(Value::LWord(parse_u64(trimmed)?)),
    }
}

fn parse_u64(text: &str) -> Result<u64, RuntimeError> {
    let trimmed = text.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).map_err(|err| {
            RuntimeError::InvalidConfig(format!("invalid hex value '{trimmed}': {err}").into())
        });
    }
    trimmed.parse::<u64>().map_err(|err| {
        RuntimeError::InvalidConfig(format!("invalid numeric value '{trimmed}': {err}").into())
    })
}

fn default_value_for_size(size: IoSize, active: bool) -> Value {
    match size {
        IoSize::Bit => Value::Bool(active),
        IoSize::Byte => Value::Byte(if active { 1 } else { 0 }),
        IoSize::Word => Value::Word(if active { 1 } else { 0 }),
        IoSize::DWord => Value::DWord(if active { 1 } else { 0 }),
        IoSize::LWord => Value::LWord(if active { 1 } else { 0 }),
    }
}

fn format_io(address: &IoAddress) -> String {
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
    if address.size == IoSize::Bit {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}
