//! The control connection's request and response envelope (specification 11, §6.8.1 "Runtime
//! control request, job, and state-surface contract"). Each request is one JSON object and each
//! response is one JSON object; how the bytes travel is the platform's link.

mod reply_writer;
mod request;
mod response;

pub(crate) use reply_writer::{ReplyWriter, Written};
pub use request::ControlRequest;
pub use response::{ControlAfterResponse, ControlResponse};

#[cfg(test)]
mod wire_tests;
