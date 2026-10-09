//! Portable numeric primitives selected by the runtime's numeric contract.
//!
//! Both hosted functions and core operators use the locked libm implementation.
//! Callers retain IEC promotion, narrowing and operation-specific fault handling.

pub use libm::{
    acos, asin, atan, atan2, atan2f, cos, exp, fabs, fabsf, log, log10, pow, round, sin, sqrt, tan,
    trunc,
};
