//! The design rules of the parked PID structures (specification 35, C6 to C10, and the decoupler of C7; numerical annex N4.4, N4.5 and
//! N5.15): the cascade, the measured-disturbance feedforward, the decentralized PI and the static decoupler of a PID group, the Smith
//! predictor, and the value rules of gain scheduling.
//!
//! Each structure reduces its model's responses to first-order paths by the two-point method, and designs each PI with Automatic's
//! single-loop design on the path. The engine's response predictor and pole placement for a path come through
//! [`path::PathSupport`]; no rule here predicts a response. The rules are pure: no I/O, no clock, no threads. The loop setup, the
//! commissioning and the wiring of these designs to the engine come in a later stage.

pub mod cascade;
pub mod channel;
pub mod feedforward;
pub mod gain_schedule;
pub mod path;
pub mod pid_group;
pub mod smith_predictor;
pub mod texts;

#[cfg(test)]
pub(crate) mod fixtures;
