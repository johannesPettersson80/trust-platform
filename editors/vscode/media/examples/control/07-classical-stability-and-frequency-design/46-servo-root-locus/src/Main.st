PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    ProportionalGain : LREAL := 4.0;
    LeadGain : LREAL := 50.0;
    LeadZero : LREAL := 2.5;
    LeadPole : LREAL := 15.0;
    CycleTime : LREAL := 0.01;
    ProportionalSetpoint : LREAL := 0.0;
    LeadSetpoint : LREAL := 0.0;
    ProportionalPosition : LREAL := 0.0;
    LeadPosition : LREAL := 0.0;
    ProportionalRequested : LREAL := 0.0;
    LeadRequested : LREAL := 0.0;
    ProportionalApplied : LREAL := 0.0;
    LeadApplied : LREAL := 0.0;
    LeadFilterState : LREAL := 0.0;
    LeadError : LREAL := 0.0;
END_VAR

IF NOT ExperimentEnabled THEN
    ProportionalSetpoint := LREAL#0.0;
    LeadSetpoint := LREAL#0.0;
    ProportionalRequested := LREAL#0.0;
    LeadRequested := LREAL#0.0;
    ProportionalApplied := LREAL#0.0;
    LeadApplied := LREAL#0.0;
    LeadFilterState := LREAL#0.0;
ELSE
    ProportionalSetpoint := LREAL#1.0;
    LeadSetpoint := LREAL#1.0;

    ProportionalRequested := ProportionalGain
        * (ProportionalSetpoint - ProportionalPosition);

    (* The state realizes (s + LeadZero)/(s + LeadPole). Calculating output
       before advancing the state preserves the declared direct term. *)
    LeadError := LeadSetpoint - LeadPosition;
    LeadRequested := LeadGain
        * (LeadError + (LeadZero - LeadPole) * LeadFilterState);
    LeadFilterState := LeadFilterState
        + CycleTime * (-LeadPole * LeadFilterState + LeadError);

    ProportionalApplied := MAX(MIN(ProportionalRequested, LREAL#100.0), LREAL#-100.0);
    LeadApplied := MAX(MIN(LeadRequested, LREAL#100.0), LREAL#-100.0);
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.ProportionalRequested AT %ML0 : LREAL;
    P1.LeadRequested AT %ML8 : LREAL;
    P1.ProportionalSetpoint AT %ML16 : LREAL;
    P1.LeadSetpoint AT %ML24 : LREAL;
    P1.ProportionalApplied AT %QL0 : LREAL;
    P1.LeadApplied AT %QL8 : LREAL;
    P1.ProportionalPosition AT %IL0 : LREAL;
    P1.LeadPosition AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
