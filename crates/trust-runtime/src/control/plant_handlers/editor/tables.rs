//! The plant editor's tables (specification 47 §7, §9, §10, §11, §12; specification 46 §7): the
//! model list in product order with each model's shape, every model's dynamics fields with their
//! defaults, the actuator fields, the starting points and the temperature example, the labelled
//! starting states, the imperfection presets and the uncertainty and scenario vocabularies. The
//! editor shows these as given; `plant.capabilities` carries them.

use serde_json::{json, Map, Value};

/// A plant model as the Plant model selector names it (specification 47 §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ModelDescriptor {
    pub(super) kind: &'static str,
    pub(super) product_name: &'static str,
    pub(super) purpose: &'static str,
    pub(super) states: usize,
    pub(super) inputs: usize,
    pub(super) outputs: usize,
}

const fn model(
    kind: &'static str,
    product_name: &'static str,
    purpose: &'static str,
    states: usize,
    inputs: usize,
    outputs: usize,
) -> ModelDescriptor {
    ModelDescriptor {
        kind,
        product_name,
        purpose,
        states,
        inputs,
        outputs,
    }
}

/// The models in the selector's product order (specification 47 §7, the cascade after §21.1).
pub(super) const MODELS: [ModelDescriptor; 10] = [
    model("static_gain", "Static gain", "The output changes immediately in proportion to the input.", 0, 1, 1),
    model("first_order", "First order", "The output moves gradually toward each new input.", 1, 1, 1),
    model("first_order_dead_time", "First order with dead time", "The output starts changing after a delay, then settles gradually.", 1, 1, 1),
    model("integrator", "Integrator", "The output keeps accumulating while the input differs from its resting point; it does not settle by itself.", 1, 1, 1),
    model("second_order", "Second order", "The output can overshoot or oscillate before it settles.", 2, 1, 1),
    model("piecewise_linear", "Piecewise linear", "A table of points defines an immediate nonlinear input-to-output curve.", 0, 1, 1),
    model("state_space", "State space", "Coupled internal states turn one or more inputs into one or more outputs.", 1, 1, 1),
    model("gravity_tank", "Gravity-drained tank", "Inflow raises the level; gravity drains it faster as the level rises.", 1, 1, 1),
    model("gravity_tank_cascade", "Cascaded gravity tanks", "The upstream tank freely drains into an equal downstream tank; coupled balances preserve the transferred water.", 2, 1, 2),
    model("robot_joint", "Gravity-loaded robot joint", "Torque changes joint angle and speed; gravity depends on angle, and friction resists motion.", 2, 1, 2),
];

/// The descriptor of `kind`; a kind the editor does not know keeps its name, without a shape.
pub(super) fn model_descriptor(kind: &str) -> (String, String, usize, usize, usize) {
    MODELS
        .iter()
        .find(|descriptor| descriptor.kind == kind)
        .map(|descriptor| {
            (
                descriptor.product_name.to_string(),
                descriptor.purpose.to_string(),
                descriptor.states,
                descriptor.inputs,
                descriptor.outputs,
            )
        })
        .unwrap_or_else(|| {
            (
                kind.replace('_', " "),
                "Custom plant model".to_string(),
                0,
                0,
                0,
            )
        })
}

/// How a dynamics field holds its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ValueKind {
    Number,
    Vector,
    Matrix,
}

/// A model's dynamics field (specification 47 §9): its configuration key, the label and help the
/// editor shows, and its default when the field may be left empty (specification 46 §7).
#[derive(Clone, Copy, Debug)]
pub(super) struct DynamicsField {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) value_kind: ValueKind,
    pub(super) plain_term: &'static str,
    pub(super) control_term: &'static str,
    pub(super) unit: &'static str,
    pub(super) effect: &'static str,
    pub(super) optional: bool,
    pub(super) default_value: Option<f64>,
    pub(super) default_from: Option<&'static str>,
}

#[allow(clippy::too_many_arguments)] // One row of the table: every column is named at the call.
const fn dynamics(
    key: &'static str,
    label: &'static str,
    value_kind: ValueKind,
    plain_term: &'static str,
    control_term: &'static str,
    unit: &'static str,
    effect: &'static str,
    optional: bool,
    default_value: Option<f64>,
    default_from: Option<&'static str>,
) -> DynamicsField {
    DynamicsField {
        key,
        label,
        value_kind,
        plain_term,
        control_term,
        unit,
        effect,
        optional,
        default_value,
        default_from,
    }
}

const GAIN: DynamicsField = dynamics(
    "gain",
    "Gain",
    ValueKind::Number,
    "Response size",
    "Process gain",
    "output per input",
    "A larger magnitude produces a larger output change for the same input change.",
    true,
    Some(1.0),
    None,
);
const INPUT_BIAS: DynamicsField = dynamics("input_bias", "Input bias", ValueKind::Number, "Input resting point", "Input operating-point bias", "input units", "The model responds to command changes relative to this value; empty uses the runtime default of 0.", true, Some(0.0), None);
const OUTPUT_BIAS: DynamicsField = dynamics("output_bias", "Output bias", ValueKind::Number, "Output resting point", "Output operating-point bias", "output units", "This offsets the model output without changing its dynamic response; empty uses the runtime default of 0.", true, Some(0.0), None);

const fn initial_state(label: &'static str) -> DynamicsField {
    dynamics(
        "initial_state",
        label,
        ValueKind::Vector,
        "Starting internal value",
        "Initial state vector",
        "state units",
        "This is the model state at simulation start or reset.",
        false,
        None,
        None,
    )
}

const STATIC_GAIN: [DynamicsField; 3] = [GAIN, INPUT_BIAS, OUTPUT_BIAS];
const FIRST_ORDER: [DynamicsField; 5] = [
    GAIN,
    dynamics("time_constant_s", "Time constant (s)", ValueKind::Number, "Response speed", "Time constant", "s", "A larger value makes the response slower; about 63% of a step is reached after one time constant.", false, None, None),
    INPUT_BIAS,
    OUTPUT_BIAS,
    initial_state("Initial state"),
];
const FIRST_ORDER_DEAD_TIME: [DynamicsField; 7] = [
    GAIN,
    dynamics("time_constant_s", "Time constant (s)", ValueKind::Number, "Response speed", "Time constant", "s", "A larger value makes the response slower after the delay has elapsed.", false, None, None),
    dynamics("dead_time_s", "Dead time (s)", ValueKind::Number, "Waiting time", "Transport delay", "s", "The output does not react to a new input until this delay has elapsed.", true, Some(0.0), None),
    INPUT_BIAS,
    OUTPUT_BIAS,
    dynamics("initial_delayed_input", "Initial delayed input", ValueKind::Number, "Input already in transit", "Initial delay-line value", "input units", "This fills the delay before the first new command reaches the model; empty uses the input bias.", true, None, Some("input_bias")),
    initial_state("Initial state"),
];
const INTEGRATOR: [DynamicsField; 6] = [
    dynamics(
        "gain",
        "Integral gain",
        ValueKind::Number,
        "Accumulation rate",
        "Integrator gain",
        "output per input-second",
        "A larger magnitude stores change faster while the input differs from its bias.",
        true,
        Some(1.0),
        None,
    ),
    INPUT_BIAS,
    OUTPUT_BIAS,
    dynamics(
        "state_minimum",
        "Minimum state",
        ValueKind::Number,
        "Lowest stored value",
        "Lower integrator clamp",
        "state units",
        "When set, the accumulated state cannot fall below this value; empty means unbounded.",
        true,
        None,
        None,
    ),
    dynamics(
        "state_maximum",
        "Maximum state",
        ValueKind::Number,
        "Highest stored value",
        "Upper integrator clamp",
        "state units",
        "When set, the accumulated state cannot rise above this value; empty means unbounded.",
        true,
        None,
        None,
    ),
    initial_state("Initial stored value"),
];
const GRAVITY_TANK: [DynamicsField; 3] = [
    dynamics(
        "area_m2",
        "Tank area (m²)",
        ValueKind::Number,
        "Tank cross-section",
        "Cross-sectional area",
        "m²",
        "A larger area makes the level change more slowly for the same flow.",
        false,
        None,
        None,
    ),
    dynamics(
        "outflow_coefficient_m2_sqrt_m_per_s",
        "Outlet coefficient",
        ValueKind::Number,
        "Outlet size",
        "Square-root discharge coefficient",
        "m²√m/s",
        "A larger coefficient drains the tank faster, especially at higher level.",
        false,
        None,
        None,
    ),
    initial_state("Initial level"),
];
const GRAVITY_TANK_CASCADE: [DynamicsField; 3] = [
    dynamics(
        "area_m2",
        "Area of each tank (m²)",
        ValueKind::Number,
        "Equal tank cross-sections",
        "Cross-sectional area",
        "m²",
        "This area applies to both freely discharging tanks.",
        false,
        None,
        None,
    ),
    dynamics(
        "outflow_coefficient_m2_sqrt_m_per_s",
        "Outlet coefficient of each tank",
        ValueKind::Number,
        "Equal outlet sizes",
        "Square-root discharge coefficient",
        "m²√m/s",
        "The upstream outlet feeds the downstream tank without an added scan delay.",
        false,
        None,
        None,
    ),
    initial_state("Initial upstream and downstream levels"),
];
const ROBOT_JOINT: [DynamicsField; 6] = [
    dynamics(
        "inertia_kg_m2",
        "Joint inertia (kg·m²)",
        ValueKind::Number,
        "Resistance to acceleration",
        "Rotational inertia",
        "kg·m²",
        "A larger inertia reduces angular acceleration for the same net torque.",
        false,
        None,
        None,
    ),
    dynamics(
        "gravity_torque_nm",
        "Gravity torque coefficient (N·m)",
        ValueKind::Number,
        "Gravity load",
        "Maximum gravity torque",
        "N·m",
        "This sets how strongly joint angle changes the gravity load.",
        true,
        Some(0.0),
        None,
    ),
    dynamics(
        "viscous_friction_nm_s_per_rad",
        "Viscous friction",
        ValueKind::Number,
        "Speed-dependent drag",
        "Viscous friction coefficient",
        "N·m·s/rad",
        "A larger value opposes angular speed more strongly.",
        true,
        Some(0.0),
        None,
    ),
    dynamics(
        "coulomb_friction_nm",
        "Coulomb friction (N·m)",
        ValueKind::Number,
        "Dry friction",
        "Coulomb friction torque",
        "N·m",
        "This opposes motion with nearly constant torque away from zero speed.",
        true,
        Some(0.0),
        None,
    ),
    dynamics(
        "friction_smoothing_rad_s",
        "Friction smoothing (rad/s)",
        ValueKind::Number,
        "Zero-speed transition",
        "Coulomb smoothing speed",
        "rad/s",
        "A smaller value makes the friction reversal sharper around zero speed.",
        false,
        None,
        None,
    ),
    initial_state("Initial angle and speed"),
];
const SECOND_ORDER: [DynamicsField; 6] = [
    GAIN,
    dynamics(
        "natural_frequency_rad_s",
        "Natural frequency (rad/s)",
        ValueKind::Number,
        "Natural response speed",
        "Undamped natural frequency",
        "rad/s",
        "A larger value moves the characteristic response and resonance to a higher frequency.",
        false,
        None,
        None,
    ),
    dynamics(
        "damping_ratio",
        "Damping ratio",
        ValueKind::Number,
        "Oscillation decay",
        "Damping ratio",
        "ratio",
        "Values below 1 can oscillate; larger values reduce overshoot and resonance.",
        false,
        None,
        None,
    ),
    INPUT_BIAS,
    OUTPUT_BIAS,
    initial_state("Initial states"),
];
const PIECEWISE_LINEAR: [DynamicsField; 2] = [
    dynamics(
        "input_points",
        "Input points",
        ValueKind::Vector,
        "Curve input coordinates",
        "Piecewise-linear abscissas",
        "input units",
        "Inputs must increase strictly; values between points are interpolated.",
        false,
        None,
        None,
    ),
    dynamics(
        "output_points",
        "Output points",
        ValueKind::Vector,
        "Curve output coordinates",
        "Piecewise-linear ordinates",
        "output units",
        "Each value is paired with the input point at the same row.",
        false,
        None,
        None,
    ),
];
const STATE_SPACE: [DynamicsField; 6] = [
    dynamics(
        "a",
        "A matrix",
        ValueKind::Matrix,
        "State-to-state dynamics",
        "State matrix A",
        "state rate per state",
        "Defines how the current states drive their own rates of change.",
        false,
        None,
        None,
    ),
    dynamics(
        "b",
        "B matrix",
        ValueKind::Matrix,
        "Input-to-state dynamics",
        "Input matrix B",
        "state rate per input",
        "Defines how each command drives each state.",
        false,
        None,
        None,
    ),
    dynamics(
        "c",
        "C matrix",
        ValueKind::Matrix,
        "State-to-output mapping",
        "Output matrix C",
        "output per state",
        "Defines how states contribute to each process output.",
        false,
        None,
        None,
    ),
    dynamics(
        "d",
        "D matrix",
        ValueKind::Matrix,
        "Direct input-to-output mapping",
        "Feedthrough matrix D",
        "output per input",
        "Defines direct output from each input after its configured transport delay.",
        false,
        None,
        None,
    ),
    dynamics(
        "input_delays_s",
        "Input delays (s)",
        ValueKind::Vector,
        "Input delivery time",
        "Input transport delays",
        "s",
        "Each input reaches the process after its own delay; omitted values use zero delay.",
        true,
        None,
        None,
    ),
    initial_state("Initial state"),
];

/// The dynamics fields of `model` in the editor's order; none for a model the editor does not know.
pub(super) fn dynamics_fields(model: &str) -> &'static [DynamicsField] {
    match model {
        "static_gain" => &STATIC_GAIN,
        "first_order" => &FIRST_ORDER,
        "first_order_dead_time" => &FIRST_ORDER_DEAD_TIME,
        "integrator" => &INTEGRATOR,
        "gravity_tank" => &GRAVITY_TANK,
        "gravity_tank_cascade" => &GRAVITY_TANK_CASCADE,
        "robot_joint" => &ROBOT_JOINT,
        "second_order" => &SECOND_ORDER,
        "piecewise_linear" => &PIECEWISE_LINEAR,
        "state_space" => &STATE_SPACE,
        _ => &[],
    }
}

/// The configuration key a dynamics field is saved under.
pub(super) fn stored_key(key: &str) -> String {
    if key == "initial_state" {
        key.to_string()
    } else {
        format!("parameters.{key}")
    }
}

/// An actuator field (specification 47 §10): its key, labels and help, what an empty field means and
/// the runtime default when there is one (specification 46 §8).
#[derive(Clone, Copy, Debug)]
pub(super) struct ActuatorField {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) plain_term: &'static str,
    pub(super) control_term: &'static str,
    pub(super) unit: &'static str,
    pub(super) effect: &'static str,
    pub(super) blank_meaning: &'static str,
    pub(super) default_value: Option<f64>,
    pub(super) default_from: Option<&'static str>,
}

#[allow(clippy::too_many_arguments)] // One row of the table: every column is named at the call.
const fn actuator(
    key: &'static str,
    label: &'static str,
    plain_term: &'static str,
    control_term: &'static str,
    unit: &'static str,
    effect: &'static str,
    blank_meaning: &'static str,
    default_value: Option<f64>,
    default_from: Option<&'static str>,
) -> ActuatorField {
    ActuatorField {
        key,
        label,
        plain_term,
        control_term,
        unit,
        effect,
        blank_meaning,
        default_value,
        default_from,
    }
}

/// The actuator fields in the editor's order.
pub(super) const ACTUATOR_FIELDS: [ActuatorField; 10] = [
    actuator("minimum", "Minimum", "Lowest actuator output", "Lower saturation limit", "output units", "A higher minimum prevents the actuator from applying smaller values.", "Unbounded", None, None),
    actuator("maximum", "Maximum", "Highest actuator output", "Upper saturation limit", "output units", "A lower maximum makes the actuator saturate sooner on increasing commands.", "Unbounded", None, None),
    actuator("rate_up_per_s", "Rising rate per second", "Maximum opening speed", "Positive slew-rate limit", "output units per second", "A smaller value makes increasing actuator movement slower; zero prevents upward movement.", "Unbounded; 0 blocks upward movement", None, None),
    actuator("rate_down_per_s", "Falling rate per second", "Maximum closing speed", "Negative slew-rate limit", "output units per second", "A smaller value makes decreasing actuator movement slower; zero prevents downward movement.", "Unbounded; 0 blocks downward movement", None, None),
    actuator("deadband", "Deadband", "Ignored command movement", "Actuator deadband", "command units", "A larger value ignores a wider band of small command changes.", "0 (no deadband)", Some(0.0), None),
    actuator("stiction_band", "Stiction band", "Breakaway movement", "Static-friction band", "command units", "A larger value makes the actuator hold until command and position differ further.", "0 (no static friction)", Some(0.0), None),
    actuator("slip_jump", "Slip jump", "Breakaway jump", "Slip-jump magnitude", "output units", "This is the first bounded position movement after static friction breaks.", "0 (no breakaway jump)", Some(0.0), None),
    actuator("characteristic_exponent", "Installed characteristic", "Position-to-process curve", "Installed characteristic exponent", "ratio", "One is linear; values above one reduce process effect at low travel, while values below one increase it.", "1 (linear characteristic)", Some(1.0), None),
    actuator("lag_s", "Actuator lag (s)", "Actuator response speed", "Actuator time constant", "seconds", "A larger value makes the applied actuator output respond more slowly.", "0 (no lag)", Some(0.0), None),
    actuator("initial_applied", "Initial applied value", "Starting actuator output", "Initial applied output", "output units", "This is the actuator output held at simulation start before command dynamics advance.", "Initial input (resolved at runtime)", None, Some("initial_input")),
];

/// A starting point (specification 47 §7): a model with explicit, editable proposal values that the
/// user must adapt to the real process. The values are unit-neutral seeds, not identified ones.
#[derive(Clone, Copy, Debug)]
pub(super) struct StartingPoint {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) model: &'static str,
    pub(super) parameters: &'static [(&'static str, Seed)],
    pub(super) initial_state: &'static [f64],
    pub(super) why_reasonable: &'static str,
    pub(super) limitation: &'static str,
}

/// A proposed parameter value: a number or a list of numbers.
#[derive(Clone, Copy, Debug)]
pub(super) enum Seed {
    Number(f64),
    List(&'static [f64]),
}

/// The starting points of specification 47 §7 (and the gravity-tank, cascade and robot-joint ones of
/// §21.1 and the cascade section), in the editor's order.
pub(super) const STARTING_POINTS: [StartingPoint; 9] = [
    StartingPoint {
        id: "tank",
        label: "Tank or accumulated level",
        model: "integrator",
        parameters: &[("gain", Seed::Number(1.0)), ("input_bias", Seed::Number(0.0)), ("output_bias", Seed::Number(0.0))],
        initial_state: &[0.0],
        why_reasonable: "It accumulates the difference between inflow and outflow.",
        limitation: "It cannot settle by itself or reproduce leakage unless those effects are added.",
    },
    StartingPoint {
        id: "gravity_drained_tank",
        label: "Gravity-drained tank",
        model: "gravity_tank",
        parameters: &[("area_m2", Seed::Number(2.0)), ("outflow_coefficient_m2_sqrt_m_per_s", Seed::Number(0.5))],
        initial_state: &[4.0],
        why_reasonable: "A physical mass balance and square-root outlet flow reproduce the level-dependent tank speed.",
        limitation: "It assumes a constant tank area and free discharge, without pump curves, pipe storage, or overflow.",
    },
    StartingPoint {
        id: "gravity_tank_cascade",
        label: "Cascaded gravity tanks",
        model: "gravity_tank_cascade",
        parameters: &[("area_m2", Seed::Number(0.5)), ("outflow_coefficient_m2_sqrt_m_per_s", Seed::Number(0.020))],
        initial_state: &[0.25, 0.25],
        why_reasonable: "Two coupled mass balances preserve water transferred from the upstream to the downstream tank.",
        limitation: "Both tanks share the same area and discharge coefficient; free discharge assumes no downstream backpressure.",
    },
    StartingPoint {
        id: "temperature",
        label: "Temperature or slow process",
        model: "first_order_dead_time",
        parameters: &[("gain", Seed::Number(1.0)), ("time_constant_s", Seed::Number(10.0)), ("dead_time_s", Seed::Number(1.0)), ("input_bias", Seed::Number(0.0)), ("output_bias", Seed::Number(0.0)), ("initial_delayed_input", Seed::Number(0.0))],
        initial_state: &[0.0],
        why_reasonable: "One dominant lag and a transport delay approximate many heating processes.",
        limitation: "It cannot reproduce resonance, multiple time scales, or nonlinear heating.",
    },
    StartingPoint {
        id: "motor_speed",
        label: "Motor speed or first-order process",
        model: "first_order",
        parameters: &[("gain", Seed::Number(1.0)), ("time_constant_s", Seed::Number(1.0)), ("input_bias", Seed::Number(0.0)), ("output_bias", Seed::Number(0.0))],
        initial_state: &[0.0],
        why_reasonable: "One dominant lag approximates speed response when electrical or mechanical dynamics dominate.",
        limitation: "It cannot reproduce resonance, transport delay, saturation, or friction.",
    },
    StartingPoint {
        id: "mechanical_position",
        label: "Mechanical position or resonant process",
        model: "second_order",
        parameters: &[("gain", Seed::Number(1.0)), ("natural_frequency_rad_s", Seed::Number(1.0)), ("damping_ratio", Seed::Number(0.7)), ("input_bias", Seed::Number(0.0)), ("output_bias", Seed::Number(0.0))],
        initial_state: &[0.0, 0.0],
        why_reasonable: "Two states represent inertia and damping, including oscillatory response.",
        limitation: "It cannot reproduce backlash, friction, hard stops, or other nonlinear mechanics.",
    },
    StartingPoint {
        id: "robot_joint",
        label: "Gravity-loaded robot joint",
        model: "robot_joint",
        parameters: &[("inertia_kg_m2", Seed::Number(2.0)), ("gravity_torque_nm", Seed::Number(4.0)), ("viscous_friction_nm_s_per_rad", Seed::Number(0.2)), ("coulomb_friction_nm", Seed::Number(0.1)), ("friction_smoothing_rad_s", Seed::Number(0.01))],
        initial_state: &[0.5, 0.0],
        why_reasonable: "The joint equation retains inertia, angle-dependent gravity, speed drag, and dry friction.",
        limitation: "It represents one rigid joint and smooths friction near zero speed; it omits gearbox elasticity and contact.",
    },
    StartingPoint {
        id: "static_scaling",
        label: "Static scaling",
        model: "static_gain",
        parameters: &[("gain", Seed::Number(1.0)), ("input_bias", Seed::Number(0.0)), ("output_bias", Seed::Number(0.0))],
        initial_state: &[],
        why_reasonable: "An immediate gain represents a sensor, conversion, or process with negligible dynamics.",
        limitation: "It cannot reproduce stored energy, lag, delay, or oscillation.",
    },
    StartingPoint {
        id: "nonlinear_characteristic",
        label: "Nonlinear characteristic",
        model: "piecewise_linear",
        parameters: &[("input_points", Seed::List(&[-1.0, 0.0, 1.0])), ("output_points", Seed::List(&[-1.0, 0.0, 1.0]))],
        initial_state: &[],
        why_reasonable: "Ordered points represent a known static nonlinear input/output curve.",
        limitation: "It cannot reproduce dynamics, hysteresis, or rate dependence.",
    },
];

/// The starting point of a custom state-space model (specification 47 §7): one state, input and
/// output, `A=[[0]]`, `B=[[1]]`, `C=[[1]]`, `D=[[0]]`, `initial_state=[0]`.
pub(super) const CUSTOM_STATE_SPACE: &str = "custom_state_space";
/// Why a custom state-space model is reasonable, and what it cannot do.
pub(super) const CUSTOM_STATE_SPACE_HELP: (&str, &str) = (
    "Coupled linear states represent multi-state or multi-channel linear dynamics.",
    "It cannot reproduce nonlinear or time-varying behavior. Input transport delays use explicit per-input settings.",
);
/// The temperature teaching example (specification 47 §7.1).
pub(super) const TEMPERATURE_EXAMPLE: &str = "temperature_example";

/// The starting point whose model is `model`, or the custom state space.
pub(super) fn starting_point_for_model(model: &str) -> &'static str {
    STARTING_POINTS
        .iter()
        .find(|point| point.model == model)
        .map_or(CUSTOM_STATE_SPACE, |point| point.id)
}

/// A starting point's proposal: the model, its parameters, the initial state, and the actuator
/// settings the temperature example brings.
pub(super) type Proposal = (
    &'static str,
    Map<String, Value>,
    Vec<Value>,
    Option<Map<String, Value>>,
);

/// The proposal of starting point `id`; `None` for an unknown starting point.
pub(super) fn proposal(id: &str) -> Option<Proposal> {
    let numbers = |values: &[f64]| {
        values
            .iter()
            .copied()
            .map(super::js::js_number)
            .collect::<Vec<_>>()
    };
    if id == TEMPERATURE_EXAMPLE {
        // Specification 47 §7.1: resting 20 °C, full-power rise 100 °C, 30 s, 2 s, starting at 20 °C;
        // gain = rise / 100, output bias = resting temperature, actuator 0–100 % from 0.
        let parameters = json!({
            "gain": 1, "time_constant_s": 30, "dead_time_s": 2,
            "input_bias": 0, "output_bias": 20, "initial_delayed_input": 0,
        });
        let actuator = json!({"minimum": 0, "maximum": 100, "initial_applied": 0});
        return Some((
            "first_order_dead_time",
            parameters.as_object().cloned().unwrap_or_default(),
            numbers(&[20.0]),
            actuator.as_object().cloned(),
        ));
    }
    if id == CUSTOM_STATE_SPACE {
        let parameters = json!({"a": [[0]], "b": [[1]], "c": [[1]], "d": [[0]]});
        return Some((
            "state_space",
            parameters.as_object().cloned().unwrap_or_default(),
            numbers(&[0.0]),
            None,
        ));
    }
    let point = STARTING_POINTS.iter().find(|point| point.id == id)?;
    let parameters = point
        .parameters
        .iter()
        .map(|(key, seed)| {
            let value = match seed {
                Seed::Number(value) => super::js::js_number(*value),
                Seed::List(values) => Value::Array(numbers(values)),
            };
            ((*key).to_string(), value)
        })
        .collect();
    Some((point.model, parameters, numbers(point.initial_state), None))
}

/// A starting point's product name, as the replacement confirmation names it.
pub(super) fn starting_point_name(id: &str) -> String {
    match id {
        TEMPERATURE_EXAMPLE => "temperature teaching example".to_string(),
        CUSTOM_STATE_SPACE => "Custom state space".to_string(),
        other => STARTING_POINTS
            .iter()
            .find(|point| point.id == other)
            .map_or_else(|| other.to_string(), |point| point.label.to_string()),
    }
}

/// The labelled starting states of a model (specification 47 §6.2), in canonical state order.
pub(super) fn starting_state_fields(model: &str) -> &'static [(&'static str, &'static str)] {
    match model {
        "first_order" => &[("Starting output", "Absolute ideal output, in the configured model's output units")],
        "integrator" => &[("Starting accumulated value", "Stored state before output bias; output equals this value plus output bias")],
        "gravity_tank" => &[("Starting level (m)", "Tank liquid level in metres")],
        "gravity_tank_cascade" => &[
            ("Starting upstream level (m)", "Upstream tank liquid level in metres"),
            ("Starting downstream level (m)", "Downstream tank liquid level in metres"),
        ],
        "second_order" => &[
            ("Starting displacement", "Output displacement before output bias"),
            ("Starting rate of change", "Time derivative in output units per second"),
        ],
        "robot_joint" => &[
            ("Starting angle (rad)", "Joint angle when simulation starts or resets. Enter radians, not degrees; 2π radians is one complete turn."),
            ("Starting angular speed (rad/s)", "Joint speed when simulation starts or resets. Zero starts at rest; the sign gives the direction of motion."),
        ],
        _ => &[],
    }
}
