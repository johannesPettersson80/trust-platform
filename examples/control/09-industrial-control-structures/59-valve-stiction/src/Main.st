(* Positioner drive force: F = ks·(ut − x), in N for ks in N per point. *)
FUNCTION DriveForce : LREAL
VAR_INPUT
    Stiffness : LREAL;
    Target : LREAL;
    Stem : LREAL;
END_VAR
DriveForce := Stiffness * (Target - Stem);
END_FUNCTION

(* Command deadband: a new target is accepted only if it differs from the last accepted target by at
   least the deadband; otherwise the last accepted target stays. *)
FUNCTION AcceptedTarget : LREAL
VAR_INPUT
    Request : LREAL;
    LastAccepted : LREAL;
    Deadband : LREAL;
END_VAR
IF ABS(Request - LastAccepted) >= Deadband THEN
    AcceptedTarget := Request;
ELSE
    AcceptedTarget := LastAccepted;
END_IF;
END_FUNCTION

(* Where a sliding stem stops: the position at which the remaining force equals the moving friction. *)
FUNCTION SlidingStop : LREAL
VAR_INPUT
    Target : LREAL;
    Force : LREAL;
    MovingFriction : LREAL;
    Stiffness : LREAL;
END_VAR
IF Force > LREAL#0.0 THEN
    SlidingStop := Target - MovingFriction / Stiffness;
ELSE
    SlidingStop := Target + MovingFriction / Stiffness;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
    ValveRequestPct : LREAL := 75.0;
    (* Probes: increasing 0.5-point steps to 80.5 %, a hold, decreasing steps to 75 %, then a reversal
       to 72 % and back to 75 %. Times are seconds after the start. *)
    ProbeTimes : ARRAY[0..23] OF LREAL := [0.0, 4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 28.0, 32.0, 36.0, 40.0,
        52.0, 56.0, 60.0, 64.0, 68.0, 72.0, 76.0, 80.0, 84.0, 88.0, 92.0, 96.0, 112.0];
    ProbeTargets : ARRAY[0..23] OF LREAL := [75.5, 76.0, 76.5, 77.0, 77.5, 78.0, 78.5, 79.0, 79.5, 80.0, 80.5,
        80.0, 79.5, 79.0, 78.5, 78.0, 77.5, 77.0, 76.5, 76.0, 75.5, 75.0, 72.0, 75.0];
    ProbeEndS : LREAL := 128.0;
    ProbeTimer : TON;
    ElapsedS : LREAL := 0.0;
    Index : INT := 0;
    (* Valve 1: its positioner ignores request changes smaller than the command deadband. *)
    CommandDeadbandPct : LREAL := 1.0;
    DeadbandTargetPct : LREAL := 75.0;
    DeadbandStemPct : LREAL := 75.0;
    DeadbandForceN : LREAL := 0.0;
    DeadbandFlowKgS : LREAL := 1.5;
    (* Valve 2: accepts every request, but friction holds the stem. Simulated valve mechanics: the
       stem is held while |F| is at most the breakaway force; once sliding it moves until |F| falls
       to the moving friction. *)
    StiffnessNPerPct : LREAL := 10.0;
    BreakawayForceN : LREAL := 50.0;
    MovingFrictionN : LREAL := 20.0;
    StickyForceN : LREAL := 0.0;
    Sliding : BOOL := FALSE;
    StickyTargetPct : LREAL := 75.0;
    StickyStemPct : LREAL := 75.0;
    StickyFlowKgS : LREAL := 1.5;
END_VAR

IF ExperimentEnabled AND NOT WasEnabled THEN
    Active := TRUE;
    ProbeTimer(IN := FALSE, PT := T#1h);
END_IF;
IF NOT ExperimentEnabled THEN
    Active := FALSE;
END_IF;
WasEnabled := ExperimentEnabled;

IF Active THEN
    ProbeTimer(IN := TRUE, PT := T#1h);
    ElapsedS := UDINT_TO_LREAL(DWORD_TO_UDINT(TIME_TO_DWORD(ProbeTimer.ET))) / LREAL#1000.0;
    FOR Index := 0 TO 23 DO
        IF ElapsedS >= ProbeTimes[Index] THEN
            ValveRequestPct := ProbeTargets[Index];
        END_IF;
    END_FOR;
    IF ElapsedS >= ProbeEndS THEN
        Active := FALSE;
    END_IF;
END_IF;
IF NOT Active THEN
    ValveRequestPct := LREAL#75.0;
    ProbeTimer(IN := FALSE, PT := T#1h);
END_IF;

(* Valve 1: command path with deadband; the healthy stem follows the accepted target. *)
DeadbandTargetPct := AcceptedTarget(Request := ValveRequestPct, LastAccepted := DeadbandTargetPct,
    Deadband := CommandDeadbandPct);
DeadbandForceN := DriveForce(Stiffness := StiffnessNPerPct, Target := DeadbandTargetPct, Stem := DeadbandStemPct);

(* Valve 2: the accepted target is the request; friction decides where the stem may go. *)
StickyForceN := DriveForce(Stiffness := StiffnessNPerPct, Target := ValveRequestPct, Stem := StickyStemPct);
IF NOT Sliding AND ABS(StickyForceN) > BreakawayForceN THEN
    Sliding := TRUE;
END_IF;
IF Sliding AND ABS(StickyForceN) <= MovingFrictionN THEN
    Sliding := FALSE;
END_IF;
IF Sliding THEN
    StickyTargetPct := SlidingStop(Target := ValveRequestPct, Force := StickyForceN,
        MovingFriction := MovingFrictionN, Stiffness := StiffnessNPerPct);
ELSE
    StickyTargetPct := StickyStemPct;
END_IF;

END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX64.0 : BOOL;
    P1.Active AT %MX64.1 : BOOL;
    P1.ValveRequestPct AT %QL0 : LREAL;
    P1.DeadbandTargetPct AT %QL8 : LREAL;
    P1.StickyTargetPct AT %QL16 : LREAL;
    P1.DeadbandFlowKgS AT %IL0 : LREAL;
    P1.StickyFlowKgS AT %IL8 : LREAL;
    P1.DeadbandStemPct AT %IL16 : LREAL;
    P1.StickyStemPct AT %IL24 : LREAL;
    P1.DeadbandForceN AT %ML0 : LREAL;
    P1.StickyForceN AT %ML8 : LREAL;
END_VAR
END_CONFIGURATION
