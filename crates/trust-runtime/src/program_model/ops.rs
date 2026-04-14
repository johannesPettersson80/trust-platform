//! Operator implementations shared by the VM and helper evaluators.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::numeric::{
    numeric_kind, signed_from_i128, to_f64, to_i64, to_u64, unsigned_from_u128, wider_numeric,
    NumericKind,
};
use crate::value::{
    DateTimeProfile, DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue, Value,
};

include!("../eval/ops/contracts.rs");
include!("../eval/ops/logical_cmp.rs");
include!("../eval/ops/time_ops.rs");
include!("../eval/ops/numeric_arith.rs");
