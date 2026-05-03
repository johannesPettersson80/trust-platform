#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::eval::{ArgValue, CallArg, EvalContext};
use crate::memory::InstanceId;
use crate::stdlib::{time, StdParams};
use crate::value::Value;

use super::ast::{Expr, LValue};
use super::lvalue::{read_lvalue, resolve_reference_for_lvalue, write_lvalue};

include!("call/target_resolution.rs");
include!("call/arg_read.rs");
include!("call/stdlib_args.rs");
include!("call/split_call.rs");
include!("call/reference.rs");

#[cfg(test)]
mod tests {
    include!("call/tests.rs");
}
