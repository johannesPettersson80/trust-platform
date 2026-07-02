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
    DAYS : INT;
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
    + INT_TO_DINT(DAYS)
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


FUNCTION MULTIME : TIME
VAR_INPUT
    t : TIME;
    M : REAL;
END_VAR

MULTIME := DWORD_TO_TIME(REAL_TO_DWORD(DWORD_TO_REAL(TIME_TO_DWORD(t)) * M));
END_FUNCTION

FUNCTION DAY_OF_WEEK : INT
VAR_INPUT
	IDATE : DATE;
END_VAR
DAY_OF_WEEK := DINT_TO_INT((DAY_OF_DATE(IDATE := IDATE) + DINT#3) MOD DINT#7) + INT#1;
END_FUNCTION

FUNCTION TIMECHECK : BOOL
VAR_INPUT
	TD : TOD;
	START : TOD;
	STOP : TOD;
END_VAR
IF stop < start THEN
	TIMECHECK := start <= TD OR TD < stop;
ELSE
	TIMECHECK := start <= TD AND TD < stop;
END_IF;
END_FUNCTION

FUNCTION DST : BOOL
VAR_INPUT
    UTC : DT;
END_VAR
VAR
    yr : INT;
    yr4 : INT;
    StartDay : INT;
    EndDay : INT;
    StartUtc : DT;
    EndUtc : DT;
END_VAR

yr := YEAR_OF_DATE(DT_TO_DATE(UTC));
yr4 := (yr * INT#5) / INT#4 + INT#1;
StartDay := INT#31 - ((yr4 + INT#3) MOD INT#7);
EndDay := INT#31 - (yr4 MOD INT#7);
StartUtc := SET_DT(yr, INT#3, StartDay, INT#1, INT#0, INT#0);
EndUtc := SET_DT(yr, INT#10, EndDay, INT#1, INT#0, INT#0);
DST := UTC >= StartUtc AND UTC < EndUtc;
END_FUNCTION

FUNCTION DT2_TO_SDT : SDT
VAR_INPUT
    DI : DATE;
    TI : TOD;
END_VAR
VAR
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
    Result : SDT;
END_VAR

SPLIT_TOD(TI, HourValue, MinuteValue, SecondValue, MillisecondValue);
Result.YEAR := YEAR_OF_DATE(DI);
Result.MONTH := MONTH_OF_DATE(DI);
Result.DAY := DAY_OF_MONTH(DI);
Result.WEEKDAY := DAY_OF_WEEK(DI);
Result.HOUR := HourValue;
Result.MINUTE := MinuteValue;
Result.SECOND := SecondValue;
Result.MS := MillisecondValue;
DT2_TO_SDT := Result;
END_FUNCTION

FUNCTION DT_TO_SDT : SDT
VAR_INPUT
    DTI : DT;
END_VAR
VAR
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
    Result : SDT;
END_VAR

SPLIT_DT(DTI, YearValue, MonthValue, DayValue, HourValue, MinuteValue, SecondValue, MillisecondValue);
Result.YEAR := YearValue;
Result.MONTH := MonthValue;
Result.DAY := DayValue;
Result.WEEKDAY := DAY_OF_WEEK(DT_TO_DATE(DTI));
Result.HOUR := HourValue;
Result.MINUTE := MinuteValue;
Result.SECOND := SecondValue;
Result.MS := MillisecondValue;
DT_TO_SDT := Result;
END_FUNCTION

FUNCTION_BLOCK EVENTS
VAR_INPUT
    DATE_IN : DATE;
    ENA : BOOL;
END_VAR
VAR_OUTPUT
    Y : BOOL;
    NAME : STRING[30];
END_VAR
VAR
    i : INT;
    last_active : DATE;
    size : INT := 49;
    day_in : DINT;
    cyr : INT;
    lday : DINT;
    check : HOLIDAY_DATA;
    y_int : BOOL;
    name_int : STRING[30];
END_VAR
VAR_IN_OUT
    ELIST : ARRAY[0..49] OF HOLIDAY_DATA;
END_VAR

IF last_active <> DATE_IN THEN
    last_active := DATE_IN;
    y_int := FALSE;
    name_int := '';
    day_in := DAY_OF_DATE(DATE_IN);
    cyr := YEAR_OF_DATE(DATE_IN);

    FOR i := 0 TO size DO
        check := ELIST[i];
        lday := DAY_OF_DATE(SET_DATE(cyr, check.MONTH, check.DAY));
        IF day_in >= lday AND day_in <= lday + check.USE - 1 THEN
            y_int := TRUE;
            name_int := check.NAME;
            EXIT;
        END_IF;
    END_FOR;
END_IF;

IF ENA THEN
    Y := y_int;
    NAME := name_int;
ELSE
    Y := FALSE;
    NAME := '';
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK HOLIDAY
VAR_INPUT
    DATE_IN : DATE;
    LANGU : INT;
    FRIDAY : BOOL;
    SATURDAY : BOOL;
    SUNDAY : BOOL;
END_VAR
VAR_IN_OUT
    HOLIDAYS : ARRAY[0..29] OF HOLIDAY_DATA;
END_VAR
VAR CONSTANT
    SIZE : INT := 29;
END_VAR
VAR_OUTPUT
    Y : BOOL;
    NAME : STRING[30];
END_VAR
VAR
    last_active : DATE;
    ostern : DATE;
    i : INT;
    jahr : INT;
    x_date : DATE;
    lx : INT;
    wdx : INT;
END_VAR

IF last_active <> DATE_IN THEN
    last_active := DATE_IN;

    IF LANGU = INT#0 THEN
        lx := LANGUAGE.DEFAULT;
    ELSE
        lx := MIN(LANGUAGE.LMAX, LANGU);
    END_IF;

    jahr := YEAR_OF_DATE(DATE_IN);
    ostern := EASTER(jahr);
    wdx := DAY_OF_WEEK(DATE_IN);
    Y := FALSE;
    NAME := '';

    FOR i := 0 TO SIZE DO
        IF NOT Y THEN
            IF HOLIDAYS[i].USE = SINT#1 AND HOLIDAYS[i].MONTH > SINT#0 THEN
                x_date := SET_DATE(jahr, HOLIDAYS[i].MONTH, HOLIDAYS[i].DAY);
                IF x_date = DATE_IN THEN
                    Y := TRUE;
                    NAME := HOLIDAYS[i].NAME;
                END_IF;
            ELSIF HOLIDAYS[i].USE = SINT#1 AND HOLIDAYS[i].MONTH = SINT#0 THEN
                IF DATE_ADD(ostern, HOLIDAYS[i].DAY, INT#0, INT#0, INT#0) = DATE_IN THEN
                    Y := TRUE;
                    NAME := HOLIDAYS[i].NAME;
                END_IF;
            ELSIF HOLIDAYS[i].USE < SINT#0 THEN
                x_date := SET_DATE(jahr, HOLIDAYS[i].MONTH, HOLIDAYS[i].DAY);
                IF DAY_OF_WEEK(DATE_IN) = ABS(HOLIDAYS[i].USE)
                    AND DATE_IN < x_date
                    AND DATE_IN >= DATE_ADD(x_date, INT#-7, INT#0, INT#0, INT#0)
                THEN
                    Y := TRUE;
                    NAME := HOLIDAYS[i].NAME;
                END_IF;
            END_IF;
        END_IF;
    END_FOR;

    IF NOT Y AND ((wdx = INT#5 AND FRIDAY) OR (wdx = INT#6 AND SATURDAY) OR (wdx = INT#7 AND SUNDAY)) THEN
        Y := TRUE;
        NAME := LANGUAGE.WEEKDAYS[lx, wdx];
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION JD2000 : REAL
VAR_INPUT
    DTI : DT;
END_VAR
VAR
    DatePart : DATE;
    TodPart : TOD;
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
END_VAR

DatePart := DT_TO_DATE(DTI);
TodPart := DT_TO_TOD(DTI);
SPLIT_TOD(TodPart, HourValue, MinuteValue, SecondValue, MillisecondValue);
JD2000 := DINT_TO_REAL(DAYS_DELTA(DATE#2000-01-01, DatePart))
    + (INT_TO_REAL(HourValue) - REAL#12.0) / REAL#24.0
    + INT_TO_REAL(MinuteValue) / REAL#1440.0
    + INT_TO_REAL(SecondValue) / REAL#86400.0
    + INT_TO_REAL(MillisecondValue) / REAL#86400000.0;
END_FUNCTION

FUNCTION LTIME_TO_UTC : DT
VAR_INPUT
    LOCAL_DT : DT;
    DST : BOOL;
    TIME_ZONE_OFFSET : INT;
END_VAR
VAR
    OffsetMinutes : INT;
    OffsetTime : TIME;
END_VAR

OffsetMinutes := TIME_ZONE_OFFSET;
IF DST THEN
    OffsetMinutes := OffsetMinutes + INT#60;
END_IF;
OffsetTime := MINUTE_TO_TIME(INT_TO_REAL(ABS(OffsetMinutes)));
IF OffsetMinutes < INT#0 THEN
    LTIME_TO_UTC := ADD_DT_TIME(LOCAL_DT, OffsetTime);
ELSE
    LTIME_TO_UTC := ADD_DT_TIME(LOCAL_DT, SUB_TIME(T#0s, OffsetTime));
END_IF;
END_FUNCTION

FUNCTION PERIOD : BOOL
VAR_INPUT
    d1 : DATE;
    dx : DATE;
    d2 : DATE;
END_VAR
VAR
    day1 : INT;
    day2 : INT;
    dayx : INT;
END_VAR

day1 := DAY_OF_YEAR(d1);
day2 := DAY_OF_YEAR(d2);
dayx := DAY_OF_YEAR(dx);
IF NOT LEAP_OF_DATE(dx) AND dayx > INT#58 THEN dayx := dayx + 1; END_IF;
IF NOT LEAP_OF_DATE(d1) AND day1 > INT#58 THEN day1 := day1 + 1; END_IF;
IF NOT LEAP_OF_DATE(d2) AND day2 > INT#58 THEN day2 := day2 + 1; END_IF;

IF day2 < day1 THEN
    PERIOD := dayx <= day2 OR dayx >= day1;
ELSE
    PERIOD := dayx >= day1 AND dayx <= day2;
END_IF;
END_FUNCTION

FUNCTION PERIOD2 : BOOL
VAR_INPUT
    DP : ARRAY[0..3, 0..1] OF DATE;
    DX : DATE;
END_VAR

PERIOD2 := (DX >= DP[0, 0] AND DX <= DP[0, 1])
    OR (DX >= DP[1, 0] AND DX <= DP[1, 1])
    OR (DX >= DP[2, 0] AND DX <= DP[2, 1])
    OR (DX >= DP[3, 0] AND DX <= DP[3, 1]);
END_FUNCTION

FUNCTION REFRACTION : REAL
VAR_INPUT
    ELEV : REAL;
END_VAR
VAR
    LimitedElev : REAL;
END_VAR

LimitedElev := LIMIT(REAL#-1.9, ELEV, REAL#80.0);
REFRACTION := REAL#0.0174532925199433 / TAN(REAL#0.0174532925199433 * (LimitedElev + REAL#10.3 / (LimitedElev + REAL#5.11)));
END_FUNCTION

FUNCTION_BLOCK RTC_MS
VAR_INPUT
    SET_RTC : BOOL;
    SDT : DT;
    SMS : INT;
END_VAR
VAR_OUTPUT
    XDT : DT;
    XMS : INT;
END_VAR
VAR
    init : BOOL;
    last : DWORD;
    Tx : DWORD;
END_VAR

Tx := T_PLC_MS();
IF SET_RTC OR NOT init THEN
    init := TRUE;
    XDT := SDT;
    XMS := SMS;
ELSE
    XMS := XMS + UDINT_TO_INT(DWORD_TO_UDINT(Tx) - DWORD_TO_UDINT(last));
    IF XMS > INT#999 THEN
        XDT := ADD_DT_TIME(XDT, T#1s);
        XMS := XMS - INT#1000;
    END_IF;
END_IF;
last := Tx;
END_FUNCTION_BLOCK

FUNCTION_BLOCK RTC_2
VAR_INPUT
    SET_RTC : BOOL;
    SDT : DT;
    SMS : INT;
    DEN : BOOL;
    OFS : INT;
END_VAR
VAR_OUTPUT
    UDT : DT;
    LOCAL_DT : DT;
    DSO : BOOL;
    XMS : INT;
END_VAR
VAR
    RT : RTC_MS;
    OffsetMinutes : INT;
    OffsetTime : TIME;
END_VAR

RT(SET_RTC := SET_RTC, SDT := SDT, SMS := SMS);
UDT := RT.XDT;
XMS := RT.XMS;

DSO := DST(UDT) AND DEN;
OffsetMinutes := OFS + BOOL_TO_INT(DSO) * INT#60;
OffsetTime := MINUTE_TO_TIME(INT_TO_REAL(ABS(OffsetMinutes)));
IF OffsetMinutes < INT#0 THEN
    LOCAL_DT := ADD_DT_TIME(UDT, SUB_TIME(T#0s, OffsetTime));
ELSE
    LOCAL_DT := ADD_DT_TIME(UDT, OffsetTime);
END_IF;
END_FUNCTION_BLOCK

FUNCTION SDT_TO_DATE : DATE
VAR_INPUT
    DTI : SDT;
END_VAR

SDT_TO_DATE := SET_DATE(DTI.YEAR, DTI.MONTH, DTI.DAY);
END_FUNCTION

FUNCTION SDT_TO_DT : DT
VAR_INPUT
    DTI : SDT;
END_VAR

SDT_TO_DT := SET_DT(DTI.YEAR, DTI.MONTH, DTI.DAY, DTI.HOUR, DTI.MINUTE, DTI.SECOND);
END_FUNCTION

FUNCTION SDT_TO_TOD : TOD
VAR_INPUT
    DTI : SDT;
END_VAR

SDT_TO_TOD := SET_TOD(
    HOUR := DTI.HOUR,
    MINUTE := DTI.MINUTE,
    SECOND := INT_TO_REAL(DTI.SECOND) + INT_TO_REAL(DTI.MS) / REAL#1000.0
);
END_FUNCTION

FUNCTION SUN_MIDDAY : TOD
VAR_INPUT
    LON : REAL;
    UTC : DATE;
END_VAR
VAR
    T : REAL;
    OFFSET : REAL;
END_VAR

T := INT_TO_REAL(DAY_OF_YEAR(UTC));
OFFSET := REAL#-0.1752 * SIN(REAL#0.033430 * T + REAL#0.5474) - REAL#0.1340 * SIN(REAL#0.018234 * T - REAL#0.1939);
SUN_MIDDAY := HOUR_TO_TOD(REAL#12.0 - OFFSET - LON * REAL#0.0666666666666);
END_FUNCTION

FUNCTION_BLOCK SUN_POS
VAR_INPUT
    latitude : REAL;
    longitude : REAL;
    utc : DT;
END_VAR
VAR_OUTPUT
    B : REAL;
    H : REAL;
    HR : REAL;
END_VAR
VAR
    g : REAL;
    a : REAL;
    d : REAL;
    t1 : REAL;
    n : REAL;
    e : REAL;
    c : REAL;
    tau : REAL;
    sin_d : REAL;
    rlat : REAL;
    sin_lat : REAL;
    cos_lat : REAL;
    cos_tau : REAL;
    cos_d : REAL;
    TodValue : TOD;
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
    DayHours : REAL;
END_VAR

n := JD2000(UTC);
g := MODR(IN := REAL#6.240040768 + REAL#0.01720197 * n, DIVI := MATH.PI2);
d := MODR(IN := REAL#4.89495042 + REAL#0.017202792 * n, DIVI := MATH.PI2)
    + REAL#0.033423055 * SIN(g)
    + REAL#0.000349066 * SIN(REAL#2.0 * g);
e := REAL#0.409087723 - REAL#0.000000006981317008 * n;
cos_d := COS(d);
sin_d := SIN(d);
a := ATAN(COS(e) * sin_d / cos_d);
IF cos_d < REAL#0.0 THEN a := a + MATH.PI; END_IF;
c := ASIN(SIN(e) * sin_d);

TodValue := DT_TO_TOD(utc);
SPLIT_TOD(TodValue, HourValue, MinuteValue, SecondValue, MillisecondValue);
DayHours := INT_TO_REAL(HourValue)
    + INT_TO_REAL(MinuteValue) / REAL#60.0
    + INT_TO_REAL(SecondValue) / REAL#3600.0
    + INT_TO_REAL(MillisecondValue) / REAL#3600000.0;
tau := RAD(
    DEG := MODR(
        IN := REAL#6.697376
            + (n - REAL#0.25) * REAL#0.0657098245037645
            + DayHours * REAL#0.041666666666666664,
        DIVI := REAL#24.0
    ) * REAL#15.0 + longitude
) - a;
rlat := RAD(DEG := latitude);
sin_lat := SIN(rlat);
cos_lat := COS(rlat);
cos_tau := COS(tau);
t1 := cos_tau * sin_lat - TAN(c) * cos_lat;
B := ATAN(SIN(tau) / t1);
IF t1 < REAL#0.0 THEN
    B := B + MATH.PI2;
ELSE
    B := B + MATH.PI;
END_IF;
B := DEG(RAD := MODR(IN := B, DIVI := MATH.PI2));
H := DEG(ASIN(COS(c) * cos_tau * cos_lat + SIN(c) * sin_lat));
IF H > REAL#180.0 THEN
    H := H - REAL#360.0;
END_IF;
HR := H + REFRACTION(H);
END_FUNCTION_BLOCK

FUNCTION_BLOCK SUN_TIME
VAR_INPUT
    LATITUDE : REAL;
    LONGITUDE : REAL;
    UTC : DATE;
    H : REAL := -0.83333333333;
END_VAR
VAR_OUTPUT
    MIDDAY : TOD;
    SUN_RISE : TOD;
    SUN_SET : TOD;
    SUN_DECLINATION : REAL;
END_VAR
VAR
    dk : REAL;
    delta : TIME;
    b : REAL;
END_VAR

b := LATITUDE * REAL#0.0174532925199433;
MIDDAY := SUN_MIDDAY(LONGITUDE, UTC);
dk := REAL#0.40954 * SIN(REAL#0.0172 * (INT_TO_REAL(DAY_OF_YEAR(UTC)) - REAL#79.35));
SUN_DECLINATION := DEG(RAD := dk);
IF SUN_DECLINATION > REAL#180.0 THEN
    SUN_DECLINATION := SUN_DECLINATION - REAL#360.0;
END_IF;
SUN_DECLINATION := REAL#90.0 - LATITUDE + SUN_DECLINATION;
delta := HOUR_TO_TIME(ACOS((SIN(RAD(DEG := H)) - SIN(b) * SIN(dk)) / (COS(b) * COS(dk))) * REAL#3.819718632);
SUN_RISE := ADD_TOD_TIME(MIDDAY, SUB_TIME(T#0s, delta));
SUN_SET := ADD_TOD_TIME(MIDDAY, delta);
END_FUNCTION_BLOCK

FUNCTION UTC_TO_LTIME : DT
VAR_INPUT
    UTC : DT;
    DST_ENABLE : BOOL;
    TIME_ZONE_OFFSET : INT;
END_VAR
VAR
    OffsetMinutes : INT;
    OffsetTime : TIME;
END_VAR

OffsetMinutes := TIME_ZONE_OFFSET + BOOL_TO_INT(DST_ENABLE AND DST(UTC)) * INT#60;
OffsetTime := MINUTE_TO_TIME(INT_TO_REAL(ABS(OffsetMinutes)));
IF OffsetMinutes < INT#0 THEN
    UTC_TO_LTIME := ADD_DT_TIME(UTC, SUB_TIME(T#0s, OffsetTime));
ELSE
    UTC_TO_LTIME := ADD_DT_TIME(UTC, OffsetTime);
END_IF;
END_FUNCTION

FUNCTION_BLOCK CALENDAR_CALC
VAR_INPUT
    SPE : BOOL;
    H : REAL := -0.83333333333;
END_VAR
VAR_IN_OUT
    XCAL : CALENDAR;
    HOLIDAYS : ARRAY[0..29] OF HOLIDAY_DATA;
END_VAR
VAR
    last : DT;
    last_day : DINT;
    holy : HOLIDAY;
    sun : SUN_TIME;
    last_hour : INT;
    pos : SUN_POS;
    plast : DT;
END_VAR
VAR_TEMP
    dtemp : DINT;
    tmp : INT;
    OffsetMinutes : INT;
    LocalOffsetTime : TIME;
END_VAR

IF XCAL.UTC <> last THEN
    last := XCAL.UTC;

    XCAL.LOCAL_DT := UTC_TO_LTIME(XCAL.UTC, XCAL.DST_EN, XCAL.OFFSET);
    XCAL.LOCAL_DATE := DT_TO_DATE(XCAL.LOCAL_DT);
    XCAL.LOCAL_TOD := DT_TO_TOD(XCAL.LOCAL_DT);
    dtemp := DAY_OF_DATE(XCAL.LOCAL_DATE);
    XCAL.NIGHT := XCAL.LOCAL_TOD < XCAL.SUN_RISE OR XCAL.LOCAL_TOD > XCAL.SUN_SET;

    tmp := HOUR(XCAL.LOCAL_TOD);
    IF tmp <> last_hour THEN
        XCAL.DST_ON := DST(XCAL.UTC) AND XCAL.DST_EN;
        last_hour := tmp;
    END_IF;

    IF dtemp <> last_day THEN
        last_day := dtemp;
        XCAL.YEAR := YEAR_OF_DATE(XCAL.LOCAL_DATE);
        XCAL.MONTH := MONTH_OF_DATE(XCAL.LOCAL_DATE);
        XCAL.DAY := DAY_OF_MONTH(XCAL.LOCAL_DATE);
        XCAL.WEEKDAY := DAY_OF_WEEK(XCAL.LOCAL_DATE);
        holy(DATE_IN := XCAL.LOCAL_DATE, LANGU := XCAL.LANGUAGE, HOLIDAYS := HOLIDAYS);
        XCAL.HOLIDAY := holy.Y;
        XCAL.HOLY_NAME := holy.NAME;
        sun(LATITUDE := XCAL.LATITUDE, LONGITUDE := XCAL.LONGITUDE, UTC := DT_TO_DATE(XCAL.UTC), H := H);
        OffsetMinutes := XCAL.OFFSET;
        IF XCAL.DST_ON THEN
            OffsetMinutes := OffsetMinutes + INT#60;
        END_IF;
        LocalOffsetTime := MINUTE_TO_TIME(INT_TO_REAL(ABS(OffsetMinutes)));
        IF OffsetMinutes < INT#0 THEN
            LocalOffsetTime := SUB_TIME(T#0s, LocalOffsetTime);
        ELSE
            LocalOffsetTime := LocalOffsetTime;
        END_IF;
        XCAL.SUN_RISE := ADD_TOD_TIME(sun.SUN_RISE, LocalOffsetTime);
        XCAL.SUN_SET := ADD_TOD_TIME(sun.SUN_SET, LocalOffsetTime);
        XCAL.SUN_MIDDAY := ADD_TOD_TIME(sun.MIDDAY, LocalOffsetTime);
        XCAL.SUN_HEIGTH := sun.SUN_DECLINATION;
        XCAL.WORK_WEEK := WORK_WEEK(XCAL.LOCAL_DATE);
    END_IF;

    IF SPE THEN
        plast := last;
        pos(LATITUDE := XCAL.LATITUDE, LONGITUDE := XCAL.LONGITUDE, UTC := XCAL.UTC);
        XCAL.SUN_HOR := pos.B;
        XCAL.SUN_VER := pos.HR;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK DCF77
VAR_INPUT
    REC : BOOL;
    SET_RTC : BOOL;
    SDT : DT;
    DSI : BOOL;
END_VAR
VAR_INPUT CONSTANT
    SYNC_TIMEOUT : TIME := T#2m;
    TIME_OFFSET : INT := 1;
    DST_EN : BOOL := TRUE;
END_VAR
VAR_OUTPUT
    TP : BOOL;
    DS : BOOL;
    WDAY : INT;
    ERROR : BOOL := TRUE;
    RTC : DT;
    RTC1 : DT;
    MSEC : INT;
    SYNC : BOOL;
END_VAR
VAR
    mez : DT;
    utc : DT;
    state : INT;
    edge : BOOL;
    tx : TIME;
    ty : TIME;
    last : TIME;
    bits : ARRAY[0..58] OF BOOL;
    cnt : INT;
    i : INT;
    old_time : DT;
    minute : INT;
    hour : INT;
    day : INT;
    month : INT;
    year : INT;
    last_sync : TIME;
    t1 : TIME;
    tz : TIME;
    zone_offset : TIME;
    init : BOOL;
END_VAR

TP := FALSE;

t1 := DWORD_TO_TIME(T_PLC_MS());
tx := SUB_TIME(t1, last);

IF REC XOR edge THEN
    edge := REC;
    IF NOT REC AND tx > T#1700ms AND tx < T#2000ms THEN
        state := INT#0;
        TP := NOT ERROR;
    ELSIF NOT REC AND tx > T#700ms AND tx < T#1000ms THEN
        IF state < INT#58 THEN
            state := state + 1;
        ELSE
            state := INT#0;
        END_IF;
    ELSIF REC AND tx < T#120ms THEN
        bits[state] := FALSE;
    ELSIF REC AND tx > T#120ms AND tx < T#250ms THEN
        bits[state] := TRUE;
    ELSE
        ERROR := TRUE;
        state := INT#0;
    END_IF;
    last := ADD_TIME(last, tx);
    IF REC AND state = INT#58 THEN
        ERROR := FALSE;

        IF bits[0] OR NOT (bits[17] XOR bits[18]) OR NOT bits[20] THEN
            ERROR := TRUE;
        END_IF;

        minute := BOOL_TO_INT(bits[21])
            + BOOL_TO_INT(bits[22]) * 2
            + BOOL_TO_INT(bits[23]) * 4
            + BOOL_TO_INT(bits[24]) * 8
            + BOOL_TO_INT(bits[25]) * 10
            + BOOL_TO_INT(bits[26]) * 20
            + BOOL_TO_INT(bits[27]) * 40;
        IF minute > INT#59
            OR (bits[21] XOR bits[22] XOR bits[23] XOR bits[24] XOR bits[25] XOR bits[26] XOR bits[27] XOR bits[28])
        THEN
            ERROR := TRUE;
        END_IF;

        hour := BOOL_TO_INT(bits[29])
            + BOOL_TO_INT(bits[30]) * 2
            + BOOL_TO_INT(bits[31]) * 4
            + BOOL_TO_INT(bits[32]) * 8
            + BOOL_TO_INT(bits[33]) * 10
            + BOOL_TO_INT(bits[34]) * 20;
        IF hour > INT#23
            OR (bits[29] XOR bits[30] XOR bits[31] XOR bits[32] XOR bits[33] XOR bits[34] XOR bits[35])
        THEN
            ERROR := TRUE;
        END_IF;

        day := BOOL_TO_INT(bits[36])
            + BOOL_TO_INT(bits[37]) * 2
            + BOOL_TO_INT(bits[38]) * 4
            + BOOL_TO_INT(bits[39]) * 8
            + BOOL_TO_INT(bits[40]) * 10
            + BOOL_TO_INT(bits[41]) * 20;
        IF day > INT#31 THEN
            ERROR := TRUE;
        END_IF;

        WDAY := BOOL_TO_INT(bits[42])
            + BOOL_TO_INT(bits[43]) * 2
            + BOOL_TO_INT(bits[44]) * 4;
        IF WDAY > INT#7 OR WDAY < INT#1 THEN
            ERROR := TRUE;
        END_IF;

        month := BOOL_TO_INT(bits[45])
            + BOOL_TO_INT(bits[46]) * 2
            + BOOL_TO_INT(bits[47]) * 4
            + BOOL_TO_INT(bits[48]) * 8
            + BOOL_TO_INT(bits[49]) * 10;
        IF month > INT#12 THEN
            ERROR := TRUE;
        END_IF;

        year := BOOL_TO_INT(bits[50])
            + BOOL_TO_INT(bits[51]) * 2
            + BOOL_TO_INT(bits[52]) * 4
            + BOOL_TO_INT(bits[53]) * 8
            + BOOL_TO_INT(bits[54]) * 10
            + BOOL_TO_INT(bits[55]) * 20
            + BOOL_TO_INT(bits[56]) * 40
            + BOOL_TO_INT(bits[57]) * 80;

        cnt := INT#0;
        FOR i := 36 TO 58 DO
            IF bits[i] THEN
                cnt := cnt + 1;
            END_IF;
        END_FOR;
        IF NOT EVEN(IN := cnt) THEN
            ERROR := TRUE;
        END_IF;

        IF NOT ERROR THEN
            old_time := mez;
            IF year >= INT#70 THEN
                year := year + 1900;
            ELSE
                year := year + 2000;
            END_IF;
            mez := SET_DT(year, month, day, hour, minute, INT#0);
            DS := bits[17];
            IF DS THEN
                utc := ADD_DT_TIME(mez, SUB_TIME(T#0s, T#2h));
            ELSE
                utc := ADD_DT_TIME(mez, SUB_TIME(T#0s, T#1h));
            END_IF;
            IF mez <> ADD_DT_TIME(old_time, T#1m) THEN
                ERROR := TRUE;
            END_IF;
        END_IF;
    END_IF;
END_IF;

tz := HOUR_TO_TIME(INT_TO_REAL(ABS(TIME_OFFSET)));
IF TIME_OFFSET < INT#0 THEN
    zone_offset := SUB_TIME(T#0s, tz);
ELSE
    zone_offset := tz;
END_IF;
IF DS AND DST_EN THEN
    zone_offset := ADD_TIME(zone_offset, T#1h);
END_IF;

IF NOT init OR SET_RTC THEN
    init := TRUE;
    utc := SDT;
    TP := TRUE;
    DS := DSI;
END_IF;

IF TP THEN
    RTC := utc;
    RTC1 := ADD_DT_TIME(RTC, zone_offset);
    SYNC := TRUE;
    last_sync := last;
    ty := last;
ELSIF init AND SUB_TIME(t1, ty) >= T#1s THEN
    RTC := ADD_DT_TIME(RTC, T#1s);
    RTC1 := ADD_DT_TIME(RTC1, T#1s);
    ty := ADD_TIME(ty, T#1s);
    SYNC := SUB_TIME(ty, last_sync) < SYNC_TIMEOUT AND last_sync > T#0s;
    WDAY := DAY_OF_WEEK(DT_TO_DATE(RTC1));
    DS := DST_EN AND DST(utc);
END_IF;
MSEC := DWORD_TO_INT(TIME_TO_DWORD(SUB_TIME(t1, ty)));
END_FUNCTION_BLOCK
