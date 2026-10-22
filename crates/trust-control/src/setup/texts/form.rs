//! The words the setup form shows for each value of its choices (specification 35, B1.3; A11.1): the structures, what an Auto/Manual
//! signal's TRUE value means, the bands and threshold policies of an ON/OFF controller, the designs of state feedback, the laws of a
//! program's own controller and where its derivative is taken from, the laws a finite search may name and where its kept values start,
//! and the controller actions. Each list pairs a value the draft reads with its words, in the order the page lists the values; the page
//! shows the words as the engine answers them (`SetupFormModel.labels`).

/// The structures the setup page may offer (B1.3, Part C), each with its words.
pub const STRUCTURES: [(&str, &str); 12] = [
    ("pid", "One PID loop"),
    ("cascade", "Cascade: one PID sets the other's setpoint"),
    (
        "feedforward",
        "Feedforward: the PID adds the program's feedforward from a measured disturbance",
    ),
    (
        "smith",
        "Smith predictor: the PID reads the measurement plus the program's predicted correction",
    ),
    (
        "schedule",
        "Gain schedule: the program interpolates the PID's gains between operating points",
    ),
    ("group", "PID group: interacting PIDs tuned together"),
    ("onoff", "ON/OFF: the program switches the output"),
    (
        "state-feedback",
        "State feedback: the program feeds measured outputs back",
    ),
    (
        "own-law",
        "Own P, PI or PID: the program computes the output from its gain variables",
    ),
    (
        "trajectory-feedforward",
        "Trajectory feedforward: the PID adds a feedforward along the program's planned trajectory",
    ),
    (
        "search",
        "Other law: Tune searches the program's coefficient values",
    ),
    ("identify", "Identify only, no controller"),
];

/// What an Auto/Manual signal's TRUE value means (`modeWhenTrue`), each with its words.
pub const MODE_WHEN_TRUE: [(&str, &str); 2] = [("Auto", "Auto"), ("Manual", "Manual")];

/// The hysteresis bands of an ON/OFF controller (`bandConvention`, C3), each with its words.
pub const BAND_CONVENTIONS: [(&str, &str); 2] = [
    (
        "half-band",
        "Half the band: switches at setpoint ± hysteresis",
    ),
    (
        "full-band",
        "The whole band: switches at setpoint ± hysteresis / 2",
    ),
];

/// The threshold-equality policies of an ON/OFF controller (`thresholdEquality`, C3, D-10), each with its words.
pub const THRESHOLD_EQUALITIES: [(&str, &str); 2] = [
    ("at-or-beyond", "At or beyond the threshold (the default)"),
    ("beyond", "Only strictly beyond the threshold"),
];

/// The designs of state feedback (`family` of the state-feedback structure, C4), each with its words.
pub const STATE_FAMILIES: [(&str, &str); 3] = [
    ("LQR", "LQR, weighted by the signal ranges"),
    ("state-feedback", "Pole placement"),
    (
        "LQG",
        "LQG: the program estimates the states from the controlled output",
    ),
];

/// The laws of a program's own controller (`law`, C2), each with its words.
pub const OWN_LAWS: [(&str, &str); 3] = [
    ("PI", "PI: u = Kp·e + I, where I integrates Ki·e"),
    ("P", "P: u = Kp·e plus the program's own offset"),
    (
        "PID",
        "PID: u = Kp·e + I + D, where D is Kd times the rate of the error or measurement",
    ),
];

/// Where a program's own PID law takes its derivative (`derivativeOn`, C2), each with its words.
pub const DERIVATIVE_ON: [(&str, &str); 2] =
    [("error", "The error"), ("measurement", "The measurement")];

/// The laws a finite search may name (`family` of the search structure, C5), each with its words.
pub const SEARCH_FAMILIES: [(&str, &str); 12] = [
    ("MPC", "Model predictive control"),
    ("ratio", "Ratio"),
    ("split-range", "Split range"),
    ("override", "Override"),
    ("gain-scheduled", "Gain schedule"),
    ("feedforward", "Feedforward"),
    ("Smith", "Smith predictor"),
    ("lead-lag", "Lead/lag"),
    ("adaptive", "Adaptive"),
    ("robust", "Robust"),
    ("feedback-linearization", "Feedback linearization"),
    ("observer-feedback", "Observer feedback"),
];

/// Where a kept value of a finite search starts (`start`, C5), each with its words.
pub const KEPT_STARTS: [(&str, &str); 3] = [
    ("zero", "Zero"),
    ("output", "The output"),
    ("measurement", "The measurement"),
];

/// The controller actions (`action`, B1.4), each with its words: the direction the output moves, because instrument practice names
/// `direct` and `reverse` the other way round (a heater's controller is "reverse acting").
pub const ACTIONS: [(&str, &str); 2] = [
    (
        "direct",
        "Increase the output while the measurement is below the setpoint",
    ),
    (
        "reverse",
        "Increase the output while the measurement is above the setpoint",
    ),
];
