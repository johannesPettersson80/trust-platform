//! Deterministic, independently seeded non-ideal effects for plant channels.

use serde::Deserialize;

use crate::error::RuntimeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Command,
    ProcessInput,
    State,
    ProcessOutput,
    Measurement,
    Parameter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Add,
    Multiply,
    Replace,
    Hold,
}

#[derive(Debug, Clone)]
enum Generator {
    UniformWhite {
        minimum: f64,
        maximum: f64,
    },
    Constant {
        value: f64,
    },
    Pulse {
        value: f64,
        duration_s: f64,
    },
    Sine {
        amplitude: f64,
        frequency_hz: f64,
        phase_rad: f64,
        bias: f64,
    },
    Quantise {
        quantum: f64,
    },
    Clip {
        minimum: f64,
        maximum: f64,
    },
    Backlash {
        width: f64,
    },
    NormalApproxWhite {
        mean: f64,
        standard_deviation: f64,
    },
    ColoredNoise {
        standard_deviation: f64,
        correlation_time_s: f64,
    },
    RandomWalk {
        diffusion_per_sqrt_s: f64,
        minimum: Option<f64>,
        maximum: Option<f64>,
    },
    Dropout {
        rate_per_s: f64,
        duration: DropoutDuration,
        policy: DropoutPolicy,
    },
}

#[derive(Debug, Clone, Copy)]
enum DropoutDuration {
    Fixed(f64),
    Range { minimum: f64, maximum: f64 },
}

#[derive(Debug, Clone)]
enum DropoutPolicy {
    HoldLast,
    Replace(f64),
    Fault,
}

#[derive(Debug, Clone)]
pub(super) struct UncertaintyConfig {
    id: String,
    target: Target,
    mode: Mode,
    channel: usize,
    parameter: Option<String>,
    start_s: f64,
    end_s: Option<f64>,
    generator: Generator,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UncertaintySection {
    id: String,
    target: String,
    mode: String,
    generator: String,
    channel: Option<usize>,
    parameter: Option<String>,
    start_s: Option<f64>,
    end_s: Option<f64>,
    minimum: Option<f64>,
    maximum: Option<f64>,
    value: Option<f64>,
    quantum: Option<f64>,
    mean: Option<f64>,
    standard_deviation: Option<f64>,
    correlation_time_s: Option<f64>,
    diffusion_per_sqrt_s: Option<f64>,
    rate_per_s: Option<f64>,
    duration_s: Option<f64>,
    duration_min_s: Option<f64>,
    duration_max_s: Option<f64>,
    dropout_policy: Option<String>,
    replacement_value: Option<f64>,
    amplitude: Option<f64>,
    frequency_hz: Option<f64>,
    phase_rad: Option<f64>,
    bias: Option<f64>,
    width: Option<f64>,
}

impl UncertaintySection {
    pub(super) fn into_config(self) -> Result<UncertaintyConfig, RuntimeError> {
        if !valid_id(&self.id) {
            return invalid("plant uncertainty id must match [A-Za-z][A-Za-z0-9_-]*");
        }
        let target = match self.target.as_str() {
            "command" => Target::Command,
            "process_input" => Target::ProcessInput,
            "state" => Target::State,
            "process_output" => Target::ProcessOutput,
            "measurement" => Target::Measurement,
            "parameter" => Target::Parameter,
            other => return invalid(format!("unsupported uncertainty target '{other}'")),
        };
        let mode = match self.mode.as_str() {
            "add" => Mode::Add,
            "multiply" => Mode::Multiply,
            "replace" => Mode::Replace,
            "hold" => Mode::Hold,
            other => return invalid(format!("unsupported uncertainty mode '{other}'")),
        };
        let (channel, parameter) = match target {
            Target::Command
            | Target::ProcessInput
            | Target::State
            | Target::ProcessOutput
            | Target::Measurement => {
                let channel = self.channel.ok_or_else(|| {
                    RuntimeError::InvalidConfig("uncertainty channel is required".into())
                })?;
                if self.parameter.is_some() {
                    return invalid("channel uncertainty must not declare parameter");
                }
                (channel, None)
            }
            Target::Parameter => {
                let parameter = self.parameter.ok_or_else(|| {
                    RuntimeError::InvalidConfig("parameter uncertainty requires parameter".into())
                })?;
                if self.channel.is_some() {
                    return invalid("parameter uncertainty must not declare channel");
                }
                (0, Some(parameter))
            }
        };
        if mode == Mode::Hold
            && !matches!(
                target,
                Target::ProcessInput | Target::ProcessOutput | Target::Measurement
            )
        {
            return invalid(
                "uncertainty mode 'hold' is valid only for process_input, process_output, or measurement",
            );
        }
        let start_s = nonnegative("uncertainty.start_s", self.start_s.unwrap_or(0.0))?;
        let end_s = self
            .end_s
            .map(|value| nonnegative("uncertainty.end_s", value))
            .transpose()?;
        if end_s.is_some_and(|end| end < start_s) {
            return invalid("uncertainty.end_s must not precede start_s");
        }
        let generator = match self.generator.as_str() {
            "uniform_white" => {
                let minimum = finite(
                    "uncertainty.minimum",
                    self.minimum.ok_or_else(|| {
                        RuntimeError::InvalidConfig(
                            "uniform_white requires uncertainty.minimum".into(),
                        )
                    })?,
                )?;
                let maximum = finite(
                    "uncertainty.maximum",
                    self.maximum.ok_or_else(|| {
                        RuntimeError::InvalidConfig(
                            "uniform_white requires uncertainty.maximum".into(),
                        )
                    })?,
                )?;
                if minimum >= maximum {
                    return invalid("uniform_white minimum must be less than maximum");
                }
                Generator::UniformWhite { minimum, maximum }
            }
            "constant" => Generator::Constant {
                value: finite(
                    "uncertainty.value",
                    self.value.ok_or_else(|| {
                        RuntimeError::InvalidConfig("constant requires uncertainty.value".into())
                    })?,
                )?,
            },
            "pulse" => {
                let duration_s = finite(
                    "uncertainty.duration_s",
                    self.duration_s.ok_or_else(|| {
                        RuntimeError::InvalidConfig("pulse requires duration_s".into())
                    })?,
                )?;
                if duration_s <= 0.0 {
                    return invalid("pulse duration_s must be > 0");
                }
                Generator::Pulse {
                    value: finite(
                        "uncertainty.value",
                        self.value.ok_or_else(|| {
                            RuntimeError::InvalidConfig("pulse requires value".into())
                        })?,
                    )?,
                    duration_s,
                }
            }
            "sine" => Generator::Sine {
                amplitude: finite(
                    "uncertainty.amplitude",
                    self.amplitude.ok_or_else(|| {
                        RuntimeError::InvalidConfig("sine requires amplitude".into())
                    })?,
                )?,
                frequency_hz: nonnegative(
                    "uncertainty.frequency_hz",
                    self.frequency_hz.ok_or_else(|| {
                        RuntimeError::InvalidConfig("sine requires frequency_hz".into())
                    })?,
                )?,
                phase_rad: finite(
                    "uncertainty.phase_rad",
                    self.phase_rad.ok_or_else(|| {
                        RuntimeError::InvalidConfig("sine requires phase_rad".into())
                    })?,
                )?,
                bias: finite("uncertainty.bias", self.bias.unwrap_or(0.0))?,
            },
            "quantise" => {
                if mode != Mode::Replace {
                    return invalid("quantise requires uncertainty mode 'replace'");
                }
                let quantum = finite(
                    "uncertainty.quantum",
                    self.quantum.ok_or_else(|| {
                        RuntimeError::InvalidConfig("quantise requires uncertainty.quantum".into())
                    })?,
                )?;
                if quantum <= 0.0 {
                    return invalid("uncertainty.quantum must be > 0");
                }
                Generator::Quantise { quantum }
            }
            "clip" => {
                if mode != Mode::Replace {
                    return invalid("clip requires uncertainty mode 'replace'");
                }
                let minimum = finite(
                    "uncertainty.minimum",
                    self.minimum.ok_or_else(|| {
                        RuntimeError::InvalidConfig("clip requires uncertainty.minimum".into())
                    })?,
                )?;
                let maximum = finite(
                    "uncertainty.maximum",
                    self.maximum.ok_or_else(|| {
                        RuntimeError::InvalidConfig("clip requires uncertainty.maximum".into())
                    })?,
                )?;
                if minimum > maximum {
                    return invalid("clip minimum must not exceed maximum");
                }
                Generator::Clip { minimum, maximum }
            }
            "backlash" => {
                if target != Target::ProcessOutput || mode != Mode::Replace {
                    return invalid("backlash requires process_output target and replace mode");
                }
                let width = finite(
                    "uncertainty.width",
                    self.width.ok_or_else(|| {
                        RuntimeError::InvalidConfig("backlash requires uncertainty.width".into())
                    })?,
                )?;
                if width <= 0.0 {
                    return invalid("uncertainty.width must be > 0");
                }
                Generator::Backlash { width }
            }
            "normal_approx_white" => Generator::NormalApproxWhite {
                mean: finite("uncertainty.mean", self.mean.unwrap_or(0.0))?,
                standard_deviation: nonnegative(
                    "uncertainty.standard_deviation",
                    self.standard_deviation.ok_or_else(|| {
                        RuntimeError::InvalidConfig(
                            "normal_approx_white requires standard_deviation".into(),
                        )
                    })?,
                )?,
            },
            "colored_noise" => {
                let correlation_time_s = finite(
                    "uncertainty.correlation_time_s",
                    self.correlation_time_s.ok_or_else(|| {
                        RuntimeError::InvalidConfig(
                            "colored_noise requires correlation_time_s".into(),
                        )
                    })?,
                )?;
                if correlation_time_s <= 0.0 {
                    return invalid("uncertainty.correlation_time_s must be > 0");
                }
                Generator::ColoredNoise {
                    standard_deviation: nonnegative(
                        "uncertainty.standard_deviation",
                        self.standard_deviation.ok_or_else(|| {
                            RuntimeError::InvalidConfig(
                                "colored_noise requires standard_deviation".into(),
                            )
                        })?,
                    )?,
                    correlation_time_s,
                }
            }
            "random_walk" => {
                let minimum = self
                    .minimum
                    .map(|value| finite("uncertainty.minimum", value))
                    .transpose()?;
                let maximum = self
                    .maximum
                    .map(|value| finite("uncertainty.maximum", value))
                    .transpose()?;
                if minimum.zip(maximum).is_some_and(|(low, high)| low > high) {
                    return invalid("random_walk minimum must not exceed maximum");
                }
                Generator::RandomWalk {
                    diffusion_per_sqrt_s: nonnegative(
                        "uncertainty.diffusion_per_sqrt_s",
                        self.diffusion_per_sqrt_s.ok_or_else(|| {
                            RuntimeError::InvalidConfig(
                                "random_walk requires diffusion_per_sqrt_s".into(),
                            )
                        })?,
                    )?,
                    minimum,
                    maximum,
                }
            }
            "dropout" => {
                if target != Target::Measurement || mode != Mode::Replace {
                    return invalid("dropout requires measurement target and replace mode");
                }
                let rate_per_s = nonnegative(
                    "uncertainty.rate_per_s",
                    self.rate_per_s.ok_or_else(|| {
                        RuntimeError::InvalidConfig("dropout requires rate_per_s".into())
                    })?,
                )?;
                let duration = match (self.duration_s, self.duration_min_s, self.duration_max_s) {
                    (Some(value), None, None) => {
                        let value = finite("uncertainty.duration_s", value)?;
                        if value <= 0.0 {
                            return invalid("uncertainty.duration_s must be > 0");
                        }
                        DropoutDuration::Fixed(value)
                    }
                    (None, Some(minimum), Some(maximum)) => {
                        let minimum = finite("uncertainty.duration_min_s", minimum)?;
                        let maximum = finite("uncertainty.duration_max_s", maximum)?;
                        if minimum <= 0.0 || maximum <= 0.0 || minimum > maximum {
                            return invalid("dropout duration range must be positive and ordered");
                        }
                        DropoutDuration::Range { minimum, maximum }
                    }
                    _ => return invalid("dropout requires either fixed or ranged duration"),
                };
                let policy = match self.dropout_policy.as_deref() {
                    Some("hold_last") if self.replacement_value.is_none() => {
                        DropoutPolicy::HoldLast
                    }
                    Some("replace") => DropoutPolicy::Replace(finite(
                        "uncertainty.replacement_value",
                        self.replacement_value.ok_or_else(|| {
                            RuntimeError::InvalidConfig(
                                "replace dropout requires replacement_value".into(),
                            )
                        })?,
                    )?),
                    Some("fault") if self.replacement_value.is_none() => DropoutPolicy::Fault,
                    Some(other) => {
                        return invalid(format!("invalid dropout policy or fields '{other}'"))
                    }
                    None => return invalid("dropout requires dropout_policy"),
                };
                Generator::Dropout {
                    rate_per_s,
                    duration,
                    policy,
                }
            }
            other => return invalid(format!("unsupported uncertainty generator '{other}'")),
        };
        Ok(UncertaintyConfig {
            id: self.id,
            target,
            mode,
            channel,
            parameter,
            start_s,
            end_s,
            generator,
        })
    }
}

impl UncertaintyConfig {
    pub(super) fn id_for_validation(&self) -> &str {
        &self.id
    }

    pub(super) fn validate_channel(
        &self,
        command_count: usize,
        state_count: usize,
        output_count: usize,
    ) -> Result<(), RuntimeError> {
        let limit = match self.target {
            Target::Command | Target::ProcessInput => command_count,
            Target::State => state_count,
            Target::ProcessOutput | Target::Measurement => output_count,
            Target::Parameter => return Ok(()),
        };
        if self.channel >= limit {
            return invalid(format!(
                "uncertainty '{}' channel {} is out of range for {} channels",
                self.id, self.channel, limit
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(super) struct UncertaintyState {
    config: UncertaintyConfig,
    random: SplitMix64,
    generated_state: f64,
    dropout_until_s: Option<f64>,
    last_valid_measurement: Option<f64>,
    held_value: Option<f64>,
}

impl UncertaintyState {
    pub(super) fn new(config: UncertaintyConfig, master_seed: u64, plant_id: &str) -> Self {
        let random = SplitMix64 {
            state: derive_stream_seed(master_seed, plant_id, &config.id),
        };
        Self {
            config,
            random,
            generated_state: 0.0,
            dropout_until_s: None,
            last_valid_measurement: None,
            held_value: None,
        }
    }

    pub(super) fn parameter_name(&self) -> Option<&str> {
        (self.config.target == Target::Parameter)
            .then_some(self.config.parameter.as_deref())
            .flatten()
    }

    pub(super) fn id(&self) -> &str {
        &self.config.id
    }

    /// Copies only mutable generator state. Configuration owns strings and is
    /// immutable, so preserving it avoids allocation in substep checkpoints.
    pub(super) fn copy_runtime_from(&mut self, source: &Self) {
        debug_assert_eq!(self.config.id, source.config.id);
        self.random.state = source.random.state;
        self.generated_state = source.generated_state;
        self.dropout_until_s = source.dropout_until_s;
        self.last_valid_measurement = source.last_valid_measurement;
        self.held_value = source.held_value;
    }

    pub(super) fn is_active(&self, time_s: f64) -> bool {
        time_s >= self.config.start_s && self.config.end_s.is_none_or(|end| time_s < end)
    }

    pub(super) fn dropout_active(&self, time_s: f64) -> bool {
        matches!(self.config.generator, Generator::Dropout { .. })
            && self.dropout_until_s.is_some_and(|until| time_s < until)
    }

    pub(super) fn initialize_measurement(&mut self, channel: usize, value: f64) -> f64 {
        if self.config.target != Target::Measurement || self.config.channel != channel {
            return value;
        }
        if self.config.mode == Mode::Hold {
            self.held_value = Some(value);
            return value;
        }
        let initialized = match self.config.generator {
            Generator::Constant { value: generated } => combine(self.config.mode, value, generated),
            Generator::Pulse {
                value: generated, ..
            } => combine(self.config.mode, value, generated),
            Generator::Sine {
                amplitude,
                phase_rad,
                bias,
                ..
            } => combine(self.config.mode, value, bias + amplitude * phase_rad.sin()),
            Generator::Quantise { quantum } => (value / quantum).round_ties_even() * quantum,
            Generator::Clip { minimum, maximum } => value.clamp(minimum, maximum),
            _ => value,
        };
        if matches!(self.config.generator, Generator::Dropout { .. }) {
            self.last_valid_measurement = Some(initialized);
        }
        initialized
    }

    pub(super) fn initialize_process_output(&mut self, channel: usize, value: f64) {
        if self.config.target == Target::ProcessOutput && self.config.channel == channel {
            if self.config.mode == Mode::Hold {
                self.held_value = Some(value);
            }
            if matches!(self.config.generator, Generator::Backlash { .. }) {
                self.generated_state = value;
            }
        }
    }

    pub(super) fn apply_measurement(
        &mut self,
        channel: usize,
        value: f64,
        time_s: f64,
        step_s: f64,
    ) -> Result<f64, RuntimeError> {
        self.apply_channel(Target::Measurement, channel, value, time_s, step_s)
    }

    pub(super) fn apply_command(
        &mut self,
        channel: usize,
        value: f64,
        time_s: f64,
        step_s: f64,
    ) -> Result<f64, RuntimeError> {
        self.apply_channel(Target::Command, channel, value, time_s, step_s)
    }

    pub(super) fn apply_process_input(
        &mut self,
        channel: usize,
        value: f64,
        time_s: f64,
        step_s: f64,
    ) -> Result<f64, RuntimeError> {
        self.apply_channel(Target::ProcessInput, channel, value, time_s, step_s)
    }

    pub(super) fn apply_process_output(
        &mut self,
        channel: usize,
        value: f64,
        time_s: f64,
        step_s: f64,
    ) -> Result<f64, RuntimeError> {
        self.apply_channel(Target::ProcessOutput, channel, value, time_s, step_s)
    }

    pub(super) fn apply_state(
        &mut self,
        channel: usize,
        value: f64,
        time_s: f64,
        step_s: f64,
    ) -> Result<f64, RuntimeError> {
        self.apply_channel(Target::State, channel, value, time_s, step_s)
    }

    fn apply_channel(
        &mut self,
        target: Target,
        channel: usize,
        value: f64,
        time_s: f64,
        step_s: f64,
    ) -> Result<f64, RuntimeError> {
        if self.config.target != target || self.config.channel != channel {
            return Ok(value);
        }
        let active =
            time_s >= self.config.start_s && self.config.end_s.is_none_or(|end| time_s < end);
        if self.config.mode == Mode::Hold {
            if !active {
                self.held_value = Some(value);
                return Ok(value);
            }
            return self.held_value.ok_or_else(|| {
                RuntimeError::SimulationFault(
                    format!("hold uncertainty '{}' has no prior value", self.config.id).into(),
                )
            });
        }
        if !active {
            if matches!(self.config.generator, Generator::Backlash { .. }) {
                // Following the input while disabled prevents stale clearance
                // from producing an artificial jump when the effect starts.
                self.generated_state = value;
            }
            return Ok(value);
        }
        if matches!(
            self.config.generator,
            Generator::Pulse { duration_s, .. }
                if time_s >= self.config.start_s + duration_s
        ) {
            return Ok(value);
        }
        let generated = match self.config.generator {
            Generator::UniformWhite { minimum, maximum } => {
                minimum + (maximum - minimum) * self.random.uniform_0_1()
            }
            Generator::Constant { value } => value,
            Generator::Pulse { value, .. } => value,
            Generator::Sine {
                amplitude,
                frequency_hz,
                phase_rad,
                bias,
            } => {
                bias + amplitude
                    * (2.0 * std::f64::consts::PI * frequency_hz * time_s + phase_rad).sin()
            }
            Generator::Quantise { quantum } => (value / quantum).round_ties_even() * quantum,
            Generator::Clip { minimum, maximum } => value.clamp(minimum, maximum),
            Generator::Backlash { width } => {
                let half_width = 0.5 * width;
                self.generated_state = self
                    .generated_state
                    .clamp(value - half_width, value + half_width);
                self.generated_state
            }
            Generator::NormalApproxWhite {
                mean,
                standard_deviation,
            } => {
                if standard_deviation == 0.0 {
                    mean
                } else {
                    mean + standard_deviation * self.random.normal_approx()
                }
            }
            Generator::ColoredNoise {
                standard_deviation,
                correlation_time_s,
            } => {
                let a = (-step_s / correlation_time_s).exp();
                self.generated_state = a * self.generated_state
                    + standard_deviation * (1.0 - a * a).sqrt() * self.random.normal_approx();
                self.generated_state
            }
            Generator::RandomWalk {
                diffusion_per_sqrt_s,
                minimum,
                maximum,
            } => {
                self.generated_state +=
                    diffusion_per_sqrt_s * step_s.sqrt() * self.random.normal_approx();
                if let Some(minimum) = minimum {
                    self.generated_state = self.generated_state.max(minimum);
                }
                if let Some(maximum) = maximum {
                    self.generated_state = self.generated_state.min(maximum);
                }
                self.generated_state
            }
            Generator::Dropout {
                rate_per_s,
                duration,
                ref policy,
            } => {
                debug_assert_eq!(target, Target::Measurement);
                if self.dropout_until_s.is_some_and(|until| time_s < until) {
                    return dropout_value(policy, self.last_valid_measurement, &self.config.id);
                }
                self.dropout_until_s = None;
                let begins = self.random.uniform_0_1() < 1.0 - (-rate_per_s * step_s).exp();
                if begins {
                    let duration_s = match duration {
                        DropoutDuration::Fixed(value) => value,
                        DropoutDuration::Range { minimum, maximum } => {
                            minimum + (maximum - minimum) * self.random.uniform_0_1()
                        }
                    };
                    self.dropout_until_s = Some(time_s + duration_s);
                    return dropout_value(policy, self.last_valid_measurement, &self.config.id);
                }
                self.last_valid_measurement = Some(value);
                return Ok(value);
            }
        };
        Ok(combine(self.config.mode, value, generated))
    }

    pub(super) fn apply_parameter(&mut self, value: f64, time_s: f64) -> f64 {
        if self.config.target != Target::Parameter
            || time_s < self.config.start_s
            || self.config.end_s.is_some_and(|end| time_s >= end)
        {
            return value;
        }
        if matches!(
            self.config.generator,
            Generator::Pulse { duration_s, .. }
                if time_s >= self.config.start_s + duration_s
        ) {
            return value;
        }
        let generated = match self.config.generator {
            Generator::UniformWhite { minimum, maximum } => {
                minimum + (maximum - minimum) * self.random.uniform_0_1()
            }
            Generator::Constant { value } => value,
            Generator::Pulse {
                value: generated, ..
            } => generated,
            Generator::Sine {
                amplitude,
                frequency_hz,
                phase_rad,
                bias,
            } => {
                bias + amplitude
                    * (2.0 * std::f64::consts::PI * frequency_hz * time_s + phase_rad).sin()
            }
            Generator::Quantise { quantum } => (value / quantum).round_ties_even() * quantum,
            Generator::Clip { minimum, maximum } => value.clamp(minimum, maximum),
            Generator::Backlash { .. } => return value,
            Generator::NormalApproxWhite {
                mean,
                standard_deviation,
            } => {
                if standard_deviation == 0.0 {
                    mean
                } else {
                    mean + standard_deviation * self.random.normal_approx()
                }
            }
            Generator::ColoredNoise { .. } | Generator::RandomWalk { .. } => return value,
            Generator::Dropout { .. } => return value,
        };
        combine(self.config.mode, value, generated)
    }
}

fn dropout_value(
    policy: &DropoutPolicy,
    last_valid: Option<f64>,
    effect_id: &str,
) -> Result<f64, RuntimeError> {
    match policy {
        DropoutPolicy::HoldLast => last_valid.ok_or_else(|| {
            RuntimeError::SimulationFault(
                format!("dropout uncertainty '{effect_id}' has no valid measurement to hold")
                    .into(),
            )
        }),
        DropoutPolicy::Replace(value) => Ok(*value),
        DropoutPolicy::Fault => Err(RuntimeError::SimulationFault(
            format!("dropout uncertainty '{effect_id}' entered fault policy").into(),
        )),
    }
}

fn combine(mode: Mode, value: f64, generated: f64) -> f64 {
    match mode {
        Mode::Add => value + generated,
        Mode::Multiply => value * generated,
        Mode::Replace => generated,
        Mode::Hold => value,
    }
}

#[derive(Debug, Clone)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn uniform_0_1(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) * (1.0 / ((1_u64 << 53) as f64))
    }

    fn normal_approx(&mut self) -> f64 {
        (0..12).map(|_| self.uniform_0_1()).sum::<f64>() - 6.0
    }
}

fn derive_stream_seed(master_seed: u64, plant_id: &str, uncertainty_id: &str) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    for byte in plant_id
        .bytes()
        .chain(std::iter::once(0))
        .chain(uncertainty_id.bytes())
        .chain(std::iter::once(0))
        .chain("splitmix64-v1".bytes())
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash ^ master_seed
}

fn valid_id(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn finite(name: &str, value: f64) -> Result<f64, RuntimeError> {
    if !value.is_finite() {
        return invalid(format!("{name} must be finite"));
    }
    Ok(value)
}

fn nonnegative(name: &str, value: f64) -> Result<f64, RuntimeError> {
    let value = finite(name, value)?;
    if value < 0.0 {
        return invalid(format!("{name} must be >= 0"));
    }
    Ok(value)
}

fn invalid<T>(message: impl Into<String>) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidConfig(message.into().into()))
}
