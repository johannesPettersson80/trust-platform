(* Pattern role: ICipState is the contract boundary for the State example. *)
INTERFACE ICipState
    PROPERTY StateId : INT
    GET
    END_GET
    END_PROPERTY

    METHOD OnEnter
    END_METHOD

    METHOD OnExecute : INT
    VAR_INPUT
        StartCommand : BOOL;
        StepComplete : BOOL;
        ConductivityUs : REAL;
        CausticTempC : REAL;
        FlowOk : BOOL;
        EStopHealthy : BOOL;
    END_VAR
    END_METHOD

    METHOD OnExit
    END_METHOD

    METHOD CanAbort : BOOL
    END_METHOD
END_INTERFACE

(* Pattern role: IdleState is a concrete implementation in the State example. *)
FUNCTION_BLOCK IdleState IMPLEMENTS ICipState
VAR
    Entered : BOOL;
END_VAR

METHOD PUBLIC OnEnter
Entered := TRUE;
END_METHOD

METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
IF NOT EStopHealthy THEN
    OnExecute := INT#90;
ELSIF StartCommand THEN
    OnExecute := INT#10;
ELSE
    OnExecute := INT#0;
END_IF;
END_METHOD

METHOD PUBLIC OnExit
Entered := FALSE;
END_METHOD

METHOD PUBLIC CanAbort : BOOL
CanAbort := TRUE;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: PreRinseState is a concrete implementation in the State example. *)
FUNCTION_BLOCK PreRinseState IMPLEMENTS ICipState
METHOD PUBLIC OnEnter
END_METHOD

METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
IF NOT EStopHealthy OR NOT FlowOk THEN
    OnExecute := INT#90;
ELSIF StepComplete THEN
    OnExecute := INT#20;
ELSE
    OnExecute := INT#10;
END_IF;
END_METHOD

METHOD PUBLIC OnExit
END_METHOD

METHOD PUBLIC CanAbort : BOOL
CanAbort := TRUE;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#10;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: CausticWashState is a concrete implementation in the State example. *)
FUNCTION_BLOCK CausticWashState IMPLEMENTS ICipState
METHOD PUBLIC OnEnter
END_METHOD

METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
IF NOT EStopHealthy OR NOT FlowOk THEN
    OnExecute := INT#90;
ELSIF CausticTempC < REAL#70.0 THEN
    OnExecute := INT#20;
ELSIF StepComplete THEN
    OnExecute := INT#30;
ELSE
    OnExecute := INT#20;
END_IF;
END_METHOD

METHOD PUBLIC OnExit
END_METHOD

METHOD PUBLIC CanAbort : BOOL
CanAbort := TRUE;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#20;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: IntermediateRinseState is a concrete implementation in the State example. *)
FUNCTION_BLOCK IntermediateRinseState IMPLEMENTS ICipState
METHOD PUBLIC OnEnter
END_METHOD

METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
IF NOT EStopHealthy OR NOT FlowOk THEN
    OnExecute := INT#90;
ELSIF StepComplete THEN
    OnExecute := INT#40;
ELSE
    OnExecute := INT#30;
END_IF;
END_METHOD

METHOD PUBLIC OnExit
END_METHOD

METHOD PUBLIC CanAbort : BOOL
CanAbort := TRUE;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#30;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: AcidWashState is a concrete implementation in the State example. *)
FUNCTION_BLOCK AcidWashState IMPLEMENTS ICipState
METHOD PUBLIC OnEnter
END_METHOD

METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
IF NOT EStopHealthy OR NOT FlowOk THEN
    OnExecute := INT#90;
ELSIF StepComplete THEN
    OnExecute := INT#50;
ELSE
    OnExecute := INT#40;
END_IF;
END_METHOD

METHOD PUBLIC OnExit
END_METHOD

METHOD PUBLIC CanAbort : BOOL
CanAbort := TRUE;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#40;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: FinalRinseState is a concrete implementation in the State example. *)
FUNCTION_BLOCK FinalRinseState IMPLEMENTS ICipState
METHOD PUBLIC OnEnter
END_METHOD

METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
IF NOT EStopHealthy OR NOT FlowOk THEN
    OnExecute := INT#90;
ELSIF ConductivityUs < REAL#50.0 THEN
    OnExecute := INT#100;
ELSE
    OnExecute := INT#50;
END_IF;
END_METHOD

METHOD PUBLIC OnExit
END_METHOD

METHOD PUBLIC CanAbort : BOOL
CanAbort := TRUE;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#50;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: DoneState is a concrete implementation in the State example. *)
FUNCTION_BLOCK DoneState IMPLEMENTS ICipState
METHOD PUBLIC OnEnter
END_METHOD

METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
OnExecute := INT#100;
END_METHOD

METHOD PUBLIC OnExit
END_METHOD

METHOD PUBLIC CanAbort : BOOL
CanAbort := FALSE;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#100;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: AbortState is a concrete implementation in the State example. *)
FUNCTION_BLOCK AbortState IMPLEMENTS ICipState
METHOD PUBLIC OnEnter
END_METHOD

METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
OnExecute := INT#90;
END_METHOD

METHOD PUBLIC OnExit
END_METHOD

METHOD PUBLIC CanAbort : BOOL
CanAbort := FALSE;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#90;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: CipController owns the collaborating objects used by this State example. *)
FUNCTION_BLOCK CipController
VAR
    Idle : IdleState;
    PreRinse : PreRinseState;
    Caustic : CausticWashState;
    Intermediate : IntermediateRinseState;
    Acid : AcidWashState;
    FinalRinse : FinalRinseState;
    CompleteState : DoneState;
    Abort : AbortState;
    Current : ICipState;
    ConductivityFilter : Pt1Filter;
    CycleCounter : DwordCounter;
    CurrentStateIdValue : INT;
    TransitionCountValue : INT;
    AbortedValue : BOOL;
END_VAR

METHOD PUBLIC Initialize
ConductivityFilter.Initialize();
ConductivityFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
CycleCounter.Initialize();
CycleCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
Current := Idle;
Current.OnEnter();
CurrentStateIdValue := Current.StateId;
TransitionCountValue := INT#0;
AbortedValue := FALSE;
END_METHOD

METHOD PUBLIC ResolveState : ICipState
VAR_INPUT
    NextId : INT;
END_VAR
IF NextId = INT#10 THEN
    ResolveState := PreRinse;
ELSIF NextId = INT#20 THEN
    ResolveState := Caustic;
ELSIF NextId = INT#30 THEN
    ResolveState := Intermediate;
ELSIF NextId = INT#40 THEN
    ResolveState := Acid;
ELSIF NextId = INT#50 THEN
    ResolveState := FinalRinse;
ELSIF NextId = INT#90 THEN
    ResolveState := Abort;
ELSIF NextId = INT#100 THEN
    ResolveState := CompleteState;
ELSE
    ResolveState := Idle;
END_IF;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    StepComplete : BOOL;
    ConductivityUs : REAL;
    CausticTempC : REAL;
    FlowOk : BOOL;
    EStopHealthy : BOOL;
END_VAR
VAR
    NextId : INT;
END_VAR
(* ConductivityFilter is declared and updated for telemetry/HMI use only.
   FinalRinseState compares against the raw ConductivityUs because the
   PT1 filter time constant (1 s) would otherwise delay the wash-complete
   decision past one scan cycle. A production CIP could drive the state
   from the filtered value with a longer minimum-rinse time. *)
ConductivityFilter.Update(Sample := ConductivityUs);
NextId := Current.OnExecute(StartCommand := StartCommand, StepComplete := StepComplete, ConductivityUs := ConductivityUs, CausticTempC := CausticTempC, FlowOk := FlowOk, EStopHealthy := EStopHealthy);
IF NextId <> Current.StateId THEN
    Current.OnExit();
    Current := ResolveState(NextId := NextId);
    Current.OnEnter();
    CurrentStateIdValue := Current.StateId;
    TransitionCountValue := TransitionCountValue + INT#1;
    IF CurrentStateIdValue = INT#90 THEN
        AbortedValue := TRUE;
    ELSIF CurrentStateIdValue = INT#100 THEN
        CycleCounter.Update(Up := TRUE, Down := FALSE);
    END_IF;
END_IF;
Update := CurrentStateIdValue;
END_METHOD

PUBLIC PROPERTY CurrentStateId : INT
GET
    CurrentStateId := CurrentStateIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY TransitionCount : INT
GET
    TransitionCount := TransitionCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY CycleCount : DWORD
GET
    CycleCount := CycleCounter.Count;
END_GET
END_PROPERTY

PUBLIC PROPERTY Aborted : BOOL
GET
    Aborted := AbortedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Cip : CipController;
    Initialized : BOOL;
    StartCommand : BOOL;
    StepComplete : BOOL;
    EStopHealthy : BOOL;
    FlowOk : BOOL;
    ConductivityRaw : INT;
    CausticTempRaw : INT;
    PumpEnableOut : BOOL;
    DrainValveOut : BOOL;
    AlarmOut : BOOL;
END_VAR

(* Initialize once on cold start; scanning Initialize every cycle would
   reset the state machine to Idle and clear retained counters. *)
IF NOT Initialized THEN
    Cip.Initialize();
    Initialized := TRUE;
END_IF;
Cip.Update(StartCommand := StartCommand, StepComplete := StepComplete, ConductivityUs := INT_TO_REAL(ConductivityRaw), CausticTempC := INT_TO_REAL(CausticTempRaw), FlowOk := FlowOk, EStopHealthy := EStopHealthy);
PumpEnableOut := (Cip.CurrentStateId >= INT#10) AND (Cip.CurrentStateId < INT#90);
DrainValveOut := Cip.CurrentStateId = INT#90;
AlarmOut := Cip.Aborted;
END_PROGRAM
