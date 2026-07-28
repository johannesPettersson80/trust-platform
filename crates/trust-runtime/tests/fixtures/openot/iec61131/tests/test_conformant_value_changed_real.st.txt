FUNCTION_BLOCK OPENOT_TestConformantValueChangedReal
VAR_OUTPUT
    Passed : BOOL;
    MismatchIndex : UINT;
    ActualLength : UINT;
    ExpectedLength : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeValueChangedReal;
    Expected : ARRAY[0..59] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#3C, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#14, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#03, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#02, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#0D, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#D1, BYTE#16#07, BYTE#16#00, BYTE#16#00,
        BYTE#16#10, BYTE#16#00, BYTE#16#09, BYTE#16#04, BYTE#16#00, BYTE#16#00, BYTE#16#48, BYTE#16#41,
        BYTE#16#2A, BYTE#16#65, BYTE#16#5B, BYTE#16#70
    ];
    Index : UINT;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#3,
    SourceTime := ULINT#1000000020,
    ValueId := UDINT#2001,
    NewValue := REAL#12.5,
    HasPrevious := FALSE,
    PreviousValue := REAL#0.0,
    HasQuality := FALSE,
    Quality := UINT#0);
ActualLength := Encoder.Length;
ExpectedLength := UINT#60;
Passed := ActualLength = ExpectedLength;
MismatchIndex := UINT#65535;

IF Passed THEN
    FOR Index := UINT#0 TO ExpectedLength - UINT#1 DO
        IF (Encoder.Buffer[Index] <> Expected[Index]) AND (MismatchIndex = UINT#65535) THEN
            Passed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION_BLOCK
