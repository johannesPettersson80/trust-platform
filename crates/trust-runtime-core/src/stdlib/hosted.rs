//! Source-backed host compatibility; portable callers use the registry or typed APIs.
/// Assertion function registration.
pub mod assertions {
    pub use super::super::assertions::*;
}
/// Bit-string function registration.
pub mod bit {
    pub use super::super::bit::*;
}
/// Comparison function registration.
pub mod comparison {
    pub use super::super::comparison::*;
}
/// Shared standard-function binding and coercion helpers.
pub mod helpers {
    pub use super::super::helpers::*;
}
/// Numeric function registration.
pub mod numeric {
    pub use super::super::numeric::*;
}
/// Selection function registration.
pub mod selection {
    pub use super::super::selection::*;
}
/// String function registration.
pub mod string {
    pub use super::super::string::*;
}
/// Validation function registration.
pub mod validate {
    pub use super::super::validate::*;
}
/// Declared native function-block state layouts used by hosted source lowering.
pub mod state {
    pub use super::super::fbs::state::*;
}
