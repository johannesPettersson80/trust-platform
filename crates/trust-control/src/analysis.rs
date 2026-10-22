//! Control Studio's analysis tools (specification 35, B10.2, A7; numerical annex N4 and N7): the declared
//! analysis and its pole paths, the Routh–Hurwitz analysis and its session, the general feedback analysis, and
//! the lead/lag design. Their rules, sentences and stored records are here; their mathematics is in
//! `trust_control_math`.

pub mod budget;
pub mod currency;
pub mod declared;
pub mod declared_tools;
pub mod entries;
pub mod feedback_declared;
pub mod feedback_margins;
pub mod feedback_tool;
pub mod frequency;
pub mod frequency_points;
pub mod general_feedback;
pub mod initial_state;
pub mod lead_lag;
pub mod lead_lag_tool;
pub mod limits;
pub mod measured_frequency;
pub mod measured_paths;
pub mod model_views;
pub mod parameter_writes;
pub mod pole_view;
pub mod routh;
pub mod routh_interval;
pub mod routh_session;
pub mod texts;
pub mod tool;
