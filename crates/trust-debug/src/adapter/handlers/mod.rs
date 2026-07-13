//! Request handlers grouped by DAP area.
//! - initialize: initialize/launch/configuration timing
//! - breakpoints: breakpoint CRUD + location resolution
//! - lifecycle: disconnect/terminate/reload
//! - threads: thread list
//! - stack_trace: stackTrace request
//! - scopes: scope enumeration
//! - run_control: continue/pause/step
//! - simulator_status: authoritative local simulator readiness

mod breakpoints;
mod initialize;
mod lifecycle;
mod run_control;
mod scopes;
mod simulator_status;
mod stack_trace;
mod threads;
