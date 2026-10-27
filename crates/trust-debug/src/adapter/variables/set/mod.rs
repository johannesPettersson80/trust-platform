//! setVariable request handling.
//! - request: setVariable
//! - directive: parse force/release directives

mod directive;
mod request;

pub(in crate::adapter) use directive::{parse_set_directive, SetDirective};
