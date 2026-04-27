FUNCTION_BLOCK ClassicDairySeparator
VAR
    StateValue : INT;
    FeedValveOpenValue : BOOL;
    DischargeValveOpenValue : BOOL;
    CipValveOpenValue : BOOL;
    BowlRunningValue : BOOL;
    FaultCodeValue : DWORD;
END_VAR

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    UseSiemens : BOOL;
    StatusWord : WORD;
    SpeedFeedbackRpm : REAL;
    VibrationMmS : REAL;
    FaultRaw : WORD;
END_VAR
VAR
    RunningBit : BOOL;
    FaultBit : BOOL;
    AtSpeed : BOOL;
    VibrationHigh : BOOL;
END_VAR
IF UseSiemens THEN
    RunningBit := (WORD_TO_DWORD(StatusWord) AND DWORD#16#0400) <> DWORD#0;
    FaultCodeValue := DWORD#16#50000000 OR WORD_TO_DWORD(FaultRaw);
ELSE
    RunningBit := (WORD_TO_DWORD(StatusWord) AND DWORD#16#0004) <> DWORD#0;
    FaultCodeValue := DWORD#16#D0000000 OR WORD_TO_DWORD(FaultRaw);
END_IF;
FaultBit := (WORD_TO_DWORD(StatusWord) AND DWORD#16#0008) <> DWORD#0;
VibrationHigh := VibrationMmS > REAL#8.0;
AtSpeed := (SpeedFeedbackRpm >= REAL#6000.0) AND NOT FaultBit AND NOT VibrationHigh;

IF VibrationHigh THEN
    IF UseSiemens THEN
        FaultCodeValue := DWORD#16#500000F1;
    ELSE
        FaultCodeValue := DWORD#16#D00000F1;
    END_IF;
END_IF;

IF FaultBit OR VibrationHigh THEN
    StateValue := INT#90;
ELSIF StateValue = INT#0 THEN
    IF CipMode THEN
        StateValue := INT#40;
    ELSIF StartCommand THEN
        StateValue := INT#10;
    END_IF;
ELSIF StateValue = INT#10 THEN
    IF CipMode THEN
        StateValue := INT#40;
    ELSIF AtSpeed THEN
        StateValue := INT#20;
    END_IF;
ELSIF StateValue = INT#20 THEN
    IF CipMode THEN
        StateValue := INT#40;
    ELSIF DischargeRequest THEN
        StateValue := INT#30;
    ELSIF NOT AtSpeed THEN
        StateValue := INT#90;
    END_IF;
ELSIF StateValue = INT#30 THEN
    IF CipMode THEN
        StateValue := INT#40;
    ELSIF NOT DischargeRequest THEN
        StateValue := INT#20;
    END_IF;
ELSIF StateValue = INT#40 THEN
    IF NOT CipMode THEN
        StateValue := INT#0;
    END_IF;
END_IF;

FeedValveOpenValue := StateValue = INT#20;
DischargeValveOpenValue := StateValue = INT#30;
CipValveOpenValue := StateValue = INT#40;
BowlRunningValue := RunningBit OR (StateValue = INT#10) OR (StateValue = INT#20) OR (StateValue = INT#30);
Update := StateValue;
END_METHOD

PUBLIC PROPERTY FeedValveOpen : BOOL
GET
    FeedValveOpen := FeedValveOpenValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY BowlRunning : BOOL
GET
    BowlRunning := BowlRunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FaultCode : DWORD
GET
    FaultCode := FaultCodeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Separator : ClassicDairySeparator;
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    UseSiemensDrive : BOOL;
    StatusWordRaw : WORD;
    SpeedFeedbackRaw : INT;
    VibrationRaw : INT;
    FaultRaw : WORD;
    FeedValveOut : BOOL;
    DischargeValveOut : BOOL;
    CipValveOut : BOOL;
END_VAR
Separator.Update(StartCommand := StartCommand, DischargeRequest := DischargeRequest, CipMode := CipMode, UseSiemens := UseSiemensDrive, StatusWord := StatusWordRaw, SpeedFeedbackRpm := INT_TO_REAL(SpeedFeedbackRaw), VibrationMmS := INT_TO_REAL(VibrationRaw) / REAL#10.0, FaultRaw := FaultRaw);
FeedValveOut := Separator.FeedValveOpen;
DischargeValveOut := DischargeRequest;
CipValveOut := CipMode;
END_PROGRAM
