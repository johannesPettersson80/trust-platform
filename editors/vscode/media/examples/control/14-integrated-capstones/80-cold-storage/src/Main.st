FUNCTION Lesson80OnThreshold : LREAL
VAR_INPUT
    Setpoint : LREAL;
    HysteresisWidth : LREAL;
END_VAR
Lesson80OnThreshold := Setpoint + HysteresisWidth * LREAL#0.5;
END_FUNCTION

FUNCTION Lesson80OffThreshold : LREAL
VAR_INPUT
    Setpoint : LREAL;
    HysteresisWidth : LREAL;
END_VAR
Lesson80OffThreshold := Setpoint - HysteresisWidth * LREAL#0.5;
END_FUNCTION

FUNCTION Lesson80QualifiedCycleTime : LREAL
VAR_INPUT
    MeasuredCycleTime : LREAL;
END_VAR
IF (MeasuredCycleTime >= LREAL#0.01) AND (MeasuredCycleTime <= LREAL#1.0) THEN
    Lesson80QualifiedCycleTime := MeasuredCycleTime;
ELSE
    Lesson80QualifiedCycleTime := LREAL#0.1;
END_IF;
END_FUNCTION

FUNCTION Lesson80DoorEquilibriumTemperature : LREAL
VAR_INPUT
    ExperimentScan : DINT;
END_VAR
IF (ExperimentScan >= DINT#601) AND (ExperimentScan <= DINT#900) THEN
    Lesson80DoorEquilibriumTemperature := LREAL#6.0;
ELSE
    Lesson80DoorEquilibriumTemperature := LREAL#4.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    AutoMode : BOOL := TRUE;
    ExperimentEnabled : BOOL := FALSE;
    EnableLifecycleChecks : BOOL := FALSE;
    DoorCloseScan : DINT := DINT#900;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : DINT := DINT#0;
    ExperimentElapsed : LREAL := 0.0;
    TemperatureQualityGood : BOOL := TRUE;
    StarterReady : BOOL := TRUE;
    Setpoint : LREAL := 2.0;
    HysteresisWidth : LREAL := 1.0;
    MinimumOnTime : LREAL := 8.0;
    MinimumOffTime : LREAL := 8.0;
    MeasuredCycleTime : LREAL := 0.1;
    ProcessValue : LREAL := 4.0;
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
    DoorEquilibriumTemperature : LREAL := 4.0;
END_VAR

(* Core assessment settings are published after reading the process image.
   The optional later lifecycle exercises are disabled for the core trial. *)
Setpoint := LREAL#2.0;
HysteresisWidth := LREAL#1.0;
MinimumOnTime := LREAL#8.0;
MinimumOffTime := LREAL#8.0;
MeasuredCycleTime := LREAL#0.1;
InputsValid := TRUE;
IF NOT EnableLifecycleChecks THEN
    TemperatureQualityGood := TRUE;
    StarterReady := TRUE;
END_IF;
AutoMode := ExperimentEnabled AND NOT SafetyAbort;
CycleTime := Lesson80QualifiedCycleTime(
    MeasuredCycleTime := MeasuredCycleTime);
RunningNow := CompressorRunning >= LREAL#50.0;

IF NOT Initialized THEN
    (* This teaching reset explicitly qualifies the initial OFF interval.
       Later transitions, including stop, reset the actual state timer. *)
    PreviousRunning := RunningNow;
    DwellElapsed := LREAL#8.0;
    Initialized := TRUE;
END_IF;

(* One reviewed Boolean starts the complete selected-candidate record. The
   project owns the interlock, quality, manual, and restoration sequence. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentScan := DINT#0;
        ExperimentElapsed := LREAL#0.0;
    END_IF;
    ExperimentScan := ExperimentScan + DINT#1;
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    AutoMode := TRUE;
    TemperatureQualityGood := TRUE;
    StarterReady := TRUE;
    IF EnableLifecycleChecks AND (ExperimentScan >= DINT#1001) AND (ExperimentScan <= DINT#1050) THEN
        StarterReady := FALSE;
    END_IF;
    IF EnableLifecycleChecks AND (ExperimentScan >= DINT#1101) AND (ExperimentScan <= DINT#1120) THEN
        TemperatureQualityGood := FALSE;
    ELSIF EnableLifecycleChecks AND (ExperimentScan >= DINT#1121) AND (ExperimentScan <= DINT#1130) THEN
        AutoMode := FALSE;
    END_IF;
    IF EnableLifecycleChecks AND (ExperimentScan >= DINT#1201) AND (ExperimentScan <= DINT#1230) THEN
        AutoMode := FALSE;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentScan := DINT#0;
    ExperimentElapsed := LREAL#0.0;
    AutoMode := FALSE;
    TemperatureQualityGood := TRUE;
    StarterReady := TRUE;
END_IF;

DoorEquilibriumTemperature := LREAL#4.0;
IF ExperimentEnabled AND NOT SafetyAbort
    AND ExperimentScan >= DINT#601 AND ExperimentScan <= DoorCloseScan THEN
    DoorEquilibriumTemperature := LREAL#6.0;
END_IF;

IF RunningNow <> PreviousRunning THEN
    SwitchCount := SwitchCount + DINT#1;
    (* The simulated contact reports the command applied on the previous scan:
       one complete update interval has already elapsed in that state. *)
    DwellElapsed := CycleTime;
    PreviousRunning := RunningNow;
ELSE
    DwellElapsed := DwellElapsed + CycleTime;
END_IF;

ConfigurationValid := (HysteresisWidth >= LREAL#0.0)
    AND (MinimumOnTime >= LREAL#0.0)
    AND (MinimumOffTime >= LREAL#0.0);
OnThreshold := Lesson80OnThreshold(
    Setpoint := Setpoint, HysteresisWidth := HysteresisWidth);
OffThreshold := Lesson80OffThreshold(
    Setpoint := Setpoint, HysteresisWidth := HysteresisWidth);

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
ExperimentWasEnabled := ExperimentEnabled;
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
    P1.ExperimentEnabled AT %MX16.6 : BOOL;
    P1.SafetyAbort AT %MX16.7 : BOOL;
    P1.InputsValid AT %MX17.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.HysteresisWidth AT %ML8 : LREAL;
    P1.MinimumOnTime AT %ML64 : LREAL;
    P1.MinimumOffTime AT %ML24 : LREAL;
    P1.MeasuredCycleTime AT %ML32 : LREAL;
    P1.DwellElapsed AT %ML40 : LREAL;
    P1.ExperimentElapsed AT %ML56 : LREAL;
    P1.SwitchCount AT %MD48 : DINT;
    P1.CompressorCommand AT %QL0 : LREAL;
    P1.DoorEquilibriumTemperature AT %QL8 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.CompressorRunning AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
