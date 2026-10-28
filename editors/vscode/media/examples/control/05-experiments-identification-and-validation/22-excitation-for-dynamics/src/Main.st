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

PROGRAM Main
VAR
    (* Three copies of the heater at 35 °C, excited with the same amplitude at three rates. The heater control is
       off: the program sets the heater command directly. *)
    MiddleExcitation : PeriodicExcitation;
    SlowExcitation : PeriodicExcitation;
    FastExcitation : PeriodicExcitation;
    AmplitudePct : LREAL := 5.0;
    MiddleOmegaRadS : LREAL := 0.2;
    SlowOmegaRadS : LREAL := 0.1;
    FastOmegaRadS : LREAL := 0.4;
    MiddleTemperatureC : LREAL := 35.0;
    SlowTemperatureC : LREAL := 35.0;
    FastTemperatureC : LREAL := 35.0;
    MiddleCommandPct : LREAL := 75.0;
    SlowCommandPct : LREAL := 75.0;
    FastCommandPct : LREAL := 75.0;
    (* Control Studio's project experiment starts all three excitations at once. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Start : BOOL := FALSE;
END_VAR

Start := ExperimentEnabled AND NOT ExperimentWasEnabled;
ExperimentWasEnabled := ExperimentEnabled;
MiddleExcitation(Start := Start, Enable := ExperimentEnabled, AmplitudePct := AmplitudePct, OmegaRadS := MiddleOmegaRadS);
SlowExcitation(Start := Start, Enable := ExperimentEnabled, AmplitudePct := AmplitudePct, OmegaRadS := SlowOmegaRadS);
FastExcitation(Start := Start, Enable := ExperimentEnabled, AmplitudePct := AmplitudePct, OmegaRadS := FastOmegaRadS);
MiddleCommandPct := MiddleExcitation.CommandPct;
SlowCommandPct := SlowExcitation.CommandPct;
FastCommandPct := FastExcitation.CommandPct;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.MiddleCommandPct AT %QL0 : LREAL;
    P1.SlowCommandPct AT %QL8 : LREAL;
    P1.FastCommandPct AT %QL16 : LREAL;
    P1.MiddleTemperatureC AT %IL0 : LREAL;
    P1.SlowTemperatureC AT %IL8 : LREAL;
    P1.FastTemperatureC AT %IL16 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
END_VAR
END_CONFIGURATION
