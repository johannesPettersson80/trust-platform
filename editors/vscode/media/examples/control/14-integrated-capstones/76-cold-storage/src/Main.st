PROGRAM Main
VAR
    AutoMode : BOOL := TRUE;
    TemperatureQualityGood : BOOL := TRUE;
    StarterReady : BOOL := TRUE;
    Setpoint : LREAL := 2.0;
    HysteresisWidth : LREAL := 1.0;
    MinimumOnTime : LREAL := 5.0;
    MinimumOffTime : LREAL := 5.0;
    MeasuredCycleTime : LREAL := 0.1;
    ProcessValue : LREAL := 2.0;
    CoolingRequested : BOOL := FALSE;
    CompressorCommand : LREAL := 0.0;
    CompressorRunning : LREAL := 0.0;
    DwellElapsed : LREAL := 0.0;
    DwellInhibit : BOOL := FALSE;
    SwitchCount : DINT := DINT#0;
    QualityFaultLatched : BOOL := FALSE;
    Initialized : BOOL := FALSE;
    PreviousRunning : BOOL := FALSE;
    RunningNow : BOOL := FALSE;
    CycleTime : LREAL := 0.1;
    OnThreshold : LREAL := 2.5;
    OffThreshold : LREAL := 1.5;
    ConfigurationValid : BOOL := TRUE;
END_VAR

IF (MeasuredCycleTime >= LREAL#0.01) AND (MeasuredCycleTime <= LREAL#1.0) THEN
    CycleTime := MeasuredCycleTime;
ELSE
    CycleTime := LREAL#0.1;
END_IF;
RunningNow := CompressorRunning >= LREAL#50.0;

IF NOT Initialized THEN
    (* Starter protection owns real restart history. The PLC begins with no
       assumed dwell credit and observes the measured compressor state. *)
    PreviousRunning := RunningNow;
    DwellElapsed := LREAL#0.0;
    Initialized := TRUE;
END_IF;

IF RunningNow <> PreviousRunning THEN
    SwitchCount := SwitchCount + DINT#1;
    DwellElapsed := LREAL#0.0;
    PreviousRunning := RunningNow;
ELSE
    DwellElapsed := DwellElapsed + CycleTime;
END_IF;

ConfigurationValid := (HysteresisWidth >= LREAL#0.0)
    AND (MinimumOnTime >= LREAL#0.0)
    AND (MinimumOffTime >= LREAL#0.0);
OnThreshold := Setpoint + HysteresisWidth * LREAL#0.5;
OffThreshold := Setpoint - HysteresisWidth * LREAL#0.5;

IF NOT TemperatureQualityGood THEN
    QualityFaultLatched := TRUE;
ELSIF NOT AutoMode THEN
    (* A valid manual acknowledgement is required before automatic recovery. *)
    QualityFaultLatched := FALSE;
END_IF;

IF AutoMode AND TemperatureQualityGood AND NOT QualityFaultLatched
    AND ConfigurationValid
THEN
    IF ProcessValue >= OnThreshold THEN
        CoolingRequested := TRUE;
    ELSIF ProcessValue <= OffThreshold THEN
        CoolingRequested := FALSE;
    END_IF;
ELSE
    CoolingRequested := FALSE;
END_IF;

DwellInhibit := FALSE;
IF NOT AutoMode OR NOT TemperatureQualityGood OR QualityFaultLatched
    OR NOT StarterReady OR NOT ConfigurationValid
THEN
    CompressorCommand := LREAL#0.0;
ELSIF CoolingRequested THEN
    IF RunningNow OR (DwellElapsed >= MinimumOffTime - LREAL#1.0e-9) THEN
        CompressorCommand := LREAL#100.0;
    ELSE
        CompressorCommand := LREAL#0.0;
        DwellInhibit := TRUE;
    END_IF;
ELSE
    IF RunningNow AND (DwellElapsed < MinimumOnTime - LREAL#1.0e-9) THEN
        CompressorCommand := LREAL#100.0;
        DwellInhibit := TRUE;
    ELSE
        CompressorCommand := LREAL#0.0;
    END_IF;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.TemperatureQualityGood AT %MX16.1 : BOOL;
    P1.StarterReady AT %MX16.2 : BOOL;
    P1.CoolingRequested AT %MX16.3 : BOOL;
    P1.DwellInhibit AT %MX16.4 : BOOL;
    P1.QualityFaultLatched AT %MX16.5 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.HysteresisWidth AT %ML8 : LREAL;
    P1.MinimumOnTime AT %ML16 : LREAL;
    P1.MinimumOffTime AT %ML24 : LREAL;
    P1.MeasuredCycleTime AT %ML32 : LREAL;
    P1.DwellElapsed AT %ML40 : LREAL;
    P1.SwitchCount AT %MD48 : DINT;
    P1.CompressorCommand AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.CompressorRunning AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
