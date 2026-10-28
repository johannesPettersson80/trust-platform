USING Control;

(* Slope of the gravity outflow q = c·√h at the level h: g = c/(2·√h), in m²/s. *)
FUNCTION OutflowSlope : LREAL
VAR_INPUT
    Coefficient : LREAL;
    Level : LREAL;
END_VAR
OutflowSlope := Coefficient / (LREAL#2.0 * SQRT(Level));
END_FUNCTION

(* The level loop of one gravity tank: Control.PID as PI, tuned for the response time λ on the local model
   A·dx/dt = v − g·x, so Kp = A/λ and Ki = g/λ. Fixed: Ki for the 0.25 m design level. Scheduled: Ki from the
   measured level at every update. Control.PID adds increments to its retained request, so a new Ki changes
   only the later integral increments and the accumulated holding flow is kept. *)
FUNCTION_BLOCK LevelController
VAR_INPUT
    Scheduled : BOOL;
    SetpointM : LREAL;
    LevelM : LREAL;
    HoldingFlowM3S : LREAL;
END_VAR
VAR_OUTPUT
    InletFlowM3S : LREAL;
    IntegralGain : LREAL;
END_VAR
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#0.05,
        IntegralGain := LREAL#0.002,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#0.020,
        RateMinimum := LREAL#-1.0,
        RateMaximum := LREAL#1.0,
        TrackingTime := LREAL#10.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
    Started : BOOL := FALSE;
    TankAreaM2 : LREAL := 0.5;
    OutletCoefficient : LREAL := 0.020;
    ResponseTimeS : LREAL := 10.0;
    DesignLevelM : LREAL := 0.25;
END_VAR
Parameters.ProportionalGain := TankAreaM2 / ResponseTimeS;
IF NOT Scheduled THEN
    Parameters.IntegralGain := OutflowSlope(Coefficient := OutletCoefficient, Level := DesignLevelM) / ResponseTimeS;
ELSIF LevelM > LREAL#0.0 THEN
    (* No square root of a level that is not positive: the last gain stays. *)
    Parameters.IntegralGain := OutflowSlope(Coefficient := OutletCoefficient, Level := LevelM) / ResponseTimeS;
END_IF;
IntegralGain := Parameters.IntegralGain;
Controller.Configure(Parameters := Parameters);
IF NOT Started THEN
    (* Start in Auto at rest: setpoint and measurement at the present level, the request at the holding inflow. *)
    State.Setpoint := SetpointM;
    State.ProcessValue := LevelM;
    State.NominalOutput := HoldingFlowM3S;
    State.SaturatedOutput := HoldingFlowM3S;
    Controller.Initialize(State := State);
    Started := TRUE;
END_IF;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#1.0;
Input.Setpoint := SetpointM;
Input.ProcessValue := LevelM;
InletFlowM3S := Controller.Update(Input := Input);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Six identical tanks: fixed and scheduled PI at 0.09, 0.25 and 0.49 m, each started at rest with its
       holding inflow c·√h0 = 0.006, 0.010 and 0.014 m³/s. *)
    LowFixed : LevelController;
    LowScheduled : LevelController;
    MidFixed : LevelController;
    MidScheduled : LevelController;
    HighFixed : LevelController;
    HighScheduled : LevelController;
    LowFixedLevelM : LREAL := 0.0;
    LowScheduledLevelM : LREAL := 0.0;
    MidFixedLevelM : LREAL := 0.0;
    MidScheduledLevelM : LREAL := 0.0;
    HighFixedLevelM : LREAL := 0.0;
    HighScheduledLevelM : LREAL := 0.0;
    LowFixedInletM3S : LREAL := 0.006;
    LowScheduledInletM3S : LREAL := 0.006;
    MidFixedInletM3S : LREAL := 0.010;
    MidScheduledInletM3S : LREAL := 0.010;
    HighFixedInletM3S : LREAL := 0.014;
    HighScheduledInletM3S : LREAL := 0.014;
    (* Level change from each starting level, in m, and the scheduled integral gains, in m²/s². *)
    LowFixedChangeM : LREAL := 0.0;
    MidFixedChangeM : LREAL := 0.0;
    HighFixedChangeM : LREAL := 0.0;
    LowScheduledChangeM : LREAL := 0.0;
    MidScheduledChangeM : LREAL := 0.0;
    HighScheduledChangeM : LREAL := 0.0;
    LowScheduledKi : LREAL := 0.0;
    MidScheduledKi : LREAL := 0.0;
    HighScheduledKi : LREAL := 0.0;
    (* Control Studio's project experiment steps every setpoint by 0.02 m while it runs. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
    StepM : LREAL := 0.0;
END_VAR

IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    Active := TRUE;
ELSIF NOT ExperimentEnabled THEN
    Active := FALSE;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
IF Active THEN
    StepM := LREAL#0.02;
ELSE
    StepM := LREAL#0.0;
END_IF;

LowFixed(Scheduled := FALSE, SetpointM := LREAL#0.09 + StepM, LevelM := LowFixedLevelM, HoldingFlowM3S := LREAL#0.006);
LowScheduled(Scheduled := TRUE, SetpointM := LREAL#0.09 + StepM, LevelM := LowScheduledLevelM, HoldingFlowM3S := LREAL#0.006);
MidFixed(Scheduled := FALSE, SetpointM := LREAL#0.25 + StepM, LevelM := MidFixedLevelM, HoldingFlowM3S := LREAL#0.010);
MidScheduled(Scheduled := TRUE, SetpointM := LREAL#0.25 + StepM, LevelM := MidScheduledLevelM, HoldingFlowM3S := LREAL#0.010);
HighFixed(Scheduled := FALSE, SetpointM := LREAL#0.49 + StepM, LevelM := HighFixedLevelM, HoldingFlowM3S := LREAL#0.014);
HighScheduled(Scheduled := TRUE, SetpointM := LREAL#0.49 + StepM, LevelM := HighScheduledLevelM, HoldingFlowM3S := LREAL#0.014);
LowFixedInletM3S := LowFixed.InletFlowM3S;
LowScheduledInletM3S := LowScheduled.InletFlowM3S;
MidFixedInletM3S := MidFixed.InletFlowM3S;
MidScheduledInletM3S := MidScheduled.InletFlowM3S;
HighFixedInletM3S := HighFixed.InletFlowM3S;
HighScheduledInletM3S := HighScheduled.InletFlowM3S;

LowFixedChangeM := LowFixedLevelM - LREAL#0.09;
MidFixedChangeM := MidFixedLevelM - LREAL#0.25;
HighFixedChangeM := HighFixedLevelM - LREAL#0.49;
LowScheduledChangeM := LowScheduledLevelM - LREAL#0.09;
MidScheduledChangeM := MidScheduledLevelM - LREAL#0.25;
HighScheduledChangeM := HighScheduledLevelM - LREAL#0.49;
LowScheduledKi := LowScheduled.IntegralGain;
MidScheduledKi := MidScheduled.IntegralGain;
HighScheduledKi := HighScheduled.IntegralGain;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.LowFixedInletM3S AT %QL0 : LREAL;
    P1.LowScheduledInletM3S AT %QL8 : LREAL;
    P1.MidFixedInletM3S AT %QL16 : LREAL;
    P1.MidScheduledInletM3S AT %QL24 : LREAL;
    P1.HighFixedInletM3S AT %QL32 : LREAL;
    P1.HighScheduledInletM3S AT %QL40 : LREAL;
    P1.LowFixedLevelM AT %IL0 : LREAL;
    P1.LowScheduledLevelM AT %IL8 : LREAL;
    P1.MidFixedLevelM AT %IL16 : LREAL;
    P1.MidScheduledLevelM AT %IL24 : LREAL;
    P1.HighFixedLevelM AT %IL32 : LREAL;
    P1.HighScheduledLevelM AT %IL40 : LREAL;
    P1.LowFixedChangeM AT %ML0 : LREAL;
    P1.MidFixedChangeM AT %ML8 : LREAL;
    P1.HighFixedChangeM AT %ML16 : LREAL;
    P1.LowScheduledChangeM AT %ML24 : LREAL;
    P1.MidScheduledChangeM AT %ML32 : LREAL;
    P1.HighScheduledChangeM AT %ML40 : LREAL;
    P1.LowScheduledKi AT %ML48 : LREAL;
    P1.MidScheduledKi AT %ML56 : LREAL;
    P1.HighScheduledKi AT %ML64 : LREAL;
    P1.ExperimentEnabled AT %MX72.0 : BOOL;
    P1.Active AT %MX72.1 : BOOL;
END_VAR
END_CONFIGURATION
