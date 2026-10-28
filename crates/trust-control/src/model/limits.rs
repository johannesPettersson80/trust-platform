//! The product limits of Control Studio's model contract. The mathematical library takes these as
//! parameters; trust-control holds the values and the rules they enforce.

/// The most coefficients one transfer-function polynomial may have, numerator or denominator.
pub const MAXIMUM_POLYNOMIAL_LENGTH: usize = 17;

/// The most inputs, outputs or states one model may declare.
pub const MAXIMUM_CHANNELS: usize = 32;

/// The highest state-space order converted to transfer polynomials.
pub const MAXIMUM_CONVERSION_ORDER: usize = 16;

/// The most states one exact delayed realization may have.
pub const MAXIMUM_DELAY_STATES: usize = 512;

/// The most values one time response may hold: samples times outputs.
pub const MAXIMUM_TIME_RESPONSE_VALUES: usize = 4096;
