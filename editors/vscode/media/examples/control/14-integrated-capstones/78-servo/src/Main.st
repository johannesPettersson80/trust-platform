USING Control;


PROGRAM Main
VAR
    PController : ServoCandidate;
    PDController : ServoCandidate;
    PIDController : ServoCandidate;
    CoarseController : ServoCandidate;
    Initialized : BOOL := FALSE;
    AutomaticEnabled : BOOL := TRUE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : DINT := DINT#0;
    ExperimentElapsed : LREAL := 0.0;
    MeasurementQualityGood : BOOL := TRUE;
    PositionSetpoint : LREAL := 0.0;
    LoadTorque : LREAL := 0.0;
    ManualTorque : LREAL := 0.0;
    FineCycleTime : LREAL := 0.01;
    PRawPosition : LREAL := 0.0;
    PTruePosition : LREAL := 0.0;
    PVelocity : LREAL := 0.0;
    PDRawPosition : LREAL := 0.0;
    PDTruePosition : LREAL := 0.0;
    PDVelocity : LREAL := 0.0;
    PIDRawPosition : LREAL := 0.0;
    PIDTruePosition : LREAL := 0.0;
    PIDVelocity : LREAL := 0.0;
    CoarseRawPosition : LREAL := 0.0;
    CoarseTruePosition : LREAL := 0.0;
    CoarseVelocity : LREAL := 0.0;
    PFilteredPosition : LREAL := 0.0;
    PDFilteredPosition : LREAL := 0.0;
    PIDFilteredPosition : LREAL := 0.0;
    CoarseFilteredPosition : LREAL := 0.0;
    PRequestedTorque : LREAL := 0.0;
    PDRequestedTorque : LREAL := 0.0;
    PIDRequestedTorque : LREAL := 0.0;
    CoarseRequestedTorque : LREAL := 0.0;
    CoarsePendingTorque : LREAL := 0.0;
    PAppliedTorque : LREAL := 0.0;
    PLoadTorque : LREAL := 0.0;
    PDAppliedTorque : LREAL := 0.0;
    PDLoadTorque : LREAL := 0.0;
    PIDAppliedTorque : LREAL := 0.0;
    PIDLoadTorque : LREAL := 0.0;
    CoarseAppliedTorque : LREAL := 0.0;
    CoarseLoadTorque : LREAL := 0.0;
    FineUpdatePulse : BOOL := FALSE;
    CoarseUpdatePulse : BOOL := FALSE;
    CoarseCounter : INT := 0;
    ControllerRevision : DINT := DINT#78;
END_VAR

IF NOT Initialized THEN
    PController.Configure(
        ProportionalGain := LREAL#7.0,
        IntegralGain := LREAL#0.0,
        DerivativeGain := LREAL#0.0,
        FilterTimeConstant := LREAL#0.05,
        InitialPosition := PRawPosition
    );
    PDController.Configure(
        ProportionalGain := LREAL#7.0,
        IntegralGain := LREAL#0.0,
        DerivativeGain := LREAL#2.0,
        FilterTimeConstant := LREAL#0.05,
        InitialPosition := PDRawPosition
    );
    PIDController.Configure(
        ProportionalGain := LREAL#7.0,
        IntegralGain := LREAL#1.2,
        DerivativeGain := LREAL#2.0,
        FilterTimeConstant := LREAL#0.05,
        InitialPosition := PIDRawPosition
    );
    CoarseController.Configure(
        ProportionalGain := LREAL#7.0,
        IntegralGain := LREAL#1.2,
        DerivativeGain := LREAL#2.0,
        FilterTimeConstant := LREAL#0.05,
        InitialPosition := CoarseRawPosition
    );
    Initialized := TRUE;
END_IF;

(* Candidate comparison never changes the confirmed controller revision. *)
ControllerRevision := DINT#78;

(* Control Studio grants one authority. This project owns the reference step,
   load step, and zero-torque restoration on the actual 10 ms timeline. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentScan := DINT#0;
        ExperimentElapsed := LREAL#0.0;
    END_IF;
    ExperimentScan := ExperimentScan + DINT#1;
    ExperimentElapsed := ExperimentElapsed + LREAL#0.01;
    AutomaticEnabled := TRUE;
    MeasurementQualityGood := TRUE;
    IF ExperimentScan <= DINT#100 THEN
        PositionSetpoint := LREAL#0.0;
    ELSE
        PositionSetpoint := LREAL#0.5;
    END_IF;
    IF ExperimentScan <= DINT#1000 THEN
        LoadTorque := LREAL#0.0;
    ELSE
        LoadTorque := LREAL#1.5;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentScan := DINT#0;
    ExperimentElapsed := LREAL#0.0;
    AutomaticEnabled := FALSE;
    MeasurementQualityGood := TRUE;
    PositionSetpoint := LREAL#0.0;
    LoadTorque := LREAL#0.0;
    ManualTorque := LREAL#0.0;
END_IF;

FineUpdatePulse := TRUE;
PAppliedTorque := PController.Update(
    Setpoint := PositionSetpoint,
    MeasuredPosition := PRawPosition,
    AutomaticMode := AutomaticEnabled,
    QualityGood := MeasurementQualityGood,
    ManualTorque := ManualTorque,
    CycleTime := FineCycleTime
);
PDAppliedTorque := PDController.Update(
    Setpoint := PositionSetpoint,
    MeasuredPosition := PDRawPosition,
    AutomaticMode := AutomaticEnabled,
    QualityGood := MeasurementQualityGood,
    ManualTorque := ManualTorque,
    CycleTime := FineCycleTime
);
PIDAppliedTorque := PIDController.Update(
    Setpoint := PositionSetpoint,
    MeasuredPosition := PIDRawPosition,
    AutomaticMode := AutomaticEnabled,
    QualityGood := MeasurementQualityGood,
    ManualTorque := ManualTorque,
    CycleTime := FineCycleTime
);
PFilteredPosition := PController.FilteredPosition;
PDFilteredPosition := PDController.FilteredPosition;
PIDFilteredPosition := PIDController.FilteredPosition;
PRequestedTorque := PController.RequestedTorque;
PDRequestedTorque := PDController.RequestedTorque;
PIDRequestedTorque := PIDController.RequestedTorque;

CoarseUpdatePulse := FALSE;
IF (NOT AutomaticEnabled) OR (NOT MeasurementQualityGood) THEN
    CoarseCounter := 0;
    CoarseRequestedTorque := LREAL#0.0;
    CoarsePendingTorque := LREAL#0.0;
    CoarseAppliedTorque := LREAL#0.0;
ELSE
    CoarseCounter := CoarseCounter + 1;
    IF CoarseCounter >= 10 THEN
        CoarseCounter := 0;
        CoarseUpdatePulse := TRUE;
        (* The drive applies the preceding request for one complete coarse
           interval, making the computation delay explicit and observable. *)
        CoarseAppliedTorque := CoarsePendingTorque;
        CoarsePendingTorque := CoarseController.Update(
            Setpoint := PositionSetpoint,
            MeasuredPosition := CoarseRawPosition,
            AutomaticMode := AutomaticEnabled,
            QualityGood := MeasurementQualityGood,
            ManualTorque := ManualTorque,
            CycleTime := LREAL#0.1
        );
        CoarseRequestedTorque := CoarseController.RequestedTorque;
        CoarseFilteredPosition := CoarseController.FilteredPosition;
    END_IF;
END_IF;

PLoadTorque := LoadTorque;
PDLoadTorque := LoadTorque;
PIDLoadTorque := LoadTorque;
CoarseLoadTorque := LoadTorque;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutomaticEnabled AT %MX192.0 : BOOL;
    P1.MeasurementQualityGood AT %MX192.1 : BOOL;
    P1.ExperimentEnabled AT %MX192.2 : BOOL;
    P1.SafetyAbort AT %MX192.3 : BOOL;
    P1.InputsValid AT %MX192.4 : BOOL;
    P1.PositionSetpoint AT %ML200 : LREAL;
    P1.LoadTorque AT %ML208 : LREAL;
    P1.ManualTorque AT %ML216 : LREAL;
    P1.ExperimentElapsed AT %ML224 : LREAL;
    P1.PRawPosition AT %IL0 : LREAL;
    P1.PTruePosition AT %IL8 : LREAL;
    P1.PVelocity AT %IL16 : LREAL;
    P1.PDRawPosition AT %IL24 : LREAL;
    P1.PDTruePosition AT %IL32 : LREAL;
    P1.PDVelocity AT %IL40 : LREAL;
    P1.PIDRawPosition AT %IL48 : LREAL;
    P1.PIDTruePosition AT %IL56 : LREAL;
    P1.PIDVelocity AT %IL64 : LREAL;
    P1.CoarseRawPosition AT %IL72 : LREAL;
    P1.CoarseTruePosition AT %IL80 : LREAL;
    P1.CoarseVelocity AT %IL88 : LREAL;
    P1.PFilteredPosition AT %ML96 : LREAL;
    P1.PDFilteredPosition AT %ML104 : LREAL;
    P1.PIDFilteredPosition AT %ML112 : LREAL;
    P1.CoarseFilteredPosition AT %ML120 : LREAL;
    P1.PRequestedTorque AT %ML128 : LREAL;
    P1.PDRequestedTorque AT %ML136 : LREAL;
    P1.PIDRequestedTorque AT %ML144 : LREAL;
    P1.CoarseRequestedTorque AT %ML152 : LREAL;
    P1.CoarsePendingTorque AT %ML160 : LREAL;
    P1.FineUpdatePulse AT %MX172.0 : BOOL;
    P1.CoarseUpdatePulse AT %MX172.1 : BOOL;
    P1.ControllerRevision AT %MD176 : DINT;
    P1.PAppliedTorque AT %QL0 : LREAL;
    P1.PLoadTorque AT %QL8 : LREAL;
    P1.PDAppliedTorque AT %QL16 : LREAL;
    P1.PDLoadTorque AT %QL24 : LREAL;
    P1.PIDAppliedTorque AT %QL32 : LREAL;
    P1.PIDLoadTorque AT %QL40 : LREAL;
    P1.CoarseAppliedTorque AT %QL48 : LREAL;
    P1.CoarseLoadTorque AT %QL56 : LREAL;
END_VAR
END_CONFIGURATION
