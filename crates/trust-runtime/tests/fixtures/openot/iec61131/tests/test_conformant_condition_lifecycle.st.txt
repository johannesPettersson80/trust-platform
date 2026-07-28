FUNCTION_BLOCK OPENOT_TestConformantConditionLifecycle
VAR_OUTPUT
    Passed : BOOL;
    AckPassed : BOOL;
    ConfirmedPassed : BOOL;
    ShelvedPassed : BOOL;
    UnshelvedPassed : BOOL;
    SuppressedPassed : BOOL;
    UnsuppressedPassed : BOOL;
    OutOfServicePassed : BOOL;
    InServicePassed : BOOL;
    ResetPassed : BOOL;
    CommentedPassed : BOOL;
    PriorityChangedPassed : BOOL;
    MismatchIndex : UINT;
END_VAR
VAR
    AckEncoder : OPENOT_EncodeConformantConditionAcknowledged;
    ConfirmedEncoder : OPENOT_EncodeConformantConditionConfirmed;
    ShelvedEncoder : OPENOT_EncodeConformantConditionShelved;
    UnshelvedEncoder : OPENOT_EncodeConformantConditionUnshelved;
    SuppressedEncoder : OPENOT_EncodeConformantConditionSuppressed;
    UnsuppressedEncoder : OPENOT_EncodeConformantConditionUnsuppressed;
    OutOfServiceEncoder : OPENOT_EncodeConformantConditionOutOfService;
    InServiceEncoder : OPENOT_EncodeConformantConditionInService;
    ResetEncoder : OPENOT_EncodeConformantConditionReset;
    CommentedEncoder : OPENOT_EncodeConformantConditionCommented;
    PriorityChangedEncoder : OPENOT_EncodeConformantConditionPriorityChanged;
    ExpectedAck : ARRAY[0..75] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#4C, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#A0, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#11, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#02, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#07, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#4D, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#1D, BYTE#16#00, BYTE#16#0C, BYTE#16#0A, BYTE#16#6F, BYTE#16#70, BYTE#16#65, BYTE#16#72,
        BYTE#16#61, BYTE#16#74, BYTE#16#6F, BYTE#16#72, BYTE#16#2D, BYTE#16#61, BYTE#16#00, BYTE#16#00,
        BYTE#16#FA, BYTE#16#EA, BYTE#16#82, BYTE#16#EC
    ];
    ExpectedConfirmed : ARRAY[0..75] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#4C, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#C8, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#15, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#03, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#07, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#4D, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#1D, BYTE#16#00, BYTE#16#0C, BYTE#16#0A, BYTE#16#6F, BYTE#16#70, BYTE#16#65, BYTE#16#72,
        BYTE#16#61, BYTE#16#74, BYTE#16#6F, BYTE#16#72, BYTE#16#2D, BYTE#16#61, BYTE#16#00, BYTE#16#00,
        BYTE#16#96, BYTE#16#12, BYTE#16#3C, BYTE#16#D3
    ];
    ExpectedShelved : ARRAY[0..83] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#54, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#AA, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#12, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#04, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#07, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#4D, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#1D, BYTE#16#00, BYTE#16#0C, BYTE#16#0A, BYTE#16#6F, BYTE#16#70, BYTE#16#65, BYTE#16#72,
        BYTE#16#61, BYTE#16#74, BYTE#16#6F, BYTE#16#72, BYTE#16#2D, BYTE#16#61, BYTE#16#00, BYTE#16#00,
        BYTE#16#1E, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#2C, BYTE#16#01, BYTE#16#00, BYTE#16#00,
        BYTE#16#E5, BYTE#16#3E, BYTE#16#92, BYTE#16#BD
    ];
    ExpectedUnshelved : ARRAY[0..59] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#3C, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#D2, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#16, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#05, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#07, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#4D, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#11, BYTE#16#63, BYTE#16#BE, BYTE#16#0A
    ];
    ExpectedSuppressed : ARRAY[0..67] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#44, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#B4, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#13, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#06, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#1F, BYTE#16#00, BYTE#16#0C, BYTE#16#0B, BYTE#16#6D, BYTE#16#61, BYTE#16#69, BYTE#16#6E,
        BYTE#16#74, BYTE#16#65, BYTE#16#6E, BYTE#16#61, BYTE#16#6E, BYTE#16#63, BYTE#16#65, BYTE#16#00,
        BYTE#16#17, BYTE#16#3E, BYTE#16#4F, BYTE#16#63
    ];
    ExpectedUnsuppressed : ARRAY[0..51] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#34, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#DC, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#17, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#07, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#B9, BYTE#16#40, BYTE#16#40, BYTE#16#76
    ];
    ExpectedOutOfService : ARRAY[0..67] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#44, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#BE, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#14, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#08, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#1D, BYTE#16#00, BYTE#16#0C, BYTE#16#0A, BYTE#16#6F, BYTE#16#70, BYTE#16#65, BYTE#16#72,
        BYTE#16#61, BYTE#16#74, BYTE#16#6F, BYTE#16#72, BYTE#16#2D, BYTE#16#61, BYTE#16#00, BYTE#16#00,
        BYTE#16#9D, BYTE#16#5C, BYTE#16#A4, BYTE#16#01
    ];
    ExpectedInService : ARRAY[0..51] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#34, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#E6, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#18, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#09, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#AB, BYTE#16#6D, BYTE#16#56, BYTE#16#DC
    ];
    ExpectedReset : ARRAY[0..75] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#4C, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#F0, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#19, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#0B, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#07, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#4D, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#1D, BYTE#16#00, BYTE#16#0C, BYTE#16#0A, BYTE#16#6F, BYTE#16#70, BYTE#16#65, BYTE#16#72,
        BYTE#16#61, BYTE#16#74, BYTE#16#6F, BYTE#16#72, BYTE#16#2D, BYTE#16#61, BYTE#16#00, BYTE#16#00,
        BYTE#16#AD, BYTE#16#17, BYTE#16#A6, BYTE#16#9C
    ];
    ExpectedCommented : ARRAY[0..95] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#60, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#FA, BYTE#16#CA, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#1A, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#0A, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#07, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#4D, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#37, BYTE#16#00, BYTE#16#0C, BYTE#16#10, BYTE#16#6F, BYTE#16#70, BYTE#16#65, BYTE#16#72,
        BYTE#16#61, BYTE#16#74, BYTE#16#6F, BYTE#16#72, BYTE#16#20, BYTE#16#63, BYTE#16#6F, BYTE#16#6D,
        BYTE#16#6D, BYTE#16#65, BYTE#16#6E, BYTE#16#74, BYTE#16#1D, BYTE#16#00, BYTE#16#0C, BYTE#16#0A,
        BYTE#16#6F, BYTE#16#70, BYTE#16#65, BYTE#16#72, BYTE#16#61, BYTE#16#74, BYTE#16#6F, BYTE#16#72,
        BYTE#16#2D, BYTE#16#61, BYTE#16#00, BYTE#16#00, BYTE#16#6B, BYTE#16#C5, BYTE#16#FE, BYTE#16#29
    ];
    ExpectedPriorityChanged : ARRAY[0..83] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#54, BYTE#16#00, BYTE#16#08, BYTE#16#00,
        BYTE#16#04, BYTE#16#CB, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#1B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#0C, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#05, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#29, BYTE#16#23, BYTE#16#00, BYTE#16#00,
        BYTE#16#2E, BYTE#16#00, BYTE#16#03, BYTE#16#02, BYTE#16#58, BYTE#16#02, BYTE#16#00, BYTE#16#00,
        BYTE#16#2C, BYTE#16#00, BYTE#16#03, BYTE#16#02, BYTE#16#84, BYTE#16#03, BYTE#16#00, BYTE#16#00,
        BYTE#16#1D, BYTE#16#00, BYTE#16#0C, BYTE#16#0A, BYTE#16#6F, BYTE#16#70, BYTE#16#65, BYTE#16#72,
        BYTE#16#61, BYTE#16#74, BYTE#16#6F, BYTE#16#72, BYTE#16#2D, BYTE#16#61, BYTE#16#00, BYTE#16#00,
        BYTE#16#CB, BYTE#16#3E, BYTE#16#BF, BYTE#16#7F
    ];
    Index : UINT;
END_VAR

AckEncoder();
ConfirmedEncoder();
ShelvedEncoder();
UnshelvedEncoder();
SuppressedEncoder();
UnsuppressedEncoder();
OutOfServiceEncoder();
InServiceEncoder();
ResetEncoder();
CommentedEncoder();
PriorityChangedEncoder();

AckPassed := AckEncoder.Length = UINT#76;
ConfirmedPassed := ConfirmedEncoder.Length = UINT#76;
ShelvedPassed := ShelvedEncoder.Length = UINT#84;
UnshelvedPassed := UnshelvedEncoder.Length = UINT#60;
SuppressedPassed := SuppressedEncoder.Length = UINT#68;
UnsuppressedPassed := UnsuppressedEncoder.Length = UINT#52;
OutOfServicePassed := OutOfServiceEncoder.Length = UINT#68;
InServicePassed := InServiceEncoder.Length = UINT#52;
ResetPassed := ResetEncoder.Length = UINT#76;
CommentedPassed := CommentedEncoder.Length = UINT#96;
PriorityChangedPassed := PriorityChangedEncoder.Length = UINT#84;
MismatchIndex := UINT#65535;

IF AckPassed THEN
    FOR Index := UINT#0 TO UINT#75 DO
        IF (AckEncoder.Buffer[Index] <> ExpectedAck[Index]) AND (MismatchIndex = UINT#65535) THEN
            AckPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF ConfirmedPassed THEN
    FOR Index := UINT#0 TO UINT#75 DO
        IF (ConfirmedEncoder.Buffer[Index] <> ExpectedConfirmed[Index]) AND (MismatchIndex = UINT#65535) THEN
            ConfirmedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF ShelvedPassed THEN
    FOR Index := UINT#0 TO UINT#83 DO
        IF (ShelvedEncoder.Buffer[Index] <> ExpectedShelved[Index]) AND (MismatchIndex = UINT#65535) THEN
            ShelvedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF UnshelvedPassed THEN
    FOR Index := UINT#0 TO UINT#59 DO
        IF (UnshelvedEncoder.Buffer[Index] <> ExpectedUnshelved[Index]) AND (MismatchIndex = UINT#65535) THEN
            UnshelvedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF SuppressedPassed THEN
    FOR Index := UINT#0 TO UINT#67 DO
        IF (SuppressedEncoder.Buffer[Index] <> ExpectedSuppressed[Index]) AND (MismatchIndex = UINT#65535) THEN
            SuppressedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF UnsuppressedPassed THEN
    FOR Index := UINT#0 TO UINT#51 DO
        IF (UnsuppressedEncoder.Buffer[Index] <> ExpectedUnsuppressed[Index]) AND (MismatchIndex = UINT#65535) THEN
            UnsuppressedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF OutOfServicePassed THEN
    FOR Index := UINT#0 TO UINT#67 DO
        IF (OutOfServiceEncoder.Buffer[Index] <> ExpectedOutOfService[Index]) AND (MismatchIndex = UINT#65535) THEN
            OutOfServicePassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF InServicePassed THEN
    FOR Index := UINT#0 TO UINT#51 DO
        IF (InServiceEncoder.Buffer[Index] <> ExpectedInService[Index]) AND (MismatchIndex = UINT#65535) THEN
            InServicePassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF ResetPassed THEN
    FOR Index := UINT#0 TO UINT#75 DO
        IF (ResetEncoder.Buffer[Index] <> ExpectedReset[Index]) AND (MismatchIndex = UINT#65535) THEN
            ResetPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF CommentedPassed THEN
    FOR Index := UINT#0 TO UINT#95 DO
        IF (CommentedEncoder.Buffer[Index] <> ExpectedCommented[Index]) AND (MismatchIndex = UINT#65535) THEN
            CommentedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF PriorityChangedPassed THEN
    FOR Index := UINT#0 TO UINT#83 DO
        IF (PriorityChangedEncoder.Buffer[Index] <> ExpectedPriorityChanged[Index]) AND (MismatchIndex = UINT#65535) THEN
            PriorityChangedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

Passed := AckPassed AND ConfirmedPassed AND ShelvedPassed AND UnshelvedPassed AND SuppressedPassed AND UnsuppressedPassed AND OutOfServicePassed AND InServicePassed AND ResetPassed AND CommentedPassed AND PriorityChangedPassed;
END_FUNCTION_BLOCK
