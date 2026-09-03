USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    AutoMode : BOOL := TRUE;
    RunApproved : BOOL := FALSE;
    GenerateCandidate : BOOL := FALSE;
    ValidationPassed : BOOL := FALSE;
    ApplyApproved : BOOL := FALSE;
    RollbackRequested : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ManualMode : BOOL := TRUE;
    AcceptedModelId : INT := 17;
    AcceptedModelPurpose : BOOL := TRUE;
    ModelWithinValidityRange : BOOL := TRUE;
    RequirementsFeasible : BOOL := TRUE;
    TuningSelection : INT := 1;
    TuningValid : BOOL := TRUE;
    CandidateValidated : BOOL := FALSE;
    CandidateApplied : BOOL := FALSE;
    ApplyRejected : BOOL := FALSE;
    CandidateActuatorFeasible : BOOL := TRUE;
    ModelGain : LREAL := 0.2;
    ModelTimeConstant : LREAL := 5.0;
    ModelDeadTime : LREAL := 1.0;
    Lambda : LREAL := 4.0;
    MaximumExpectedError : LREAL := 0.4;
    SelectedProportionalGain : LREAL := 4.0;
    SelectedIntegralGain : LREAL := 0.8;
    SelectedIntegralTime : LREAL := 5.0;
    ActiveProportionalGain : LREAL := 4.0;
    ActiveIntegralGain : LREAL := 0.8;
    ActiveIntegralTime : LREAL := 5.0;
    PreviousProportionalGain : LREAL := 4.0;
    PreviousIntegralGain : LREAL := 0.8;
    PreviousIntegralTime : LREAL := 5.0;
    PreviousGenerateCandidate : BOOL := FALSE;
    PreviousValidationPassed : BOOL := FALSE;
    PreviousApplyApproved : BOOL := FALSE;
    PreviousRollbackRequested : BOOL := FALSE;
    ReconfigureRequested : BOOL := FALSE;
    Setpoint : LREAL := 30.6;
    ProcessValue : LREAL := 30.2;
    RequestedManipulatedValue : LREAL := 0.0;
    ManipulatedValue : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    (* Start from the proven empirical PI. A calculated candidate remains
       separate until validation and a later Apply approval both succeed. *)
    Parameters.ProportionalGain := ActiveProportionalGain;
    Parameters.IntegralGain := ActiveIntegralGain;
    Parameters.DerivativeGain := LREAL#0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#20.0;
    (* The physical heater supports 0-100%, but this model was validated only
       for 20-25%; qualification therefore stays inside that local range. *)
    Parameters.OutputMinimum := LREAL#20.0;
    Parameters.OutputMaximum := LREAL#25.0;
    Parameters.RateMinimum := LREAL#-100;
    Parameters.RateMaximum := LREAL#100;
    Parameters.TrackingTime := LREAL#1;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Control Studio writes one reviewed Boolean. On its rising edge the project
   executes the selected candidate lifecycle in program order. The falling
   edge removes run authority and rolls the Simulation controller back. *)
IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    TuningSelection := 1;
    GenerateCandidate := TRUE;
    ValidationPassed := TRUE;
    ApplyApproved := TRUE;
    RollbackRequested := FALSE;
    RunApproved := TRUE;
ELSIF ExperimentEnabled THEN
    GenerateCandidate := FALSE;
    ValidationPassed := FALSE;
    ApplyApproved := FALSE;
    RollbackRequested := FALSE;
    RunApproved := TRUE;
ELSIF ExperimentWasEnabled THEN
    GenerateCandidate := FALSE;
    ValidationPassed := FALSE;
    ApplyApproved := FALSE;
    RollbackRequested := TRUE;
    RunApproved := FALSE;
ELSIF PreviousRollbackRequested THEN
    RollbackRequested := FALSE;
END_IF;

IF GenerateCandidate AND NOT PreviousGenerateCandidate THEN
    CandidateValidated := FALSE;
    CandidateApplied := FALSE;
    ApplyRejected := FALSE;
    IF TuningSelection = 0 THEN
        (* Selection 0 reproduces the empirical Lesson 7 tuning. *)
        SelectedProportionalGain := LREAL#4.0;
        SelectedIntegralGain := LREAL#0.8;
        SelectedIntegralTime := LREAL#5.0;
        TuningValid := TRUE;
    ELSIF TuningSelection = 1 AND AcceptedModelId = 17
        AND AcceptedModelPurpose AND ModelWithinValidityRange
        AND RequirementsFeasible AND ModelGain > LREAL#0.0
        AND ModelGain <= LREAL#1.0
        AND ModelTimeConstant > LREAL#0.0
        AND ModelTimeConstant <= LREAL#3600.0
        AND ModelDeadTime >= LREAL#0.0
        AND ModelDeadTime <= LREAL#600.0
        AND Lambda > LREAL#0.0 AND Lambda <= LREAL#3600.0 THEN
        (* Lambda/IMC PI for this eligible FOPDT model. *)
        SelectedProportionalGain := ModelTimeConstant
            / (ModelGain * (Lambda + ModelDeadTime));
        SelectedIntegralTime := MIN(
            ModelTimeConstant,
            LREAL#4.0 * (Lambda + ModelDeadTime)
        );
        SelectedIntegralGain := SelectedProportionalGain / SelectedIntegralTime;
        (* The validated local command range has only five percentage points
           above its 20% operating bias, even though the heater can reach 100%. *)
        CandidateActuatorFeasible :=
            ABS(SelectedProportionalGain * MaximumExpectedError) <= LREAL#5.0;
        TuningValid := CandidateActuatorFeasible;
    ELSE
        (* An ineligible model cannot create a recommendation. *)
        SelectedProportionalGain := LREAL#0.0;
        SelectedIntegralGain := LREAL#0.0;
        SelectedIntegralTime := LREAL#0.0;
        CandidateActuatorFeasible := FALSE;
        TuningValid := FALSE;
    END_IF;
END_IF;

IF ValidationPassed THEN
    CandidateValidated := TuningValid;
END_IF;

ReconfigureRequested := FALSE;
IF ApplyApproved AND NOT PreviousApplyApproved THEN
    IF TuningValid AND CandidateValidated THEN
        PreviousProportionalGain := ActiveProportionalGain;
        PreviousIntegralGain := ActiveIntegralGain;
        PreviousIntegralTime := ActiveIntegralTime;
        ActiveProportionalGain := SelectedProportionalGain;
        ActiveIntegralGain := SelectedIntegralGain;
        ActiveIntegralTime := SelectedIntegralTime;
        CandidateApplied := TRUE;
        ReconfigureRequested := TRUE;
    ELSE
        ApplyRejected := TRUE;
    END_IF;
END_IF;

IF RollbackRequested AND NOT PreviousRollbackRequested AND CandidateApplied THEN
    ActiveProportionalGain := PreviousProportionalGain;
    ActiveIntegralGain := PreviousIntegralGain;
    ActiveIntegralTime := PreviousIntegralTime;
    CandidateApplied := FALSE;
    ReconfigureRequested := TRUE;
END_IF;

IF ReconfigureRequested THEN
    (* Rebuild controller history from the current plant state so Apply and
       rollback do not manufacture an output jump from stale state. *)
    Parameters.ProportionalGain := ActiveProportionalGain;
    Parameters.IntegralGain := ActiveIntegralGain;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := ManipulatedValue;
    InitialState.SaturatedOutput := ManipulatedValue;
    Controller.Initialize(State := InitialState);
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
(* Gain calculation is not permission to run. Explicit approval, valid inputs,
   and a valid tuning are all required before Auto can command the heater. *)
IF AutoMode AND RunApproved AND TuningValid AND InputsValid AND NOT SafetyAbort THEN
    Input.Mode := PIDMode#Auto;
    ManualMode := FALSE;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    ManualMode := TRUE;
END_IF;
Input.CycleTime := LREAL#0.1;
RequestedManipulatedValue := Controller.Update(Input := Input);
IF SafetyAbort OR NOT InputsValid OR NOT RunApproved THEN
    (* Safety and removal of run authority bypass the normal slew history. *)
    ManipulatedValue := LREAL#0.0;
ELSE
    ManipulatedValue := MIN(MAX(RequestedManipulatedValue, LREAL#0.0), LREAL#100.0);
END_IF;
PreviousGenerateCandidate := GenerateCandidate;
PreviousValidationPassed := ValidationPassed;
PreviousApplyApproved := ApplyApproved;
PreviousRollbackRequested := RollbackRequested;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX85.0 : BOOL;
    P1.AutoMode AT %MX144.0 : BOOL;
    P1.RunApproved AT %MX144.1 : BOOL;
    P1.GenerateCandidate AT %MX84.1 : BOOL;
    P1.ValidationPassed AT %MX84.2 : BOOL;
    P1.ApplyApproved AT %MX84.3 : BOOL;
    P1.RollbackRequested AT %MX84.4 : BOOL;
    P1.SafetyAbort AT %MX144.2 : BOOL;
    P1.InputsValid AT %MX144.3 : BOOL;
    P1.ManualMode AT %MX144.4 : BOOL;
    P1.TuningSelection AT %MW80 : INT;
    P1.TuningValid AT %MX84.0 : BOOL;
    P1.CandidateValidated AT %MX84.5 : BOOL;
    P1.CandidateApplied AT %MX84.6 : BOOL;
    P1.ApplyRejected AT %MX84.7 : BOOL;
    P1.AcceptedModelId AT %MW136 : INT;
    P1.AcceptedModelPurpose AT %MX138.0 : BOOL;
    P1.ModelWithinValidityRange AT %MX138.1 : BOOL;
    P1.RequirementsFeasible AT %MX138.2 : BOOL;
    P1.CandidateActuatorFeasible AT %MX138.3 : BOOL;
    P1.ModelGain AT %ML8 : LREAL;
    P1.ModelTimeConstant AT %ML16 : LREAL;
    P1.ModelDeadTime AT %ML24 : LREAL;
    P1.Lambda AT %ML32 : LREAL;
    P1.SelectedProportionalGain AT %ML40 : LREAL;
    P1.SelectedIntegralGain AT %ML48 : LREAL;
    P1.SelectedIntegralTime AT %ML56 : LREAL;
    P1.ActiveProportionalGain AT %ML88 : LREAL;
    P1.ActiveIntegralGain AT %ML96 : LREAL;
    P1.ActiveIntegralTime AT %ML104 : LREAL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.RequestedManipulatedValue AT %ML64 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
