FUNCTION_BLOCK ClassicDosingScheduler
VAR
    PendingCommandType : INT;
    HasPending : BOOL;
    ExecutedCountValue : INT;
    RejectedCountValue : INT;
    LastCommandIdValue : DWORD;
    LastBeforePHValue : REAL;
END_VAR

METHOD PUBLIC Enqueue : BOOL
VAR_INPUT
    CommandType : INT;
END_VAR
IF HasPending THEN
    Enqueue := FALSE;
ELSE
    PendingCommandType := CommandType;
    HasPending := TRUE;
    Enqueue := TRUE;
END_IF;
END_METHOD

METHOD PUBLIC ExecuteNext : BOOL
VAR_INPUT
    CalibrationValid : BOOL;
    TankLevelMl : REAL;
    CurrentPH : REAL;
END_VAR
VAR
    RequiredVolume : REAL;
    CommandId : DWORD;
    SafePH : BOOL;
END_VAR
CASE PendingCommandType OF
    INT#1:
        RequiredVolume := REAL#120.0;
        CommandId := DWORD#16#C001;
    INT#2:
        RequiredVolume := REAL#250.0;
        CommandId := DWORD#16#A001;
ELSE
    RequiredVolume := REAL#80.0;
    CommandId := DWORD#16#5001;
END_CASE;
SafePH := (CurrentPH >= REAL#4.0) AND (CurrentPH <= REAL#9.0);
IF HasPending AND CalibrationValid AND SafePH AND (TankLevelMl >= RequiredVolume) THEN
    LastBeforePHValue := CurrentPH;
    LastCommandIdValue := CommandId;
    ExecutedCountValue := ExecutedCountValue + INT#1;
    HasPending := FALSE;
    ExecuteNext := TRUE;
ELSE
    RejectedCountValue := RejectedCountValue + INT#1;
    ExecuteNext := FALSE;
END_IF;
END_METHOD

PUBLIC PROPERTY ExecutedCount : INT
GET
    ExecutedCount := ExecutedCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RejectedCount : INT
GET
    RejectedCount := RejectedCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCommandId : DWORD
GET
    LastCommandId := LastCommandIdValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Scheduler : ClassicDosingScheduler;
    PumpStart : BOOL;
END_VAR
Scheduler.Enqueue(CommandType := INT#1);
PumpStart := Scheduler.ExecuteNext(CalibrationValid := TRUE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#7.2);
END_PROGRAM
