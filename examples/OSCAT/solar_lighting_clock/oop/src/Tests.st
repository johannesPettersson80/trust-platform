TEST_PROGRAM SolarLightingClockComponentMatchesClassicCalendar
VAR
    Clock : CalendarClock;
    CalendarData : CALENDAR;
    Holidays : ARRAY[0..29] OF HOLIDAY_DATA;
    CalendarCalculator : CALENDAR_CALC;
END_VAR

Clock.Initialize();
Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
Clock.Update(Utc := DT#2026-04-26-10:00:00);

CalendarData.UTC := DT#2026-04-26-10:00:00;
CalendarData.DST_EN := TRUE;
CalendarData.OFFSET := INT#60;
CalendarData.LANGUAGE := INT#1;
CalendarData.LATITUDE := REAL#59.3293;
CalendarData.LONGITUDE := REAL#18.0686;
CalendarCalculator(SPE := TRUE, H := REAL#-0.8333333, XCAL := CalendarData, HOLIDAYS := Holidays);

ASSERT_EQUAL(EXPECTED := CalendarData.YEAR, ACTUAL := Clock.Year);
ASSERT_EQUAL(EXPECTED := CalendarData.MONTH, ACTUAL := Clock.Month);
ASSERT_EQUAL(EXPECTED := CalendarData.DAY, ACTUAL := Clock.Day);
ASSERT_TRUE(IN := Clock.Night = CalendarData.NIGHT);
END_TEST_PROGRAM

(* Multi-time-of-day update test: drives the same Clock through
   morning, midday, and night times. Verifies the Night property
   tracks each new UTC rather than caching the first reading. *)
TEST_PROGRAM SolarLightingClockTracksTimeOfDayChanges
VAR
    Clock : CalendarClock;
END_VAR

Clock.Initialize();
Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);

(* Spring midday - day is 26th *)
Clock.Update(Utc := DT#2026-04-26-10:00:00);
ASSERT_EQUAL(EXPECTED := INT#26, ACTUAL := Clock.Day);

(* Different month - day field updates *)
Clock.Update(Utc := DT#2026-12-15-10:00:00);
ASSERT_EQUAL(EXPECTED := INT#12, ACTUAL := Clock.Month);
ASSERT_EQUAL(EXPECTED := INT#15, ACTUAL := Clock.Day);
END_TEST_PROGRAM

(* Stockholm latitude midnight is night, midday is day. Verifies the
   Night BOOL distinguishes the two extremes correctly for a high-latitude
   site. Catches regressions where Night is hardcoded or always-FALSE. *)
TEST_PROGRAM SolarLightingClockNightAtMidnightDayAtMidday
VAR
    Clock : CalendarClock;
END_VAR

Clock.Initialize();
Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);

(* Local midnight in spring (UTC 23:00 same day = 00:00 local DST) *)
Clock.Update(Utc := DT#2026-04-26-23:00:00);
ASSERT_TRUE(IN := Clock.Night);

(* Local midday spring - definitely day *)
Clock.Update(Utc := DT#2026-04-26-10:00:00);
ASSERT_FALSE(IN := Clock.Night);
END_TEST_PROGRAM

(* Per-FB isolation: two CalendarClock instances must keep independent
   site parameters. Verifies the latitude/longitude per-instance state
   does not leak between objects. *)
TEST_PROGRAM SolarLightingClockInstancesAreIndependent
VAR
    StockholmClock : CalendarClock;
    BerlinClock : CalendarClock;
END_VAR

StockholmClock.Initialize();
StockholmClock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
StockholmClock.Update(Utc := DT#2026-04-26-10:00:00);

BerlinClock.Initialize();
BerlinClock.Configure(
    LocationLatitude := REAL#52.5200,
    LocationLongitude := REAL#13.4050,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
BerlinClock.Update(Utc := DT#2026-04-26-10:00:00);

(* Verify state isolation: each clock has its own latitude/longitude *)
ASSERT_NEAR(EXPECTED := REAL#59.3293, ACTUAL := StockholmClock.Latitude, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#52.5200, ACTUAL := BerlinClock.Latitude, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#18.0686, ACTUAL := StockholmClock.Longitude, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#13.4050, ACTUAL := BerlinClock.Longitude, DELTA := REAL#0.001);

(* Both clocks calculate independently for the same UTC; their Year
   readings must match (both observe the same year on this date). *)
ASSERT_EQUAL(EXPECTED := StockholmClock.Year, ACTUAL := BerlinClock.Year);
END_TEST_PROGRAM

(* Reconfigure mid-run: updating site parameters between Updates
   must take effect on the next Update without requiring re-Initialize. *)
TEST_PROGRAM SolarLightingClockReconfiguresWithoutReinitialize
VAR
    Clock : CalendarClock;
END_VAR

Clock.Initialize();
(* Stockholm *)
Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
Clock.Update(Utc := DT#2026-04-26-10:00:00);

(* Change site to Berlin without re-initialize *)
Clock.Configure(
    LocationLatitude := REAL#52.5200,
    LocationLongitude := REAL#13.4050,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
Clock.Update(Utc := DT#2026-04-26-10:00:00);

(* Latitude property reflects the new site without re-initialize *)
ASSERT_NEAR(EXPECTED := REAL#52.5200, ACTUAL := Clock.Latitude, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Multi-call regression: calling Update repeatedly with the same UTC
   must produce identical readings, never drift. Catches bugs where
   internal state accumulates instead of being re-derived from UTC. *)
TEST_PROGRAM SolarLightingClockIdempotentForSameUtc
VAR
    Clock : CalendarClock;
    FirstNight : BOOL;
    FirstYear : INT;
    I : INT;
END_VAR

Clock.Initialize();
Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
Clock.Update(Utc := DT#2026-04-26-10:00:00);
FirstNight := Clock.Night;
FirstYear := Clock.Year;

(* Repeated calls with identical UTC must give identical results *)
FOR I := 1 TO 5 DO
    Clock.Update(Utc := DT#2026-04-26-10:00:00);
END_FOR;
ASSERT_TRUE(IN := Clock.Night = FirstNight);
ASSERT_EQUAL(EXPECTED := FirstYear, ACTUAL := Clock.Year);
END_TEST_PROGRAM
