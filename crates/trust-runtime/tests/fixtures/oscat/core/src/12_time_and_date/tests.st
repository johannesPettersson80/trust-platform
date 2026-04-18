TEST_PROGRAM oscat_time_conversion_helpers_behave
VAR
DayTime : TIME;
HourTime : TIME;
MinuteTime : TIME;
SecondTime : TIME;
END_VAR

DayTime := DAY_TO_TIME(IN := REAL#1.5);
HourTime := HOUR_TO_TIME(IN := REAL#1.25);
MinuteTime := MINUTE_TO_TIME(IN := REAL#2.5);
SecondTime := SECOND_TO_TIME(IN := REAL#3.75);

ASSERT_EQUAL(EXPECTED := DWORD#129600000, ACTUAL := TIME_TO_DWORD(DayTime));
ASSERT_EQUAL(EXPECTED := DWORD#4500000, ACTUAL := TIME_TO_DWORD(HourTime));
ASSERT_EQUAL(EXPECTED := DWORD#150000, ACTUAL := TIME_TO_DWORD(MinuteTime));
ASSERT_EQUAL(EXPECTED := DWORD#3750, ACTUAL := TIME_TO_DWORD(SecondTime));
END_TEST_PROGRAM

TEST_PROGRAM oscat_days_delta_behaves
VAR
DeltaForward : DINT;
DeltaBackward : DINT;
END_VAR

DeltaForward := DAYS_DELTA(date_1 := DATE#2026-04-15, date_2 := DATE#2026-04-20);
DeltaBackward := DAYS_DELTA(date_1 := DATE#2026-04-20, date_2 := DATE#2026-04-15);

ASSERT_EQUAL(EXPECTED := DINT#5, ACTUAL := DeltaForward);
ASSERT_EQUAL(EXPECTED := DINT#-5, ACTUAL := DeltaBackward);
END_TEST_PROGRAM

TEST_PROGRAM oscat_days_in_month_and_year_behave
VAR
MonthDays : INT;
LeapMonthDays : INT;
YearDays : INT;
LeapYearDays : INT;
END_VAR

MonthDays := DAYS_IN_MONTH(IDATE := DATE#2026-04-15);
LeapMonthDays := DAYS_IN_MONTH(IDATE := DATE#2024-02-15);
YearDays := DAYS_IN_YEAR(IDATE := DATE#2025-06-01);
LeapYearDays := DAYS_IN_YEAR(IDATE := DATE#2024-06-01);

ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := MonthDays);
ASSERT_EQUAL(EXPECTED := INT#29, ACTUAL := LeapMonthDays);
ASSERT_EQUAL(EXPECTED := INT#365, ACTUAL := YearDays);
ASSERT_EQUAL(EXPECTED := INT#366, ACTUAL := LeapYearDays);
END_TEST_PROGRAM

TEST_PROGRAM oscat_easter_behaves
VAR
EasterDate : DATE;
EasterYear : INT;
EasterMonth : INT;
EasterDay : INT;
END_VAR

EasterDate := EASTER(year := INT#2026);
SPLIT_DATE(EasterDate, EasterYear, EasterMonth, EasterDay);

ASSERT_EQUAL(EXPECTED := INT#2026, ACTUAL := EasterYear);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := EasterMonth);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := EasterDay);
END_TEST_PROGRAM

TEST_PROGRAM oscat_date_component_helpers_behave
VAR
ConstructedDate : DATE;
DayCount : DINT;
DayOfMonthValue : INT;
DayOfYearValue : INT;
MonthOfDateValue : INT;
YearOfDateValue : INT;
LeapYearValue : BOOL;
LeapOfDateValue : BOOL;
LeapDayValue : BOOL;
CenturyLeap1900 : BOOL;
CenturyLeap2000 : BOOL;
CenturyLeap2100 : BOOL;
ConstructedYear : INT;
ConstructedMonth : INT;
ConstructedDay : INT;
END_VAR

ConstructedDate := SET_DATE(YEAR := INT#2024, MONTH := INT#2, DAY := INT#29);
DayCount := DAY_OF_DATE(IDATE := DATE#1970-01-11);
DayOfMonthValue := DAY_OF_MONTH(IDATE := DATE#2024-02-29);
DayOfYearValue := DAY_OF_YEAR(IDATE := DATE#2024-12-31);
MonthOfDateValue := MONTH_OF_DATE(IDATE := DATE#2024-02-29);
YearOfDateValue := YEAR_OF_DATE(IDATE := DATE#2024-02-29);
LeapYearValue := LEAP_YEAR(yr := INT#2024);
LeapOfDateValue := LEAP_OF_DATE(IDATE := DATE#2024-02-29);
LeapDayValue := LEAP_DAY(IDATE := DATE#2024-02-29);
CenturyLeap1900 := LEAP_YEAR(yr := INT#1900);
CenturyLeap2000 := LEAP_YEAR(yr := INT#2000);
CenturyLeap2100 := LEAP_YEAR(yr := INT#2100);
SPLIT_DATE(ConstructedDate, ConstructedYear, ConstructedMonth, ConstructedDay);

ASSERT_EQUAL(EXPECTED := DINT#10, ACTUAL := DayCount);
ASSERT_EQUAL(EXPECTED := INT#29, ACTUAL := DayOfMonthValue);
ASSERT_EQUAL(EXPECTED := INT#366, ACTUAL := DayOfYearValue);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := MonthOfDateValue);
ASSERT_EQUAL(EXPECTED := INT#2024, ACTUAL := YearOfDateValue);
ASSERT_TRUE(IN := LeapYearValue);
ASSERT_TRUE(IN := LeapOfDateValue);
ASSERT_TRUE(IN := LeapDayValue);
ASSERT_FALSE(IN := CenturyLeap1900);
ASSERT_TRUE(IN := CenturyLeap2000);
ASSERT_FALSE(IN := CenturyLeap2100);
ASSERT_EQUAL(EXPECTED := INT#2024, ACTUAL := ConstructedYear);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := ConstructedMonth);
ASSERT_EQUAL(EXPECTED := INT#29, ACTUAL := ConstructedDay);
END_TEST_PROGRAM

TEST_PROGRAM oscat_date_boundary_helpers_behave
VAR
MonthBeginDate : DATE;
MonthEndDate : DATE;
YearBeginDate : DATE;
YearEndDate : DATE;
MonthBeginYear : INT;
MonthBeginMonth : INT;
MonthBeginDay : INT;
MonthEndYear : INT;
MonthEndMonth : INT;
MonthEndDay : INT;
YearBeginYear : INT;
YearBeginMonth : INT;
YearBeginDay : INT;
YearEndYear : INT;
YearEndMonth : INT;
YearEndDay : INT;
END_VAR

MonthBeginDate := MONTH_BEGIN(IDATE := DATE#2024-02-29);
MonthEndDate := MONTH_END(IDATE := DATE#2024-02-15);
YearBeginDate := YEAR_BEGIN(y := INT#2024);
YearEndDate := YEAR_END(y := INT#2024);
SPLIT_DATE(MonthBeginDate, MonthBeginYear, MonthBeginMonth, MonthBeginDay);
SPLIT_DATE(MonthEndDate, MonthEndYear, MonthEndMonth, MonthEndDay);
SPLIT_DATE(YearBeginDate, YearBeginYear, YearBeginMonth, YearBeginDay);
SPLIT_DATE(YearEndDate, YearEndYear, YearEndMonth, YearEndDay);

ASSERT_EQUAL(EXPECTED := INT#2024, ACTUAL := MonthBeginYear);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := MonthBeginMonth);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := MonthBeginDay);
ASSERT_EQUAL(EXPECTED := INT#2024, ACTUAL := MonthEndYear);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := MonthEndMonth);
ASSERT_EQUAL(EXPECTED := INT#29, ACTUAL := MonthEndDay);
ASSERT_EQUAL(EXPECTED := INT#2024, ACTUAL := YearBeginYear);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := YearBeginMonth);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := YearBeginDay);
ASSERT_EQUAL(EXPECTED := INT#2024, ACTUAL := YearEndYear);
ASSERT_EQUAL(EXPECTED := INT#12, ACTUAL := YearEndMonth);
ASSERT_EQUAL(EXPECTED := INT#31, ACTUAL := YearEndDay);
END_TEST_PROGRAM

TEST_PROGRAM oscat_date_add_and_work_week_behave
VAR
AdjustedDate : DATE;
AdjustedYear : INT;
AdjustedMonth : INT;
AdjustedDay : INT;
WeekNumber : INT;
END_VAR

AdjustedDate := DATE_ADD(DATE#2026-04-15, INT#1, INT#1, INT#1, INT#1);
WeekNumber := WORK_WEEK(idate := DATE#2026-04-15);
SPLIT_DATE(AdjustedDate, AdjustedYear, AdjustedMonth, AdjustedDay);

ASSERT_EQUAL(EXPECTED := INT#2027, ACTUAL := AdjustedYear);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := AdjustedMonth);
ASSERT_EQUAL(EXPECTED := INT#23, ACTUAL := AdjustedDay);
ASSERT_EQUAL(EXPECTED := INT#16, ACTUAL := WeekNumber);
END_TEST_PROGRAM

TEST_PROGRAM oscat_dt_component_helpers_behave
VAR
HourPart : INT;
MinutePart : INT;
SecondPart : INT;
END_VAR

HourPart := HOUR_OF_DT(XDT := DT#2026-04-15-13:14:15);
MinutePart := MINUTE_OF_DT(XDT := DT#2026-04-15-13:14:15);
SecondPart := SECOND_OF_DT(XDT := DT#2026-04-15-13:14:15);

ASSERT_EQUAL(EXPECTED := INT#13, ACTUAL := HourPart);
ASSERT_EQUAL(EXPECTED := INT#14, ACTUAL := MinutePart);
ASSERT_EQUAL(EXPECTED := INT#15, ACTUAL := SecondPart);
END_TEST_PROGRAM

TEST_PROGRAM oscat_tod_and_dt_constructor_helpers_behave
VAR
    HourPart : INT;
    MinutePart : INT;
    SecondPart : REAL;
    HalfDay : TOD;
    RebuiltTod : TOD;
    RebuiltDt : DT;
END_VAR

HourPart := HOUR(ITOD := TOD#13:14:15.250);
MinutePart := MINUTE(ITOD := TOD#13:14:15.250);
SecondPart := SECOND(ITOD := TOD#13:14:15.250);
HalfDay := HOUR_TO_TOD(IN := REAL#13.5);
RebuiltTod := SET_TOD(HOUR := INT#13, MINUTE := INT#14, SECOND := REAL#15.250);
RebuiltDt := SET_DT(
    YEAR := INT#2026,
    MONTH := INT#4,
    DAY := INT#15,
    HOUR := INT#13,
    MINUTE := INT#14,
    SECOND := INT#15
);

ASSERT_EQUAL(EXPECTED := INT#13, ACTUAL := HourPart);
ASSERT_EQUAL(EXPECTED := INT#14, ACTUAL := MinutePart);
ASSERT_NEAR(EXPECTED := REAL#15.25, ACTUAL := SecondPart, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := TOD#13:30:00, ACTUAL := HalfDay);
ASSERT_EQUAL(EXPECTED := TOD#13:14:15.250, ACTUAL := RebuiltTod);
ASSERT_EQUAL(EXPECTED := DT#2026-04-15-13:14:15, ACTUAL := RebuiltDt);
END_TEST_PROGRAM

TEST_PROGRAM oscat_time_multiplication_helper_behaves
VAR
ScaledTime : TIME;
END_VAR

ScaledTime := MULTIME(t := T#2s, M := REAL#1.5);
ASSERT_EQUAL(EXPECTED := DWORD#3000, ACTUAL := TIME_TO_DWORD(ScaledTime));
END_TEST_PROGRAM

TEST_PROGRAM oscat_time_and_date_weekday_and_timecheck_behave
ASSERT_EQUAL(EXPECTED := INT#7, ACTUAL := DAY_OF_WEEK(IDATE := DATE#1970-01-04));
ASSERT_TRUE(IN := TIMECHECK(TD := TOD#23:30:00, START := TOD#22:00:00, STOP := TOD#01:00:00));
ASSERT_FALSE(IN := TIMECHECK(TD := TOD#12:00:00, START := TOD#22:00:00, STOP := TOD#01:00:00));
END_TEST_PROGRAM

TEST_PROGRAM oscat_structured_date_time_helpers_behave
VAR
StructuredFromParts : SDT;
StructuredFromDateTime : SDT;
END_VAR

StructuredFromParts := DT2_TO_SDT(
    DI := DATE#2026-04-15,
    TI := TOD#13:14:15.250
);
StructuredFromDateTime := DT_TO_SDT(
    DTI := CONCAT_DATE_TOD(DATE#2026-04-15, TOD#13:14:15.250)
);

ASSERT_EQUAL(EXPECTED := INT#2026, ACTUAL := StructuredFromParts.YEAR);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := StructuredFromParts.MONTH);
ASSERT_EQUAL(EXPECTED := INT#15, ACTUAL := StructuredFromParts.DAY);
ASSERT_EQUAL(EXPECTED := INT#13, ACTUAL := StructuredFromParts.HOUR);
ASSERT_EQUAL(EXPECTED := INT#14, ACTUAL := StructuredFromParts.MINUTE);
ASSERT_EQUAL(EXPECTED := INT#15, ACTUAL := StructuredFromParts.SECOND);
ASSERT_EQUAL(EXPECTED := INT#250, ACTUAL := StructuredFromParts.MS);
ASSERT_EQUAL(EXPECTED := DATE#2026-04-15, ACTUAL := SDT_TO_DATE(DTI := StructuredFromParts));
ASSERT_EQUAL(EXPECTED := DT#2026-04-15-13:14:15, ACTUAL := SDT_TO_DT(DTI := StructuredFromDateTime));
ASSERT_EQUAL(EXPECTED := TOD#13:14:15.250, ACTUAL := SDT_TO_TOD(DTI := StructuredFromParts));
END_TEST_PROGRAM

TEST_PROGRAM oscat_time_zone_period_and_dst_helpers_behave
VAR
LocalDateTime : DT;
UtcAgain : DT;
Periods : ARRAY[0..3, 0..1] OF DATE;
END_VAR

Periods[0, 0] := DATE#2026-01-01;
Periods[0, 1] := DATE#2026-01-10;
Periods[1, 0] := DATE#2026-04-01;
Periods[1, 1] := DATE#2026-04-05;

LocalDateTime := UTC_TO_LTIME(
    UTC := DT#2026-06-01-12:00:00,
    DST_ENABLE := TRUE,
    TIME_ZONE_OFFSET := INT#120
);
UtcAgain := LTIME_TO_UTC(
    LOCAL_DT := LocalDateTime,
    DST := TRUE,
    TIME_ZONE_OFFSET := INT#120
);

ASSERT_TRUE(IN := DST(UTC := DT#2026-06-01-12:00:00));
ASSERT_FALSE(IN := DST(UTC := DT#2026-01-01-12:00:00));
ASSERT_EQUAL(EXPECTED := DT#2026-06-01-15:00:00, ACTUAL := LocalDateTime);
ASSERT_EQUAL(EXPECTED := DT#2026-06-01-12:00:00, ACTUAL := UtcAgain);
ASSERT_TRUE(IN := PERIOD(d1 := DATE#2026-12-20, dx := DATE#2027-01-02, d2 := DATE#2026-01-10));
ASSERT_TRUE(IN := PERIOD2(DP := Periods, DX := DATE#2026-04-03));
END_TEST_PROGRAM

TEST_PROGRAM oscat_sun_and_julian_helpers_behave
VAR
NoonOffset : REAL;
RefractionNearHorizon : REAL;
Midday : TOD;
Sun : SUN_TIME;
END_VAR

NoonOffset := JD2000(DTI := DT#2000-01-01-12:00:00);
RefractionNearHorizon := REFRACTION(ELEV := REAL#0.0);
Midday := SUN_MIDDAY(LON := REAL#13.0, UTC := DATE#2026-06-21);
Sun(LATITUDE := REAL#52.0, LONGITUDE := REAL#13.0, UTC := DATE#2026-06-21);

ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := NoonOffset, DELTA := REAL#0.001);
ASSERT_TRUE(IN := RefractionNearHorizon > REAL#0.0);
ASSERT_TRUE(IN := Midday > TOD#11:00:00);
ASSERT_TRUE(IN := Midday < TOD#13:00:00);
ASSERT_TRUE(IN := Sun.SUN_RISE < Sun.MIDDAY);
ASSERT_TRUE(IN := Sun.SUN_SET > Sun.MIDDAY);
END_TEST_PROGRAM

TEST_PROGRAM oscat_calendar_event_and_clock_helpers_behave
VAR
ConstantsReady : BOOL;
HolidayList : ARRAY[0..29] OF HOLIDAY_DATA;
EventList : ARRAY[0..49] OF HOLIDAY_DATA;
HolidayCheck : HOLIDAY;
EventCheck : EVENTS;
CalendarValue : CALENDAR;
CalendarCalc : CALENDAR_CALC;
RtcMs : RTC_MS;
Rtc2 : RTC_2;
DcfClock : DCF77;
END_VAR

ConstantsReady := OSCAT_BASIC_Constants();

HolidayList[0].NAME := 'XMAS';
HolidayList[0].DAY := SINT#25;
HolidayList[0].MONTH := SINT#12;
HolidayList[0].USE := SINT#1;

EventList[0].NAME := 'WEEK';
EventList[0].DAY := SINT#15;
EventList[0].MONTH := SINT#4;
EventList[0].USE := SINT#3;

HolidayCheck(DATE_IN := DATE#2026-12-25, LANGU := INT#1, HOLIDAYS := HolidayList);
EventCheck(DATE_IN := DATE#2026-04-16, ENA := TRUE, ELIST := EventList);

RtcMs(SET_RTC := TRUE, SDT := DT#2026-06-01-12:00:00, SMS := INT#123);
Rtc2(SET_RTC := TRUE, SDT := DT#2026-06-01-12:00:00, SMS := INT#250, DEN := TRUE, OFS := INT#120);

CalendarValue.UTC := DT#2026-06-01-12:00:00;
CalendarValue.DST_EN := TRUE;
CalendarValue.OFFSET := INT#120;
CalendarValue.LATITUDE := REAL#0.0;
CalendarValue.LONGITUDE := REAL#0.0;
CalendarValue.LANGUAGE := INT#1;
CalendarCalc(XCAL := CalendarValue, HOLIDAYS := HolidayList);

DcfClock(SET_RTC := TRUE, SDT := DT#2026-06-01-12:00:00, DSI := FALSE);

ASSERT_TRUE(IN := ConstantsReady);
ASSERT_TRUE(IN := HolidayCheck.Y);
ASSERT_EQUAL(EXPECTED := 'XMAS', ACTUAL := HolidayCheck.NAME);
ASSERT_TRUE(IN := EventCheck.Y);
ASSERT_EQUAL(EXPECTED := 'WEEK', ACTUAL := EventCheck.NAME);
ASSERT_EQUAL(EXPECTED := DT#2026-06-01-12:00:00, ACTUAL := RtcMs.XDT);
ASSERT_EQUAL(EXPECTED := INT#123, ACTUAL := RtcMs.XMS);
ASSERT_EQUAL(EXPECTED := DT#2026-06-01-15:00:00, ACTUAL := Rtc2.LOCAL_DT);
ASSERT_EQUAL(EXPECTED := DATE#2026-06-01, ACTUAL := CalendarValue.LOCAL_DATE);
ASSERT_TRUE(IN := CalendarValue.SUN_SET > CalendarValue.SUN_RISE);
ASSERT_TRUE(IN := DcfClock.TP);
ASSERT_EQUAL(EXPECTED := DT#2026-06-01-12:00:00, ACTUAL := DcfClock.RTC);
ASSERT_EQUAL(EXPECTED := DT#2026-06-01-13:00:00, ACTUAL := DcfClock.RTC1);
END_TEST_PROGRAM
