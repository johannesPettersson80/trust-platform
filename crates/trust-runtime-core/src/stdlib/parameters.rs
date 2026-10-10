//! Canonical immutable signatures shared across all built-in families.
//! A descriptor borrows one signature; repeated parameter lists are not expanded
//! into separate flash objects for every function.
use super::StdParams;
use alloc::borrow::Cow;
use smol_str::SmolStr;

pub(super) static EXPECTED_ACTUAL: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("EXPECTED"),
    SmolStr::new_inline("ACTUAL"),
]));
pub(super) static IN: StdParams = StdParams::Fixed(Cow::Borrowed(&[SmolStr::new_inline("IN")]));
pub(super) static VALUE_BOUND: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("VALUE"),
    SmolStr::new_inline("BOUND"),
]));
pub(super) static EXPECTED_ACTUAL_DELTA: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("EXPECTED"),
    SmolStr::new_inline("ACTUAL"),
    SmolStr::new_inline("DELTA"),
]));
pub(super) static VAR_IN_1_2: StdParams = StdParams::Variadic {
    fixed: Cow::Borrowed(&[]),
    prefix: SmolStr::new_inline("IN"),
    start: 1,
    min: 2,
};
pub(super) static Y_X: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("Y"),
    SmolStr::new_inline("X"),
]));
pub(super) static IN1_IN2: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("IN1"),
    SmolStr::new_inline("IN2"),
]));
pub(super) static IN_N: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("IN"),
    SmolStr::new_inline("N"),
]));
pub(super) static MN_IN_MX: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("MN"),
    SmolStr::new_inline("IN"),
    SmolStr::new_inline("MX"),
]));
pub(super) static VAR_K_IN_0_2: StdParams = StdParams::Variadic {
    fixed: Cow::Borrowed(&[SmolStr::new_inline("K")]),
    prefix: SmolStr::new_inline("IN"),
    start: 0,
    min: 2,
};
pub(super) static G_IN0_IN1: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("G"),
    SmolStr::new_inline("IN0"),
    SmolStr::new_inline("IN1"),
]));
pub(super) static IN_L_P: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("IN"),
    SmolStr::new_inline("L"),
    SmolStr::new_inline("P"),
]));
pub(super) static IN1_IN2_P: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("IN1"),
    SmolStr::new_inline("IN2"),
    SmolStr::new_inline("P"),
]));
pub(super) static IN_L: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("IN"),
    SmolStr::new_inline("L"),
]));
pub(super) static IN1_IN2_L_P: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("IN1"),
    SmolStr::new_inline("IN2"),
    SmolStr::new_inline("L"),
    SmolStr::new_inline("P"),
]));
pub(super) static YEAR_MONTH_DAY: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("YEAR"),
    SmolStr::new_inline("MONTH"),
    SmolStr::new_inline("DAY"),
]));
pub(super) static DATE_LTOD: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("DATE"),
    SmolStr::new_inline("LTOD"),
]));
pub(super) static DATE_TOD: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("DATE"),
    SmolStr::new_inline("TOD"),
]));
pub(super) static YEAR_MONTH_DAY_HOUR_MINUTE_SECOND_MILLISECOND: StdParams =
    StdParams::Fixed(Cow::Borrowed(&[
        SmolStr::new_inline("YEAR"),
        SmolStr::new_inline("MONTH"),
        SmolStr::new_inline("DAY"),
        SmolStr::new_inline("HOUR"),
        SmolStr::new_inline("MINUTE"),
        SmolStr::new_inline("SECOND"),
        SmolStr::new_inline("MILLISECOND"),
    ]));
pub(super) static HOUR_MINUTE_SECOND_MILLISECOND: StdParams = StdParams::Fixed(Cow::Borrowed(&[
    SmolStr::new_inline("HOUR"),
    SmolStr::new_inline("MINUTE"),
    SmolStr::new_inline("SECOND"),
    SmolStr::new_inline("MILLISECOND"),
]));
