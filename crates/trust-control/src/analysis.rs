//! Control Studio's analysis tools (specification 35, B10.2, A7; numerical annex N4 and N7): the declared
//! analysis and its pole paths, the Routh–Hurwitz analysis and its session, the general feedback analysis, and
//! the lead/lag design. Their rules, sentences and stored records are here; their mathematics is in
//! `trust_control_math`.

pub mod declared;
pub mod general_feedback;
pub mod lead_lag;
pub mod limits;
pub mod routh;
pub mod routh_session;
pub mod texts;
