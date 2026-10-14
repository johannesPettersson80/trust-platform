//! Control-system mathematics for truST: linear models, discretization, simulation, even sampling
//! and step-response identification.
//!
//! The crate knows no product: no Control Studio study, no runtime protocol and no user-facing
//! sentence. It takes numbers and returns numbers, typed results and typed refusals; `trust-control`
//! turns them into Control Studio's rules and texts. Times are plain seconds passed in, so every
//! function is deterministic. Each public function documents the formula it computes, and its tests
//! compare it with closed-form or independently computed reference values.

#![forbid(unsafe_code)]

pub mod affine;
pub mod classical;
pub mod closed_loop;
pub mod design_matrix;
pub mod discretize;
pub mod exact_polynomial;
pub mod excitation;
pub mod feedback_poles;
pub mod frequency;
pub mod frequency_identification;
pub mod identify;
pub mod inverse;
pub mod lead_lag;
pub mod linalg;
pub mod lqg;
pub mod lqr;
pub mod margins;
pub mod measure;
pub mod model;
pub mod multichannel;
pub mod notch;
pub mod nyquist;
pub mod placement;
pub mod polynomial;
pub mod proportional;
pub mod root_locus;
pub mod routh;
pub mod sampled_loop;
pub mod sampling;
pub mod sensor_lag;
pub mod settling_threshold;
pub mod simulate;
pub mod state_feedback;
pub mod state_space_analysis;
pub mod tuning;
