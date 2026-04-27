FUNCTION_BLOCK ClassicMotorCell
VAR
    RunningCountValue : INT;
    FaultCountValue : INT;
    LastFaultCodeValue : DWORD;
END_VAR

METHOD PUBLIC Update : INT
VAR_INPUT
    AbbStatus : WORD;
    DanfossStatus : WORD;
    SiemensStatus : WORD;
    RunCommand : BOOL;
    PullCordOk : BOOL;
END_VAR
RunningCountValue := INT#0;
FaultCountValue := INT#0;
LastFaultCodeValue := DWORD#0;
IF PullCordOk AND RunCommand THEN
    IF (WORD_TO_DWORD(AbbStatus) AND DWORD#16#0004) <> DWORD#0 THEN
        RunningCountValue := RunningCountValue + INT#1;
    END_IF;
    IF (WORD_TO_DWORD(AbbStatus) AND DWORD#16#0008) <> DWORD#0 THEN
        FaultCountValue := FaultCountValue + INT#1;
        LastFaultCodeValue := DWORD#16#10000011;
    END_IF;
    IF (WORD_TO_DWORD(DanfossStatus) AND DWORD#16#0100) <> DWORD#0 THEN
        RunningCountValue := RunningCountValue + INT#1;
    END_IF;
    IF (WORD_TO_DWORD(DanfossStatus) AND DWORD#16#0002) <> DWORD#0 THEN
        FaultCountValue := FaultCountValue + INT#1;
        LastFaultCodeValue := DWORD#16#20000022;
    END_IF;
    IF (WORD_TO_DWORD(SiemensStatus) AND DWORD#16#0400) <> DWORD#0 THEN
        RunningCountValue := RunningCountValue + INT#1;
    END_IF;
    IF (WORD_TO_DWORD(SiemensStatus) AND DWORD#16#0008) <> DWORD#0 THEN
        FaultCountValue := FaultCountValue + INT#1;
        LastFaultCodeValue := DWORD#16#30000033;
    END_IF;
END_IF;
Update := FaultCountValue;
END_METHOD

PUBLIC PROPERTY RunningCount : INT
GET
    RunningCount := RunningCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastFaultCode : DWORD
GET
    LastFaultCode := LastFaultCodeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Cell : ClassicMotorCell;
    Faults : INT;
END_VAR
Faults := Cell.Update(AbbStatus := WORD#16#0004, DanfossStatus := WORD#16#0100, SiemensStatus := WORD#16#0400, RunCommand := TRUE, PullCordOk := TRUE);
END_PROGRAM
