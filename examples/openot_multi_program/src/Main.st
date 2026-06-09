TYPE E_FillStep : (Idle := 0, Filling := 1, Holding := 2, Complete := 3) END_TYPE

PROGRAM Filler
VAR
    StartCmd : BOOL := TRUE;
    FillStarted : BOOL {attribute 'oot' := 'message', 'template' := 'filler started'};
    Step : E_FillStep {attribute 'oot' := 'state', 'category' := 'process'} := Idle;
    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L', 'deadband' := '0.5'};
END_VAR

CASE Step OF
    Idle:
        IF StartCmd THEN
            FillStarted := TRUE;
            Step := Filling;
        END_IF;
    Filling:
        Level := Level + REAL#5.0;
        IF Level >= REAL#10.0 THEN
            Step := Holding;
        END_IF;
    Holding:
        Step := Complete;
    Complete:
        ;
ELSE
END_CASE;
END_PROGRAM

PROGRAM Quality
VAR
    Scan : UINT;
    OperatorName : STRING[32] := 'qa-operator';
    ReasonText : STRING[32] := 'setpoint adjustment';
    SampleStarted : BOOL {attribute 'oot' := 'message', 'template' := 'quality sample started'};
    RejectCount : DINT {attribute 'oot' := 'value'};
    SetPoint : REAL {attribute 'oot' := 'value', 'audit' := 'true', 'actor' := OperatorName, 'reason' := ReasonText, 'unit' := 'L'};
    HighRejectAlarm : BOOL {attribute 'oot' := 'alarm', 'class' := 'alarm', 'severity' := '700'};
END_VAR

CASE Scan OF
    UINT#0:
        SampleStarted := TRUE;
        RejectCount := DINT#1;
        SetPoint := REAL#12.0;
    UINT#1:
        SampleStarted := FALSE;
    UINT#2:
        RejectCount := DINT#3;
        SetPoint := REAL#12.5;
        HighRejectAlarm := TRUE;
ELSE
END_CASE;

Scan := Scan + UINT#1;
END_PROGRAM
