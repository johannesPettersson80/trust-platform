FUNCTION_BLOCK ClassicCipController
VAR
    CurrentStateValue : INT;
    TransitionCountValue : INT;
    CycleCounter : DwordCounter;
    AbortedValue : BOOL;
END_VAR

METHOD PUBLIC Initialize
CurrentStateValue := INT#0;
TransitionCountValue := INT#0;
AbortedValue := FALSE;
CycleCounter.Initialize();
CycleCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PRIVATE TransitionTo
VAR_INPUT
    NextId : INT;
END_VAR
IF NextId <> CurrentStateValue THEN
    CurrentStateValue := NextId;
    TransitionCountValue := TransitionCountValue + INT#1;
    IF CurrentStateValue = INT#90 THEN
        AbortedValue := TRUE;
    ELSIF CurrentStateValue = INT#100 THEN
        CycleCounter.Update(Up := TRUE, Down := FALSE);
    END_IF;
END_IF;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
IF NOT EStopHealthy THEN
    TransitionTo(NextId := INT#90);
ELSE
    CASE CurrentStateValue OF
        INT#0:
            IF StartCommand THEN
                TransitionTo(NextId := INT#10);
            END_IF;
        INT#10:
            IF NOT FlowOk THEN
                TransitionTo(NextId := INT#90);
            ELSIF StepComplete THEN
                TransitionTo(NextId := INT#20);
            END_IF;
        INT#20:
            IF NOT FlowOk THEN
                TransitionTo(NextId := INT#90);
            ELSIF (CausticTempC >= REAL#70.0) AND StepComplete THEN
                TransitionTo(NextId := INT#30);
            END_IF;
        INT#30:
            IF NOT FlowOk THEN
                TransitionTo(NextId := INT#90);
            ELSIF StepComplete THEN
                TransitionTo(NextId := INT#40);
            END_IF;
        INT#40:
            IF NOT FlowOk THEN
                TransitionTo(NextId := INT#90);
            ELSIF StepComplete THEN
                TransitionTo(NextId := INT#50);
            END_IF;
        INT#50:
            IF NOT FlowOk THEN
                TransitionTo(NextId := INT#90);
            ELSIF ConductivityUs < REAL#50.0 THEN
                TransitionTo(NextId := INT#100);
            END_IF;
    END_CASE;
END_IF;
Update := CurrentStateValue;
END_METHOD

PUBLIC PROPERTY CurrentStateId : INT
GET
    CurrentStateId := CurrentStateValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY CycleCount : DWORD
GET
    CycleCount := CycleCounter.Count;
END_GET
END_PROPERTY

PUBLIC PROPERTY Aborted : BOOL
GET
    Aborted := AbortedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Cip : ClassicCipController;
    StateId : INT;
END_VAR
Cip.Initialize();
StateId := Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
END_PROGRAM
