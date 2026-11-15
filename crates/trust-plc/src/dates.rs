//! The dates and times of day a Rust POU's variables have (specification 48 RU-25): `DATE`,
//! `TIME_OF_DAY` and `DATE_AND_TIME` and their long forms. Each holds the value as the program's
//! areas store it (specification 44 §6.2): milliseconds since 1970-01-01, a time of day's since
//! midnight, and nanoseconds for the long forms. Each is built, split and moved by a duration with
//! the program library's code, the code `CONCAT_DATE`, `SPLIT_DT`, `ADD_DT_TIME` and the other
//! functions of IEC §6.6.2.5.12 and §6.6.2.5.13 (Tables 36 and 37) run in Structured Text, with
//! their faults.

use core::cell::Cell;

use crate::types::{LTime, Placed, Stored, Time};
use crate::{Cx, Fault};

macro_rules! instant {
    ($(#[$doc:meta])* $name:ident, $from:ident, $as:ident, $unit:literal) => {
        $(#[$doc])*
        ///
        /// Its default is 1970-01-01 at midnight (specification 02).
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct $name(i64);

        impl $name {
            #[doc = concat!("The value `value` ", $unit, ", as the areas store it.")]
            #[must_use]
            pub const fn $from(value: i64) -> Self {
                Self(value)
            }

            #[doc = concat!("Its ", $unit, ", the value the areas store.")]
            #[must_use]
            pub const fn $as(self) -> i64 {
                self.0
            }
        }

        impl Stored for $name {
            type Raw = i64;
            #[inline]
            fn from_raw(raw: i64) -> Self {
                Self(raw)
            }
            #[inline]
            fn into_raw(self) -> i64 {
                self.0
            }
        }

        impl Placed for $name {
            type Area = Cell<i64>;
            #[inline]
            fn load(area: &Cell<i64>) -> Self {
                Self(area.get())
            }
            #[inline]
            fn store(&self, area: &Cell<i64>) {
                area.set(self.0);
            }
        }
    };
}

instant!(
    /// `DATE`: a day, held as the milliseconds from 1970-01-01 to its midnight.
    Date,
    from_millis,
    as_millis,
    "milliseconds from 1970-01-01"
);
instant!(
    /// `LDATE`: a day, held as the nanoseconds from 1970-01-01 to its midnight.
    LDate,
    from_nanos,
    as_nanos,
    "nanoseconds from 1970-01-01"
);
instant!(
    /// `TIME_OF_DAY`: a time of day, held as the milliseconds since midnight.
    Tod,
    from_millis,
    as_millis,
    "milliseconds since midnight"
);
instant!(
    /// `LTIME_OF_DAY`: a time of day, held as the nanoseconds since midnight.
    LTod,
    from_nanos,
    as_nanos,
    "nanoseconds since midnight"
);
instant!(
    /// `DATE_AND_TIME`: a day and a time of day, held as the milliseconds since 1970-01-01.
    Dt,
    from_millis,
    as_millis,
    "milliseconds since 1970-01-01"
);
instant!(
    /// `LDATE_AND_TIME`: a day and a time of day, held as the nanoseconds since 1970-01-01.
    Ldt,
    from_nanos,
    as_nanos,
    "nanoseconds since 1970-01-01"
);

impl Date {
    /// The day `year-month-day`, as `CONCAT_DATE` makes it.
    ///
    /// # Errors
    ///
    /// `Overflow` for a date that does not exist or that no `DATE` holds.
    pub fn from_ymd(cx: &mut Cx, year: i64, month: i64, day: i64) -> Result<Self, Fault> {
        trust_plc_lib::concat_date(cx.raw(), year, month, day).map(Self)
    }

    /// Its year, month and day, as `SPLIT_DATE` gives them.
    #[must_use]
    pub fn ymd(self) -> [i64; 3] {
        trust_plc_lib::split_date(self.0)
    }

    /// Its day of the week, 0 for a Sunday to 6 for a Saturday, as `DAY_OF_WEEK` gives it.
    #[must_use]
    pub fn day_of_week(self) -> i16 {
        trust_plc_lib::day_of_week(self.0)
    }

    /// The time from `earlier` to it, `DATE - DATE`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a difference no `TIME` holds.
    pub fn since(self, cx: &mut Cx, earlier: Self) -> Result<Time, Fault> {
        trust_plc_lib::ticks_diff(cx.raw(), self.0, earlier.0).map(Time::from_nanos)
    }
}

impl LDate {
    /// The day `year-month-day`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a date that does not exist or that no `LDATE` holds.
    pub fn from_ymd(cx: &mut Cx, year: i64, month: i64, day: i64) -> Result<Self, Fault> {
        let ticks = trust_plc_lib::concat_date(cx.raw(), year, month, day)?;
        trust_plc_lib::ticks_to_nanos(cx.raw(), ticks).map(Self)
    }

    /// Its year, month and day.
    #[must_use]
    pub fn ymd(self) -> [i64; 3] {
        trust_plc_lib::split_date(trust_plc_lib::ldt_to_date(self.0))
    }
}

impl Tod {
    /// The time of day `hour:minute:second.millisecond`, as `CONCAT_TOD` makes it.
    ///
    /// # Errors
    ///
    /// `Overflow` for a component outside its range.
    pub fn from_hms(
        cx: &mut Cx,
        hour: i64,
        minute: i64,
        second: i64,
        millisecond: i64,
    ) -> Result<Self, Fault> {
        trust_plc_lib::concat_tod(cx.raw(), hour, minute, second, millisecond).map(Self)
    }

    /// Its hour, minute, second and millisecond, as `SPLIT_TOD` gives them.
    ///
    /// # Errors
    ///
    /// `Overflow` for a time of day whose nanoseconds no `i64` holds.
    pub fn hms(self, cx: &mut Cx) -> Result<[i64; 4], Fault> {
        trust_plc_lib::split_tod(cx.raw(), self.0)
    }

    /// The time of day `duration` later, `TOD + TIME`: the duration in whole milliseconds toward
    /// zero, not wrapped at midnight.
    ///
    /// # Errors
    ///
    /// `DateTimeRange` for a result no `TIME_OF_DAY` holds.
    pub fn checked_add(self, cx: &mut Cx, duration: Time) -> Result<Self, Fault> {
        trust_plc_lib::ticks_add(cx.raw(), self.0, duration.as_nanos(), true).map(Self)
    }

    /// The time of day `duration` earlier, `TOD - TIME`.
    ///
    /// # Errors
    ///
    /// `DateTimeRange` for a result no `TIME_OF_DAY` holds.
    pub fn checked_sub(self, cx: &mut Cx, duration: Time) -> Result<Self, Fault> {
        trust_plc_lib::ticks_add(cx.raw(), self.0, duration.as_nanos(), false).map(Self)
    }

    /// The time from `earlier` to it, `TOD - TOD`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a difference no `TIME` holds.
    pub fn since(self, cx: &mut Cx, earlier: Self) -> Result<Time, Fault> {
        trust_plc_lib::ticks_diff(cx.raw(), self.0, earlier.0).map(Time::from_nanos)
    }
}

impl LTod {
    /// The time of day `hour:minute:second.millisecond`, as `CONCAT_LTOD` makes it.
    ///
    /// # Errors
    ///
    /// `Overflow` for a component outside its range.
    pub fn from_hms(
        cx: &mut Cx,
        hour: i64,
        minute: i64,
        second: i64,
        millisecond: i64,
    ) -> Result<Self, Fault> {
        trust_plc_lib::concat_ltod(cx.raw(), hour, minute, second, millisecond).map(Self)
    }

    /// Its hour, minute, second and millisecond, as `SPLIT_LTOD` gives them.
    #[must_use]
    pub fn hms(self) -> [i64; 4] {
        trust_plc_lib::split_ltod(self.0)
    }

    /// The time of day `duration` later, `LTOD + LTIME`, not wrapped at midnight.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result no `LTIME_OF_DAY` holds.
    pub fn checked_add(self, cx: &mut Cx, duration: LTime) -> Result<Self, Fault> {
        trust_plc_lib::nanos_add(cx.raw(), self.0, duration.as_nanos(), true).map(Self)
    }

    /// The time of day `duration` earlier, `LTOD - LTIME`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result no `LTIME_OF_DAY` holds.
    pub fn checked_sub(self, cx: &mut Cx, duration: LTime) -> Result<Self, Fault> {
        trust_plc_lib::nanos_add(cx.raw(), self.0, duration.as_nanos(), false).map(Self)
    }

    /// The time from `earlier` to it, `LTOD - LTOD`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a difference no `LTIME` holds.
    pub fn since(self, cx: &mut Cx, earlier: Self) -> Result<LTime, Fault> {
        trust_plc_lib::nanos_add(cx.raw(), self.0, earlier.0, false).map(LTime::from_nanos)
    }
}

impl Dt {
    /// The day `date` at the time of day `tod`, as `CONCAT_DATE_TOD` makes it.
    ///
    /// # Errors
    ///
    /// `DateTimeRange` for a sum no `DATE_AND_TIME` holds.
    pub fn new(cx: &mut Cx, date: Date, tod: Tod) -> Result<Self, Fault> {
        trust_plc_lib::concat_date_tod(cx.raw(), date.0, tod.0).map(Self)
    }

    /// `year-month-day-hour:minute:second.millisecond`, as `CONCAT_DT` makes it.
    ///
    /// # Errors
    ///
    /// `Overflow` for a date that does not exist, a component outside its range or a value no
    /// `DATE_AND_TIME` holds.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        cx: &mut Cx,
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        minute: i64,
        second: i64,
        millisecond: i64,
    ) -> Result<Self, Fault> {
        trust_plc_lib::concat_dt(
            cx.raw(),
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        )
        .map(Self)
    }

    /// Its year, month, day, hour, minute, second and millisecond, as `SPLIT_DT` gives them.
    #[must_use]
    pub fn parts(self) -> [i64; 7] {
        trust_plc_lib::split_dt(self.0)
    }

    /// Its day, as `DT_TO_DATE` gives it.
    ///
    /// # Errors
    ///
    /// `Overflow` on the first day of the range, whose midnight no `DATE` holds.
    pub fn date(self, cx: &mut Cx) -> Result<Date, Fault> {
        trust_plc_lib::dt_to_date(cx.raw(), self.0).map(Date)
    }

    /// Its time of day, as `DT_TO_TOD` gives it.
    #[must_use]
    pub fn tod(self) -> Tod {
        Tod(trust_plc_lib::dt_to_tod(self.0))
    }

    /// The moment `duration` later, `DT + TIME`: the duration in whole milliseconds toward zero.
    ///
    /// # Errors
    ///
    /// `DateTimeRange` for a result no `DATE_AND_TIME` holds.
    pub fn checked_add(self, cx: &mut Cx, duration: Time) -> Result<Self, Fault> {
        trust_plc_lib::ticks_add(cx.raw(), self.0, duration.as_nanos(), true).map(Self)
    }

    /// The moment `duration` earlier, `DT - TIME`.
    ///
    /// # Errors
    ///
    /// `DateTimeRange` for a result no `DATE_AND_TIME` holds.
    pub fn checked_sub(self, cx: &mut Cx, duration: Time) -> Result<Self, Fault> {
        trust_plc_lib::ticks_add(cx.raw(), self.0, duration.as_nanos(), false).map(Self)
    }

    /// The time from `earlier` to it, `DT - DT`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a difference no `TIME` holds.
    pub fn since(self, cx: &mut Cx, earlier: Self) -> Result<Time, Fault> {
        trust_plc_lib::ticks_diff(cx.raw(), self.0, earlier.0).map(Time::from_nanos)
    }
}

impl Ldt {
    /// The day `date` at the time of day `tod`, as `CONCAT_DATE_LTOD` makes it.
    ///
    /// # Errors
    ///
    /// `Overflow` for a sum no `LDATE_AND_TIME` holds.
    pub fn new(cx: &mut Cx, date: Date, tod: LTod) -> Result<Self, Fault> {
        trust_plc_lib::concat_date_ltod(cx.raw(), date.0, tod.0).map(Self)
    }

    /// `year-month-day-hour:minute:second.millisecond`, as `CONCAT_LDT` makes it.
    ///
    /// # Errors
    ///
    /// `Overflow` for a date that does not exist, a component outside its range or a value no
    /// `LDATE_AND_TIME` holds.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        cx: &mut Cx,
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        minute: i64,
        second: i64,
        millisecond: i64,
    ) -> Result<Self, Fault> {
        trust_plc_lib::concat_ldt(
            cx.raw(),
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        )
        .map(Self)
    }

    /// Its year, month, day, hour, minute, second and millisecond, as `SPLIT_LDT` gives them.
    #[must_use]
    pub fn parts(self) -> [i64; 7] {
        trust_plc_lib::split_ldt(self.0)
    }

    /// Its day, as `LDT_TO_DATE` gives it.
    #[must_use]
    pub fn date(self) -> Date {
        Date(trust_plc_lib::ldt_to_date(self.0))
    }

    /// Its time of day, as `LDT_TO_LTOD` gives it.
    #[must_use]
    pub fn tod(self) -> LTod {
        LTod(trust_plc_lib::ldt_to_ltod(self.0))
    }

    /// The moment `duration` later, `LDT + LTIME`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result no `LDATE_AND_TIME` holds.
    pub fn checked_add(self, cx: &mut Cx, duration: LTime) -> Result<Self, Fault> {
        trust_plc_lib::nanos_add(cx.raw(), self.0, duration.as_nanos(), true).map(Self)
    }

    /// The moment `duration` earlier, `LDT - LTIME`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result no `LDATE_AND_TIME` holds.
    pub fn checked_sub(self, cx: &mut Cx, duration: LTime) -> Result<Self, Fault> {
        trust_plc_lib::nanos_add(cx.raw(), self.0, duration.as_nanos(), false).map(Self)
    }

    /// The time from `earlier` to it, `LDT - LDT`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a difference no `LTIME` holds.
    pub fn since(self, cx: &mut Cx, earlier: Self) -> Result<LTime, Fault> {
        trust_plc_lib::nanos_add(cx.raw(), self.0, earlier.0, false).map(LTime::from_nanos)
    }
}
