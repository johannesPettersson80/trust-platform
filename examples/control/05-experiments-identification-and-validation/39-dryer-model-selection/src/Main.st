PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentInput : LREAL := 30.0;
    ProcessValue : LREAL := 80.0;
    ManipulatedValue : LREAL := 30.0;
    Initialized : BOOL := FALSE;
END_VAR

(* Mapped memory starts at zero in the real runtime. Establish the declared
   30% operating point once, then leave the writable experiment input alone. *)
IF NOT Initialized THEN
    ExperimentInput := LREAL#30.0;
    Initialized := TRUE;
END_IF;

InputsValid := ProcessValue >= LREAL#75.0 AND ProcessValue <= LREAL#90.0;
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentInput := LREAL#40.0;
ELSE
    ExperimentInput := LREAL#30.0;
END_IF;

(* Keep the identification experiment open loop. The learner chooses the
   reviewed manual demand; this program only enforces the declared envelope. *)
IF ExperimentInput < LREAL#20.0 THEN
    ManipulatedValue := LREAL#20.0;
ELSIF ExperimentInput > LREAL#50.0 THEN
    ManipulatedValue := LREAL#50.0;
ELSE
    ManipulatedValue := ExperimentInput;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX8.0 : BOOL;
    P1.SafetyAbort AT %MX8.1 : BOOL;
    P1.InputsValid AT %MX8.2 : BOOL;
    P1.ExperimentInput AT %ML0 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
