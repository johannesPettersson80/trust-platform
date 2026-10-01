(* The steady gain matrix K = [[A, B], [C, D]] in °C per point; its determinant is A·D − B·C. *)
FUNCTION Determinant : LREAL
VAR_INPUT
    A : LREAL;
    B : LREAL;
    C : LREAL;
    D : LREAL;
END_VAR
Determinant := A * D - B * C;
END_FUNCTION

(* The correction of input 2 that holds output 2 while input 1 changes by V1Pct: C·v1 + D·v2 = 0 gives v2 = −(C/D)·v1.
   0.0 when D = 0: input 2 cannot hold output 2. *)
FUNCTION CompensatingCommandPct : LREAL
VAR_INPUT
    V1Pct : LREAL;
    C : LREAL;
    D : LREAL;
END_VAR
IF ABS(D) > LREAL#1.0E-12 THEN
    CompensatingCommandPct := -C / D * V1Pct;
ELSE
    CompensatingCommandPct := LREAL#0.0;
END_IF;
END_FUNCTION

(* Path 1 → 1 with output 2 held: x1 = A·v1 + B·v2 with v2 = −(C/D)·v1 gives (A − B·C/D)·v1 = (det/D)·v1.
   0.0 when D = 0. *)
FUNCTION HeldOutputGain : LREAL
VAR_INPUT
    A : LREAL;
    B : LREAL;
    C : LREAL;
    D : LREAL;
END_VAR
IF ABS(D) > LREAL#1.0E-12 THEN
    HeldOutputGain := A - B * C / D;
ELSE
    HeldOutputGain := LREAL#0.0;
END_IF;
END_FUNCTION

(* Λ = K ∘ (K⁻¹)ᵀ, element by element: λ11 = A·(D/det), the isolated gain over the held-output gain, and
   λ12 = B·(−C/det). 0.0 when det = 0: K has no inverse and the array is undefined. *)
FUNCTION RelativeGain11 : LREAL
VAR_INPUT
    A : LREAL;
    B : LREAL;
    C : LREAL;
    D : LREAL;
END_VAR
IF ABS(Determinant(A := A, B := B, C := C, D := D)) > LREAL#1.0E-12 THEN
    RelativeGain11 := A * (D / Determinant(A := A, B := B, C := C, D := D));
ELSE
    RelativeGain11 := LREAL#0.0;
END_IF;
END_FUNCTION

FUNCTION RelativeGain12 : LREAL
VAR_INPUT
    A : LREAL;
    B : LREAL;
    C : LREAL;
    D : LREAL;
END_VAR
IF ABS(Determinant(A := A, B := B, C := C, D := D)) > LREAL#1.0E-12 THEN
    RelativeGain12 := B * (-C / Determinant(A := A, B := B, C := C, D := D));
ELSE
    RelativeGain12 := LREAL#0.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    (* Two copies of the coupled vessels of Lesson 69, no controller. The first pair takes isolated 5-point steps; in
       the second, the other heater is corrected at the same time so that its vessel returns to 35 °C. *)
    Heater1Pct : LREAL := 75.0;
    Heater2Pct : LREAL := 75.0;
    Vessel1C : LREAL := 35.0;
    Vessel2C : LREAL := 35.0;
    Heater3Pct : LREAL := 75.0;
    Heater4Pct : LREAL := 75.0;
    Vessel3C : LREAL := 35.0;
    Vessel4C : LREAL := 35.0;
    (* Control Studio's project experiment steps heater 1 and heater 3 for 30 s, returns them for 30 s, then steps
       heater 2 and heater 4. *)
    ElapsedS : LREAL := 0.0;
    CorrectionPct : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
END_VAR
Heater1Pct := LREAL#75.0;
Heater2Pct := LREAL#75.0;
Heater3Pct := LREAL#75.0;
Heater4Pct := LREAL#75.0;
CorrectionPct := LREAL#0.0;
IF ExperimentEnabled THEN
    IF ElapsedS < LREAL#29.95 THEN
        CorrectionPct := CompensatingCommandPct(V1Pct := LREAL#5.0, C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0);
        Heater1Pct := LREAL#80.0;
        Heater3Pct := LREAL#80.0;
        Heater4Pct := LREAL#75.0 + CorrectionPct;
    ELSIF ElapsedS >= LREAL#59.95 THEN
        CorrectionPct := CompensatingCommandPct(V1Pct := LREAL#5.0, C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0);
        Heater2Pct := LREAL#80.0;
        Heater4Pct := LREAL#80.0;
        Heater3Pct := LREAL#75.0 + CorrectionPct;
    END_IF;
    ElapsedS := ElapsedS + LREAL#0.1;
ELSE
    ElapsedS := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Heater1Pct AT %QL0 : LREAL;
    P1.Heater2Pct AT %QL8 : LREAL;
    P1.Vessel1C AT %IL0 : LREAL;
    P1.Vessel2C AT %IL8 : LREAL;
    P1.Heater3Pct AT %QL16 : LREAL;
    P1.Heater4Pct AT %QL24 : LREAL;
    P1.Vessel3C AT %IL16 : LREAL;
    P1.Vessel4C AT %IL24 : LREAL;
    P1.CorrectionPct AT %ML0 : LREAL;
    P1.ExperimentEnabled AT %MX8.0 : BOOL;
END_VAR
END_CONFIGURATION
