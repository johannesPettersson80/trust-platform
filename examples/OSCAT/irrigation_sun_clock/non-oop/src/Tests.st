TEST_PROGRAM IrrigationSunClockClassicCalculatesSummerDate
VAR
    ConstantsReady : BOOL;
    CalendarData : CALENDAR;
    Holidays : ARRAY[0..29] OF HOLIDAY_DATA;
    CalendarCalculator : CALENDAR_CALC;
END_VAR

ConstantsReady := OSCAT_BASIC_Constants();
CalendarData.UTC := DT#2026-07-01-02:00:00;
CalendarData.DST_EN := TRUE;
CalendarData.OFFSET := INT#60;
CalendarData.LANGUAGE := INT#1;
CalendarData.LATITUDE := REAL#59.3293;
CalendarData.LONGITUDE := REAL#18.0686;
CalendarCalculator(SPE := TRUE, H := REAL#-0.8333333, XCAL := CalendarData, HOLIDAYS := Holidays);

ASSERT_TRUE(IN := ConstantsReady);
ASSERT_EQUAL(EXPECTED := INT#2026, ACTUAL := CalendarData.YEAR);
ASSERT_EQUAL(EXPECTED := INT#7, ACTUAL := CalendarData.MONTH);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := CalendarData.DAY);
END_TEST_PROGRAM
