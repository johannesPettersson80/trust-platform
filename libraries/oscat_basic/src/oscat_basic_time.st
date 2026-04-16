FUNCTION DAY_TO_TIME : TIME
VAR_INPUT
    IN : REAL;
END_VAR
DAY_TO_TIME := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(IN * 86400000.0)));
END_FUNCTION

FUNCTION HOUR_TO_TIME : TIME
VAR_INPUT
    IN : REAL;
END_VAR
HOUR_TO_TIME := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(IN * 3600000.0)));
END_FUNCTION

FUNCTION MINUTE_TO_TIME : TIME
VAR_INPUT
    IN : REAL;
END_VAR
MINUTE_TO_TIME := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(IN * 60000.0)));
END_FUNCTION

FUNCTION SECOND_TO_TIME : TIME
VAR_INPUT
    IN : REAL;
END_VAR
SECOND_TO_TIME := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(IN * 1000.0)));
END_FUNCTION

FUNCTION HOUR : INT
VAR_INPUT
    ITOD : TOD;
END_VAR
VAR
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
END_VAR

SPLIT_TOD(ITOD, HourValue, MinuteValue, SecondValue, MillisecondValue);
HOUR := HourValue;
END_FUNCTION

FUNCTION MINUTE : INT
VAR_INPUT
    ITOD : TOD;
END_VAR
VAR
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
END_VAR

SPLIT_TOD(ITOD, HourValue, MinuteValue, SecondValue, MillisecondValue);
MINUTE := MinuteValue;
END_FUNCTION

FUNCTION SECOND : REAL
VAR_INPUT
    ITOD : TOD;
END_VAR
VAR
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
END_VAR

SPLIT_TOD(ITOD, HourValue, MinuteValue, SecondValue, MillisecondValue);
SECOND := INT_TO_REAL(SecondValue) + INT_TO_REAL(MillisecondValue) / REAL#1000.0;
END_FUNCTION

FUNCTION HOUR_TO_TOD : TOD
VAR_INPUT
    IN : REAL;
END_VAR

HOUR_TO_TOD := ADD_TOD_TIME(TOD#00:00:00, HOUR_TO_TIME(IN));
END_FUNCTION

FUNCTION SET_TOD : TOD
VAR_INPUT
    HOUR : INT;
    MINUTE : INT;
    SECOND : REAL;
END_VAR

SET_TOD := ADD_TOD_TIME(
    TOD#00:00:00,
    ADD_TIME(
        HOUR_TO_TIME(INT_TO_REAL(HOUR)),
        ADD_TIME(
            MINUTE_TO_TIME(INT_TO_REAL(MINUTE)),
            SECOND_TO_TIME(SECOND)
        )
    )
);
END_FUNCTION

FUNCTION SET_DT : DT
VAR_INPUT
    YEAR : INT;
    MONTH : INT;
    DAY : INT;
    HOUR : INT;
    MINUTE : INT;
    SECOND : INT;
END_VAR

SET_DT := ADD_DT_TIME(
    CONCAT_DATE_TOD(
        SET_DATE(YEAR, MONTH, DAY),
        TOD#00:00:00
    ),
    ADD_TIME(
        HOUR_TO_TIME(INT_TO_REAL(HOUR)),
        ADD_TIME(
            MINUTE_TO_TIME(INT_TO_REAL(MINUTE)),
            SECOND_TO_TIME(INT_TO_REAL(SECOND))
        )
    )
);
END_FUNCTION

FUNCTION DAYS_DELTA : DINT
VAR_INPUT
    date_1 : DATE;
    date_2 : DATE;
END_VAR
VAR
    Year1 : INT;
    Month1 : INT;
    Day1 : INT;
    Year2 : INT;
    Month2 : INT;
    Day2 : INT;
    Adjust1 : DINT;
    Adjust2 : DINT;
    CalcYear1 : DINT;
    CalcYear2 : DINT;
    CalcMonth1 : DINT;
    CalcMonth2 : DINT;
    Ordinal1 : DINT;
    Ordinal2 : DINT;
END_VAR

SPLIT_DATE(date_1, Year1, Month1, Day1);
SPLIT_DATE(date_2, Year2, Month2, Day2);

Adjust1 := (DINT#14 - INT_TO_DINT(Month1)) / DINT#12;
Adjust2 := (DINT#14 - INT_TO_DINT(Month2)) / DINT#12;
CalcYear1 := INT_TO_DINT(Year1) + DINT#4800 - Adjust1;
CalcYear2 := INT_TO_DINT(Year2) + DINT#4800 - Adjust2;
CalcMonth1 := INT_TO_DINT(Month1) + DINT#12 * Adjust1 - DINT#3;
CalcMonth2 := INT_TO_DINT(Month2) + DINT#12 * Adjust2 - DINT#3;
Ordinal1 := INT_TO_DINT(Day1)
    + (DINT#153 * CalcMonth1 + DINT#2) / DINT#5
    + DINT#365 * CalcYear1
    + CalcYear1 / DINT#4
    - CalcYear1 / DINT#100
    + CalcYear1 / DINT#400
    - DINT#32045;
Ordinal2 := INT_TO_DINT(Day2)
    + (DINT#153 * CalcMonth2 + DINT#2) / DINT#5
    + DINT#365 * CalcYear2
    + CalcYear2 / DINT#4
    - CalcYear2 / DINT#100
    + CalcYear2 / DINT#400
    - DINT#32045;

DAYS_DELTA := Ordinal2 - Ordinal1;
END_FUNCTION

FUNCTION DAYS_IN_MONTH : INT
VAR_INPUT
    IDATE : DATE;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
END_VAR

SPLIT_DATE(IDATE, YearValue, MonthValue, DayValue);

CASE MonthValue OF
    INT#1, INT#3, INT#5, INT#7, INT#8, INT#10, INT#12:
        DAYS_IN_MONTH := INT#31;
    INT#4, INT#6, INT#9, INT#11:
        DAYS_IN_MONTH := INT#30;
    INT#2:
        IF (YearValue MOD INT#4) = INT#0 THEN
            IF (YearValue MOD INT#100) = INT#0 THEN
                IF (YearValue MOD INT#400) = INT#0 THEN
                    DAYS_IN_MONTH := INT#29;
                ELSE
                    DAYS_IN_MONTH := INT#28;
                END_IF;
            ELSE
                DAYS_IN_MONTH := INT#29;
            END_IF;
        ELSE
            DAYS_IN_MONTH := INT#28;
        END_IF;
    ELSE
        DAYS_IN_MONTH := INT#0;
END_CASE;
END_FUNCTION

FUNCTION DAYS_IN_YEAR : INT
VAR_INPUT
    IDATE : DATE;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
END_VAR

SPLIT_DATE(IDATE, YearValue, MonthValue, DayValue);

IF (YearValue MOD INT#4) = INT#0 THEN
    IF (YearValue MOD INT#100) = INT#0 THEN
        IF (YearValue MOD INT#400) = INT#0 THEN
            DAYS_IN_YEAR := INT#366;
        ELSE
            DAYS_IN_YEAR := INT#365;
        END_IF;
    ELSE
        DAYS_IN_YEAR := INT#366;
    END_IF;
ELSE
    DAYS_IN_YEAR := INT#365;
END_IF;
END_FUNCTION

FUNCTION DAY_OF_DATE : DINT
VAR_INPUT
    IDATE : DATE;
END_VAR

DAY_OF_DATE := DAYS_DELTA(
    date_1 := CONCAT_DATE(INT#1970, INT#1, INT#1),
    date_2 := IDATE
);
END_FUNCTION

FUNCTION DAY_OF_MONTH : INT
VAR_INPUT
    IDATE : DATE;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
END_VAR

SPLIT_DATE(IDATE, YearValue, MonthValue, DayValue);
DAY_OF_MONTH := DayValue;
END_FUNCTION

FUNCTION DAY_OF_YEAR : INT
VAR_INPUT
    IDATE : DATE;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
    DaysBefore : INT;
END_VAR

SPLIT_DATE(IDATE, YearValue, MonthValue, DayValue);

CASE MonthValue OF
    INT#1:
        DaysBefore := INT#0;
    INT#2:
        DaysBefore := INT#31;
    INT#3:
        DaysBefore := INT#59;
    INT#4:
        DaysBefore := INT#90;
    INT#5:
        DaysBefore := INT#120;
    INT#6:
        DaysBefore := INT#151;
    INT#7:
        DaysBefore := INT#181;
    INT#8:
        DaysBefore := INT#212;
    INT#9:
        DaysBefore := INT#243;
    INT#10:
        DaysBefore := INT#273;
    INT#11:
        DaysBefore := INT#304;
    INT#12:
        DaysBefore := INT#334;
    ELSE
        DaysBefore := INT#0;
END_CASE;

DAY_OF_YEAR := DaysBefore + DayValue;
IF MonthValue > INT#2 THEN
    IF LEAP_YEAR(yr := YearValue) THEN
        DAY_OF_YEAR := DAY_OF_YEAR + INT#1;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION EASTER : DATE
VAR_INPUT
    year : INT;
END_VAR
VAR
    b : INT;
    c : INT;
    oday : INT;
END_VAR

b := (INT#204 - INT#11 * (year MOD INT#19)) MOD INT#30;
IF b > INT#27 THEN
    b := b - INT#1;
END_IF;
c := (year + year / INT#4 + b - INT#13) MOD INT#7;
oday := INT#28 + b - c;
IF oday > INT#33 THEN
    EASTER := CONCAT_DATE(year, INT#4, oday - INT#31);
ELSE
    EASTER := CONCAT_DATE(year, INT#3, oday);
END_IF;
END_FUNCTION

FUNCTION DATE_ADD : DATE
VAR_INPUT
    IDATE : DATE;
    D : INT;
    W : INT;
    M : INT;
    Y : INT;
END_VAR
VAR
    BaseDays : DINT;
    Jdn : DINT;
    L : DINT;
    N : DINT;
    I : DINT;
    J : DINT;
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
END_VAR

BaseDays := DAY_OF_DATE(IDATE := IDATE)
    + INT_TO_DINT(D)
    + INT_TO_DINT(W) * DINT#7;
Jdn := BaseDays + DINT#2440588;
L := Jdn + DINT#68569;
N := (DINT#4 * L) / DINT#146097;
L := L - (DINT#146097 * N + DINT#3) / DINT#4;
I := (DINT#4000 * (L + DINT#1)) / DINT#1461001;
L := L - (DINT#1461 * I) / DINT#4 + DINT#31;
J := (DINT#80 * L) / DINT#2447;
DayValue := DINT_TO_INT(L - (DINT#2447 * J) / DINT#80);
L := J / DINT#11;
MonthValue := DINT_TO_INT(J + DINT#2 - DINT#12 * L);
YearValue := DINT_TO_INT(DINT#100 * (N - DINT#49) + I + L);

YearValue := YearValue + Y;
MonthValue := MonthValue + M;

WHILE MonthValue > INT#12 DO
    MonthValue := MonthValue - INT#12;
    YearValue := YearValue + INT#1;
END_WHILE;

WHILE MonthValue < INT#1 DO
    MonthValue := MonthValue + INT#12;
    YearValue := YearValue - INT#1;
END_WHILE;

DATE_ADD := SET_DATE(
    YEAR := YearValue,
    MONTH := MonthValue,
    DAY := DayValue
);
END_FUNCTION

FUNCTION LEAP_DAY : BOOL
VAR_INPUT
    IDATE : DATE;
END_VAR

LEAP_DAY := MONTH_OF_DATE(IDATE := IDATE) = INT#2
    AND DAY_OF_MONTH(IDATE := IDATE) = INT#29
    AND LEAP_OF_DATE(IDATE := IDATE);
END_FUNCTION

FUNCTION LEAP_OF_DATE : BOOL
VAR_INPUT
    IDATE : DATE;
END_VAR

LEAP_OF_DATE := LEAP_YEAR(yr := YEAR_OF_DATE(IDATE := IDATE));
END_FUNCTION

FUNCTION LEAP_YEAR : BOOL
VAR_INPUT
    yr : INT;
END_VAR

IF (yr MOD INT#4) = INT#0 THEN
    IF (yr MOD INT#100) = INT#0 THEN
        IF (yr MOD INT#400) = INT#0 THEN
            LEAP_YEAR := TRUE;
        ELSE
            LEAP_YEAR := FALSE;
        END_IF;
    ELSE
        LEAP_YEAR := TRUE;
    END_IF;
ELSE
    LEAP_YEAR := FALSE;
END_IF;
END_FUNCTION

FUNCTION MONTH_BEGIN : DATE
VAR_INPUT
    IDATE : DATE;
END_VAR

MONTH_BEGIN := CONCAT_DATE(
    YEAR := YEAR_OF_DATE(IDATE := IDATE),
    MONTH := MONTH_OF_DATE(IDATE := IDATE),
    DAY := INT#1
);
END_FUNCTION

FUNCTION MONTH_END : DATE
VAR_INPUT
    IDATE : DATE;
END_VAR

MONTH_END := CONCAT_DATE(
    YEAR := YEAR_OF_DATE(IDATE := IDATE),
    MONTH := MONTH_OF_DATE(IDATE := IDATE),
    DAY := DAYS_IN_MONTH(IDATE := IDATE)
);
END_FUNCTION

FUNCTION MONTH_OF_DATE : INT
VAR_INPUT
    IDATE : DATE;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
END_VAR

SPLIT_DATE(IDATE, YearValue, MonthValue, DayValue);
MONTH_OF_DATE := MonthValue;
END_FUNCTION

FUNCTION HOUR_OF_DT : INT
VAR_INPUT
    XDT : DT;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
END_VAR

SPLIT_DT(
    XDT,
    YearValue,
    MonthValue,
    DayValue,
    HourValue,
    MinuteValue,
    SecondValue,
    MillisecondValue
);
HOUR_OF_DT := HourValue;
END_FUNCTION

FUNCTION MINUTE_OF_DT : INT
VAR_INPUT
    XDT : DT;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
END_VAR

SPLIT_DT(
    XDT,
    YearValue,
    MonthValue,
    DayValue,
    HourValue,
    MinuteValue,
    SecondValue,
    MillisecondValue
);
MINUTE_OF_DT := MinuteValue;
END_FUNCTION

FUNCTION SECOND_OF_DT : INT
VAR_INPUT
    XDT : DT;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
END_VAR

SPLIT_DT(
    XDT,
    YearValue,
    MonthValue,
    DayValue,
    HourValue,
    MinuteValue,
    SecondValue,
    MillisecondValue
);
SECOND_OF_DT := SecondValue;
END_FUNCTION

FUNCTION SET_DATE : DATE
VAR_INPUT
    YEAR : INT;
    MONTH : INT;
    DAY : INT;
END_VAR

SET_DATE := CONCAT_DATE(YEAR, MONTH, DAY);
END_FUNCTION

FUNCTION YEAR_BEGIN : DATE
VAR_INPUT
    y : INT;
END_VAR

YEAR_BEGIN := CONCAT_DATE(y, INT#1, INT#1);
END_FUNCTION

FUNCTION YEAR_END : DATE
VAR_INPUT
    y : INT;
END_VAR

YEAR_END := CONCAT_DATE(y, INT#12, INT#31);
END_FUNCTION

FUNCTION YEAR_OF_DATE : INT
VAR_INPUT
    IDATE : DATE;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
END_VAR

SPLIT_DATE(IDATE, YearValue, MonthValue, DayValue);
YEAR_OF_DATE := YearValue;
END_FUNCTION

FUNCTION WORK_WEEK : INT
VAR_INPUT
    idate : DATE;
END_VAR
VAR
    YearValue : INT;
    DayOfYearValue : INT;
    DayOfWeekValue : INT;
    IsoDayOfWeek : INT;
    WeekValue : INT;
    WeeksInYear : INT;
    PreviousYear : INT;
    PreviousJan1DayOfWeek : INT;
    Jan1DayOfWeek : INT;
END_VAR

YearValue := YEAR_OF_DATE(IDATE := idate);
DayOfYearValue := DAY_OF_YEAR(IDATE := idate);
DayOfWeekValue := DAY_OF_WEEK(idate);

IF DayOfWeekValue = INT#0 THEN
    IsoDayOfWeek := INT#7;
ELSE
    IsoDayOfWeek := DayOfWeekValue;
END_IF;

WeekValue := (DayOfYearValue - IsoDayOfWeek + INT#10) / INT#7;
Jan1DayOfWeek := DAY_OF_WEEK(CONCAT_DATE(YearValue, INT#1, INT#1));
WeeksInYear := INT#52;
IF Jan1DayOfWeek = INT#4 OR (Jan1DayOfWeek = INT#3 AND LEAP_YEAR(yr := YearValue)) THEN
    WeeksInYear := INT#53;
END_IF;

IF WeekValue < INT#1 THEN
    PreviousYear := YearValue - INT#1;
    PreviousJan1DayOfWeek := DAY_OF_WEEK(CONCAT_DATE(PreviousYear, INT#1, INT#1));
    WORK_WEEK := INT#52;
    IF PreviousJan1DayOfWeek = INT#4
        OR (PreviousJan1DayOfWeek = INT#3 AND LEAP_YEAR(yr := PreviousYear))
    THEN
        WORK_WEEK := INT#53;
    END_IF;
ELSIF WeekValue > WeeksInYear THEN
    WORK_WEEK := INT#1;
ELSE
    WORK_WEEK := WeekValue;
END_IF;
END_FUNCTION

FUNCTION T_PLC_MS : DWORD
VAR
    tx : TIME;
END_VAR
tx := TIME();
T_PLC_MS := TIME_TO_DWORD(tx);
END_FUNCTION

FUNCTION T_PLC_US : DWORD
VAR
    tx : TIME;
END_VAR
tx := TIME();
T_PLC_US := UDINT_TO_DWORD(DWORD_TO_UDINT(TIME_TO_DWORD(tx)) * UDINT#1000);
END_FUNCTION
