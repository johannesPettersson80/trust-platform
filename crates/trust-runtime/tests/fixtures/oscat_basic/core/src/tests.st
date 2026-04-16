TYPE OSCAT_NESTED_ITEM :
STRUCT
    VALUE : INT;
END_STRUCT
END_TYPE

TYPE OSCAT_NESTED_OUTER :
STRUCT
    ARR : ARRAY[0..2] OF INT;
END_STRUCT
END_TYPE

TEST_PROGRAM oscat_basic_constants_resolve_and_match_ported_values
ASSERT_TRUE(IN := OSCAT_BASIC_Constants());
ASSERT_EQUAL(EXPECTED := INT#250, ACTUAL := STRING_LENGTH);
ASSERT_EQUAL(EXPECTED := INT#250, ACTUAL := LIST_LENGTH);
ASSERT_NEAR(EXPECTED := REAL#6.283185307179586, ACTUAL := MATH.PI2, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#-273.15, ACTUAL := PHYS.T0, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#101325.0, ACTUAL := PHYS.PN, DELTA := REAL#0.1);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := LANGUAGE.DEFAULT);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := LANGUAGE.LMAX);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_temperature_conversions_behave
ASSERT_TRUE(IN := OSCAT_BASIC_Constants());
ASSERT_NEAR(EXPECTED := REAL#77.0, ACTUAL := C_TO_F(celsius := REAL#25.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#25.0, ACTUAL := F_TO_C(fahrenheit := REAL#77.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#298.15, ACTUAL := C_TO_K(Celsius := REAL#25.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#25.0, ACTUAL := K_TO_C(Kelvin := REAL#298.15), DELTA := REAL#0.01);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_speed_and_frequency_conversions_behave
VAR
    SpeedMps : REAL;
    SpeedKmh : REAL;
    AngularFrequency : REAL;
    RoundTripFrequency : REAL;
    Period : TIME;
END_VAR

ASSERT_TRUE(IN := OSCAT_BASIC_Constants());
SpeedMps := KMH_TO_MS(kmh := REAL#36.0);
SpeedKmh := MS_TO_KMH(ms := SpeedMps);
AngularFrequency := F_TO_OM(F := REAL#2.0);
RoundTripFrequency := OM_TO_F(OM := AngularFrequency);
Period := F_TO_PT(F := REAL#2.0);

ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := SpeedMps, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#36.0, ACTUAL := SpeedKmh, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := MATH.PI2 * REAL#2.0, ACTUAL := AngularFrequency, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := RoundTripFrequency, DELTA := REAL#0.01);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := MS_TO_BFT(MS := BFT_TO_MS(BFT := INT#5)));
ASSERT_EQUAL(EXPECTED := T#500ms, ACTUAL := Period);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := PT_TO_F(PT := T#500ms), DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_direction_helpers_behave
ASSERT_TRUE(IN := OSCAT_BASIC_Constants());
ASSERT_EQUAL(EXPECTED := 'N', ACTUAL := DEG_TO_DIR(DEG := INT#0, N := INT#1, L := INT#1));
ASSERT_EQUAL(EXPECTED := 'E', ACTUAL := DEG_TO_DIR(DEG := INT#90, N := INT#1, L := INT#1));
ASSERT_EQUAL(EXPECTED := 'O', ACTUAL := DEG_TO_DIR(DEG := INT#90, N := INT#1, L := INT#2));
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := DIR_TO_DEG(DIR := 'E', L := INT#1));
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := DIR_TO_DEG(DIR := 'O', L := INT#2));
ASSERT_EQUAL(EXPECTED := 'E', ACTUAL := LANGUAGE.DIRS[INT#1, INT#4]);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_comparison_regression_behaves
ASSERT_TRUE(IN := 'ABC' = 'ABC');
ASSERT_TRUE(IN := 'ABC' < 'ABD');
ASSERT_TRUE(IN := "ABC" = "ABC");
ASSERT_TRUE(IN := "ABC" < "ABD");
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_case_regression_behaves
VAR
    TextValue : STRING := 'E';
    WideValue : WSTRING := "B";
    DirectionIndex : INT := INT#0;
    WideIndex : INT := INT#0;
END_VAR

CASE TextValue OF
    'N':
        DirectionIndex := INT#1;
    'E':
        DirectionIndex := INT#2;
    ELSE
        DirectionIndex := INT#3;
END_CASE;

CASE WideValue OF
    "A":
        WideIndex := INT#1;
    "B":
        WideIndex := INT#2;
    ELSE
        WideIndex := INT#3;
END_CASE;

ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := DirectionIndex);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := WideIndex);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_geographic_conversion_behaves
ASSERT_NEAR(
    EXPECTED := REAL#59.5,
    ACTUAL := GEO_TO_DEG(D := INT#59, M := INT#30, SEC := REAL#0.0),
    DELTA := REAL#0.001
);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_math_helpers_behave
ASSERT_TRUE(IN := OSCAT_BASIC_Constants());
ASSERT_EQUAL(EXPECTED := DINT#3, ACTUAL := FLOOR2(X := REAL#3.14));
ASSERT_EQUAL(EXPECTED := DINT#-4, ACTUAL := FLOOR2(X := REAL#-3.14));
ASSERT_NEAR(EXPECTED := REAL#0.5, ACTUAL := MODR(IN := REAL#5.5, DIVI := REAL#2.5), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#180.0, ACTUAL := DEG(rad := MATH.PI), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := MATH.PI, ACTUAL := RAD(DEG := REAL#180.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := HYPOT(X := REAL#3.0, Y := REAL#4.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#1000.0, ACTUAL := EXP10(X := REAL#3.0), DELTA := REAL#0.1);
ASSERT_NEAR(
    EXPECTED := REAL#11.0,
    ACTUAL := SCALE(X := REAL#5.0, K := REAL#2.0, O := REAL#1.0, MX := REAL#20.0, MN := REAL#0.0),
    DELTA := REAL#0.001
);
ASSERT_NEAR(
    EXPECTED := REAL#20.0,
    ACTUAL := SCALE(X := REAL#15.0, K := REAL#2.0, O := REAL#1.0, MX := REAL#20.0, MN := REAL#0.0),
    DELTA := REAL#0.001
);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_range_helpers_behave
ASSERT_NEAR(
    EXPECTED := REAL#100.0,
    ACTUAL := BYTE_TO_RANGE(X := BYTE#16#FF, low := REAL#0.0, high := REAL#100.0),
    DELTA := REAL#0.001
);
ASSERT_NEAR(
    EXPECTED := REAL#100.0,
    ACTUAL := WORD_TO_RANGE(X := WORD#16#FFFF, low := REAL#0.0, high := REAL#100.0),
    DELTA := REAL#0.001
);
ASSERT_EQUAL(
    EXPECTED := BYTE#16#7F,
    ACTUAL := RANGE_TO_BYTE(X := REAL#50.0, low := REAL#0.0, high := REAL#100.0)
);
ASSERT_EQUAL(
    EXPECTED := WORD#16#7FFF,
    ACTUAL := RANGE_TO_WORD(X := REAL#50.0, low := REAL#0.0, high := REAL#100.0)
);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_extended_math_helpers_behave
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := ACOSH(X := REAL#1.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.549306, ACTUAL := ACOTH(X := REAL#2.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := ASINH(X := REAL#0.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.549306, ACTUAL := ATANH(X := REAL#0.5), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := COSH(X := REAL#0.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := SINH(X := REAL#0.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.761594, ACTUAL := TANH(X := REAL#1.0), DELTA := REAL#0.001);
ASSERT_TRUE(IN := CMP(X := REAL#3.141516, Y := REAL#3.141517, N := INT#6));
ASSERT_FALSE(IN := CMP(X := REAL#3.141516, Y := REAL#3.141517, N := INT#7));
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := CEIL(X := REAL#3.14));
ASSERT_EQUAL(EXPECTED := DINT#-3, ACTUAL := CEIL2(X := REAL#-3.14));
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := FLOOR(X := REAL#3.14));
ASSERT_EQUAL(EXPECTED := DINT#-1, ACTUAL := D_TRUNC(X := REAL#-1.5));
ASSERT_NEAR(EXPECTED := REAL#0.14, ACTUAL := FRACT(X := REAL#3.14), DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := DEC1(X := INT#0, N := INT#3));
ASSERT_TRUE(IN := EVEN(IN := DINT#4));
ASSERT_FALSE(IN := EVEN(IN := DINT#5));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := INC(X := INT#2, D := INT#2, M := INT#3));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := INC1(X := INT#2, N := INT#3));
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := INC2(X := INT#5, D := INT#2, L := INT#3, U := INT#6));
ASSERT_NEAR(EXPECTED := REAL#0.25, ACTUAL := INV(X := REAL#4.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := MAX3(IN1 := REAL#3.0, IN2 := REAL#1.0, IN3 := REAL#2.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := MID3(IN1 := REAL#3.0, IN2 := REAL#1.0, IN3 := REAL#2.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := MIN3(IN1 := REAL#3.0, IN2 := REAL#1.0, IN3 := REAL#2.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := MUL_ADD(X := REAL#2.0, K := REAL#3.0, O := REAL#4.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#-2.5, ACTUAL := NEGX(X := REAL#2.5), DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := INT#-1, ACTUAL := SGN(X := REAL#-0.5));
END_TEST_PROGRAM

FUNCTION_BLOCK OSCAT_FB_INPUT_DEFAULT_PROBE
VAR_INPUT
    BASE : INT;
    INC : INT := INT#5;
END_VAR
VAR_OUTPUT
    RESULT : INT;
END_VAR

RESULT := BASE + INC;
END_FUNCTION_BLOCK

TEST_PROGRAM oscat_basic_fb_omitted_input_defaults_follow_iec_state_rules
VAR
Probe : OSCAT_FB_INPUT_DEFAULT_PROBE;
END_VAR

Probe(BASE := INT#3);
ASSERT_EQUAL(EXPECTED := INT#8, ACTUAL := Probe.RESULT);
Probe(BASE := INT#3, INC := INT#9);
ASSERT_EQUAL(EXPECTED := INT#12, ACTUAL := Probe.RESULT);
Probe(BASE := INT#3);
ASSERT_EQUAL(EXPECTED := INT#12, ACTUAL := Probe.RESULT);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_nested_field_index_regression_behaves
VAR
Outer : OSCAT_NESTED_OUTER;
Items : ARRAY[0..1] OF OSCAT_NESTED_ITEM;
Idx : INT := INT#1;
TextValue : STRING[8] := 'ABCD';
WideValue : WSTRING[8] := "WXYZ";
Ch : CHAR;
WCh : WCHAR;
END_VAR

Outer.ARR[Idx] := INT#9;
Items[Idx].VALUE := INT#7;
Ch := TextValue[Idx + INT#1];
WCh := WideValue[Idx + INT#2];

ASSERT_EQUAL(EXPECTED := INT#9, ACTUAL := Outer.ARR[Idx]);
ASSERT_EQUAL(EXPECTED := INT#7, ACTUAL := Items[Idx].VALUE);
ASSERT_EQUAL(EXPECTED := STRING_TO_CHAR('B'), ACTUAL := Ch);
ASSERT_EQUAL(EXPECTED := CHAR_TO_WCHAR(STRING_TO_CHAR('Y')), ACTUAL := WCh);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_plc_clock_helpers_behave
VAR
FirstMs : DWORD;
SecondMs : DWORD;
ClockUs : DWORD;
END_VAR

FirstMs := T_PLC_MS();
SecondMs := T_PLC_MS();
ClockUs := T_PLC_US();

ASSERT_TRUE(IN := SecondMs >= FirstMs);
ASSERT_TRUE(IN := DWORD_TO_UDINT(ClockUs) >= DWORD_TO_UDINT(FirstMs) * UDINT#1000);
END_TEST_PROGRAM

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

TEST_PROGRAM oscat_basic_month_and_weekday_strings_behave
VAR
MonthFull : STRING[10];
MonthShort : STRING[10];
WeekdayFull : STRING[10];
WeekdayShort : STRING[10];
END_VAR

ASSERT_TRUE(IN := OSCAT_BASIC_Constants());
MonthFull := MONTH_TO_STRING(MTH := INT#4, LANG := INT#1, LX := INT#0);
MonthShort := MONTH_TO_STRING(MTH := INT#4, LANG := INT#1, LX := INT#3);
WeekdayFull := WEEKDAY_TO_STRING(WDAY := INT#1, LANG := INT#1, LX := INT#0);
WeekdayShort := WEEKDAY_TO_STRING(WDAY := INT#1, LANG := INT#1, LX := INT#2);

ASSERT_EQUAL(EXPECTED := 'April', ACTUAL := MonthFull);
ASSERT_EQUAL(EXPECTED := 'Apr', ACTUAL := MonthShort);
ASSERT_EQUAL(EXPECTED := 'Monday', ACTUAL := WeekdayFull);
ASSERT_EQUAL(EXPECTED := 'Mo', ACTUAL := WeekdayShort);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_dt_to_strf_behaves
VAR
Formatted : STRING[32];
END_VAR

ASSERT_TRUE(IN := OSCAT_BASIC_Constants());
Formatted := DT_TO_STRF(
    DTI := DT#2026-04-15-13:14:15,
    MS := INT#0,
    FMT := '#A-#D-#H #N:#R',
    LANG := INT#1
);

ASSERT_EQUAL(EXPECTED := '2026-04-15 13:14', ACTUAL := Formatted);
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

TEST_PROGRAM oscat_basic_string_formatting_helpers_behave
VAR
    SingleChar : STRING[1];
    Filled : STRING[8];
    FixedLeft : STRING[8];
    FixedRight : STRING[8];
    RealFormatted : STRING[20];
    DWordFormatted : STRING[20];
END_VAR

SingleChar := CHR_TO_STRING(C := BYTE#65);
Filled := FILL(C := BYTE#45, L := INT#4);
FixedLeft := FIX(STR := '42', L := INT#4, C := BYTE#48, M := INT#0);
FixedRight := FIX(STR := '42', L := INT#4, C := BYTE#48, M := INT#1);
RealFormatted := REAL_TO_STRF(IN := REAL#12.346, N := INT#2, D := '.');
DWordFormatted := DWORD_TO_STRF(IN := DWORD#123, N := INT#4);

ASSERT_EQUAL(EXPECTED := 'A', ACTUAL := SingleChar);
ASSERT_EQUAL(EXPECTED := '----', ACTUAL := Filled);
ASSERT_EQUAL(EXPECTED := '4200', ACTUAL := FixedLeft);
ASSERT_EQUAL(EXPECTED := '0042', ACTUAL := FixedRight);
ASSERT_EQUAL(EXPECTED := '12.35', ACTUAL := RealFormatted);
ASSERT_EQUAL(EXPECTED := '0123', ACTUAL := DWordFormatted);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_trim_helpers_behave
VAR
    NoSpaces : STRING[20];
    SingleSpaces : STRING[20];
    EdgeTrimmed : STRING[20];
END_VAR

NoSpaces := TRIM(STR := ' A B  C ');
SingleSpaces := TRIM1(STR := '  A   B  C  ');
EdgeTrimmed := TRIME(STR := '  A B  ');

ASSERT_EQUAL(EXPECTED := 'ABC', ACTUAL := NoSpaces);
ASSERT_EQUAL(EXPECTED := 'A B C', ACTUAL := SingleSpaces);
ASSERT_EQUAL(EXPECTED := 'A B', ACTUAL := EdgeTrimmed);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_case_conversion_helpers_behave
VAR
    LowerByte : BYTE;
    UpperByte : BYTE;
    LowerText : STRING[20];
    UpperText : STRING[20];
END_VAR

LowerByte := TO_LOWER(IN := BYTE#65);
UpperByte := TO_UPPER(IN := BYTE#97);
LowerText := LOWERCASE(STR := 'AbC-19');
UpperText := UPPERCASE(STR := 'AbC-19');

ASSERT_EQUAL(EXPECTED := BYTE#97, ACTUAL := LowerByte);
ASSERT_EQUAL(EXPECTED := BYTE#65, ACTUAL := UpperByte);
ASSERT_EQUAL(EXPECTED := 'abc-19', ACTUAL := LowerText);
ASSERT_EQUAL(EXPECTED := 'ABC-19', ACTUAL := UpperText);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_classifier_helpers_behave
VAR
    ControlText : STRING[4];
END_VAR

ControlText := CONCAT(
    IN1 := CHR_TO_STRING(C := BYTE#9),
    IN2 := CHR_TO_STRING(C := BYTE#10)
);

ASSERT_TRUE(IN := ISC_ALPHA(IN := BYTE#65));
ASSERT_TRUE(IN := ISC_CTRL(IN := BYTE#31));
ASSERT_TRUE(IN := ISC_HEX(IN := BYTE#70));
ASSERT_TRUE(IN := ISC_LOWER(IN := BYTE#97));
ASSERT_TRUE(IN := ISC_NUM(IN := BYTE#57));
ASSERT_TRUE(IN := ISC_UPPER(IN := BYTE#90));

ASSERT_TRUE(IN := IS_ALNUM(STR := 'Ab19'));
ASSERT_TRUE(IN := IS_ALPHA(STR := 'AbCd'));
ASSERT_TRUE(IN := IS_CC(STR := 'AB', CMP := 'ABC'));
ASSERT_TRUE(IN := IS_CTRL(STR := ControlText));
ASSERT_TRUE(IN := IS_HEX(STR := '0Af9'));
ASSERT_TRUE(IN := IS_LOWER(STR := 'abcd'));
ASSERT_TRUE(IN := IS_NCC(STR := 'AB', CMP := 'xyz'));
ASSERT_TRUE(IN := IS_NUM(STR := '190'));
ASSERT_TRUE(IN := IS_UPPER(STR := 'ABCD'));

ASSERT_FALSE(IN := ISC_ALPHA(IN := BYTE#49));
ASSERT_FALSE(IN := ISC_CTRL(IN := BYTE#65));
ASSERT_FALSE(IN := ISC_HEX(IN := BYTE#71));
ASSERT_FALSE(IN := ISC_LOWER(IN := BYTE#65));
ASSERT_FALSE(IN := ISC_NUM(IN := BYTE#65));
ASSERT_FALSE(IN := ISC_UPPER(IN := BYTE#97));
ASSERT_FALSE(IN := IS_ALNUM(STR := 'Ab-19'));
ASSERT_FALSE(IN := IS_ALPHA(STR := 'Ab1'));
ASSERT_FALSE(IN := IS_CC(STR := 'ABZ', CMP := 'ABC'));
ASSERT_FALSE(IN := IS_CTRL(STR := 'A'));
ASSERT_FALSE(IN := IS_HEX(STR := '0AG'));
ASSERT_FALSE(IN := IS_LOWER(STR := 'abC'));
ASSERT_FALSE(IN := IS_NCC(STR := 'ABx', CMP := 'xyz'));
ASSERT_FALSE(IN := IS_NUM(STR := '19A'));
ASSERT_FALSE(IN := IS_UPPER(STR := 'ABc'));
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_search_helpers_behave
VAR
    ControlPrefix : STRING[8];
    ControlMiddle : STRING[8];
    FirstCharPos : INT;
    FirstCtrlPos : INT;
    FirstNonNumPos : INT;
    FirstNumPos : INT;
    LastFindPos : INT;
    LastNonNumPos : INT;
    LastNumPos : INT;
    FindFromPos : INT;
END_VAR

ControlPrefix := CONCAT(IN1 := CHR_TO_STRING(C := BYTE#9), IN2 := 'A');
ControlMiddle := CONCAT(
    IN1 := 'AB',
    IN2 := CONCAT(IN1 := CHR_TO_STRING(C := BYTE#10), IN2 := 'C')
);

FirstCharPos := FIND_CHAR(STR := ControlPrefix, POS := INT#1);
FirstCtrlPos := FIND_CTRL(STR := ControlMiddle, POS := INT#1);
FirstNonNumPos := FIND_NONUM(STR := '12.3A', POS := INT#1);
FirstNumPos := FIND_NUM(STR := 'AB.5', POS := INT#1);
LastFindPos := FINDB(STR1 := 'abcabc', STR2 := 'ab');
LastNonNumPos := FINDB_NONUM(STR := '12.3A');
LastNumPos := FINDB_NUM(STR := 'A12.3B');
FindFromPos := FINDP(STR := 'abcabc', SRC := 'bc', POS := INT#3);

ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := FirstCharPos);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := FirstCtrlPos);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := FirstNonNumPos);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := FirstNumPos);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := LastFindPos);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := LastNonNumPos);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := LastNumPos);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := FindFromPos);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_cleanup_and_count_helpers_behave
VAR
    Capitalized : STRING[32];
    Cleaned : STRING[16];
    CharCount : INT;
    SubstringCount : INT;
END_VAR

Capitalized := CAPITALIZE(STR := 'oscat basic demo');
Cleaned := CLEAN(
    IN := CONCAT(IN1 := 'A', IN2 := CONCAT(IN1 := CHR_TO_STRING(C := BYTE#9), IN2 := 'B1')),
    CX := 'AB1'
);
CharCount := COUNT_CHAR(STR := 'banana', CHR := BYTE#97);
SubstringCount := COUNT_SUBSTRING(STR := 'banana', SEARCH := 'an');

ASSERT_EQUAL(EXPECTED := 'Oscat Basic Demo', ACTUAL := Capitalized);
ASSERT_EQUAL(EXPECTED := 'AB1', ACTUAL := Cleaned);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := CharCount);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := SubstringCount);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_code_delete_and_umlaut_helpers_behave
VAR
    CharacterCode : BYTE;
    MissingCode : BYTE;
    DeletedChars : STRING[16];
    UmlautAscii : STRING[2];
    PlainAscii : STRING[2];
END_VAR

CharacterCode := CODE(STR := 'AB', POS := INT#2);
MissingCode := CODE(STR := 'AB', POS := INT#5);
DeletedChars := DEL_CHARS(IN := 'A B-C', CX := ' -');
UmlautAscii := TO_UML(IN := BYTE#196);
PlainAscii := TO_UML(IN := BYTE#65);

ASSERT_EQUAL(EXPECTED := BYTE#66, ACTUAL := CharacterCode);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := MissingCode);
ASSERT_EQUAL(EXPECTED := 'ABC', ACTUAL := DeletedChars);
ASSERT_EQUAL(EXPECTED := 'Ae', ACTUAL := UmlautAscii);
ASSERT_EQUAL(EXPECTED := 'A', ACTUAL := PlainAscii);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_decimal_decoders_behave
VAR
    ByteValue : BYTE;
    DWordValue : DWORD;
    IntValue : INT;
END_VAR

ByteValue := DEC_TO_BYTE(DEC := '2x5');
DWordValue := DEC_TO_DWORD(DEC := '65_535');
IntValue := DEC_TO_INT(DEC := 'A-42');

ASSERT_EQUAL(EXPECTED := BYTE#25, ACTUAL := ByteValue);
ASSERT_EQUAL(EXPECTED := DWORD#65535, ACTUAL := DWordValue);
ASSERT_EQUAL(EXPECTED := INT#-42, ACTUAL := IntValue);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_bit_and_hex_formatters_behave
VAR
    ByteBits : STRING[8];
    ByteHex : STRING[2];
    DWordBits : STRING[32];
    DWordHex : STRING[8];
END_VAR

ByteBits := BYTE_TO_STRB(IN := BYTE#165);
ByteHex := BYTE_TO_STRH(IN := BYTE#165);
DWordBits := DWORD_TO_STRB(IN := DWORD#16#1234ABCD);
DWordHex := DWORD_TO_STRH(IN := DWORD#16#1234ABCD);

ASSERT_EQUAL(EXPECTED := '10100101', ACTUAL := ByteBits);
ASSERT_EQUAL(EXPECTED := 'A5', ACTUAL := ByteHex);
ASSERT_EQUAL(EXPECTED := '00010010001101001010101111001101', ACTUAL := DWordBits);
ASSERT_EQUAL(EXPECTED := '1234ABCD', ACTUAL := DWordHex);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_binary_hex_and_octal_decoders_behave
VAR
    ByteFromBinary : BYTE;
    DWordFromBinary : DWORD;
    ByteFromHex : BYTE;
    DWordFromHex : DWORD;
    ByteFromOctal : BYTE;
    DWordFromOctal : DWORD;
END_VAR

ByteFromBinary := BIN_TO_BYTE(BIN := '10-10_0101');
DWordFromBinary := BIN_TO_DWORD(BIN := '0001 0010 0011 0100 1010 1011 1100 1101');
ByteFromHex := HEX_TO_BYTE(HEX := 'A-5');
DWordFromHex := HEX_TO_DWORD(HEX := '12 34-abcd');
ByteFromOctal := OCT_TO_BYTE(OCT := '2_4_7');
DWordFromOctal := OCT_TO_DWORD(OCT := '1234_5670');

ASSERT_EQUAL(EXPECTED := BYTE#165, ACTUAL := ByteFromBinary);
ASSERT_EQUAL(EXPECTED := DWORD#16#1234ABCD, ACTUAL := DWordFromBinary);
ASSERT_EQUAL(EXPECTED := BYTE#16#A5, ACTUAL := ByteFromHex);
ASSERT_EQUAL(EXPECTED := DWORD#16#1234ABCD, ACTUAL := DWordFromHex);
ASSERT_EQUAL(EXPECTED := BYTE#167, ACTUAL := ByteFromOctal);
ASSERT_EQUAL(EXPECTED := DWORD#2739128, ACTUAL := DWordFromOctal);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_mirror_and_replacement_helpers_behave
VAR
    Mirrored : STRING[16];
    ReplacedAll : STRING[16];
    ReplacedChars : STRING[16];
    ReplacedUmlauts : STRING[16];
END_VAR

Mirrored := MIRROR(STR := 'AB 12');
ReplacedAll := REPLACE_ALL(STR := 'abcabc', SRC := 'ab', REP := 'Z');
ReplacedChars := REPLACE_CHARS(STR := 'A B-C', SRC := ' -', REP := '_+');
ReplacedUmlauts := REPLACE_UML(STR := 'Ae/ss');

ASSERT_EQUAL(EXPECTED := '21 BA', ACTUAL := Mirrored);
ASSERT_EQUAL(EXPECTED := 'ZcZc', ACTUAL := ReplacedAll);
ASSERT_EQUAL(EXPECTED := 'A_B+C', ACTUAL := ReplacedChars);
ASSERT_EQUAL(EXPECTED := 'Ae/ss', ACTUAL := ReplacedUmlauts);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_formatted_value_parsers_behave
VAR
    ConstantsReady : BOOL;
    FloatValue : REAL;
    ByteFromFormat : BYTE;
    DWordFromFormat : DWORD;
    ParsedStamp : DT;
    MonthIndex : INT;
    WeekMask : BYTE;
    WeekdayIndex : INT;
END_VAR

ConstantsReady := OSCAT_BASIC_Constants();
FloatValue := FLOAT_TO_REAL(FLT := '-12,5e1');
ByteFromFormat := FSTRING_TO_BYTE(IN := '16#A5');
DWordFromFormat := FSTRING_TO_DWORD(IN := '2#10100101');
ParsedStamp := FSTRING_TO_DT(SDT := '2026-04-15 13:14:15', FMT := '#Y-#M-#D #h:#m:#s');
MonthIndex := FSTRING_TO_MONTH(MTH := 'apRil', LANG := INT#1);
WeekMask := FSTRING_TO_WEEK(WEEK := 'Mo,We,Su', LANG := INT#1);
WeekdayIndex := FSTRING_TO_WEEKDAY(WDAY := 'mo', LANG := INT#1);

ASSERT_TRUE(IN := ConstantsReady);
ASSERT_NEAR(EXPECTED := REAL#-125.0, ACTUAL := FloatValue, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := BYTE#16#A5, ACTUAL := ByteFromFormat);
ASSERT_EQUAL(EXPECTED := DWORD#165, ACTUAL := DWordFromFormat);
ASSERT_EQUAL(EXPECTED := DT#2026-04-15-13:14:15, ACTUAL := ParsedStamp);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := MonthIndex);
ASSERT_EQUAL(EXPECTED := BYTE#81, ACTUAL := WeekMask);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := WeekdayIndex);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_exec_and_message_helpers_behave
VAR
    ExecResult : STRING[20];
    Msg4 : MESSAGE_4R;
    Msg8 : MESSAGE_8;
END_VAR

ExecResult := EXEC(STR := '3+4');
Msg4(
    M0 := 'zero',
    M1 := 'one',
    M2 := 'two',
    M3 := 'three',
    MM := INT#3,
    ENQ := TRUE,
    CLK := TRUE,
    T1 := T#1s
);
Msg8(
    IN1 := FALSE,
    IN2 := FALSE,
    IN3 := TRUE,
    IN4 := FALSE,
    IN5 := TRUE,
    IN6 := FALSE,
    IN7 := FALSE,
    IN8 := FALSE,
    S1 := 'one',
    S2 := 'two',
    S3 := 'three',
    S4 := 'four',
    S5 := 'five',
    S6 := 'six',
    S7 := 'seven',
    S8 := 'eight'
);

ASSERT_EQUAL(EXPECTED := '7.0', ACTUAL := ExecResult);
ASSERT_EQUAL(EXPECTED := 'one', ACTUAL := Msg4.MX);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Msg4.MN);
ASSERT_EQUAL(EXPECTED := 'three', ACTUAL := Msg8.M);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_html_name_helpers_behave
VAR
    EuroCode : BYTE;
    AmpCode : BYTE;
    DirectCharCode : BYTE;
    EuroName : STRING[10];
    QuotName : STRING[10];
    FallbackName : STRING[10];
END_VAR

EuroCode := CHARCODE(STR := 'euro');
AmpCode := CHARCODE(STR := 'amp');
DirectCharCode := CHARCODE(STR := 'A');
EuroName := CHARNAME(C := BYTE#128);
QuotName := CHARNAME(C := BYTE#34);
FallbackName := CHARNAME(C := BYTE#65);

ASSERT_EQUAL(EXPECTED := BYTE#128, ACTUAL := EuroCode);
ASSERT_EQUAL(EXPECTED := BYTE#38, ACTUAL := AmpCode);
ASSERT_EQUAL(EXPECTED := BYTE#65, ACTUAL := DirectCharCode);
ASSERT_EQUAL(EXPECTED := 'euro', ACTUAL := EuroName);
ASSERT_EQUAL(EXPECTED := 'quot', ACTUAL := QuotName);
ASSERT_EQUAL(EXPECTED := 'A', ACTUAL := FallbackName);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_string_ticker_fb_scrolls_text
VAR
    Tick : TICKER;
    TickText : STRING[STRING_LENGTH] := 'HELLO';
END_VAR

Tick(N := INT#3, PT := T#0ms, Text := TickText);
ASSERT_EQUAL(EXPECTED := 'HEL', ACTUAL := Tick.Display);

Tick(N := INT#3, PT := T#0ms, Text := TickText);
ASSERT_EQUAL(EXPECTED := 'ELL', ACTUAL := Tick.Display);

Tick(N := INT#3, PT := T#0ms, Text := TickText);
ASSERT_EQUAL(EXPECTED := 'LLO', ACTUAL := Tick.Display);

Tick(N := INT#8, PT := T#0ms, Text := TickText);
ASSERT_EQUAL(EXPECTED := 'HELLO', ACTUAL := Tick.Display);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_pulse_latches_and_store_behave
VAR
    Latch : LTCH;
    Latch4 : LTCH_4;
    Store : STORE_8;
END_VAR

Latch(D := TRUE, L := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Latch.Q);

Latch(D := FALSE, L := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Latch.Q);

Latch(D := FALSE, L := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Latch.Q);

Latch(D := TRUE, L := TRUE, RST := TRUE);
ASSERT_FALSE(IN := Latch.Q);

Latch4(D0 := TRUE, D1 := FALSE, D2 := TRUE, D3 := FALSE, L := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Latch4.Q0);
ASSERT_FALSE(IN := Latch4.Q1);
ASSERT_TRUE(IN := Latch4.Q2);
ASSERT_FALSE(IN := Latch4.Q3);

Latch4(D0 := FALSE, D1 := TRUE, D2 := FALSE, D3 := TRUE, L := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Latch4.Q0);
ASSERT_FALSE(IN := Latch4.Q1);
ASSERT_TRUE(IN := Latch4.Q2);
ASSERT_FALSE(IN := Latch4.Q3);

Latch4(D0 := FALSE, D1 := TRUE, D2 := FALSE, D3 := TRUE, L := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Latch4.Q0);
ASSERT_TRUE(IN := Latch4.Q1);
ASSERT_FALSE(IN := Latch4.Q2);
ASSERT_TRUE(IN := Latch4.Q3);

Latch4(D0 := FALSE, D1 := FALSE, D2 := FALSE, D3 := FALSE, L := TRUE, RST := TRUE);
ASSERT_FALSE(IN := Latch4.Q0);
ASSERT_FALSE(IN := Latch4.Q1);
ASSERT_FALSE(IN := Latch4.Q2);
ASSERT_FALSE(IN := Latch4.Q3);

Store(
    SetIn := FALSE,
    D0 := TRUE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := TRUE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := FALSE,
    Rst := FALSE
);
ASSERT_TRUE(IN := Store.Q0);
ASSERT_FALSE(IN := Store.Q1);
ASSERT_FALSE(IN := Store.Q2);
ASSERT_TRUE(IN := Store.Q3);

Store(
    SetIn := FALSE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := TRUE,
    Rst := FALSE
);
ASSERT_FALSE(IN := Store.Q0);
ASSERT_TRUE(IN := Store.Q3);

Store(
    SetIn := FALSE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := FALSE,
    Rst := FALSE
);
Store(
    SetIn := FALSE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := TRUE,
    Rst := FALSE
);
ASSERT_FALSE(IN := Store.Q0);
ASSERT_FALSE(IN := Store.Q1);
ASSERT_FALSE(IN := Store.Q2);
ASSERT_FALSE(IN := Store.Q3);

Store(
    SetIn := TRUE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := FALSE,
    Rst := FALSE
);
ASSERT_TRUE(IN := Store.Q0);
ASSERT_TRUE(IN := Store.Q7);

Store(
    SetIn := FALSE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := FALSE,
    Rst := TRUE
);
ASSERT_FALSE(IN := Store.Q0);
ASSERT_FALSE(IN := Store.Q7);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_edge_counters_and_toggle_behave
VAR
    ByteCounter : COUNT_BR;
    DWordCounter : COUNT_DR;
    ToggleFf : TOGGLE;
END_VAR

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#4, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := TRUE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#4, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := TRUE, IN := BYTE#9, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#5, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := TRUE);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := ByteCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#4, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := TRUE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#4, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := TRUE, IN := DWORD#9, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#5, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := TRUE);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := DWordCounter.CNT);

ToggleFf(CLK := FALSE, RST := FALSE);
ASSERT_FALSE(IN := ToggleFf.Q);

ToggleFf(CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := ToggleFf.Q);

ToggleFf(CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := ToggleFf.Q);

ToggleFf(CLK := FALSE, RST := FALSE);
ToggleFf(CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := ToggleFf.Q);

ToggleFf(CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := ToggleFf.Q);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_edge_flip_flops_behave
VAR
    DualD : FF_D2E;
    QuadD : FF_D4E;
    DWithSetReset : FF_DRE;
END_VAR

DualD(D0 := TRUE, D1 := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := DualD.Q0);
ASSERT_FALSE(IN := DualD.Q1);

DualD(D0 := FALSE, D1 := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := DualD.Q0);
ASSERT_FALSE(IN := DualD.Q1);

DualD(D0 := FALSE, D1 := TRUE, CLK := FALSE, RST := FALSE);
DualD(D0 := FALSE, D1 := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := DualD.Q0);
ASSERT_TRUE(IN := DualD.Q1);

DualD(D0 := FALSE, D1 := FALSE, CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := DualD.Q0);
ASSERT_FALSE(IN := DualD.Q1);

QuadD(D0 := TRUE, D1 := FALSE, D2 := TRUE, D3 := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := QuadD.Q0);
ASSERT_FALSE(IN := QuadD.Q1);
ASSERT_TRUE(IN := QuadD.Q2);
ASSERT_FALSE(IN := QuadD.Q3);

QuadD(D0 := FALSE, D1 := TRUE, D2 := FALSE, D3 := TRUE, CLK := FALSE, RST := FALSE);
QuadD(D0 := FALSE, D1 := TRUE, D2 := FALSE, D3 := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := QuadD.Q0);
ASSERT_TRUE(IN := QuadD.Q1);
ASSERT_FALSE(IN := QuadD.Q2);
ASSERT_TRUE(IN := QuadD.Q3);

QuadD(D0 := FALSE, D1 := FALSE, D2 := FALSE, D3 := FALSE, CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := QuadD.Q0);
ASSERT_FALSE(IN := QuadD.Q1);
ASSERT_FALSE(IN := QuadD.Q2);
ASSERT_FALSE(IN := QuadD.Q3);

DWithSetReset(SetIn := TRUE, D := FALSE, CLK := FALSE, RST := FALSE);
ASSERT_TRUE(IN := DWithSetReset.Q);

DWithSetReset(SetIn := FALSE, D := FALSE, CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := DWithSetReset.Q);

DWithSetReset(SetIn := FALSE, D := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := DWithSetReset.Q);

DWithSetReset(SetIn := FALSE, D := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := DWithSetReset.Q);

DWithSetReset(SetIn := FALSE, D := FALSE, CLK := FALSE, RST := FALSE);
DWithSetReset(SetIn := FALSE, D := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := DWithSetReset.Q);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_jk_rs_and_selector_behave
VAR
    JkFf : FF_JKE;
    RsFf : FF_RSE;
    Selector : SELECT_8;
END_VAR

JkFf(SetIn := TRUE, J := FALSE, CLK := FALSE, K := FALSE, RST := FALSE);
ASSERT_TRUE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := FALSE, CLK := FALSE, K := FALSE, RST := TRUE);
ASSERT_FALSE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := TRUE, CLK := TRUE, K := FALSE, RST := FALSE);
ASSERT_TRUE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := FALSE, CLK := TRUE, K := FALSE, RST := FALSE);
ASSERT_TRUE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := FALSE, CLK := FALSE, K := FALSE, RST := FALSE);
JkFf(SetIn := FALSE, J := FALSE, CLK := TRUE, K := TRUE, RST := FALSE);
ASSERT_FALSE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := FALSE, CLK := FALSE, K := FALSE, RST := FALSE);
JkFf(SetIn := FALSE, J := TRUE, CLK := TRUE, K := TRUE, RST := FALSE);
ASSERT_TRUE(IN := JkFf.Q);

RsFf(CS := FALSE, CR := FALSE, RST := TRUE);
ASSERT_FALSE(IN := RsFf.Q);

RsFf(CS := TRUE, CR := FALSE, RST := FALSE);
ASSERT_TRUE(IN := RsFf.Q);

RsFf(CS := TRUE, CR := FALSE, RST := FALSE);
ASSERT_TRUE(IN := RsFf.Q);

RsFf(CS := FALSE, CR := FALSE, RST := FALSE);
RsFf(CS := FALSE, CR := TRUE, RST := FALSE);
ASSERT_FALSE(IN := RsFf.Q);

RsFf(CS := FALSE, CR := FALSE, RST := FALSE);
RsFf(CS := TRUE, CR := TRUE, RST := FALSE);
ASSERT_FALSE(IN := RsFf.Q);

Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, RST := TRUE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q0);

Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q1);

Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q1);

Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, RST := FALSE);
Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := TRUE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q0);

Selector(E := TRUE, SetIn := TRUE, IN := BYTE#6, UP := FALSE, DN := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q6);

Selector(E := FALSE, SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, RST := FALSE);
ASSERT_FALSE(IN := Selector.Q0);
ASSERT_FALSE(IN := Selector.Q6);
ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Selector.STATE);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_shift_registers_behave
VAR
    Shift4 : SHR_4E;
    Shift4Dir : SHR_4UDE;
    Shift8Load : SHR_8PLE;
    Shift8Dir : SHR_8UDE;
END_VAR

Shift4(SetIn := TRUE, D0 := FALSE, CLK := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift4.Q0);
ASSERT_TRUE(IN := Shift4.Q3);

Shift4(SetIn := FALSE, D0 := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Shift4.Q0);
ASSERT_TRUE(IN := Shift4.Q1);

Shift4(SetIn := FALSE, D0 := FALSE, CLK := FALSE, RST := FALSE);
Shift4(SetIn := FALSE, D0 := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Shift4.Q0);
ASSERT_TRUE(IN := Shift4.Q2);

Shift4(SetIn := FALSE, D0 := FALSE, CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := Shift4.Q0);
ASSERT_FALSE(IN := Shift4.Q3);

Shift4Dir(SetIn := TRUE, D0 := FALSE, D3 := FALSE, CLK := FALSE, DN := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift4Dir.Q0);
ASSERT_TRUE(IN := Shift4Dir.Q3);

Shift4Dir(SetIn := FALSE, D0 := FALSE, D3 := FALSE, CLK := FALSE, DN := FALSE, RST := TRUE);
Shift4Dir(SetIn := FALSE, D0 := TRUE, D3 := FALSE, CLK := TRUE, DN := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift4Dir.Q0);
ASSERT_FALSE(IN := Shift4Dir.Q1);

Shift4Dir(SetIn := FALSE, D0 := FALSE, D3 := FALSE, CLK := FALSE, DN := FALSE, RST := FALSE);
Shift4Dir(SetIn := FALSE, D0 := FALSE, D3 := TRUE, CLK := TRUE, DN := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Shift4Dir.Q3);
ASSERT_FALSE(IN := Shift4Dir.Q0);

Shift8Load(Din := FALSE, Dload := BYTE#16#A5, CLK := FALSE, UP := TRUE, LoadIn := FALSE, RST := TRUE);
ASSERT_FALSE(IN := Shift8Load.DOut);

Shift8Load(Din := FALSE, Dload := BYTE#16#A5, CLK := TRUE, UP := TRUE, LoadIn := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Shift8Load.DOut);

Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := FALSE, UP := TRUE, LoadIn := FALSE, RST := FALSE);
Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := TRUE, UP := TRUE, LoadIn := FALSE, RST := FALSE);
ASSERT_FALSE(IN := Shift8Load.DOut);

Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := FALSE, UP := FALSE, LoadIn := FALSE, RST := FALSE);
Shift8Load(Din := TRUE, Dload := BYTE#16#80, CLK := TRUE, UP := FALSE, LoadIn := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Shift8Load.DOut);

Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := FALSE, UP := FALSE, LoadIn := FALSE, RST := FALSE);
Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := TRUE, UP := FALSE, LoadIn := FALSE, RST := FALSE);
ASSERT_FALSE(IN := Shift8Load.DOut);

Shift8Dir(SetIn := TRUE, D0 := FALSE, D7 := FALSE, CLK := FALSE, DN := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift8Dir.Q0);
ASSERT_TRUE(IN := Shift8Dir.Q7);

Shift8Dir(SetIn := FALSE, D0 := FALSE, D7 := FALSE, CLK := FALSE, DN := FALSE, RST := TRUE);
Shift8Dir(SetIn := FALSE, D0 := TRUE, D7 := FALSE, CLK := TRUE, DN := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift8Dir.Q0);
ASSERT_FALSE(IN := Shift8Dir.Q1);

Shift8Dir(SetIn := FALSE, D0 := FALSE, D7 := FALSE, CLK := FALSE, DN := FALSE, RST := FALSE);
Shift8Dir(SetIn := FALSE, D0 := FALSE, D7 := TRUE, CLK := TRUE, DN := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Shift8Dir.Q7);
ASSERT_FALSE(IN := Shift8Dir.Q0);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_decoders_and_muxes_behave
VAR
    Dec2 : DEC_2;
    Dec4 : DEC_4;
    Dec8 : DEC_8;
    Selected2 : BOOL;
    Selected4 : BOOL;
END_VAR

Dec2(D := TRUE, A := FALSE);
ASSERT_TRUE(IN := Dec2.Q0);
ASSERT_FALSE(IN := Dec2.Q1);

Dec2(D := TRUE, A := TRUE);
ASSERT_FALSE(IN := Dec2.Q0);
ASSERT_TRUE(IN := Dec2.Q1);

Dec2(D := FALSE, A := TRUE);
ASSERT_FALSE(IN := Dec2.Q0);
ASSERT_FALSE(IN := Dec2.Q1);

Dec4(D := TRUE, A0 := TRUE, A1 := FALSE);
ASSERT_FALSE(IN := Dec4.Q0);
ASSERT_TRUE(IN := Dec4.Q1);
ASSERT_FALSE(IN := Dec4.Q2);
ASSERT_FALSE(IN := Dec4.Q3);

Dec4(D := TRUE, A0 := FALSE, A1 := TRUE);
ASSERT_FALSE(IN := Dec4.Q0);
ASSERT_FALSE(IN := Dec4.Q1);
ASSERT_TRUE(IN := Dec4.Q2);
ASSERT_FALSE(IN := Dec4.Q3);

Dec8(D := TRUE, A0 := TRUE, A1 := FALSE, A2 := TRUE);
ASSERT_FALSE(IN := Dec8.Q0);
ASSERT_FALSE(IN := Dec8.Q1);
ASSERT_FALSE(IN := Dec8.Q2);
ASSERT_FALSE(IN := Dec8.Q3);
ASSERT_FALSE(IN := Dec8.Q4);
ASSERT_TRUE(IN := Dec8.Q5);
ASSERT_FALSE(IN := Dec8.Q6);
ASSERT_FALSE(IN := Dec8.Q7);

Dec8(D := FALSE, A0 := TRUE, A1 := TRUE, A2 := TRUE);
ASSERT_FALSE(IN := Dec8.Q0);
ASSERT_FALSE(IN := Dec8.Q7);

Selected2 := MUX_2(D0 := FALSE, D1 := TRUE, A0 := FALSE);
ASSERT_FALSE(IN := Selected2);

Selected2 := MUX_2(D0 := FALSE, D1 := TRUE, A0 := TRUE);
ASSERT_TRUE(IN := Selected2);

Selected4 := MUX_4(D0 := FALSE, D1 := TRUE, D2 := TRUE, D3 := FALSE, A0 := FALSE, A1 := FALSE);
ASSERT_FALSE(IN := Selected4);

Selected4 := MUX_4(D0 := FALSE, D1 := TRUE, D2 := TRUE, D3 := FALSE, A0 := TRUE, A1 := FALSE);
ASSERT_TRUE(IN := Selected4);

Selected4 := MUX_4(D0 := FALSE, D1 := TRUE, D2 := TRUE, D3 := FALSE, A0 := FALSE, A1 := TRUE);
ASSERT_TRUE(IN := Selected4);

Selected4 := MUX_4(D0 := FALSE, D1 := TRUE, D2 := TRUE, D3 := FALSE, A0 := TRUE, A1 := TRUE);
ASSERT_FALSE(IN := Selected4);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_bit_helpers_behave
VAR
    ByteBits : BYTE_TO_BITS;
    ByteValue : BYTE;
    DWordValue : DWORD;
    WordValue : WORD;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#42, ACTUAL := BCDC_TO_INT(IN := BYTE#16#42));
ASSERT_EQUAL(EXPECTED := BYTE#16#42, ACTUAL := INT_TO_BCDC(IN := INT#42));

ASSERT_EQUAL(EXPECTED := INT#9, ACTUAL := BIT_COUNT(IN := DWORD#16#F0F00001));
ASSERT_TRUE(IN := BIT_OF_DWORD(IN := DWORD#16#10, N := INT#4));
ASSERT_FALSE(IN := BIT_OF_DWORD(IN := DWORD#16#10, N := INT#3));

ASSERT_EQUAL(EXPECTED := BYTE#8, ACTUAL := BIT_TOGGLE_B(IN := BYTE#0, POS := INT#3));
ASSERT_EQUAL(EXPECTED := WORD#16#1000, ACTUAL := BIT_TOGGLE_W(IN := WORD#0, POS := INT#12));
ASSERT_EQUAL(EXPECTED := DWORD#16#80000000, ACTUAL := BIT_TOGGLE_DW(IN := DWORD#0, POS := INT#31));

ByteValue := BYTE_OF_BIT(
    B0 := TRUE,
    B1 := FALSE,
    B2 := TRUE,
    B3 := FALSE,
    B4 := FALSE,
    B5 := TRUE,
    B6 := FALSE,
    B7 := TRUE
);
ASSERT_EQUAL(EXPECTED := BYTE#16#A5, ACTUAL := ByteValue);

ASSERT_EQUAL(EXPECTED := BYTE#16#AB, ACTUAL := BYTE_OF_DWORD(IN := DWORD#16#1234ABCD, N := BYTE#1));

ByteBits(IN := BYTE#16#A5);
ASSERT_TRUE(IN := ByteBits.B0);
ASSERT_FALSE(IN := ByteBits.B1);
ASSERT_TRUE(IN := ByteBits.B2);
ASSERT_TRUE(IN := ByteBits.B5);
ASSERT_TRUE(IN := ByteBits.B7);

ASSERT_EQUAL(EXPECTED := BYTE#8, ACTUAL := BYTE_TO_GRAY(IN := BYTE#15));
ASSERT_EQUAL(EXPECTED := BYTE#15, ACTUAL := GRAY_TO_BYTE(IN := BYTE#8));

ASSERT_TRUE(IN := CHECK_PARITY(IN := DWORD#1, P := TRUE));
ASSERT_FALSE(IN := CHECK_PARITY(IN := DWORD#1, P := FALSE));
ASSERT_FALSE(IN := PARITY(IN := DWORD#0));
ASSERT_TRUE(IN := PARITY(IN := DWORD#1));

DWordValue := DWORD_OF_BYTE(B3 := BYTE#16#12, B2 := BYTE#16#34, B1 := BYTE#16#AB, B0 := BYTE#16#CD);
ASSERT_EQUAL(EXPECTED := DWORD#16#1234ABCD, ACTUAL := DWordValue);

DWordValue := DWORD_OF_WORD(W1 := WORD#16#1234, W0 := WORD#16#ABCD);
ASSERT_EQUAL(EXPECTED := DWORD#16#1234ABCD, ACTUAL := DWordValue);

ASSERT_EQUAL(EXPECTED := DWORD#16#2345678F, ACTUAL := SHL1(IN := DWORD#16#12345678, N := INT#4));
ASSERT_EQUAL(EXPECTED := DWORD#16#F1234567, ACTUAL := SHR1(IN := DWORD#16#12345678, N := INT#4));

ASSERT_EQUAL(EXPECTED := WORD#16#3412, ACTUAL := SWAP_BYTE(IN := WORD#16#1234));
ASSERT_EQUAL(EXPECTED := DWORD#16#CDAB3412, ACTUAL := SWAP_BYTE2(IN := DWORD#16#1234ABCD));

WordValue := WORD_OF_BYTE(B1 := BYTE#16#12, B0 := BYTE#16#34);
ASSERT_EQUAL(EXPECTED := WORD#16#1234, ACTUAL := WordValue);

ASSERT_EQUAL(EXPECTED := WORD#16#ABCD, ACTUAL := WORD_OF_DWORD(IN := DWORD#16#1234ABCD, N := BYTE#0));
ASSERT_EQUAL(EXPECTED := WORD#16#1234, ACTUAL := WORD_OF_DWORD(IN := DWORD#16#1234ABCD, N := BYTE#1));
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_bit_load_helpers_behave
ASSERT_EQUAL(EXPECTED := BYTE#8, ACTUAL := BIT_LOAD_B(IN := BYTE#0, VAL := TRUE, POS := INT#3));
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := BIT_LOAD_B(IN := BYTE#8, VAL := FALSE, POS := INT#3));
ASSERT_EQUAL(EXPECTED := BYTE#14, ACTUAL := BIT_LOAD_B2(I := BYTE#0, D := TRUE, P := INT#1, N := INT#3));
ASSERT_EQUAL(EXPECTED := BYTE#16#E3, ACTUAL := BIT_LOAD_B2(I := BYTE#16#FF, D := FALSE, P := INT#2, N := INT#3));

ASSERT_EQUAL(EXPECTED := WORD#16#1000, ACTUAL := BIT_LOAD_W(IN := WORD#0, VAL := TRUE, POS := INT#12));
ASSERT_EQUAL(EXPECTED := WORD#0, ACTUAL := BIT_LOAD_W(IN := WORD#16#1000, VAL := FALSE, POS := INT#12));
ASSERT_EQUAL(EXPECTED := WORD#16#01F0, ACTUAL := BIT_LOAD_W2(I := WORD#0, D := TRUE, P := INT#4, N := INT#5));
ASSERT_EQUAL(EXPECTED := WORD#16#FE0F, ACTUAL := BIT_LOAD_W2(I := WORD#16#FFFF, D := FALSE, P := INT#4, N := INT#5));

ASSERT_EQUAL(EXPECTED := DWORD#16#80000000, ACTUAL := BIT_LOAD_DW(IN := DWORD#0, VAL := TRUE, POS := INT#31));
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := BIT_LOAD_DW(IN := DWORD#16#80000000, VAL := FALSE, POS := INT#31));
ASSERT_EQUAL(EXPECTED := DWORD#16#0000FF00, ACTUAL := BIT_LOAD_DW2(I := DWORD#0, D := TRUE, P := INT#8, N := INT#8));
ASSERT_EQUAL(EXPECTED := DWORD#16#FFFF00FF, ACTUAL := BIT_LOAD_DW2(I := DWORD#16#FFFFFFFF, D := FALSE, P := INT#8, N := INT#8));
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_bit_cast_and_reflection_helpers_behave
VAR
    PositiveInfinity : REAL;
    NegativeInfinity : REAL;
    NotANumber : REAL;
END_VAR

ASSERT_EQUAL(EXPECTED := DWORD#16#3FC00000, ACTUAL := REAL_TO_DW(X := REAL#1.5));
ASSERT_NEAR(EXPECTED := REAL#1.5, ACTUAL := DW_TO_REAL(X := DWORD#16#3FC00000), DELTA := REAL#0.0001);

ASSERT_EQUAL(EXPECTED := DWORD#16#19, ACTUAL := REFLECT(D := DWORD#16#13, L := INT#5));
ASSERT_EQUAL(EXPECTED := BYTE#16#69, ACTUAL := REVERSE(IN := BYTE#16#96));

ASSERT_EQUAL(EXPECTED := BYTE#16#00, ACTUAL := CHK_REAL(X := REAL#0.0));

PositiveInfinity := DWORD_TO_REAL(DWORD#16#7F800000);
NegativeInfinity := DWORD_TO_REAL(DWORD#16#FF800000);
NotANumber := DWORD_TO_REAL(DWORD#16#7FC00000);

ASSERT_EQUAL(EXPECTED := BYTE#16#20, ACTUAL := CHK_REAL(X := PositiveInfinity));
ASSERT_EQUAL(EXPECTED := BYTE#16#40, ACTUAL := CHK_REAL(X := NegativeInfinity));
ASSERT_EQUAL(EXPECTED := BYTE#16#80, ACTUAL := CHK_REAL(X := NotANumber));
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_generator_triggers_behave
VAR
    AnalogTrigger : A_TRIG;
    BothEdgeTrigger : B_TRIG;
    DWordTrigger : D_TRIG;
END_VAR

AnalogTrigger(IN := REAL#1.0, RES := REAL#0.5);
ASSERT_TRUE(IN := AnalogTrigger.Q);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := AnalogTrigger.D, DELTA := REAL#0.0001);

AnalogTrigger(IN := REAL#1.0, RES := REAL#0.5);
ASSERT_FALSE(IN := AnalogTrigger.Q);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := AnalogTrigger.D, DELTA := REAL#0.0001);

AnalogTrigger(IN := REAL#1.3, RES := REAL#0.5);
ASSERT_FALSE(IN := AnalogTrigger.Q);
ASSERT_NEAR(EXPECTED := REAL#0.3, ACTUAL := AnalogTrigger.D, DELTA := REAL#0.0001);

AnalogTrigger(IN := REAL#1.6, RES := REAL#0.5);
ASSERT_TRUE(IN := AnalogTrigger.Q);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := AnalogTrigger.D, DELTA := REAL#0.0001);

BothEdgeTrigger(CLK := FALSE);
ASSERT_FALSE(IN := BothEdgeTrigger.Q);

BothEdgeTrigger(CLK := TRUE);
ASSERT_TRUE(IN := BothEdgeTrigger.Q);

BothEdgeTrigger(CLK := TRUE);
ASSERT_FALSE(IN := BothEdgeTrigger.Q);

BothEdgeTrigger(CLK := FALSE);
ASSERT_TRUE(IN := BothEdgeTrigger.Q);

DWordTrigger(IN := DWORD#0);
ASSERT_FALSE(IN := DWordTrigger.Q);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := DWordTrigger.X);

DWordTrigger(IN := DWORD#1);
ASSERT_TRUE(IN := DWordTrigger.Q);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := DWordTrigger.X);

DWordTrigger(IN := DWORD#1);
ASSERT_FALSE(IN := DWordTrigger.Q);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := DWordTrigger.X);

DWordTrigger(IN := DWORD#2);
ASSERT_TRUE(IN := DWordTrigger.Q);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := DWordTrigger.X);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_generator_click_and_divider_helpers_behave
VAR
    ClickCounter : CLICK_CNT;
    ClickDecoder : CLICK_DEC;
    Divider : CLK_DIV;
END_VAR

ClickCounter(IN := TRUE, N := INT#0, TC := T#0ms);
ASSERT_FALSE(IN := ClickCounter.Q);

ClickCounter(IN := FALSE, N := INT#0, TC := T#0ms);
ASSERT_TRUE(IN := ClickCounter.Q);

ClickCounter(IN := FALSE, N := INT#0, TC := T#0ms);
ASSERT_FALSE(IN := ClickCounter.Q);

ClickDecoder(IN := TRUE, TC := T#0ms);
ASSERT_FALSE(IN := ClickDecoder.Q0);
ASSERT_FALSE(IN := ClickDecoder.Q1);

ClickDecoder(IN := FALSE, TC := T#0ms);
ASSERT_TRUE(IN := ClickDecoder.Q0);
ASSERT_FALSE(IN := ClickDecoder.Q1);

ClickDecoder(IN := FALSE, TC := T#0ms);
ASSERT_FALSE(IN := ClickDecoder.Q0);
ASSERT_FALSE(IN := ClickDecoder.Q1);

Divider(CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := Divider.Q0);
ASSERT_FALSE(IN := Divider.Q1);

Divider(CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Divider.Q0);
ASSERT_FALSE(IN := Divider.Q1);

Divider(CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Divider.Q0);
ASSERT_TRUE(IN := Divider.Q1);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_generator_pattern_and_cycle_helpers_behave
VAR
    Pattern : GEN_BIT;
    Cycle4 : CYCLE_4;
END_VAR

Pattern(
    IN0 := DWORD#16#5,
    IN1 := DWORD#16#3,
    IN2 := DWORD#16#1,
    IN3 := DWORD#16#0,
    CLK := TRUE,
    STEPS := INT#3,
    REP := INT#1,
    RST := FALSE
);
ASSERT_TRUE(IN := Pattern.RUN);
ASSERT_TRUE(IN := Pattern.Q0);
ASSERT_TRUE(IN := Pattern.Q1);
ASSERT_TRUE(IN := Pattern.Q2);
ASSERT_FALSE(IN := Pattern.Q3);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Pattern.CNT);

Pattern(
    IN0 := DWORD#16#5,
    IN1 := DWORD#16#3,
    IN2 := DWORD#16#1,
    IN3 := DWORD#16#0,
    CLK := TRUE,
    STEPS := INT#3,
    REP := INT#1,
    RST := FALSE
);
ASSERT_TRUE(IN := Pattern.RUN);
ASSERT_FALSE(IN := Pattern.Q0);
ASSERT_TRUE(IN := Pattern.Q1);
ASSERT_FALSE(IN := Pattern.Q2);
ASSERT_FALSE(IN := Pattern.Q3);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Pattern.CNT);

Pattern(
    IN0 := DWORD#16#5,
    IN1 := DWORD#16#3,
    IN2 := DWORD#16#1,
    IN3 := DWORD#16#0,
    CLK := TRUE,
    STEPS := INT#3,
    REP := INT#1,
    RST := FALSE
);
ASSERT_FALSE(IN := Pattern.RUN);
ASSERT_TRUE(IN := Pattern.Q0);
ASSERT_FALSE(IN := Pattern.Q1);
ASSERT_FALSE(IN := Pattern.Q2);
ASSERT_FALSE(IN := Pattern.Q3);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Pattern.CNT);

Cycle4(
    E := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms,
    S0 := TRUE,
    SX := INT#0,
    SL := FALSE
);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cycle4.STATE);

Cycle4(
    E := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms,
    S0 := TRUE,
    SX := INT#0,
    SL := FALSE
);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Cycle4.STATE);

Cycle4(
    E := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms,
    S0 := TRUE,
    SX := INT#0,
    SL := FALSE
);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Cycle4.STATE);

Cycle4(
    E := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms,
    S0 := TRUE,
    SX := INT#0,
    SL := FALSE
);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cycle4.STATE);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_scheduler_helpers_behave
VAR
    TimeScheduler : SCHEDULER;
    CycleScheduler : SCHEDULER_2;
END_VAR

TimeScheduler(
    E0 := TRUE,
    E1 := TRUE,
    E2 := TRUE,
    E3 := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms
);
ASSERT_TRUE(IN := TimeScheduler.Q0);
ASSERT_FALSE(IN := TimeScheduler.Q1);
ASSERT_FALSE(IN := TimeScheduler.Q2);
ASSERT_FALSE(IN := TimeScheduler.Q3);

TimeScheduler(
    E0 := TRUE,
    E1 := TRUE,
    E2 := TRUE,
    E3 := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms
);
ASSERT_FALSE(IN := TimeScheduler.Q0);
ASSERT_TRUE(IN := TimeScheduler.Q1);
ASSERT_FALSE(IN := TimeScheduler.Q2);
ASSERT_FALSE(IN := TimeScheduler.Q3);

TimeScheduler(
    E0 := TRUE,
    E1 := TRUE,
    E2 := TRUE,
    E3 := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms
);
ASSERT_FALSE(IN := TimeScheduler.Q0);
ASSERT_FALSE(IN := TimeScheduler.Q1);
ASSERT_TRUE(IN := TimeScheduler.Q2);
ASSERT_FALSE(IN := TimeScheduler.Q3);

TimeScheduler(
    E0 := TRUE,
    E1 := TRUE,
    E2 := TRUE,
    E3 := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms
);
ASSERT_FALSE(IN := TimeScheduler.Q0);
ASSERT_FALSE(IN := TimeScheduler.Q1);
ASSERT_FALSE(IN := TimeScheduler.Q2);
ASSERT_TRUE(IN := TimeScheduler.Q3);

CycleScheduler(
    E0 := TRUE,
    E1 := FALSE,
    E2 := FALSE,
    E3 := FALSE,
    C0 := UINT#2,
    C1 := UINT#1,
    C2 := UINT#1,
    C3 := UINT#1,
    O0 := UINT#0,
    O1 := UINT#0,
    O2 := UINT#0,
    O3 := UINT#0
);
ASSERT_TRUE(IN := CycleScheduler.Q0);

CycleScheduler(
    E0 := TRUE,
    E1 := FALSE,
    E2 := FALSE,
    E3 := FALSE,
    C0 := UINT#2,
    C1 := UINT#1,
    C2 := UINT#1,
    C3 := UINT#1,
    O0 := UINT#0,
    O1 := UINT#0,
    O2 := UINT#0,
    O3 := UINT#0
);
ASSERT_FALSE(IN := CycleScheduler.Q0);

CycleScheduler(
    E0 := TRUE,
    E1 := FALSE,
    E2 := FALSE,
    E3 := FALSE,
    C0 := UINT#2,
    C1 := UINT#1,
    C2 := UINT#1,
    C3 := UINT#1,
    O0 := UINT#0,
    O1 := UINT#0,
    O2 := UINT#0,
    O3 := UINT#0
);
ASSERT_TRUE(IN := CycleScheduler.Q0);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_sequence4_helpers_behave
VAR
    Sequence4 : SEQUENCE_4;
END_VAR

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := TRUE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_TRUE(IN := Sequence4.RUN);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Sequence4.STATE);
ASSERT_TRUE(IN := Sequence4.Q0);
ASSERT_TRUE(IN := Sequence4.QX);

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Sequence4.STATE);
ASSERT_FALSE(IN := Sequence4.Q0);
ASSERT_TRUE(IN := Sequence4.Q1);

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Sequence4.STATE);
ASSERT_TRUE(IN := Sequence4.Q2);

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Sequence4.STATE);
ASSERT_TRUE(IN := Sequence4.Q3);

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_FALSE(IN := Sequence4.RUN);
ASSERT_EQUAL(EXPECTED := BYTE#110, ACTUAL := Sequence4.STATUS);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_sequence8_helpers_behave
VAR
    Sequence8 : SEQUENCE_8;
END_VAR

Sequence8(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    IN4 := TRUE,
    IN5 := TRUE,
    IN6 := TRUE,
    IN7 := TRUE,
    START := TRUE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms,
    WAIT4 := T#0ms,
    DELAY4 := T#0ms,
    WAIT5 := T#0ms,
    DELAY5 := T#0ms,
    WAIT6 := T#0ms,
    DELAY6 := T#0ms,
    WAIT7 := T#0ms,
    DELAY7 := T#0ms
);
ASSERT_TRUE(IN := Sequence8.Q0);

Sequence8(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    IN4 := TRUE,
    IN5 := TRUE,
    IN6 := TRUE,
    IN7 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms,
    WAIT4 := T#0ms,
    DELAY4 := T#0ms,
    WAIT5 := T#0ms,
    DELAY5 := T#0ms,
    WAIT6 := T#0ms,
    DELAY6 := T#0ms,
    WAIT7 := T#0ms,
    DELAY7 := T#0ms
);
ASSERT_TRUE(IN := Sequence8.Q1);
ASSERT_FALSE(IN := Sequence8.Q0);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_memory_helpers_behave
VAR
    Fifo16 : FIFO_16;
    Fifo32 : FIFO_32;
    Stack16 : STACK_16;
    Stack32 : STACK_32;
END_VAR

Fifo16(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ASSERT_TRUE(IN := Fifo16.EMPTY);
ASSERT_FALSE(IN := Fifo16.FULL);

Fifo16(DIN := DWORD#11, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Fifo16.EMPTY);
ASSERT_FALSE(IN := Fifo16.FULL);

Fifo16(DIN := DWORD#22, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
Fifo16(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#11, ACTUAL := Fifo16.DOUT);
ASSERT_FALSE(IN := Fifo16.EMPTY);

Fifo16(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#22, ACTUAL := Fifo16.DOUT);
ASSERT_TRUE(IN := Fifo16.EMPTY);

Fifo32(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
Fifo32(DIN := DWORD#33, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
Fifo32(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#33, ACTUAL := Fifo32.DOUT);
ASSERT_TRUE(IN := Fifo32.EMPTY);

Stack16(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ASSERT_TRUE(IN := Stack16.EMPTY);

Stack16(DIN := DWORD#11, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
Stack16(DIN := DWORD#22, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
Stack16(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#22, ACTUAL := Stack16.DOUT);
ASSERT_FALSE(IN := Stack16.EMPTY);

Stack16(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#11, ACTUAL := Stack16.DOUT);
ASSERT_TRUE(IN := Stack16.EMPTY);

Stack32(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
Stack32(DIN := DWORD#44, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
Stack32(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#44, ACTUAL := Stack32.DOUT);
ASSERT_TRUE(IN := Stack32.EMPTY);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_delay_behaves
VAR
    DelayLine : DELAY;
END_VAR

DelayLine(IN := REAL#1.0, N := INT#2, RST := TRUE);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);

DelayLine(IN := REAL#2.0, N := INT#2, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);

DelayLine(IN := REAL#3.0, N := INT#2, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);

DelayLine(IN := REAL#4.0, N := INT#2, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);

DelayLine(IN := REAL#9.0, N := INT#2, RST := TRUE);
ASSERT_NEAR(EXPECTED := REAL#9.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_crc_gen_behaves
VAR
    Data : ARRAY[0..3] OF BYTE;
    Checksum : DWORD;
END_VAR

Data[0] := BYTE#1;
Data[1] := BYTE#2;
Data[2] := BYTE#3;
Data[3] := BYTE#4;

Checksum := CRC_GEN(
    PT := ADR(Data),
    SIZE := INT#4,
    PL := INT#8,
    PN := DWORD#16#07,
    INIT := DWORD#0,
    REV_IN := FALSE,
    REV_OUT := FALSE,
    XOR_OUT := DWORD#0
);
ASSERT_EQUAL(EXPECTED := DWORD#16#E3, ACTUAL := Checksum);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_matrix_helpers_behave
VAR
    PressOnly : MATRIX;
    WithRelease : MATRIX;
END_VAR

PressOnly(X1 := TRUE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := FALSE);
ASSERT_TRUE(IN := PressOnly.TP);
ASSERT_EQUAL(EXPECTED := BYTE#16#81, ACTUAL := PressOnly.CODE);

PressOnly(X1 := FALSE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := FALSE);
ASSERT_FALSE(IN := PressOnly.TP);
PressOnly(X1 := FALSE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := FALSE);
ASSERT_FALSE(IN := PressOnly.TP);
PressOnly(X1 := FALSE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := FALSE);
ASSERT_FALSE(IN := PressOnly.TP);
PressOnly(X1 := FALSE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := FALSE);
ASSERT_FALSE(IN := PressOnly.TP);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := PressOnly.CODE);

WithRelease(X1 := FALSE, X2 := TRUE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := TRUE);
ASSERT_TRUE(IN := WithRelease.TP);
ASSERT_EQUAL(EXPECTED := BYTE#16#82, ACTUAL := WithRelease.CODE);

WithRelease(X1 := FALSE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := TRUE);
ASSERT_FALSE(IN := WithRelease.TP);
WithRelease(X1 := FALSE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := TRUE);
ASSERT_FALSE(IN := WithRelease.TP);
WithRelease(X1 := FALSE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := TRUE);
ASSERT_FALSE(IN := WithRelease.TP);
WithRelease(X1 := FALSE, X2 := FALSE, X3 := FALSE, X4 := FALSE, X5 := FALSE, Release := TRUE);
ASSERT_TRUE(IN := WithRelease.TP);
ASSERT_EQUAL(EXPECTED := BYTE#16#02, ACTUAL := WithRelease.CODE);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_logic_pin_code_helpers_behave
VAR
    Door : PIN_CODE;
END_VAR

Door(CB := BYTE#49, E := TRUE, PIN := '12');
ASSERT_FALSE(IN := Door.TP);
Door(CB := BYTE#50, E := TRUE, PIN := '12');
ASSERT_TRUE(IN := Door.TP);

Door(CB := BYTE#50, E := TRUE, PIN := '12');
ASSERT_FALSE(IN := Door.TP);
Door(CB := BYTE#49, E := TRUE, PIN := '12');
ASSERT_FALSE(IN := Door.TP);
Door(CB := BYTE#50, E := TRUE, PIN := '12');
ASSERT_TRUE(IN := Door.TP);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_buffer_management_copy_helpers_behave
VAR
    Buffer : ARRAY[0..7] OF BYTE;
    NextPos : INT;
    InsertPos : INT;
END_VAR

ASSERT_TRUE(IN := _BUFFER_INIT(PT := Buffer, SIZE := UINT#6, INIT := BYTE#42));
ASSERT_EQUAL(EXPECTED := BYTE#42, ACTUAL := Buffer[0]);
ASSERT_EQUAL(EXPECTED := BYTE#42, ACTUAL := Buffer[5]);

ASSERT_TRUE(IN := _BUFFER_CLEAR(PT := Buffer, SIZE := UINT#2));
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Buffer[0]);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Buffer[1]);
ASSERT_EQUAL(EXPECTED := BYTE#42, ACTUAL := Buffer[2]);

NextPos := _STRING_TO_BUFFER(STR := 'Ab', POS := INT#1, PT := Buffer, SIZE := UINT#8);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := NextPos);
ASSERT_EQUAL(EXPECTED := BYTE#65, ACTUAL := Buffer[1]);
ASSERT_EQUAL(EXPECTED := BYTE#98, ACTUAL := Buffer[2]);

ASSERT_TRUE(IN := _BUFFER_UPPERCASE(PT := Buffer, SIZE := INT#3));
ASSERT_EQUAL(EXPECTED := BYTE#66, ACTUAL := Buffer[2]);

ASSERT_TRUE(IN := _BUFFER_CLEAR(PT := Buffer, SIZE := UINT#8));
NextPos := _STRING_TO_BUFFER(STR := 'ABCD', POS := INT#0, PT := Buffer, SIZE := UINT#8);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := NextPos);

InsertPos := _BUFFER_INSERT(STR := 'xy', POS := INT#2, PT := Buffer, SIZE := UINT#8);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := InsertPos);
ASSERT_EQUAL(EXPECTED := BYTE#65, ACTUAL := Buffer[0]);
ASSERT_EQUAL(EXPECTED := BYTE#66, ACTUAL := Buffer[1]);
ASSERT_EQUAL(EXPECTED := BYTE#120, ACTUAL := Buffer[2]);
ASSERT_EQUAL(EXPECTED := BYTE#121, ACTUAL := Buffer[3]);
ASSERT_EQUAL(EXPECTED := BYTE#67, ACTUAL := Buffer[4]);
ASSERT_EQUAL(EXPECTED := BYTE#68, ACTUAL := Buffer[5]);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_buffer_management_search_helpers_behave
VAR
    Buffer : ARRAY[0..7] OF BYTE;
    Pattern : ARRAY[0..7] OF BYTE;
    MatchPos : INT;
    SearchPos : INT;
    Extracted : STRING[STRING_LENGTH];
END_VAR

ASSERT_TRUE(IN := _BUFFER_CLEAR(PT := Buffer, SIZE := UINT#8));
ASSERT_TRUE(IN := _BUFFER_CLEAR(PT := Pattern, SIZE := UINT#8));
ASSERT_EQUAL(
    EXPECTED := INT#6,
    ACTUAL := _STRING_TO_BUFFER(STR := 'abxycd', POS := INT#0, PT := Buffer, SIZE := UINT#8)
);
ASSERT_EQUAL(
    EXPECTED := INT#3,
    ACTUAL := _STRING_TO_BUFFER(STR := 'xyc', POS := INT#0, PT := Pattern, SIZE := UINT#8)
);

MatchPos := BUFFER_COMP(PT1 := Buffer, SIZE1 := INT#6, PT2 := Pattern, SIZE2 := INT#3, START := INT#0);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := MatchPos);

SearchPos := BUFFER_SEARCH(PT := Buffer, SIZE := INT#6, STR := 'XYC', POS := INT#0, IGN := TRUE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := SearchPos);
SearchPos := BUFFER_SEARCH(PT := Buffer, SIZE := INT#6, STR := 'XYC', POS := INT#0, IGN := FALSE);
ASSERT_EQUAL(EXPECTED := INT#-1, ACTUAL := SearchPos);

ASSERT_TRUE(IN := _BUFFER_UPPERCASE(PT := Buffer, SIZE := INT#6));
Extracted := BUFFER_TO_STRING(PT := Buffer, SIZE := UINT#6, START := UINT#2, STOP := UINT#4);
ASSERT_EQUAL(EXPECTED := 'XYC', ACTUAL := Extracted);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_list_processing_mutation_helpers_behave
VAR
    List : STRING[LIST_LENGTH];
END_VAR

List := '';
ASSERT_TRUE(IN := LIST_ADD(SEP := BYTE#44, INS := 'alpha', LIST := List));
ASSERT_TRUE(IN := LIST_ADD(SEP := BYTE#44, INS := 'beta', LIST := List));
ASSERT_EQUAL(EXPECTED := ',alpha,beta', ACTUAL := List);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := LIST_LEN(SEP := BYTE#44, LIST := List));
ASSERT_EQUAL(EXPECTED := 'alpha', ACTUAL := LIST_GET(SEP := BYTE#44, POS := INT#1, LIST := List));
ASSERT_EQUAL(EXPECTED := 'beta', ACTUAL := LIST_GET(SEP := BYTE#44, POS := INT#2, LIST := List));

ASSERT_TRUE(IN := LIST_INSERT(SEP := BYTE#44, POS := INT#2, INS := 'mid', LIST := List));
ASSERT_EQUAL(EXPECTED := ',alpha,mid,beta', ACTUAL := List);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := LIST_LEN(SEP := BYTE#44, LIST := List));
ASSERT_EQUAL(EXPECTED := 'mid', ACTUAL := LIST_GET(SEP := BYTE#44, POS := INT#2, LIST := List));

List := ',alpha,,beta,';
ASSERT_TRUE(IN := LIST_CLEAN(SEP := BYTE#44, LIST := List));
ASSERT_EQUAL(EXPECTED := ',alpha,beta', ACTUAL := List);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_list_processing_iteration_and_retrieval_helpers_behave
VAR
    List : STRING[LIST_LENGTH];
    Walker : LIST_NEXT;
    Value : STRING[LIST_LENGTH];
END_VAR

List := ',alpha,mid,beta';
Walker(SEP := BYTE#44, RST := TRUE, LIST := List);
ASSERT_FALSE(IN := Walker.NUL);
ASSERT_EQUAL(EXPECTED := 'alpha', ACTUAL := Walker.LEL);

Walker(SEP := BYTE#44, RST := FALSE, LIST := List);
ASSERT_FALSE(IN := Walker.NUL);
ASSERT_EQUAL(EXPECTED := 'mid', ACTUAL := Walker.LEL);

Value := LIST_RETRIEVE(SEP := BYTE#44, POS := INT#2, LIST := List);
ASSERT_EQUAL(EXPECTED := 'mid', ACTUAL := Value);
ASSERT_EQUAL(EXPECTED := ',alpha,beta', ACTUAL := List);

Value := LIST_RETRIEVE_LAST(SEP := BYTE#44, LIST := List);
ASSERT_EQUAL(EXPECTED := 'beta', ACTUAL := Value);
ASSERT_EQUAL(EXPECTED := ',alpha', ACTUAL := List);

Walker(SEP := BYTE#44, RST := TRUE, LIST := List);
ASSERT_FALSE(IN := Walker.NUL);
ASSERT_EQUAL(EXPECTED := 'alpha', ACTUAL := Walker.LEL);
Walker(SEP := BYTE#44, RST := FALSE, LIST := List);
ASSERT_TRUE(IN := Walker.NUL);
ASSERT_EQUAL(EXPECTED := '', ACTUAL := Walker.LEL);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_math_geometry_helpers_behave
ASSERT_NEAR(EXPECTED := REAL#3.1415926, ACTUAL := CIRCLE_A(Rx := REAL#1.0, Ax := REAL#360.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#6.2831853, ACTUAL := CIRCLE_C(Rx := REAL#1.0, Ax := REAL#360.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#1.5707963, ACTUAL := CIRCLE_SEG(Rx := REAL#1.0, Hx := REAL#1.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#3.1415926, ACTUAL := CONE_V(Rx := REAL#1.0, Hx := REAL#3.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#18.8495559, ACTUAL := ELLIPSE_A(R1 := REAL#2.0, R2 := REAL#3.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#15.8654376, ACTUAL := ELLIPSE_C(R1 := REAL#2.0, R2 := REAL#3.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#4.1887902, ACTUAL := SPHERE_V(Rx := REAL#1.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#6.0, ACTUAL := TRIANGLE_A(S1 := REAL#3.0, A := REAL#0.0, S2 := REAL#4.0, S3 := REAL#5.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_math_real2_helpers_behave
VAR
    Base : REAL2;
    Sum1 : REAL2;
    Sum2 : REAL2;
    Product : REAL2;
    Negative : REAL2;
    Absolute : REAL2;
END_VAR

Base := R2_SET(X := REAL#1.5);
ASSERT_NEAR(EXPECTED := REAL#1.5, ACTUAL := Base.RX, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Base.R1, DELTA := REAL#0.0001);

Sum1 := R2_ADD(X := Base, Y := REAL#0.25);
ASSERT_NEAR(EXPECTED := REAL#1.75, ACTUAL := Sum1.RX, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Sum1.R1, DELTA := REAL#0.0001);

Sum2 := R2_ADD2(X := Base, Y := R2_SET(X := REAL#2.0));
ASSERT_NEAR(EXPECTED := REAL#3.5, ACTUAL := Sum2.RX, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Sum2.R1, DELTA := REAL#0.0001);

Product := R2_MUL(X := Base, Y := REAL#2.0);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := Product.RX, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Product.R1, DELTA := REAL#0.0001);

Negative.RX := REAL#-1.5;
Negative.R1 := REAL#-0.25;
Absolute := R2_ABS(X := Negative);
ASSERT_NEAR(EXPECTED := REAL#1.5, ACTUAL := Absolute.RX, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#0.25, ACTUAL := Absolute.R1, DELTA := REAL#0.0001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_math_linear_and_polynomial_helpers_behave
VAR
    Coefficients : ARRAY[0..7] OF REAL;
END_VAR

Coefficients[0] := REAL#1.0;
Coefficients[1] := REAL#2.0;
Coefficients[2] := REAL#3.0;
Coefficients[3] := REAL#0.0;
Coefficients[4] := REAL#0.0;
Coefficients[5] := REAL#0.0;
Coefficients[6] := REAL#0.0;
Coefficients[7] := REAL#0.0;

ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := F_LIN(X := REAL#2.0, A := REAL#3.0, B := REAL#4.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := F_LIN2(X := REAL#2.0, X1 := REAL#0.0, Y1 := REAL#0.0, X2 := REAL#4.0, Y2 := REAL#8.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#17.0, ACTUAL := F_POLY(X := REAL#2.0, C := Coefficients), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#18.0, ACTUAL := F_POWER(A := REAL#2.0, X := REAL#3.0, N := REAL#2.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#11.0, ACTUAL := F_QUAD(X := REAL#2.0, A := REAL#1.0, B := REAL#2.0, C := REAL#3.0), DELTA := REAL#0.0001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_math_frmp_b_behaves
ASSERT_EQUAL(EXPECTED := BYTE#35, ACTUAL := FRMP_B(START := BYTE#10, DIR := TRUE, TD := T#100ms, TR := T#1s));
ASSERT_EQUAL(EXPECTED := BYTE#189, ACTUAL := FRMP_B(START := BYTE#240, DIR := FALSE, TD := T#200ms, TR := T#1s));
ASSERT_EQUAL(EXPECTED := BYTE#255, ACTUAL := FRMP_B(START := BYTE#10, DIR := TRUE, TD := T#1s, TR := T#1s));
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := FRMP_B(START := BYTE#10, DIR := FALSE, TD := T#1s, TR := T#1s));
END_TEST_PROGRAM

TEST_PROGRAM oscat_basic_math_ft_avg_behaves
VAR
    Avg : FT_AVG;
END_VAR

Avg(IN := REAL#4.0, E := TRUE, N := INT#4, RST := TRUE);
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := Avg.AVG, DELTA := REAL#0.0001);

Avg(IN := REAL#8.0, E := TRUE, N := INT#4, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := Avg.AVG, DELTA := REAL#0.0001);

Avg(IN := REAL#12.0, E := FALSE, N := INT#4, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := Avg.AVG, DELTA := REAL#0.0001);

Avg(IN := REAL#10.0, E := TRUE, N := INT#4, RST := TRUE);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := Avg.AVG, DELTA := REAL#0.0001);
END_TEST_PROGRAM

TEST_FUNCTION_BLOCK oscat_basic_energy_fb_converts_units
VAR
    Dut : ENERGY;
END_VAR

Dut(Wh := REAL#1.0);
ASSERT_NEAR(EXPECTED := REAL#3600.0, ACTUAL := Dut.YJ, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := Dut.YWh, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#859.8452, ACTUAL := Dut.YC, DELTA := REAL#0.1);
END_TEST_FUNCTION_BLOCK

TEST_FUNCTION_BLOCK oscat_basic_length_fb_converts_units
VAR
    Dut : LENGTH;
END_VAR

Dut(m := REAL#1.0);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := Dut.Ym, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#39.37007874016, ACTUAL := Dut.Yin, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#3.28083989501, ACTUAL := Dut.Yft, DELTA := REAL#0.001);
END_TEST_FUNCTION_BLOCK

TEST_FUNCTION_BLOCK oscat_basic_pressure_fb_converts_units
VAR
    Dut : PRESSURE;
END_VAR

Dut(bar := REAL#1.0);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := Dut.Ybar, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#100000.0, ACTUAL := Dut.Ypa, DELTA := REAL#0.1);
ASSERT_NEAR(EXPECTED := REAL#0.9869232667160, ACTUAL := Dut.Yatm, DELTA := REAL#0.001);
END_TEST_FUNCTION_BLOCK

TEST_FUNCTION_BLOCK oscat_basic_speed_fb_converts_units
VAR
    Dut : SPEED;
END_VAR

Dut(ms := REAL#10.0);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := Dut.Yms, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#36.0, ACTUAL := Dut.Ykmh, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#22.369362920544, ACTUAL := Dut.Ymh, DELTA := REAL#0.001);
END_TEST_FUNCTION_BLOCK

TEST_FUNCTION_BLOCK oscat_basic_temperature_fb_converts_units
VAR
    Dut : TEMPERATURE;
END_VAR

Dut(
    K := REAL#0.0,
    C := REAL#25.0,
    F := REAL#-459.67,
    Re := REAL#-218.52,
    Ra := REAL#0.0
);
ASSERT_NEAR(EXPECTED := REAL#298.15, ACTUAL := Dut.YK, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#25.0, ACTUAL := Dut.YC, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#77.0, ACTUAL := Dut.YF, DELTA := REAL#0.01);
END_TEST_FUNCTION_BLOCK
