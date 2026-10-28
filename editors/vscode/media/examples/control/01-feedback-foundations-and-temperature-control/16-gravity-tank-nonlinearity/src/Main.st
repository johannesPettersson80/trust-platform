(* Gravity outflow through a fixed opening: q_out = c·√h, for a level h ≥ 0 in m and c in m^(5/2)/s. *)
FUNCTION GravityOutflow : LREAL
VAR_INPUT
    Coefficient : LREAL;
    Level : LREAL;
END_VAR
GravityOutflow := Coefficient * SQRT(Level);
END_FUNCTION

(* Equilibrium level: q_in = c·√h_eq gives h_eq = (q_in/c)². *)
FUNCTION EquilibriumLevel : LREAL
VAR_INPUT
    InletFlow : LREAL;
    Coefficient : LREAL;
END_VAR
EquilibriumLevel := (InletFlow / Coefficient) * (InletFlow / Coefficient);
END_FUNCTION

PROGRAM Main
VAR
    (* The tank of Lesson 43, now draining through a fixed opening: A = 0.5 m², c = 0.020 m^(5/2)/s. The
       inlet pump delivers the commanded flow in m³/s. *)
    TankAreaM2 : LREAL := 0.5;
    OutletCoefficient : LREAL := 0.020;
    InletFlowM3S : LREAL := 0.010;
    (* The measured level, in m. The program only reads it. *)
    LevelM : LREAL := 0.0;
    (* The operating level is the level found at startup, set under Starting conditions in the hardware canvas.
       The inlet pump holds the flow that balances it, c·√h0, so the tank stays there until the experiment. *)
    OperatingLevelM : LREAL := 0.25;
    HoldInflowM3S : LREAL := 0.010;
    (* Two models, advanced every 1 s update with the inflow applied during that second and started from the
       measured level: gravity outflow, and Lesson 43's constant outflow, fixed at the holding flow. *)
    GravityModelLevelM : LREAL := 0.0;
    GravityModelOutflowM3S : LREAL := 0.0;
    ConstantOutflowModelLevelM : LREAL := 0.0;
    EquilibriumLevelM : LREAL := 0.25;
    NegativeModelLevel : BOOL := FALSE;
    ModelStarted : BOOL := FALSE;
    AppliedInletM3S : LREAL := 0.010;
    (* Control Studio's project experiment: 0.001 m³/s more than the holding flow while it runs. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
END_VAR

IF NOT ModelStarted THEN
    OperatingLevelM := LevelM;
    HoldInflowM3S := GravityOutflow(Coefficient := OutletCoefficient, Level := OperatingLevelM);
    GravityModelLevelM := LevelM;
    ConstantOutflowModelLevelM := LevelM;
    ModelStarted := TRUE;
ELSIF NOT NegativeModelLevel THEN
    GravityModelLevelM := GravityModelLevelM + (AppliedInletM3S - GravityModelOutflowM3S) / TankAreaM2 * LREAL#1.0;
    ConstantOutflowModelLevelM := ConstantOutflowModelLevelM + (AppliedInletM3S - HoldInflowM3S) / TankAreaM2 * LREAL#1.0;
END_IF;

IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    Active := TRUE;
    (* Each experiment compares from the level measured at its step. *)
    GravityModelLevelM := LevelM;
    ConstantOutflowModelLevelM := LevelM;
ELSIF NOT ExperimentEnabled THEN
    Active := FALSE;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;

(* A negative model level is invalid: the model stops instead of taking the square root of it. *)
IF GravityModelLevelM < LREAL#0.0 THEN
    NegativeModelLevel := TRUE;
ELSIF NOT NegativeModelLevel THEN
    GravityModelOutflowM3S := GravityOutflow(Coefficient := OutletCoefficient, Level := GravityModelLevelM);
END_IF;

IF Active THEN
    InletFlowM3S := HoldInflowM3S + LREAL#0.001;
ELSE
    InletFlowM3S := HoldInflowM3S;
END_IF;
EquilibriumLevelM := EquilibriumLevel(InletFlow := InletFlowM3S, Coefficient := OutletCoefficient);
AppliedInletM3S := InletFlowM3S;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InletFlowM3S AT %QL0 : LREAL;
    P1.LevelM AT %IL0 : LREAL;
    P1.GravityModelLevelM AT %ML0 : LREAL;
    P1.GravityModelOutflowM3S AT %ML8 : LREAL;
    P1.ConstantOutflowModelLevelM AT %ML16 : LREAL;
    P1.EquilibriumLevelM AT %ML32 : LREAL;
    P1.NegativeModelLevel AT %MX24.0 : BOOL;
    P1.ExperimentEnabled AT %MX24.1 : BOOL;
    P1.Active AT %MX24.2 : BOOL;
END_VAR
END_CONFIGURATION
