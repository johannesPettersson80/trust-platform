FUNCTION Lesson18LocalLevelNext : LREAL
VAR_INPUT
    CurrentDeviation : LREAL;
    InputDeviation : LREAL;
    Gain : LREAL;
    TimeConstant : LREAL;
    ElapsedTime : LREAL;
END_VAR
Lesson18LocalLevelNext := CurrentDeviation + ElapsedTime
    * (Gain * InputDeviation - CurrentDeviation) / TimeConstant;
END_FUNCTION

FUNCTION Lesson18TransferStepResponse : LREAL
VAR_INPUT
    InputDeviation : LREAL;
    Gain : LREAL;
    TimeConstant : LREAL;
    ElapsedTime : LREAL;
END_VAR
Lesson18TransferStepResponse := Gain * InputDeviation
    * (LREAL#1.0 - EXP(-ElapsedTime / TimeConstant));
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : DINT := 0;
    OperatingLevel : LREAL := 0.25;
    OperatingFlow : LREAL := 0.01;
    InputDeviation : LREAL := 0.0;
    InletFlow : LREAL := 0.01;
    Gain : LREAL := 50.0;
    TimeConstant : LREAL := 25.0;
    ElapsedTime : LREAL := 0.0;
    DifferentialDeviation : LREAL := 0.0;
    TransferDeviation : LREAL := 0.0;
    DifferentialLevel : LREAL := 0.25;
    TransferLevel : LREAL := 0.25;
    NonlinearTankLevel : LREAL := 0.25;
END_VAR

OperatingLevel := LREAL#0.25;
OperatingFlow := LREAL#0.01;
Gain := LREAL#50.0;
TimeConstant := LREAL#25.0;

IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := 0;
        DifferentialDeviation := LREAL#0.0;
    END_IF;
    ExperimentCycle := ExperimentCycle + 1;
    ElapsedTime := DINT_TO_LREAL(ExperimentCycle) * LREAL#0.1;
    InputDeviation := LREAL#0.001;
ELSE
    ExperimentCycle := 0;
    ElapsedTime := LREAL#0.0;
    InputDeviation := LREAL#0.0;
    DifferentialDeviation := LREAL#0.0;
END_IF;
InletFlow := OperatingFlow + InputDeviation;

(* These are two representations of the same local model. The first advances
   the differential equation; the second evaluates its transformed solution. *)
DifferentialDeviation := Lesson18LocalLevelNext(
    CurrentDeviation := DifferentialDeviation,
    InputDeviation := InputDeviation,
    Gain := Gain,
    TimeConstant := TimeConstant,
    ElapsedTime := LREAL#0.1
);
TransferDeviation := Lesson18TransferStepResponse(
    InputDeviation := InputDeviation,
    Gain := Gain,
    TimeConstant := TimeConstant,
    ElapsedTime := ElapsedTime
);
DifferentialLevel := OperatingLevel + DifferentialDeviation;
TransferLevel := OperatingLevel + TransferDeviation;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InletFlow AT %QL0 : LREAL;
    P1.NonlinearTankLevel AT %IL0 : LREAL;
    P1.InputDeviation AT %ML0 : LREAL;
    P1.ElapsedTime AT %ML8 : LREAL;
    P1.DifferentialLevel AT %ML16 : LREAL;
    P1.TransferLevel AT %ML24 : LREAL;
    P1.Gain AT %ML32 : LREAL;
    P1.TimeConstant AT %ML40 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
