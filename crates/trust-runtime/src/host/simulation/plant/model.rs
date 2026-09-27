//! Built-in plant model state equations and fixed-step integrators.

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub(super) enum PlantModelConfig {
    StaticGain {
        gain: f64,
        input_bias: f64,
        output_bias: f64,
    },
    FirstOrder {
        gain: f64,
        time_constant_s: f64,
        input_bias: f64,
        output_bias: f64,
    },
    FirstOrderDeadTime {
        gain: f64,
        time_constant_s: f64,
        input_bias: f64,
        output_bias: f64,
        dead_time_s: f64,
        initial_delayed_input: f64,
    },
    Integrator {
        gain: f64,
        input_bias: f64,
        output_bias: f64,
    },
    IntegratorDeadTime {
        gain: f64,
        input_bias: f64,
        output_bias: f64,
        dead_time_s: f64,
        initial_delayed_input: f64,
    },
    GravityTank {
        area_m2: f64,
        outflow_coefficient: f64,
    },
    GravityTankCascade {
        area_m2: f64,
        outflow_coefficient: f64,
    },
    RobotJoint {
        inertia: f64,
        gravity_torque: f64,
        viscous_friction: f64,
        coulomb_friction: f64,
        friction_smoothing: f64,
    },
    PiecewiseLinear {
        input_points: Vec<f64>,
        output_points: Vec<f64>,
    },
    SecondOrder {
        gain: f64,
        natural_frequency: f64,
        damping_ratio: f64,
        input_bias: f64,
        output_bias: f64,
    },
    StateSpace {
        a: Vec<Vec<f64>>,
        b: Vec<Vec<f64>>,
        c: Vec<Vec<f64>>,
        d: Vec<Vec<f64>>,
    },
}

impl PlantModelConfig {
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::StaticGain { .. } => "static_gain",
            Self::FirstOrder { .. } => "first_order",
            Self::FirstOrderDeadTime { .. } => "first_order_dead_time",
            Self::Integrator { .. } => "integrator",
            Self::IntegratorDeadTime { .. } => "integrator_dead_time",
            Self::GravityTank { .. } => "gravity_tank",
            Self::GravityTankCascade { .. } => "gravity_tank_cascade",
            Self::RobotJoint { .. } => "robot_joint",
            Self::PiecewiseLinear { .. } => "piecewise_linear",
            Self::SecondOrder { .. } => "second_order",
            Self::StateSpace { .. } => "state_space",
        }
    }

    /// Restores values changed by events or uncertainty while retaining all
    /// matrix and lookup-table allocations created during activation.
    pub(super) fn reset_values_from(&mut self, nominal: &Self) {
        match (self, nominal) {
            (
                Self::StaticGain {
                    gain,
                    input_bias,
                    output_bias,
                },
                Self::StaticGain {
                    gain: source_gain,
                    input_bias: source_input_bias,
                    output_bias: source_output_bias,
                },
            )
            | (
                Self::Integrator {
                    gain,
                    input_bias,
                    output_bias,
                },
                Self::Integrator {
                    gain: source_gain,
                    input_bias: source_input_bias,
                    output_bias: source_output_bias,
                },
            ) => {
                *gain = *source_gain;
                *input_bias = *source_input_bias;
                *output_bias = *source_output_bias;
            }
            (
                Self::IntegratorDeadTime {
                    gain,
                    input_bias,
                    output_bias,
                    dead_time_s,
                    initial_delayed_input,
                },
                Self::IntegratorDeadTime {
                    gain: source_gain,
                    input_bias: source_input_bias,
                    output_bias: source_output_bias,
                    dead_time_s: source_dead_time,
                    initial_delayed_input: source_initial_delayed_input,
                },
            ) => {
                *gain = *source_gain;
                *input_bias = *source_input_bias;
                *output_bias = *source_output_bias;
                *dead_time_s = *source_dead_time;
                *initial_delayed_input = *source_initial_delayed_input;
            }
            (
                Self::GravityTank {
                    area_m2,
                    outflow_coefficient,
                },
                Self::GravityTank {
                    area_m2: source_area,
                    outflow_coefficient: source_outflow,
                },
            )
            | (
                Self::GravityTankCascade {
                    area_m2,
                    outflow_coefficient,
                },
                Self::GravityTankCascade {
                    area_m2: source_area,
                    outflow_coefficient: source_outflow,
                },
            ) => {
                *area_m2 = *source_area;
                *outflow_coefficient = *source_outflow;
            }
            (
                Self::RobotJoint {
                    inertia,
                    gravity_torque,
                    viscous_friction,
                    coulomb_friction,
                    friction_smoothing,
                },
                Self::RobotJoint {
                    inertia: source_inertia,
                    gravity_torque: source_gravity,
                    viscous_friction: source_viscous,
                    coulomb_friction: source_coulomb,
                    friction_smoothing: source_smoothing,
                },
            ) => {
                *inertia = *source_inertia;
                *gravity_torque = *source_gravity;
                *viscous_friction = *source_viscous;
                *coulomb_friction = *source_coulomb;
                *friction_smoothing = *source_smoothing;
            }
            (
                Self::FirstOrder {
                    gain,
                    time_constant_s,
                    input_bias,
                    output_bias,
                },
                Self::FirstOrder {
                    gain: source_gain,
                    time_constant_s: source_time_constant,
                    input_bias: source_input_bias,
                    output_bias: source_output_bias,
                },
            ) => {
                *gain = *source_gain;
                *time_constant_s = *source_time_constant;
                *input_bias = *source_input_bias;
                *output_bias = *source_output_bias;
            }
            (
                Self::FirstOrderDeadTime {
                    gain,
                    time_constant_s,
                    input_bias,
                    output_bias,
                    dead_time_s,
                    initial_delayed_input,
                },
                Self::FirstOrderDeadTime {
                    gain: source_gain,
                    time_constant_s: source_time_constant,
                    input_bias: source_input_bias,
                    output_bias: source_output_bias,
                    dead_time_s: source_dead_time,
                    initial_delayed_input: source_initial_delayed_input,
                },
            ) => {
                *gain = *source_gain;
                *time_constant_s = *source_time_constant;
                *input_bias = *source_input_bias;
                *output_bias = *source_output_bias;
                *dead_time_s = *source_dead_time;
                *initial_delayed_input = *source_initial_delayed_input;
            }
            (
                Self::SecondOrder {
                    gain,
                    natural_frequency,
                    damping_ratio,
                    input_bias,
                    output_bias,
                },
                Self::SecondOrder {
                    gain: source_gain,
                    natural_frequency: source_frequency,
                    damping_ratio: source_damping,
                    input_bias: source_input_bias,
                    output_bias: source_output_bias,
                },
            ) => {
                *gain = *source_gain;
                *natural_frequency = *source_frequency;
                *damping_ratio = *source_damping;
                *input_bias = *source_input_bias;
                *output_bias = *source_output_bias;
            }
            (Self::PiecewiseLinear { .. }, Self::PiecewiseLinear { .. }) => {}
            (
                Self::StateSpace { a, b, c, d },
                Self::StateSpace {
                    a: source_a,
                    b: source_b,
                    c: source_c,
                    d: source_d,
                },
            ) => {
                for (target, source) in a.iter_mut().zip(source_a) {
                    target.clone_from_slice(source);
                }
                for (target, source) in b.iter_mut().zip(source_b) {
                    target.clone_from_slice(source);
                }
                for (target, source) in c.iter_mut().zip(source_c) {
                    target.clone_from_slice(source);
                }
                for (target, source) in d.iter_mut().zip(source_d) {
                    target.clone_from_slice(source);
                }
            }
            _ => unreachable!("validated plant model kind cannot change after activation"),
        }
    }
}

pub(super) fn model_output_into(
    model: &PlantModelConfig,
    state: &[f64],
    inputs: &[f64],
    outputs: &mut [f64],
) {
    match model {
        PlantModelConfig::StaticGain {
            gain,
            input_bias,
            output_bias,
        } => outputs[0] = output_bias + gain * (inputs[0] - input_bias),
        PlantModelConfig::FirstOrder { .. } => outputs[0] = state[0],
        PlantModelConfig::FirstOrderDeadTime { .. } => outputs[0] = state[0],
        PlantModelConfig::Integrator { output_bias, .. } => outputs[0] = output_bias + state[0],
        PlantModelConfig::IntegratorDeadTime { output_bias, .. } => {
            outputs[0] = output_bias + state[0]
        }
        PlantModelConfig::GravityTank { .. } => outputs[0] = state[0],
        PlantModelConfig::RobotJoint { .. } | PlantModelConfig::GravityTankCascade { .. } => {
            outputs[0] = state[0];
            outputs[1] = state[1];
        }
        PlantModelConfig::PiecewiseLinear {
            input_points,
            output_points,
        } => {
            let input = inputs[0];
            if input <= input_points[0] {
                outputs[0] = output_points[0];
            } else if input >= input_points[input_points.len() - 1] {
                outputs[0] = output_points[output_points.len() - 1];
            } else {
                let index = input_points.partition_point(|point| *point < input);
                let x0 = input_points[index - 1];
                let x1 = input_points[index];
                let y0 = output_points[index - 1];
                let y1 = output_points[index];
                outputs[0] = y0 + (input - x0) * (y1 - y0) / (x1 - x0);
            }
        }
        PlantModelConfig::SecondOrder { output_bias, .. } => outputs[0] = output_bias + state[0],
        PlantModelConfig::StateSpace { c, d, .. } => {
            for (output, (state_row, input_row)) in outputs.iter_mut().zip(c.iter().zip(d)) {
                *output = state_row
                    .iter()
                    .zip(state)
                    .fold(0.0, |sum, (coefficient, value)| sum + coefficient * value)
                    + input_row
                        .iter()
                        .zip(inputs)
                        .fold(0.0, |sum, (coefficient, value)| sum + coefficient * value)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ModelWorkspace {
    k1: Vec<f64>,
    k2: Vec<f64>,
    k3: Vec<f64>,
    k4: Vec<f64>,
    temporary: Vec<f64>,
}

impl ModelWorkspace {
    pub(super) fn new(state_count: usize) -> Self {
        Self {
            k1: vec![0.0; state_count],
            k2: vec![0.0; state_count],
            k3: vec![0.0; state_count],
            k4: vec![0.0; state_count],
            temporary: vec![0.0; state_count],
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn advance_fopdt(
    state: &mut f64,
    start_nanos: i64,
    step_nanos: i64,
    history: &VecDeque<(i64, f64)>,
    gain: f64,
    time_constant_s: f64,
    input_bias: f64,
    output_bias: f64,
    dead_time_s: f64,
    initial_delayed_input: f64,
) {
    let delay_nanos = (dead_time_s * 1_000_000_000.0).round() as i64;
    let end_nanos = start_nanos + step_nanos;
    let mut cursor = start_nanos;
    let mut delayed = history
        .iter()
        .rev()
        .find(|(time, _)| *time + delay_nanos <= cursor)
        .map_or(initial_delayed_input, |(_, value)| *value);
    for (boundary, next_value) in history.iter().filter_map(|(time, value)| {
        let due = time.saturating_add(delay_nanos);
        (due > start_nanos && due < end_nanos).then_some((due, *value))
    }) {
        let segment_s = (boundary - cursor) as f64 / 1_000_000_000.0;
        let target = output_bias + gain * (delayed - input_bias);
        let a = (-segment_s / time_constant_s).exp();
        *state = target + (*state - target) * a;
        delayed = next_value;
        cursor = boundary;
    }
    let segment_s = (end_nanos - cursor) as f64 / 1_000_000_000.0;
    let target = output_bias + gain * (delayed - input_bias);
    let a = (-segment_s / time_constant_s).exp();
    *state = target + (*state - target) * a;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn advance_delayed_integrator(
    state: &mut f64,
    start_nanos: i64,
    step_nanos: i64,
    history: &VecDeque<(i64, f64)>,
    gain: f64,
    input_bias: f64,
    dead_time_s: f64,
    initial_delayed_input: f64,
) {
    let delay_nanos = (dead_time_s * 1_000_000_000.0).round() as i64;
    let end_nanos = start_nanos + step_nanos;
    let mut cursor = start_nanos;
    let mut delayed = history
        .iter()
        .rev()
        .find(|(time, _)| *time + delay_nanos <= cursor)
        .map_or(initial_delayed_input, |(_, value)| *value);
    for (boundary, next_value) in history.iter().filter_map(|(time, value)| {
        let due = time.saturating_add(delay_nanos);
        (due > start_nanos && due < end_nanos).then_some((due, *value))
    }) {
        *state += (boundary - cursor) as f64 / 1_000_000_000.0 * gain * (delayed - input_bias);
        delayed = next_value;
        cursor = boundary;
    }
    *state += (end_nanos - cursor) as f64 / 1_000_000_000.0 * gain * (delayed - input_bias);
}

pub(super) fn rk4_state_space(
    state: &mut [f64],
    inputs: &[f64],
    h: f64,
    a: &[Vec<f64>],
    b: &[Vec<f64>],
    workspace: &mut ModelWorkspace,
) {
    derivative_into(state, inputs, a, b, &mut workspace.k1);
    for ((temporary, state), derivative) in workspace
        .temporary
        .iter_mut()
        .zip(&*state)
        .zip(&workspace.k1)
    {
        *temporary = state + 0.5 * h * derivative;
    }
    derivative_into(&workspace.temporary, inputs, a, b, &mut workspace.k2);
    for ((temporary, state), derivative) in workspace
        .temporary
        .iter_mut()
        .zip(&*state)
        .zip(&workspace.k2)
    {
        *temporary = state + 0.5 * h * derivative;
    }
    derivative_into(&workspace.temporary, inputs, a, b, &mut workspace.k3);
    for ((temporary, state), derivative) in workspace
        .temporary
        .iter_mut()
        .zip(&*state)
        .zip(&workspace.k3)
    {
        *temporary = state + h * derivative;
    }
    derivative_into(&workspace.temporary, inputs, a, b, &mut workspace.k4);
    for (index, value) in state.iter_mut().enumerate() {
        *value += h
            * (workspace.k1[index]
                + 2.0 * workspace.k2[index]
                + 2.0 * workspace.k3[index]
                + workspace.k4[index])
            / 6.0;
    }
}

pub(super) fn rk4_gravity_tank(
    level: &mut f64,
    inlet_flow: f64,
    h: f64,
    area_m2: f64,
    outflow_coefficient: f64,
) {
    let derivative =
        |height: f64| (inlet_flow - outflow_coefficient * height.max(0.0).sqrt()) / area_m2;
    let k1 = derivative(*level);
    let k2 = derivative(*level + 0.5 * h * k1);
    let k3 = derivative(*level + 0.5 * h * k2);
    let k4 = derivative(*level + h * k3);
    *level = (*level + h * (k1 + 2.0 * k2 + 2.0 * k3 + k4) / 6.0).max(0.0);
}

pub(super) fn rk4_gravity_tank_cascade(
    state: &mut [f64],
    inlet_flow: f64,
    h: f64,
    area_m2: f64,
    outflow_coefficient: f64,
) {
    let derivative = |upstream: f64, downstream: f64| {
        let transfer = outflow_coefficient * upstream.max(0.0).sqrt();
        let outflow = outflow_coefficient * downstream.max(0.0).sqrt();
        (
            (inlet_flow - transfer) / area_m2,
            (transfer - outflow) / area_m2,
        )
    };
    let (k1_up, k1_down) = derivative(state[0], state[1]);
    let (k2_up, k2_down) = derivative(state[0] + 0.5 * h * k1_up, state[1] + 0.5 * h * k1_down);
    let (k3_up, k3_down) = derivative(state[0] + 0.5 * h * k2_up, state[1] + 0.5 * h * k2_down);
    let (k4_up, k4_down) = derivative(state[0] + h * k3_up, state[1] + h * k3_down);
    state[0] = (state[0] + h * (k1_up + 2.0 * k2_up + 2.0 * k3_up + k4_up) / 6.0).max(0.0);
    state[1] = (state[1] + h * (k1_down + 2.0 * k2_down + 2.0 * k3_down + k4_down) / 6.0).max(0.0);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rk4_robot_joint(
    state: &mut [f64],
    torque: f64,
    h: f64,
    inertia: f64,
    gravity_torque: f64,
    viscous_friction: f64,
    coulomb_friction: f64,
    friction_smoothing: f64,
) {
    let derivative = |theta: f64, omega: f64| {
        let friction =
            viscous_friction * omega + coulomb_friction * (omega / friction_smoothing).tanh();
        (
            omega,
            (torque - gravity_torque * theta.sin() - friction) / inertia,
        )
    };
    let (k1_theta, k1_omega) = derivative(state[0], state[1]);
    let (k2_theta, k2_omega) =
        derivative(state[0] + 0.5 * h * k1_theta, state[1] + 0.5 * h * k1_omega);
    let (k3_theta, k3_omega) =
        derivative(state[0] + 0.5 * h * k2_theta, state[1] + 0.5 * h * k2_omega);
    let (k4_theta, k4_omega) = derivative(state[0] + h * k3_theta, state[1] + h * k3_omega);
    state[0] += h * (k1_theta + 2.0 * k2_theta + 2.0 * k3_theta + k4_theta) / 6.0;
    state[1] += h * (k1_omega + 2.0 * k2_omega + 2.0 * k3_omega + k4_omega) / 6.0;
}

fn derivative_into(
    state: &[f64],
    inputs: &[f64],
    a: &[Vec<f64>],
    b: &[Vec<f64>],
    derivative: &mut [f64],
) {
    for (value, (row, input_row)) in derivative.iter_mut().zip(a.iter().zip(b)) {
        *value = row
            .iter()
            .zip(state)
            .fold(0.0, |sum, (coefficient, state)| sum + coefficient * state)
            + input_row
                .iter()
                .zip(inputs)
                .fold(0.0, |sum, (coefficient, input)| sum + coefficient * input);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rk4_second_order(
    state: &mut [f64],
    input: f64,
    h: f64,
    gain: f64,
    wn: f64,
    damping: f64,
    input_bias: f64,
) {
    let derivative = |x1: f64, x2: f64| {
        (
            x2,
            gain * wn * wn * (input - input_bias) - 2.0 * damping * wn * x2 - wn * wn * x1,
        )
    };
    let (k1x, k1v) = derivative(state[0], state[1]);
    let (k2x, k2v) = derivative(state[0] + 0.5 * h * k1x, state[1] + 0.5 * h * k1v);
    let (k3x, k3v) = derivative(state[0] + 0.5 * h * k2x, state[1] + 0.5 * h * k2v);
    let (k4x, k4v) = derivative(state[0] + h * k3x, state[1] + h * k3v);
    state[0] += h * (k1x + 2.0 * k2x + 2.0 * k3x + k4x) / 6.0;
    state[1] += h * (k1v + 2.0 * k2v + 2.0 * k3v + k4v) / 6.0;
}

/// Execute one discrete update without treating A and B as derivatives.
pub(super) fn step_discrete_state_space(
    state: &mut [f64],
    inputs: &[f64],
    a: &[Vec<f64>],
    b: &[Vec<f64>],
    workspace: &mut ModelWorkspace,
) {
    derivative_into(state, inputs, a, b, &mut workspace.temporary);
    state.copy_from_slice(&workspace.temporary);
}
