//! The resource (specification 11, §6.6 and §6.9): the semantics of its command queue, the
//! control operations that pause, resume, restart and stop it, their stable replies, and the
//! supervisor's decisions after a fault.
//!
//! Platforms keep the mechanisms (threads or a main loop, queues, waits and timers) and call these
//! rules at the same points.

pub mod commands;
pub mod operations;
pub mod replies;
pub mod supervisor;
