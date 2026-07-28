FUNCTION_BLOCK OPENOT_TestConformantValueChangedInt
VAR_OUTPUT
    Passed : BOOL;
    MismatchIndex : UINT;
    ActualLength : UINT;
    ExpectedLength : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeValueChangedInt;
    Expected : ARRAY[0..75] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#4C, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#1E, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#04, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#02, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#0D, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#D2, BYTE#16#07, BYTE#16#00, BYTE#16#00,
        BYTE#16#10, BYTE#16#00, BYTE#16#06, BYTE#16#04, BYTE#16#2A, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#0F, BYTE#16#00, BYTE#16#06, BYTE#16#04, BYTE#16#28, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#11, BYTE#16#00, BYTE#16#03, BYTE#16#02, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#BC, BYTE#16#5C, BYTE#16#E1, BYTE#16#04
    ];
    Index : UINT;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#4,
    SourceTime := ULINT#1000000030,
    ValueId := UDINT#2002,
    NewValue := DINT#42,
    HasPrevious := TRUE,
    PreviousValue := DINT#40,
    HasQuality := TRUE,
    Quality := UINT#0);
ActualLength := Encoder.Length;
ExpectedLength := UINT#76;
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
