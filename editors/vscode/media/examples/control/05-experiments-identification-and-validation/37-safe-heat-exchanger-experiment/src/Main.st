PROGRAM Main
VAR
    ProposedStep : LREAL := LREAL#10.0;
    ProposedDuration : LREAL := LREAL#63.0;
    ReviewPlan : BOOL := FALSE;
    StartExperiment : BOOL := FALSE;
    ResetExperiment : BOOL := FALSE;
    SignalQualityGood : BOOL := TRUE;
    OperatingModeAllowed : BOOL := TRUE;
    ConfoundingDisturbanceActive : BOOL := FALSE;
    TopologyReviewed : BOOL := FALSE;
    ImmediatePhysicalApproval : BOOL := FALSE;
    PhysicalPlantRequested : BOOL := FALSE;
    ProcessValue : LREAL := LREAL#50.0;
    RequestedValve : LREAL := LREAL#60.0;
    AppliedValve : LREAL := LREAL#60.0;
    PlanAccepted : BOOL := FALSE;
    ExperimentRunning : BOOL := FALSE;
    ExperimentComplete : BOOL := FALSE;
    ExperimentAborted : BOOL := FALSE;
    Restoring : BOOL := FALSE;
    PhysicalExecutionEligible : BOOL := FALSE;
    PlanRejectReason : INT := 0;
    AbortReason : INT := 0;
    PredictedFinalTemperature : LREAL := LREAL#55.0;
    BaselineRange : LREAL := LREAL#0.0;
    BaselineSlope : LREAL := LREAL#0.0;
    State : INT := 0;
    LatchedStep : LREAL := LREAL#0.0;
    LatchedDuration : LREAL := LREAL#0.0;
    PhaseElapsed : LREAL := LREAL#0.0;
    BaselineFirst : LREAL := LREAL#50.0;
    BaselineMinimum : LREAL := LREAL#50.0;
    BaselineMaximum : LREAL := LREAL#50.0;
END_VAR

(* This executable is deliberately simulation-only. Review flags are visible
   for teaching, but they cannot grant authority to move a physical valve. *)
PhysicalExecutionEligible := FALSE;
RequestedValve := LREAL#60.0;

IF ResetExperiment THEN
    State := 0;
    PlanAccepted := FALSE;
    ExperimentRunning := FALSE;
    ExperimentComplete := FALSE;
    ExperimentAborted := FALSE;
    Restoring := FALSE;
    PlanRejectReason := 0;
    AbortReason := 0;
    PhaseElapsed := LREAL#0.0;
END_IF;

IF (State = 0) AND ReviewPlan AND NOT ResetExperiment THEN
    PredictedFinalTemperature := LREAL#50.0 + LREAL#0.5 * ProposedStep;
    PlanAccepted := FALSE;
    PlanRejectReason := 0;

    IF ((LREAL#60.0 + ProposedStep) < LREAL#40.0) OR
       ((LREAL#60.0 + ProposedStep) > LREAL#80.0) OR
       (PredictedFinalTemperature < LREAL#45.0) OR
       (PredictedFinalTemperature > LREAL#58.0) THEN
        PlanRejectReason := 1;
    ELSIF (ProposedDuration < LREAL#63.0) OR (ProposedDuration > LREAL#120.0) THEN
        PlanRejectReason := 2;
    ELSE
        LatchedStep := ProposedStep;
        LatchedDuration := ProposedDuration;
        PlanAccepted := TRUE;
    END_IF;
END_IF;

IF (State = 0) AND StartExperiment AND PlanAccepted AND NOT ResetExperiment THEN
    State := 1;
    ExperimentRunning := TRUE;
    ExperimentComplete := FALSE;
    ExperimentAborted := FALSE;
    Restoring := FALSE;
    AbortReason := 0;
    PhaseElapsed := LREAL#0.0;
    BaselineFirst := ProcessValue;
    BaselineMinimum := ProcessValue;
    BaselineMaximum := ProcessValue;
END_IF;

IF State = 1 THEN
    IF (NOT SignalQualityGood) OR (NOT OperatingModeAllowed) OR
       ConfoundingDisturbanceActive OR (ProcessValue <> ProcessValue) OR
       (ProcessValue < LREAL#45.0) OR (ProcessValue > LREAL#58.0) THEN
        State := 3;
        ExperimentRunning := FALSE;
        ExperimentAborted := TRUE;
        Restoring := TRUE;
        AbortReason := 1;
    ELSE
        IF ProcessValue < BaselineMinimum THEN BaselineMinimum := ProcessValue; END_IF;
        IF ProcessValue > BaselineMaximum THEN BaselineMaximum := ProcessValue; END_IF;
        PhaseElapsed := PhaseElapsed + LREAL#0.1;
        IF PhaseElapsed >= LREAL#10.0 THEN
            BaselineRange := BaselineMaximum - BaselineMinimum;
            BaselineSlope := (ProcessValue - BaselineFirst) / PhaseElapsed;
            IF (BaselineRange <= LREAL#0.05) AND (ABS(BaselineSlope) <= LREAL#0.01) THEN
                State := 2;
                PhaseElapsed := LREAL#0.0;
            ELSE
                State := 3;
                ExperimentRunning := FALSE;
                ExperimentAborted := TRUE;
                Restoring := TRUE;
                AbortReason := 2;
            END_IF;
        END_IF;
    END_IF;
END_IF;

IF State = 2 THEN
    IF (NOT SignalQualityGood) OR (NOT OperatingModeAllowed) OR
       ConfoundingDisturbanceActive OR (ProcessValue <> ProcessValue) OR
       (ProcessValue < LREAL#45.0) OR (ProcessValue > LREAL#58.0) THEN
        State := 3;
        ExperimentRunning := FALSE;
        ExperimentAborted := TRUE;
        Restoring := TRUE;
        AbortReason := 1;
    ELSE
        RequestedValve := LREAL#60.0 + LatchedStep;
        PhaseElapsed := PhaseElapsed + LREAL#0.1;
        IF PhaseElapsed >= LatchedDuration THEN
            RequestedValve := LREAL#60.0;
            State := 3;
            ExperimentRunning := FALSE;
            Restoring := TRUE;
            PhaseElapsed := LREAL#0.0;
        END_IF;
    END_IF;
END_IF;

IF State = 3 THEN
    RequestedValve := LREAL#60.0;
    IF SignalQualityGood AND (ABS(ProcessValue - LREAL#50.0) <= LREAL#0.1) THEN
        State := 4;
        Restoring := FALSE;
        IF NOT ExperimentAborted THEN ExperimentComplete := TRUE; END_IF;
    END_IF;
END_IF;

(* Requested and applied valve positions remain separate so later actuator
   lessons can introduce limits and dynamics without changing their meaning. *)
IF RequestedValve < LREAL#0.0 THEN
    AppliedValve := LREAL#0.0;
ELSIF RequestedValve > LREAL#100.0 THEN
    AppliedValve := LREAL#100.0;
ELSE
    AppliedValve := RequestedValve;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ProposedStep AT %ML0 : LREAL;
    P1.ProposedDuration AT %ML8 : LREAL;
    P1.ReviewPlan AT %MX16.0 : BOOL;
    P1.StartExperiment AT %MX16.1 : BOOL;
    P1.ResetExperiment AT %MX16.2 : BOOL;
    P1.SignalQualityGood AT %MX16.3 : BOOL;
    P1.OperatingModeAllowed AT %MX16.4 : BOOL;
    P1.ConfoundingDisturbanceActive AT %MX16.5 : BOOL;
    P1.TopologyReviewed AT %MX16.6 : BOOL;
    P1.ImmediatePhysicalApproval AT %MX16.7 : BOOL;
    P1.PhysicalPlantRequested AT %MX17.0 : BOOL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.RequestedValve AT %ML32 : LREAL;
    P1.AppliedValve AT %QL0 : LREAL;
    P1.PlanAccepted AT %MX18.0 : BOOL;
    P1.ExperimentRunning AT %MX18.1 : BOOL;
    P1.ExperimentComplete AT %MX18.2 : BOOL;
    P1.ExperimentAborted AT %MX18.3 : BOOL;
    P1.Restoring AT %MX18.4 : BOOL;
    P1.PhysicalExecutionEligible AT %MX18.5 : BOOL;
    P1.PlanRejectReason AT %MW20 : INT;
    P1.AbortReason AT %MW22 : INT;
END_VAR
END_CONFIGURATION
