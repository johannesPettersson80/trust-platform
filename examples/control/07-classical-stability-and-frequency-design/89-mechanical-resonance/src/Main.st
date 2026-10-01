USING Control;

(* The loop gain |C(jω)|·|G(jω)| at the resonance: the rigid axis 1 / (s(Js + B)) times the peak factor 1 / (2ζ). *)
FUNCTION LoopGainAtResonance : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    IntegralGain : LREAL;
    DerivativeGain : LREAL;
    Inertia : LREAL;
    Friction : LREAL;
    ResonanceRadS : LREAL;
    DampingRatio : LREAL;
END_VAR
LoopGainAtResonance := SQRT(ProportionalGain * ProportionalGain
        + (DerivativeGain * ResonanceRadS - IntegralGain / ResonanceRadS) * (DerivativeGain * ResonanceRadS - IntegralGain / ResonanceRadS))
    / (ResonanceRadS * SQRT(Friction * Friction + Inertia * Inertia * ResonanceRadS * ResonanceRadS))
    / (LREAL#2.0 * DampingRatio);
END_FUNCTION

(* The notch's gain |N(jω)| = |ω_n² − ω²| / √((ω_n² − ω²)² + (ω·ω_n/Q)²). *)
FUNCTION NotchGain : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
    NotchRadS : LREAL;
    Quality : LREAL;
END_VAR
NotchGain := ABS(NotchRadS * NotchRadS - OmegaRadS * OmegaRadS)
    / SQRT((NotchRadS * NotchRadS - OmegaRadS * OmegaRadS) * (NotchRadS * NotchRadS - OmegaRadS * OmegaRadS)
        + (OmegaRadS * NotchRadS / Quality) * (OmegaRadS * NotchRadS / Quality));
END_FUNCTION

(* The phase lag the notch adds below its frequency, in degrees: atan((ω·ω_n/Q) / (ω_n² − ω²)). *)
FUNCTION NotchPhaseLagDeg : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
    NotchRadS : LREAL;
    Quality : LREAL;
END_VAR
NotchPhaseLagDeg := ATAN((OmegaRadS * NotchRadS / Quality) / (NotchRadS * NotchRadS - OmegaRadS * OmegaRadS))
    * LREAL#180.0 / LREAL#3.141592653589793;
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    (* The PID Control Studio recommended for the rigid axis in Lesson 42. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#18.77,
        IntegralGain := LREAL#17.46,
        DerivativeGain := LREAL#7.27,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#-20.0,
        OutputMaximum := LREAL#20.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualTorqueNm : LREAL := 0.0;
    SafetyAbort : BOOL := FALSE;
    SetpointRad : LREAL := 0.0;
    (* The load encoder, and the notch's output that the PID measures. *)
    AngleRad : LREAL := 0.0;
    FilteredAngleRad : LREAL := 0.0;
    TorqueNm : LREAL := 0.0;
    (* Control Studio's Tune sets the notch's frequency and quality factor; 0 Hz switches it off. *)
    NotchFrequencyHz : LREAL := 0.0;
    NotchQuality : LREAL := 1.0;
    NotchOmega : LREAL;
    NotchAlpha : LREAL;
    B0 : LREAL := 1.0;
    B1 : LREAL := 0.0;
    B2 : LREAL := 0.0;
    A1 : LREAL := 0.0;
    A2 : LREAL := 0.0;
    PreviousAngle1 : LREAL := 0.0;
    PreviousAngle2 : LREAL := 0.0;
    PreviousFiltered1 : LREAL := 0.0;
    PreviousFiltered2 : LREAL := 0.0;
    (* Frequency-response experiment in Manual: seven 1 N·m tones, each a whole number of periods in 2 s. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentTimeS : LREAL := 0.0;
    ToneHz : ARRAY[1..7] OF LREAL := [1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0];
    ToneAmplitudeNm : LREAL := 1.0;
    ExcitationNm : LREAL := 0.0;
    Tone : INT;
    CycleTimeS : LREAL := 0.02;
    AngleLimitRad : LREAL := 1.8;
END_VAR

(* Leaving the travel range removes the torque until a full restart. *)
IF ABS(AngleRad) > AngleLimitRad THEN
    SafetyAbort := TRUE;
END_IF;

(* The measurement notch as a digital filter; its coefficients follow from its frequency and quality factor. *)
IF NotchFrequencyHz > LREAL#0.0 THEN
    NotchOmega := LREAL#2.0 * LREAL#3.141592653589793 * NotchFrequencyHz * CycleTimeS;
    NotchAlpha := SIN(NotchOmega) / (LREAL#2.0 * NotchQuality);
    B0 := LREAL#1.0 / (LREAL#1.0 + NotchAlpha);
    B1 := LREAL#-2.0 * COS(NotchOmega) * B0;
    B2 := B0;
    A1 := B1;
    A2 := (LREAL#1.0 - NotchAlpha) * B0;
ELSE
    B0 := LREAL#1.0;
    B1 := LREAL#0.0;
    B2 := LREAL#0.0;
    A1 := LREAL#0.0;
    A2 := LREAL#0.0;
END_IF;
FilteredAngleRad := B0 * AngleRad + B1 * PreviousAngle1 + B2 * PreviousAngle2 - A1 * PreviousFiltered1 - A2 * PreviousFiltered2;
PreviousAngle2 := PreviousAngle1;
PreviousAngle1 := AngleRad;
PreviousFiltered2 := PreviousFiltered1;
PreviousFiltered1 := FilteredAngleRad;

ExcitationNm := LREAL#0.0;
IF ExperimentEnabled AND NOT AutoMode AND NOT SafetyAbort THEN
    FOR Tone := 1 TO 7 DO
        ExcitationNm := ExcitationNm + ToneAmplitudeNm * SIN(LREAL#2.0 * LREAL#3.141592653589793 * ToneHz[Tone] * ExperimentTimeS);
    END_FOR;
    ExperimentTimeS := ExperimentTimeS + CycleTimeS;
ELSE
    ExperimentTimeS := LREAL#0.0;
END_IF;

Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    (* Hot reset: the axis rests without torque and the setpoint follows the measured angle. *)
    ManualTorqueNm := LREAL#0.0;
    IF NOT AutoMode THEN
        SetpointRad := AngleRad;
    END_IF;
    InitialState.Setpoint := SetpointRad;
    InitialState.ProcessValue := FilteredAngleRad;
    InitialState.NominalOutput := ManualTorqueNm;
    InitialState.SaturatedOutput := ManualTorqueNm;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := SetpointRad;
Input.ProcessValue := FilteredAngleRad;
Input.CycleTime := CycleTimeS;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := ManualTorqueNm + ExcitationNm;
IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
ELSIF AutoMode THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualTorqueNm;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualTorqueNm + ExcitationNm;
END_IF;
TorqueNm := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX96.0 : BOOL;
    P1.ExperimentEnabled AT %MX96.1 : BOOL;
    P1.TorqueNm AT %QL0 : LREAL;
    P1.AngleRad AT %IL0 : LREAL;
    P1.SetpointRad AT %ML0 : LREAL;
    P1.ManualTorqueNm AT %ML8 : LREAL;
END_VAR
END_CONFIGURATION
