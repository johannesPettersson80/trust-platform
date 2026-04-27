TEST_PROGRAM IrrigationSunClockComponentMatchesClassicSummerDate
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
Clock.Update(Utc := DT#2026-07-01-02:00:00);

CalendarData.UTC := DT#2026-07-01-02:00:00;
CalendarData.DST_EN := TRUE;
CalendarData.OFFSET := INT#60;
CalendarData.LANGUAGE := INT#1;
CalendarData.LATITUDE := REAL#59.3293;
CalendarData.LONGITUDE := REAL#18.0686;
CalendarCalculator(SPE := TRUE, H := REAL#-0.8333333, XCAL := CalendarData, HOLIDAYS := Holidays);

ASSERT_EQUAL(EXPECTED := CalendarData.YEAR, ACTUAL := Clock.Year);
ASSERT_EQUAL(EXPECTED := CalendarData.MONTH, ACTUAL := Clock.Month);
ASSERT_EQUAL(EXPECTED := CalendarData.DAY, ACTUAL := Clock.Day);
END_TEST_PROGRAM

(* Multi-date update test: drives the same Clock through summer and
   winter dates without re-Configuring. Verifies the clock recomputes
   month/day from each new UTC rather than caching the first one. *)
TEST_PROGRAM IrrigationSunClockTracksDifferentDatesOnSameInstance
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

(* Summer date *)
Clock.Update(Utc := DT#2026-07-01-02:00:00);
ASSERT_EQUAL(EXPECTED := INT#7, ACTUAL := Clock.Month);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Clock.Day);

(* Winter date - month/day must update *)
Clock.Update(Utc := DT#2026-12-25-12:00:00);
ASSERT_EQUAL(EXPECTED := INT#12, ACTUAL := Clock.Month);
ASSERT_EQUAL(EXPECTED := INT#25, ACTUAL := Clock.Day);
END_TEST_PROGRAM

(* Multi-call regression: the wrapper must work correctly across many
   consecutive Update() calls within a single scan-cycle simulation.
   Tracks the day-of-month across an in-month sweep. *)
TEST_PROGRAM IrrigationSunClockSweepsAcrossDaysWithoutStaleState
VAR
    Clock : CalendarClock;
    DayCount : INT;
END_VAR

Clock.Initialize();
Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);

Clock.Update(Utc := DT#2026-06-15-12:00:00);
DayCount := Clock.Day;
ASSERT_EQUAL(EXPECTED := INT#15, ACTUAL := DayCount);

Clock.Update(Utc := DT#2026-06-16-12:00:00);
ASSERT_EQUAL(EXPECTED := INT#16, ACTUAL := Clock.Day);

Clock.Update(Utc := DT#2026-06-30-12:00:00);
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Clock.Day);
ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Clock.Month);

(* Cross month boundary in the same instance *)
Clock.Update(Utc := DT#2026-07-01-12:00:00);
ASSERT_EQUAL(EXPECTED := INT#7, ACTUAL := Clock.Month);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Clock.Day);
END_TEST_PROGRAM

(* Two-site isolation: two CalendarClock instances configured for
   different latitudes report independent calendar fields. The
   Stockholm clock and an equator clock should derive the same date
   from the same UTC but track their own state. *)
TEST_PROGRAM IrrigationSunClockTwoSitesAreIndependent
VAR
    Stockholm : CalendarClock;
    Equator : CalendarClock;
END_VAR

Stockholm.Initialize();
Stockholm.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
Equator.Initialize();
Equator.Configure(
    LocationLatitude := REAL#0.0,
    LocationLongitude := REAL#0.0,
    OffsetMinutes := INT#0,
    DstEnabled := FALSE,
    LanguageIndex := INT#1
);

Stockholm.Update(Utc := DT#2026-06-21-12:00:00);
Equator.Update(Utc := DT#2026-06-21-12:00:00);

(* Both at noon UTC on the same date *)
ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Stockholm.Month);
ASSERT_EQUAL(EXPECTED := INT#21, ACTUAL := Stockholm.Day);
ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Equator.Month);
ASSERT_EQUAL(EXPECTED := INT#21, ACTUAL := Equator.Day);

(* Latitudes are reflected by each clock's configuration *)
ASSERT_NEAR(EXPECTED := REAL#59.3293, ACTUAL := Stockholm.Latitude, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Equator.Latitude, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Configure-after-Update regression: a runtime reconfigure with new
   location must be honoured on the next Update without leaking the
   prior site's state. *)
TEST_PROGRAM IrrigationSunClockAcceptsRuntimeReconfigure
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
Clock.Update(Utc := DT#2026-07-01-02:00:00);
ASSERT_NEAR(EXPECTED := REAL#59.3293, ACTUAL := Clock.Latitude, DELTA := REAL#0.001);

(* Runtime move to a different site *)
Clock.Configure(
    LocationLatitude := REAL#-33.8688,
    LocationLongitude := REAL#151.2093,
    OffsetMinutes := INT#600,
    DstEnabled := FALSE,
    LanguageIndex := INT#1
);
Clock.Update(Utc := DT#2026-07-01-02:00:00);
ASSERT_NEAR(EXPECTED := REAL#-33.8688, ACTUAL := Clock.Latitude, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := INT#600, ACTUAL := Clock.OffsetMinutes);
END_TEST_PROGRAM

(* Lifecycle pin: Initialize then Snapshot reports Ready. The wrapper
   must not surface an Error after a clean Initialize/Configure. *)
TEST_PROGRAM IrrigationSunClockSnapshotIsReadyAfterInitialize
VAR
    Clock : CalendarClock;
    Status : ComponentStatus;
END_VAR

Clock.Initialize();
Status := Clock.Snapshot();
ASSERT_TRUE(IN := Status.Ready);
ASSERT_FALSE(IN := Status.Error);
ASSERT_EQUAL(EXPECTED := ComponentErrorNone, ACTUAL := Status.ErrorId);

Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
Status := Clock.Snapshot();
ASSERT_TRUE(IN := Status.Ready);
ASSERT_FALSE(IN := Status.Error);
END_TEST_PROGRAM
