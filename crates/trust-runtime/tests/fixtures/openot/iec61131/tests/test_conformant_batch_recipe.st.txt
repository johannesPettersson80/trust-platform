FUNCTION_BLOCK OPENOT_TestConformantBatchRecipe
VAR_OUTPUT
    Passed : BOOL;
    RecipeLoadedPassed : BOOL;
    RecipeApprovedPassed : BOOL;
    BatchEventPassed : BOOL;
    MaterialAdditionPassed : BOOL;
    MismatchIndex : UINT;
END_VAR
VAR
    RecipeLoadedEncoder : OPENOT_EncodeConformantRecipeLoaded;
    RecipeApprovedEncoder : OPENOT_EncodeConformantRecipeApproved;
    BatchEventEncoder : OPENOT_EncodeConformantBatchEvent;
    MaterialAdditionEncoder : OPENOT_EncodeConformantMaterialAddition;
    ExpectedRecipeLoaded : ARRAY[0..71] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#48, BYTE#16#00, BYTE#16#08, BYTE#16#00, BYTE#16#0E, BYTE#16#CB, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#1C, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#01, BYTE#16#03, BYTE#16#00, BYTE#16#00, BYTE#16#23, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#B9, BYTE#16#0B, BYTE#16#00, BYTE#16#00,
        BYTE#16#24, BYTE#16#00, BYTE#16#0C, BYTE#16#06, BYTE#16#76, BYTE#16#31, BYTE#16#2E, BYTE#16#32, BYTE#16#2E, BYTE#16#33, BYTE#16#00, BYTE#16#00, BYTE#16#25, BYTE#16#00, BYTE#16#05, BYTE#16#04,
        BYTE#16#A1, BYTE#16#0F, BYTE#16#00, BYTE#16#00, BYTE#16#D3, BYTE#16#FA, BYTE#16#CF, BYTE#16#78
    ];
    ExpectedRecipeApproved : ARRAY[0..87] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#58, BYTE#16#00, BYTE#16#08, BYTE#16#00, BYTE#16#18, BYTE#16#CB, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#1D, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#02, BYTE#16#03, BYTE#16#00, BYTE#16#00, BYTE#16#23, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#B9, BYTE#16#0B, BYTE#16#00, BYTE#16#00,
        BYTE#16#24, BYTE#16#00, BYTE#16#0C, BYTE#16#06, BYTE#16#76, BYTE#16#31, BYTE#16#2E, BYTE#16#32, BYTE#16#2E, BYTE#16#33, BYTE#16#00, BYTE#16#00, BYTE#16#20, BYTE#16#00, BYTE#16#03, BYTE#16#02,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#1D, BYTE#16#00, BYTE#16#0C, BYTE#16#0A, BYTE#16#61, BYTE#16#70, BYTE#16#70, BYTE#16#72, BYTE#16#6F, BYTE#16#76, BYTE#16#65, BYTE#16#72,
        BYTE#16#2D, BYTE#16#61, BYTE#16#00, BYTE#16#00, BYTE#16#01, BYTE#16#12, BYTE#16#BE, BYTE#16#41
    ];
    ExpectedBatchEvent : ARRAY[0..67] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#44, BYTE#16#00, BYTE#16#08, BYTE#16#00, BYTE#16#22, BYTE#16#CB, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#1E, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#03, BYTE#16#03, BYTE#16#00, BYTE#16#00, BYTE#16#25, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#A1, BYTE#16#0F, BYTE#16#00, BYTE#16#00,
        BYTE#16#04, BYTE#16#00, BYTE#16#03, BYTE#16#02, BYTE#16#02, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#23, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#B9, BYTE#16#0B, BYTE#16#00, BYTE#16#00,
        BYTE#16#AF, BYTE#16#44, BYTE#16#C0, BYTE#16#7E
    ];
    ExpectedMaterialAddition : ARRAY[0..79] OF BYTE := [
        BYTE#16#4F, BYTE#16#4F, BYTE#16#54, BYTE#16#32, BYTE#16#50, BYTE#16#00, BYTE#16#08, BYTE#16#00, BYTE#16#2C, BYTE#16#CB, BYTE#16#9A, BYTE#16#3B, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#01, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#1F, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#42, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#04, BYTE#16#03, BYTE#16#00, BYTE#16#00, BYTE#16#25, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#A1, BYTE#16#0F, BYTE#16#00, BYTE#16#00,
        BYTE#16#26, BYTE#16#00, BYTE#16#05, BYTE#16#04, BYTE#16#89, BYTE#16#13, BYTE#16#00, BYTE#16#00, BYTE#16#27, BYTE#16#00, BYTE#16#0A, BYTE#16#08, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#00,
        BYTE#16#00, BYTE#16#80, BYTE#16#28, BYTE#16#40, BYTE#16#13, BYTE#16#00, BYTE#16#03, BYTE#16#02, BYTE#16#08, BYTE#16#00, BYTE#16#00, BYTE#16#00, BYTE#16#11, BYTE#16#A1, BYTE#16#C4, BYTE#16#48
    ];
    Index : UINT;
END_VAR

RecipeLoadedEncoder();
RecipeApprovedEncoder();
BatchEventEncoder();
MaterialAdditionEncoder();

RecipeLoadedPassed := RecipeLoadedEncoder.Length = UINT#72;
RecipeApprovedPassed := RecipeApprovedEncoder.Length = UINT#88;
BatchEventPassed := BatchEventEncoder.Length = UINT#68;
MaterialAdditionPassed := MaterialAdditionEncoder.Length = UINT#80;
MismatchIndex := UINT#65535;

IF RecipeLoadedPassed THEN
    FOR Index := UINT#0 TO UINT#71 DO
        IF (RecipeLoadedEncoder.Buffer[Index] <> ExpectedRecipeLoaded[Index]) AND (MismatchIndex = UINT#65535) THEN
            RecipeLoadedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF RecipeApprovedPassed THEN
    FOR Index := UINT#0 TO UINT#87 DO
        IF (RecipeApprovedEncoder.Buffer[Index] <> ExpectedRecipeApproved[Index]) AND (MismatchIndex = UINT#65535) THEN
            RecipeApprovedPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF BatchEventPassed THEN
    FOR Index := UINT#0 TO UINT#67 DO
        IF (BatchEventEncoder.Buffer[Index] <> ExpectedBatchEvent[Index]) AND (MismatchIndex = UINT#65535) THEN
            BatchEventPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

IF MaterialAdditionPassed THEN
    FOR Index := UINT#0 TO UINT#79 DO
        IF (MaterialAdditionEncoder.Buffer[Index] <> ExpectedMaterialAddition[Index]) AND (MismatchIndex = UINT#65535) THEN
            MaterialAdditionPassed := FALSE;
            MismatchIndex := Index;
        END_IF;
    END_FOR;
END_IF;

Passed := RecipeLoadedPassed AND RecipeApprovedPassed AND BatchEventPassed AND MaterialAdditionPassed;
END_FUNCTION_BLOCK
