(* Command of a periodic excitation about an operating point: u = u0 + d·sin(ω·t), with d in percentage points, ω in
   rad/s and t in s. *)
FUNCTION SineCommand : LREAL
VAR_INPUT
    OperatingPct : LREAL;
    AmplitudePct : LREAL;
    OmegaRadS : LREAL;
    ElapsedS : LREAL;
END_VAR
SineCommand := OperatingPct + AmplitudePct * SIN(OmegaRadS * ElapsedS);
END_FUNCTION

(* One heater's excitation: from Start, five periods P = 2π/ω of the sine about 75 %, then 75 % again. A setting
   outside 5–10 points or 0.05–1 rad/s is rejected and excites nothing; the command never leaves 0–100 %. *)
FUNCTION_BLOCK PeriodicExcitation
VAR_INPUT
    Start : BOOL;
    Enable : BOOL;
    AmplitudePct : LREAL;
    OmegaRadS : LREAL;
END_VAR
VAR_OUTPUT
    CommandPct : LREAL := 75.0;
    ElapsedS : LREAL := 0.0;
    Running : BOOL := FALSE;
    Rejected : BOOL := FALSE;
END_VAR
VAR
    OperatingPct : LREAL := 75.0;
    Periods : LREAL := 5.0;
    UpdateS : LREAL := 0.1;
END_VAR
Rejected := AmplitudePct < LREAL#5.0 OR AmplitudePct > LREAL#10.0 OR OmegaRadS < LREAL#0.05 OR OmegaRadS > LREAL#1.0;
IF Start AND NOT Rejected THEN
    Running := TRUE;
    ElapsedS := LREAL#0.0;
END_IF;
IF NOT Enable OR Rejected THEN
    Running := FALSE;
END_IF;
IF Running AND ElapsedS >= Periods * LREAL#2.0 * LREAL#3.14159265358979 / OmegaRadS - LREAL#1.0E-9 THEN
    Running := FALSE;
END_IF;
IF Running THEN
    CommandPct := LIMIT(LREAL#0.0, SineCommand(OperatingPct := OperatingPct, AmplitudePct := AmplitudePct,
        OmegaRadS := OmegaRadS, ElapsedS := ElapsedS), LREAL#100.0);
    ElapsedS := ElapsedS + UpdateS;
ELSE
    CommandPct := OperatingPct;
END_IF;
END_FUNCTION_BLOCK

(* The heater's frequency response G(jω) = 0.2·e^(−jω)/(1 + 5jω): the amplitude ratio M in °C per command point and
   the phase φ in rad. *)
FUNCTION HeaterAmplitudeRatio : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
END_VAR
HeaterAmplitudeRatio := LREAL#0.2 / SQRT(LREAL#1.0 + (LREAL#5.0 * OmegaRadS) ** LREAL#2.0);
END_FUNCTION

FUNCTION HeaterPhaseRad : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
END_VAR
HeaterPhaseRad := -OmegaRadS - ATAN(LREAL#5.0 * OmegaRadS);
END_FUNCTION

PROGRAM Main
VAR
    (* The heater of Lessons 12–30 driven with Lesson 53's sine at 0.2 rad/s, heater control off. *)
    Excitation : PeriodicExcitation;
    AmplitudePct : LREAL := 5.0;
    OmegaRadS : LREAL := 0.2;
    TemperatureC : LREAL := 35.0;
    HeaterCommandPct : LREAL := 75.0;
    (* The model's prediction at OmegaRadS: amplitude ratio, phase and the temperature amplitude M·d. *)
    ModelAmplitudeRatio : LREAL := 0.0;
    ModelPhaseRad : LREAL := 0.0;
    ModelTemperatureAmplitudeC : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Start : BOOL := FALSE;
END_VAR

ModelAmplitudeRatio := HeaterAmplitudeRatio(OmegaRadS := OmegaRadS);
ModelPhaseRad := HeaterPhaseRad(OmegaRadS := OmegaRadS);
ModelTemperatureAmplitudeC := ModelAmplitudeRatio * AmplitudePct;
Start := ExperimentEnabled AND NOT ExperimentWasEnabled;
ExperimentWasEnabled := ExperimentEnabled;
Excitation(Start := Start, Enable := ExperimentEnabled, AmplitudePct := AmplitudePct, OmegaRadS := OmegaRadS);
HeaterCommandPct := Excitation.CommandPct;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.HeaterCommandPct AT %QL0 : LREAL;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.ModelAmplitudeRatio AT %ML0 : LREAL;
    P1.ModelPhaseRad AT %ML8 : LREAL;
    P1.ModelTemperatureAmplitudeC AT %ML16 : LREAL;
END_VAR
END_CONFIGURATION
