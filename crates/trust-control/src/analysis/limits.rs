//! The limits of Control Studio's analysis tools (specification 35, A7; numerical annex N4, N7). Each
//! limit is enforced where its quantity is checked, and quoted from here.

/// The largest degree of a polynomial in a Routh table: sixteen, so seventeen coefficients (N7.2).
pub const ROUTH_MAXIMUM_COEFFICIENTS: usize = 17;

/// The most coefficients of a declared polynomial, a numerator or a denominator (N7.6).
pub const DECLARED_MAXIMUM_COEFFICIENTS: usize = 17;

/// The most parameters of a declared controller.
pub const DECLARED_MAXIMUM_PARAMETERS: usize = 64;

/// The most values of a declared parameter sweep (N7.6).
pub const DECLARED_MAXIMUM_SWEEP_VALUES: usize = 1_001;

/// The evenly spaced gains of a declared pole path across the parameter's whole range.
pub const POLE_PATH_POINTS: usize = 201;

/// The most poles stored for one gain of a declared sweep.
pub const STORED_POLES_PER_GAIN: usize = 34;

/// The most frequency points of a general feedback analysis.
pub const FEEDBACK_MAXIMUM_FREQUENCIES: usize = 4_096;

/// The number of samples per decade that a bounded margin estimate may use, `⌊511 / decades⌋` (N7.8).
pub const MARGIN_POINT_BUDGET: f64 = 511.0;

/// The fewest and the most points per decade of a Nyquist contour of a general feedback analysis.
pub const NYQUIST_MINIMUM_POINTS_PER_DECADE: u32 = 10;

/// See [`NYQUIST_MINIMUM_POINTS_PER_DECADE`].
pub const NYQUIST_MAXIMUM_POINTS_PER_DECADE: u32 = 100;

/// The highest degree of a characteristic polynomial whose stable parameter region is solved. The
/// specification's N7.4 names degrees one and two; this bound is the implemented solver's own (see the
/// report that accompanies this module).
pub const INTERVAL_MAXIMUM_DEGREE: usize = 16;
