(* Volume balance of a constant-area tank: A·dh/dt = q_in − q_out, so the level changes at (q_in − q_out)/A. *)
FUNCTION LevelRate : LREAL
VAR_INPUT
    InletFlow : LREAL;
    OutletFlow : LREAL;
    Area : LREAL;
END_VAR
LevelRate := (InletFlow - OutletFlow) / Area;
END_FUNCTION

PROGRAM Main
VAR
    (* The tank rig: a 0.5 m² cross-section, an outlet pump that removes 0.010 m³/s and an inlet pump that
       delivers the commanded flow, both in m³/s. *)
    TankAreaM2 : LREAL := 0.5;
    InletFlowM3S : LREAL := 0.010;
    OutletFlowM3S : LREAL := 0.010;
    (* The measured level, in m. The program only reads it. *)
    LevelM : LREAL := 0.0;
    (* The model: the volume balance advanced every 1 s update with the flows applied during that second,
       started from the measured level. It never replaces the measurement. *)
    ModelLevelM : LREAL := 0.0;
    ModelLevelRateMS : LREAL := 0.0;
    AppliedInletM3S : LREAL := 0.010;
    AppliedOutletM3S : LREAL := 0.010;
    ModelStarted : BOOL := FALSE;
    (* Control Studio's project experiment: 0.012 m³/s for 10 s, then 0.010 m³/s again. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
    ExperimentSeconds : DINT := 0;
END_VAR

(* The model starts from the first measured level and then advances one update with the flows applied over
   the last second. *)
IF ModelStarted THEN
    ModelLevelRateMS := LevelRate(InletFlow := AppliedInletM3S, OutletFlow := AppliedOutletM3S, Area := TankAreaM2);
    ModelLevelM := ModelLevelM + ModelLevelRateMS * LREAL#1.0;
ELSE
    ModelLevelM := LevelM;
    ModelStarted := TRUE;
END_IF;

IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    Active := TRUE;
    ExperimentSeconds := 0;
    (* Each experiment compares from the level measured at its step. *)
    ModelLevelM := LevelM;
    ModelLevelRateMS := LREAL#0.0;
ELSIF NOT ExperimentEnabled THEN
    Active := FALSE;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;

IF Active AND ExperimentSeconds < 10 THEN
    InletFlowM3S := LREAL#0.012;
ELSE
    InletFlowM3S := LREAL#0.010;
END_IF;
IF Active THEN
    ExperimentSeconds := ExperimentSeconds + 1;
END_IF;
OutletFlowM3S := LREAL#0.010;
AppliedInletM3S := InletFlowM3S;
AppliedOutletM3S := OutletFlowM3S;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InletFlowM3S AT %QL0 : LREAL;
    P1.OutletFlowM3S AT %QL8 : LREAL;
    P1.LevelM AT %IL0 : LREAL;
    P1.ModelLevelM AT %ML0 : LREAL;
    P1.ModelLevelRateMS AT %ML8 : LREAL;
    P1.ExperimentEnabled AT %MX16.0 : BOOL;
    P1.Active AT %MX16.1 : BOOL;
END_VAR
END_CONFIGURATION
