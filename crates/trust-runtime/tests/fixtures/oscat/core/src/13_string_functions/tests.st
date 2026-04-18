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

TEST_PROGRAM oscat_string_comparison_regression_behaves
ASSERT_TRUE(IN := 'ABC' = 'ABC');
ASSERT_TRUE(IN := 'ABC' < 'ABD');
ASSERT_TRUE(IN := "ABC" = "ABC");
ASSERT_TRUE(IN := "ABC" < "ABD");
END_TEST_PROGRAM

TEST_PROGRAM oscat_string_case_regression_behaves
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

TEST_PROGRAM oscat_nested_field_index_regression_behaves
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

TEST_PROGRAM oscat_month_and_weekday_strings_behave
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

TEST_PROGRAM oscat_dt_to_strf_behaves
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

TEST_PROGRAM oscat_string_formatting_helpers_behave
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
RealFormatted := REAL_TO_STRF(IN := REAL#12.346, N := INT#2, DECIMAL := '.');
DWordFormatted := DWORD_TO_STRF(IN := DWORD#123, N := INT#4);

ASSERT_EQUAL(EXPECTED := 'A', ACTUAL := SingleChar);
ASSERT_EQUAL(EXPECTED := '----', ACTUAL := Filled);
ASSERT_EQUAL(EXPECTED := '4200', ACTUAL := FixedLeft);
ASSERT_EQUAL(EXPECTED := '0042', ACTUAL := FixedRight);
ASSERT_EQUAL(EXPECTED := '12.35', ACTUAL := RealFormatted);
ASSERT_EQUAL(EXPECTED := '0123', ACTUAL := DWordFormatted);
END_TEST_PROGRAM

TEST_PROGRAM oscat_string_trim_helpers_behave
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

TEST_PROGRAM oscat_string_case_conversion_helpers_behave
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

TEST_PROGRAM oscat_string_classifier_helpers_behave
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

TEST_PROGRAM oscat_string_search_helpers_behave
VAR
    ControlPrefix : STRING[8];
    ControlMiddle : STRING[8];
    ExtendedAscii : STRING[2];
    FirstCharPos : INT;
    FirstCtrlPos : INT;
    FirstNonNumPos : INT;
    FirstNumPos : INT;
    LastFindPos : INT;
    LastNonNumPos : INT;
    LastNumPos : INT;
    FindFromPos : INT;
    ExtendedCharPos : INT;
END_VAR

ControlPrefix := CONCAT(IN1 := CHR_TO_STRING(C := BYTE#9), IN2 := 'A');
ControlMiddle := CONCAT(
    IN1 := 'AB',
    IN2 := CONCAT(IN1 := CHR_TO_STRING(C := BYTE#10), IN2 := 'C')
);
ExtendedAscii := CHR_TO_STRING(C := BYTE#196);

FirstCharPos := FIND_CHAR(STR := ControlPrefix, POS := INT#1);
FirstCtrlPos := FIND_CTRL(STR := ControlMiddle, POS := INT#1);
FirstNonNumPos := FIND_NONUM(STR := '12.3A', POS := INT#1);
FirstNumPos := FIND_NUM(STR := 'AB.5', POS := INT#1);
LastFindPos := FINDB(STR1 := 'abcabc', STR2 := 'ab');
LastNonNumPos := FINDB_NONUM(STR := '12.3A');
LastNumPos := FINDB_NUM(STR := 'A12.3B');
FindFromPos := FINDP(STR := 'abcabc', SRC := 'bc', POS := INT#3);
ExtendedCharPos := FIND_CHAR(STR := ExtendedAscii, POS := INT#1);

ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := FirstCharPos);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := FirstCtrlPos);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := FirstNonNumPos);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := FirstNumPos);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := LastFindPos);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := LastNonNumPos);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := LastNumPos);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := FindFromPos);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := ExtendedCharPos);
END_TEST_PROGRAM

TEST_PROGRAM oscat_string_cleanup_and_count_helpers_behave
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

TEST_PROGRAM oscat_string_code_delete_and_umlaut_helpers_behave
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

TEST_PROGRAM oscat_string_decimal_decoders_behave
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

TEST_PROGRAM oscat_string_bit_and_hex_formatters_behave
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

TEST_PROGRAM oscat_string_binary_hex_and_octal_decoders_behave
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

TEST_PROGRAM oscat_string_mirror_and_replacement_helpers_behave
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

TEST_PROGRAM oscat_string_formatted_value_parsers_behave
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

TEST_PROGRAM oscat_string_exec_and_message_helpers_behave
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

TEST_PROGRAM oscat_string_html_name_helpers_behave
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

TEST_PROGRAM oscat_string_ticker_fb_scrolls_text
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
