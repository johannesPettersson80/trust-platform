FUNCTION_BLOCK AutomationContext IMPLEMENTS IAutomationContext
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    ConstantsLoadedValue : BOOL;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ClearError();
ConstantsLoadedValue := FALSE;
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := ComponentErrorNone;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC LoadConstants : BOOL
LoadConstants := OSCAT_BASIC_Constants();
ConstantsLoadedValue := LoadConstants;
IF LoadConstants THEN
    ReadyValue := TRUE;
    ClearError();
END_IF;
END_METHOD

METHOD PUBLIC SetDefaultLanguage
VAR_INPUT
    LanguageIndex : INT;
END_VAR
IF NOT ConstantsLoadedValue THEN
    ConstantsLoadedValue := OSCAT_BASIC_Constants();
END_IF;
IF LanguageIndex >= INT#1 AND LanguageIndex <= LANGUAGE.LMAX THEN
    LANGUAGE.DEFAULT := LanguageIndex;
    ClearError();
ELSE
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorInvalidConfiguration;
    StatusValue := BYTE#1;
END_IF;
END_METHOD

METHOD PUBLIC DirectionName : STRING[3]
VAR_INPUT
    Degrees : REAL;
END_VAR
IF NOT ConstantsLoadedValue THEN
    ConstantsLoadedValue := OSCAT_BASIC_Constants();
END_IF;
DirectionName := DEG_TO_DIR(DEG := REAL_TO_INT(Degrees), N := INT#4, L := LANGUAGE.DEFAULT);
END_METHOD

METHOD PUBLIC DirectionDegrees : INT
VAR_INPUT
    Name : STRING[3];
END_VAR
IF NOT ConstantsLoadedValue THEN
    ConstantsLoadedValue := OSCAT_BASIC_Constants();
END_IF;
DirectionDegrees := DIR_TO_DEG(DIR := Name, L := LANGUAGE.DEFAULT);
END_METHOD

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ConstantsLoaded : BOOL
GET
    ConstantsLoaded := ConstantsLoadedValue OR g_oscat_constants_loaded;
END_GET
END_PROPERTY

PUBLIC PROPERTY Version : DWORD
GET
    Version := OSCAT_VERSION(IN := FALSE);
END_GET
END_PROPERTY

PUBLIC PROPERTY Math : CONSTANTS_MATH
GET
    IF NOT ConstantsLoadedValue THEN
        ConstantsLoadedValue := OSCAT_BASIC_Constants();
    END_IF;
    Math := MATH;
END_GET
END_PROPERTY

PUBLIC PROPERTY Pi2 : REAL
GET
    IF NOT ConstantsLoadedValue THEN
        ConstantsLoadedValue := OSCAT_BASIC_Constants();
    END_IF;
    Pi2 := MATH.PI2;
END_GET
END_PROPERTY

PUBLIC PROPERTY Phys : CONSTANTS_PHYS
GET
    IF NOT ConstantsLoadedValue THEN
        ConstantsLoadedValue := OSCAT_BASIC_Constants();
    END_IF;
    Phys := PHYS;
END_GET
END_PROPERTY

PUBLIC PROPERTY LanguageValues : CONSTANTS_LANGUAGE
GET
    IF NOT ConstantsLoadedValue THEN
        ConstantsLoadedValue := OSCAT_BASIC_Constants();
    END_IF;
    LanguageValues := LANGUAGE;
END_GET
END_PROPERTY

PUBLIC PROPERTY DefaultLanguage : INT
GET
    IF NOT ConstantsLoadedValue THEN
        ConstantsLoadedValue := OSCAT_BASIC_Constants();
    END_IF;
    DefaultLanguage := LANGUAGE.DEFAULT;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
