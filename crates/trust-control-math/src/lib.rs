//! Control-system mathematics for truST: linear models, discretization, simulation, even sampling
//! and step-response identification.
//!
//! The crate knows no product: no Control Studio study, no runtime protocol and no user-facing
//! sentence. It takes numbers and returns numbers, typed results and typed refusals; `trust-control`
//! turns them into Control Studio's rules and texts. Times are plain seconds passed in, so every
//! function is deterministic. Each public function documents the formula it computes, and its tests
//! compare it with closed-form or independently computed reference values.

#![forbid(unsafe_code)]

pub mod closed_loop;
pub mod discretize;
pub mod frequency;
pub mod identify;
pub mod linalg;
pub mod margins;
pub mod measure;
pub mod model;
pub mod notch;
pub mod placement;
pub mod polynomial;
pub mod proportional;
pub mod sampling;
pub mod settling_threshold;
pub mod simulate;
pub mod tuning;
