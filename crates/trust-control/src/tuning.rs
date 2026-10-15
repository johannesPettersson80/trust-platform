//! Tune's product rules (specification 35, B4 and Part C; numerical annex N5, N6 and N8.6): the requirements and the
//! test criteria that Automatic fills, the formula methods and their refusals, the Automatic recommendation and its
//! design to requirements, the candidates and their pins, and the program's own laws.
//!
//! The numbers come from `trust_control_math` (the formulas, the proportional predictions and the sign rule); this
//! module holds the rules about what is allowed, the choices between candidates and the sentences the engineer reads.
//! Nothing here reads a clock or does I/O, and nothing writes to a runtime: every candidate stays unapplied.
//!
//! The response of a candidate on the model is not computed here. The rules that need it take a
//! [`shortfall::ResponsePredictor`], which the prediction of specification 35 B4.5 implements.

pub mod automatic_criteria;
pub mod candidate;
pub mod candidate_margins;
pub mod design_to_requirements;
pub mod formula;
pub mod method_description;
pub mod objective;
pub mod path_support;
pub mod placement;
pub mod plant;
pub mod prediction;
pub mod project_candidate;
pub mod project_components;
pub mod project_model;
pub mod project_seed;
pub mod proportional;
pub mod recommendation;
pub mod relay_rule;
pub mod requirements;
pub mod robustness;
pub mod settings_pins;
pub mod shortfall;
pub mod state_controller;
pub mod structure;
pub mod structures;
pub mod texts;
pub mod typed_pid;
