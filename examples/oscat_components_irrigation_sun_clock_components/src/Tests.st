TEST_PROGRAM IrrigationSunClockComponentMatchesClassicSummerDate
VAR
    Clock : CalendarClock;
    ClockObject : ICalendarClock;
    CalendarData : CALENDAR;
    Holidays : ARRAY[0..29] OF HOLIDAY_DATA;
    CalendarCalculator : CALENDAR_CALC;
END_VAR

ClockObject := Clock;
ClockObject.Initialize();
ClockObject.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    Language := INT#1
);
ClockObject.Update(Utc := DT#2026-07-01-02:00:00);

CalendarData.UTC := DT#2026-07-01-02:00:00;
CalendarData.DST_EN := TRUE;
CalendarData.OFFSET := INT#60;
CalendarData.LANGUAGE := INT#1;
CalendarData.LATITUDE := REAL#59.3293;
CalendarData.LONGITUDE := REAL#18.0686;
CalendarCalculator(SPE := TRUE, H := REAL#-0.8333333, XCAL := CalendarData, HOLIDAYS := Holidays);

ASSERT_EQUAL(EXPECTED := CalendarData.YEAR, ACTUAL := ClockObject.Year);
ASSERT_EQUAL(EXPECTED := CalendarData.MONTH, ACTUAL := ClockObject.Month);
ASSERT_EQUAL(EXPECTED := CalendarData.DAY, ACTUAL := ClockObject.Day);
END_TEST_PROGRAM
