PROGRAM Main
VAR
    ConstantsReady : BOOL;
    Celsius : REAL := 25.0;
    Kelvin : REAL;
    Fahrenheit : REAL;
    SpeedMps : REAL;
    SpeedKmh : REAL;
    AngularFrequency : REAL;
    FrequencyHz : REAL;
    CyclePeriod : TIME;
    CycleFrequency : REAL;
    Degrees : REAL;
    DirectionLabel : STRING[3];
    DirectionDegrees : INT;
    DefaultEast : STRING[3];
    Beaufort : INT;
    DaySpan : TIME;
    HourSpan : TIME;
    MinuteSpan : TIME;
    SecondSpan : TIME;
    PlcMs : DWORD;
    PlcUs : DWORD;
    SampleDate : DATE := DATE#2024-02-29;
    DaysSinceEpoch : DINT;
    DayOfYearValue : INT;
    MonthStart : DATE;
    MonthEndDate : DATE;
    YearEndDate : DATE;
    ShiftedDate : DATE;
    WeekNumber : INT;
    LeapDayActive : BOOL;
    Easter2026 : DATE;
    SampleDateTime : DT := DT#2026-04-15-13:14:15;
    HourPart : INT;
    MinutePart : INT;
    SecondPart : INT;
    HourOfTod : INT;
    MinuteOfTod : INT;
    SecondOfTod : REAL;
    NoonThirty : TOD;
    BuiltDateTime : DT;
    MonthName : STRING[10];
    WeekdayName : STRING[10];
    FormattedStamp : STRING[32];
    FixedCounter : STRING[8];
    CapitalizedLabel : STRING[32];
    CleanedText : STRING[16];
    DeletedDelimiters : STRING[16];
    TrimmedLabel : STRING[20];
    UpperLabel : STRING[20];
    AlphaOnly : BOOL;
    SearchFromPos : INT;
    CharacterCode : BYTE;
    UmlautAscii : STRING[2];
    ByteBits : STRING[8];
    ByteHex : STRING[2];
    DWordBits : STRING[32];
    DWordHex : STRING[8];
    MirroredLabel : STRING[16];
    ReplacedAllLabel : STRING[16];
    ReplacedCharsLabel : STRING[16];
    ReplacedUmlautLabel : STRING[16];
    HtmlEuroCode : BYTE;
    HtmlAmpName : STRING[10];
    ParsedFloat : REAL;
    ParsedByteFromFormat : BYTE;
    ParsedDWordFromFormat : DWORD;
    ParsedStamp : DT;
    ParsedMonthIndex : INT;
    ParsedWeekMask : BYTE;
    ParsedWeekdayIndex : INT;
    ExecTextResult : STRING[20];
    ParsedByte : BYTE;
    ParsedDWord : DWORD;
    ParsedInt : INT;
    ParsedBinaryByte : BYTE;
    ParsedBinaryDWord : DWORD;
    ParsedHexByte : BYTE;
    ParsedHexDWord : DWORD;
    ParsedOctalByte : BYTE;
    ParsedOctalDWord : DWORD;
    MessageCycle : MESSAGE_4R;
    PriorityMessage : MESSAGE_8;
    RotatingMessage : STRING[STRING_LENGTH];
    RotatingIndex : INT;
    ActiveMessage : STRING[STRING_LENGTH];
    TickerText : STRING[STRING_LENGTH] := 'HELLO';
    TickerFb : TICKER;
    TickerWindow : STRING[STRING_LENGTH];
    LogicLatch : LTCH;
    LogicCounter : COUNT_BR;
    LogicToggle : TOGGLE;
    LogicLatched : BOOL;
    LogicCount : BYTE;
    LogicState : BOOL;
    EnergyConv : ENERGY;
    SpeedConv : SPEED;
    TempConv : TEMPERATURE;
    Joules : REAL;
    MilesPerHour : REAL;
    TempFahrenheit : REAL;
END_VAR

ConstantsReady := OSCAT_BASIC_Constants();
Kelvin := C_TO_K(Celsius := Celsius);
Fahrenheit := C_TO_F(celsius := Celsius);
SpeedMps := KMH_TO_MS(kmh := 36.0);
SpeedKmh := MS_TO_KMH(ms := SpeedMps);
AngularFrequency := F_TO_OM(F := 2.0);
FrequencyHz := OM_TO_F(OM := AngularFrequency);
CyclePeriod := F_TO_PT(F := 2.0);
CycleFrequency := PT_TO_F(PT := CyclePeriod);
Degrees := GEO_TO_DEG(DEG := 59, M := 19, SEC := 30.0);
DirectionLabel := DEG_TO_DIR(DEG := 90, N := 1, L := 1);
DirectionDegrees := DIR_TO_DEG(DIR := DirectionLabel, L := 1);
DefaultEast := LANGUAGE.DIRS[LANGUAGE.DEFAULT, 4];
Beaufort := MS_TO_BFT(MS := SpeedMps);
DaySpan := DAY_TO_TIME(IN := 1.5);
HourSpan := HOUR_TO_TIME(IN := 1.25);
MinuteSpan := MINUTE_TO_TIME(IN := 2.5);
SecondSpan := SECOND_TO_TIME(IN := 3.75);
PlcMs := T_PLC_MS();
PlcUs := T_PLC_US();
DaysSinceEpoch := DAY_OF_DATE(IDATE := SampleDate);
DayOfYearValue := DAY_OF_YEAR(IDATE := SampleDate);
MonthStart := MONTH_BEGIN(IDATE := SampleDate);
MonthEndDate := MONTH_END(IDATE := SampleDate);
YearEndDate := YEAR_END(y := YEAR_OF_DATE(IDATE := SampleDate));
ShiftedDate := DATE_ADD(IDATE := SampleDate, DAYS := 1, W := 1, M := 0, Y := 0);
WeekNumber := WORK_WEEK(idate := DATE#2026-04-15);
LeapDayActive := LEAP_DAY(IDATE := SampleDate);
Easter2026 := EASTER(year := 2026);
HourPart := HOUR_OF_DT(XDT := SampleDateTime);
MinutePart := MINUTE_OF_DT(XDT := SampleDateTime);
SecondPart := SECOND_OF_DT(XDT := SampleDateTime);
HourOfTod := HOUR(ITOD := TOD#13:14:15.250);
MinuteOfTod := MINUTE(ITOD := TOD#13:14:15.250);
SecondOfTod := SECOND(ITOD := TOD#13:14:15.250);
NoonThirty := HOUR_TO_TOD(IN := 12.5);
BuiltDateTime := SET_DT(
    YEAR := 2026,
    MONTH := 4,
    DAY := 15,
    HOUR := 13,
    MINUTE := 14,
    SECOND := 15
);
MonthName := MONTH_TO_STRING(MTH := 4, LANG := 1, LX := 0);
WeekdayName := WEEKDAY_TO_STRING(WDAY := 1, LANG := 1, LX := 0);
FormattedStamp := DT_TO_STRF(
    DTI := SampleDateTime,
    MS := 0,
    FMT := '#A-#D-#H #N:#R',
    LANG := 1
);
FixedCounter := DWORD_TO_STRF(IN := DWORD#123, N := 4);
CapitalizedLabel := CAPITALIZE(STR := 'oscat demo');
CleanedText := CLEAN(
    IN := CONCAT(IN1 := 'A', IN2 := CONCAT(IN1 := CHR_TO_STRING(C := BYTE#9), IN2 := 'B1')),
    CX := 'AB1'
);
DeletedDelimiters := DEL_CHARS(IN := 'A B-C', CX := ' -');
TrimmedLabel := TRIM1(STR := '  OSCAT   BASIC  ');
UpperLabel := UPPERCASE(STR := 'oscat');
AlphaOnly := IS_ALPHA(STR := 'OSCAT');
SearchFromPos := FINDP(STR := 'abcabc', SRC := 'bc', POS := 3);
CharacterCode := CODE(STR := 'AB', POS := 2);
UmlautAscii := TO_UML(IN := BYTE#196);
ByteBits := BYTE_TO_STRB(IN := BYTE#165);
ByteHex := BYTE_TO_STRH(IN := BYTE#165);
DWordBits := DWORD_TO_STRB(IN := DWORD#16#1234ABCD);
DWordHex := DWORD_TO_STRH(IN := DWORD#16#1234ABCD);
MirroredLabel := MIRROR(STR := 'AB 12');
ReplacedAllLabel := REPLACE_ALL(STR := 'abcabc', SRC := 'ab', REP := 'Z');
ReplacedCharsLabel := REPLACE_CHARS(STR := 'A B-C', SRC := ' -', REP := '_+');
ReplacedUmlautLabel := REPLACE_UML(STR := 'Ae/ss');
HtmlEuroCode := CHARCODE(STR := 'euro');
HtmlAmpName := CHARNAME(C := BYTE#38);
ParsedFloat := FLOAT_TO_REAL(FLT := '-12,5e1');
ParsedByteFromFormat := FSTRING_TO_BYTE(IN := '16#A5');
ParsedDWordFromFormat := FSTRING_TO_DWORD(IN := '2#10100101');
ParsedStamp := FSTRING_TO_DT(SDT := '2026-04-15 13:14:15', FMT := '#Y-#M-#D #h:#m:#s');
ParsedMonthIndex := FSTRING_TO_MONTH(MTH := 'apRil', LANG := 1);
ParsedWeekMask := FSTRING_TO_WEEK(WEEK := 'Mo,We,Su', LANG := 1);
ParsedWeekdayIndex := FSTRING_TO_WEEKDAY(WDAY := 'mo', LANG := 1);
ExecTextResult := EXEC(STR := '3+4');
ParsedByte := DEC_TO_BYTE(DEC := '2x5');
ParsedDWord := DEC_TO_DWORD(DEC := '65_535');
ParsedInt := DEC_TO_INT(DEC := 'A-42');
ParsedBinaryByte := BIN_TO_BYTE(BIN := '10-10_0101');
ParsedBinaryDWord := BIN_TO_DWORD(BIN := '0001 0010 0011 0100 1010 1011 1100 1101');
ParsedHexByte := HEX_TO_BYTE(HEX := 'A-5');
ParsedHexDWord := HEX_TO_DWORD(HEX := '12 34-abcd');
ParsedOctalByte := OCT_TO_BYTE(OCT := '2_4_7');
ParsedOctalDWord := OCT_TO_DWORD(OCT := '1234_5670');
MessageCycle(
    M0 := 'zero',
    M1 := 'one',
    M2 := 'two',
    M3 := 'three',
    MM := 3,
    ENQ := TRUE,
    CLK := TRUE,
    T1 := T#1s
);
PriorityMessage(
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
RotatingMessage := MessageCycle.MX;
RotatingIndex := MessageCycle.MN;
ActiveMessage := PriorityMessage.M;
TickerFb(N := 3, PT := T#0ms, Text := TickerText);
TickerWindow := TickerFb.Display;
LogicLatch(DATA := TRUE, L := TRUE, RST := FALSE);
LogicLatched := LogicLatch.Q;
LogicCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#1, MX := BYTE#7, RST := FALSE);
LogicCount := LogicCounter.CNT;
LogicToggle(CLK := TRUE, RST := FALSE);
LogicState := LogicToggle.Q;

EnergyConv(Wh := 1.0);
SpeedConv(ms := SpeedMps);
TempConv(C := Celsius);

Joules := EnergyConv.YJ;
MilesPerHour := SpeedConv.Ymh;
TempFahrenheit := TempConv.YF;
END_PROGRAM
