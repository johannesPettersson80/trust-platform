PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    BaselineAborted : BOOL := FALSE;
    ShapedAborted : BOOL := FALSE;
    ElapsedSeconds : LREAL := 0.0;
    CycleInterval : LREAL := 0.01;
    SpeedSetpoint : LREAL := 1.0;
    ProportionalGain : LREAL := 0.4;
    LeadZero : LREAL := 0.3;
    LeadPole : LREAL := 3.0;
    LagZero : LREAL := 0.8;
    LagPole : LREAL := 0.08;
    RollOffPole : LREAL := 8.0;
    TorqueMinimum : LREAL := -20.0;
    TorqueMaximum : LREAL := 20.0;
    BaselineMeasuredSpeed : LREAL := 0.0;
    BaselineTrueSpeed : LREAL := 0.0;
    ShapedMeasuredSpeed : LREAL := 0.0;
    ShapedTrueSpeed : LREAL := 0.0;
    BaselineError : LREAL := 0.0;
    ShapedError : LREAL := 0.0;
    BaselineRequestedTorque : LREAL := 0.0;
    BaselineAppliedTorque : LREAL := 0.0;
    BaselineLoadTorque : LREAL := 0.0;
    ShapedRequestedTorque : LREAL := 0.0;
    ShapedAppliedTorque : LREAL := 0.0;
    ShapedLoadTorque : LREAL := 0.0;
    LeadState : LREAL := 0.0;
    LeadOutput : LREAL := 0.0;
    LagState : LREAL := 0.0;
    LagOutput : LREAL := 0.0;
    RollOffState : LREAL := 0.0;
    ShapedErrorFiltered : LREAL := 0.0;
END_VAR

IF ExperimentEnabled THEN
    ElapsedSeconds := ElapsedSeconds + CycleInterval;

    IF ElapsedSeconds >= LREAL#60.0 THEN
        BaselineLoadTorque := LREAL#0.1;
        ShapedLoadTorque := LREAL#0.1;
    ELSE
        BaselineLoadTorque := LREAL#0.0;
        ShapedLoadTorque := LREAL#0.0;
    END_IF;

    BaselineError := SpeedSetpoint - BaselineMeasuredSpeed;
    BaselineRequestedTorque := ProportionalGain * BaselineError;
    BaselineAppliedTorque := MAX(MIN(BaselineRequestedTorque, TorqueMaximum), TorqueMinimum);

    ShapedError := SpeedSetpoint - ShapedMeasuredSpeed;

    // These three states implement the declared proper lead-lag-roll-off
    // controller. The roll-off pole prevents ideal derivative behavior.
    LeadState := LeadState + CycleInterval * (-LeadPole * LeadState + ShapedError);
    LeadOutput := (LeadPole / LeadZero) * ShapedError
        + (LeadPole / LeadZero) * (LeadZero - LeadPole) * LeadState;
    LagState := LagState + CycleInterval * (-LagPole * LagState + LeadOutput);
    LagOutput := (LagPole / LagZero) * LeadOutput
        + (LagPole / LagZero) * (LagZero - LagPole) * LagState;
    RollOffState := RollOffState
        + CycleInterval * (-RollOffPole * RollOffState + LagOutput);
    ShapedErrorFiltered := RollOffPole * RollOffState;
    ShapedRequestedTorque := ProportionalGain * ShapedErrorFiltered;
    ShapedAppliedTorque := MAX(MIN(ShapedRequestedTorque, TorqueMaximum), TorqueMinimum);

    IF (BaselineTrueSpeed < LREAL#-5.0) OR (BaselineTrueSpeed > LREAL#5.0) THEN
        BaselineAborted := TRUE;
        BaselineAppliedTorque := LREAL#0.0;
        BaselineLoadTorque := LREAL#0.0;
    END_IF;
    IF (ShapedTrueSpeed < LREAL#-5.0) OR (ShapedTrueSpeed > LREAL#5.0) THEN
        ShapedAborted := TRUE;
        ShapedAppliedTorque := LREAL#0.0;
        ShapedLoadTorque := LREAL#0.0;
    END_IF;
ELSE
    ElapsedSeconds := LREAL#0.0;
    BaselineAborted := FALSE;
    ShapedAborted := FALSE;
    BaselineRequestedTorque := LREAL#0.0;
    BaselineAppliedTorque := LREAL#0.0;
    BaselineLoadTorque := LREAL#0.0;
    ShapedRequestedTorque := LREAL#0.0;
    ShapedAppliedTorque := LREAL#0.0;
    ShapedLoadTorque := LREAL#0.0;
    LeadState := LREAL#0.0;
    LagState := LREAL#0.0;
    RollOffState := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.BaselineAborted AT %MX97.0 : BOOL;
    P1.ShapedAborted AT %MX97.1 : BOOL;
    P1.BaselineRequestedTorque AT %ML0 : LREAL;
    P1.ShapedRequestedTorque AT %ML8 : LREAL;
    P1.SpeedSetpoint AT %ML16 : LREAL;
    P1.BaselineAppliedTorque AT %QL0 : LREAL;
    P1.BaselineLoadTorque AT %QL8 : LREAL;
    P1.ShapedAppliedTorque AT %QL16 : LREAL;
    P1.ShapedLoadTorque AT %QL24 : LREAL;
    P1.BaselineMeasuredSpeed AT %IL0 : LREAL;
    P1.BaselineTrueSpeed AT %IL8 : LREAL;
    P1.ShapedMeasuredSpeed AT %IL16 : LREAL;
    P1.ShapedTrueSpeed AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
