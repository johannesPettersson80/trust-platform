//! Hosted standard library facade. Scalar semantics live in the portable core.

pub mod fbs;
pub mod time;
pub use trust_runtime_core::stdlib::hosted::{
    assertions, bit, comparison, helpers, numeric, selection, string, validate,
};
pub use trust_runtime_core::stdlib::{
    conversions, StandardLibrary, StdFunc, StdFunction, StdParams,
};
