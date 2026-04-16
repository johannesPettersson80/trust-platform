TEST_PROGRAM oscat_basic_time_conversion_helpers_behave
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

TEST_PROGRAM oscat_basic_days_delta_behaves
VAR
DeltaForward : DINT;
DeltaBackward : DINT;
END_VAR

DeltaForward := DAYS_DELTA(date_1 := DATE#2026-04-15, date_2 := DATE#2026-04-20);
DeltaBackward := DAYS_DELTA(date_1 := DATE#2026-04-20, date_2 := DATE#2026-04-15);

ASSERT_EQUAL(EXPECTED := DINT#5, ACTUAL := DeltaForward);
ASSERT_EQUAL(EXPECTED := DINT#-5, ACTUAL := DeltaBackward);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_days_in_month_and_year_behave
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

TEST_PROGRAM oscat_basic_easter_behaves
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

TEST_PROGRAM oscat_basic_date_component_helpers_behave
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
SPLIT_DATE(ConstructedDate, ConstructedYear, ConstructedMonth, ConstructedDay);

ASSERT_EQUAL(EXPECTED := DINT#10, ACTUAL := DayCount);
ASSERT_EQUAL(EXPECTED := INT#29, ACTUAL := DayOfMonthValue);
ASSERT_EQUAL(EXPECTED := INT#366, ACTUAL := DayOfYearValue);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := MonthOfDateValue);
ASSERT_EQUAL(EXPECTED := INT#2024, ACTUAL := YearOfDateValue);
ASSERT_TRUE(IN := LeapYearValue);
ASSERT_TRUE(IN := LeapOfDateValue);
ASSERT_TRUE(IN := LeapDayValue);
ASSERT_EQUAL(EXPECTED := INT#2024, ACTUAL := ConstructedYear);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := ConstructedMonth);
ASSERT_EQUAL(EXPECTED := INT#29, ACTUAL := ConstructedDay);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_date_boundary_helpers_behave
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

TEST_PROGRAM oscat_basic_date_add_and_work_week_behave
VAR
AdjustedDate : DATE;
AdjustedYear : INT;
AdjustedMonth : INT;
AdjustedDay : INT;
WeekNumber : INT;
END_VAR

AdjustedDate := DATE_ADD(
    IDATE := DATE#2026-04-15,
    D := INT#1,
    W := INT#1,
    M := INT#1,
    Y := INT#1
);
WeekNumber := WORK_WEEK(idate := DATE#2026-04-15);
SPLIT_DATE(AdjustedDate, AdjustedYear, AdjustedMonth, AdjustedDay);

ASSERT_EQUAL(EXPECTED := INT#2027, ACTUAL := AdjustedYear);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := AdjustedMonth);
ASSERT_EQUAL(EXPECTED := INT#23, ACTUAL := AdjustedDay);
ASSERT_EQUAL(EXPECTED := INT#16, ACTUAL := WeekNumber);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_dt_component_helpers_behave
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

TEST_PROGRAM oscat_basic_tod_and_dt_constructor_helpers_behave
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
