(* Record role: FillRecipe is a deterministic PLC-side record for tests and runtime readers. *)
TYPE FillRecipe :
STRUCT
    VolumeMl : REAL;
    PumpSpeedPct : REAL;
    LowWeightG : REAL;
    HighWeightG : REAL;
    Valid : BOOL;
END_STRUCT
END_TYPE

(* Composition role: FillRecipeBuilder owns the collaborating objects used by this Builder and State example. *)
FUNCTION_BLOCK FillRecipeBuilder
VAR
    Recipe : FillRecipe;
END_VAR

METHOD PUBLIC Reset
Recipe.VolumeMl := REAL#0.0;
Recipe.PumpSpeedPct := REAL#0.0;
Recipe.LowWeightG := REAL#0.0;
Recipe.HighWeightG := REAL#0.0;
Recipe.Valid := FALSE;
END_METHOD

METHOD PUBLIC SetVolume : FillRecipeBuilder
VAR_INPUT
    VolumeMl : REAL;
END_VAR
Recipe.VolumeMl := VolumeMl;
SetVolume := THIS;
END_METHOD

METHOD PUBLIC SetPumpSpeed : FillRecipeBuilder
VAR_INPUT
    PumpSpeedPct : REAL;
END_VAR
Recipe.PumpSpeedPct := PumpSpeedPct;
SetPumpSpeed := THIS;
END_METHOD

METHOD PUBLIC SetWeightWindow : FillRecipeBuilder
VAR_INPUT
    LowWeightG : REAL;
    HighWeightG : REAL;
END_VAR
Recipe.LowWeightG := LowWeightG;
Recipe.HighWeightG := HighWeightG;
SetWeightWindow := THIS;
END_METHOD

METHOD PUBLIC Build : FillRecipe
Recipe.Valid := (Recipe.VolumeMl > REAL#0.0) AND (Recipe.VolumeMl <= REAL#100.0) AND (Recipe.PumpSpeedPct > REAL#0.0) AND (Recipe.PumpSpeedPct <= REAL#100.0) AND (Recipe.HighWeightG > Recipe.LowWeightG);
Build := Recipe;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: IFillingState is the contract boundary for the Builder and State example. *)
INTERFACE IFillingState
    PROPERTY StateId : INT
    GET
    END_GET
    END_PROPERTY

    METHOD OnExecute : INT
    VAR_INPUT
        StartCommand : BOOL;
        VialPresent : BOOL;
        FillDone : BOOL;
        WeightG : REAL;
        Recipe : FillRecipe;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: IdleFillState is a concrete implementation in the Builder and State example. *)
FUNCTION_BLOCK IdleFillState IMPLEMENTS IFillingState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
    Recipe : FillRecipe;
END_VAR
IF StartCommand AND VialPresent AND Recipe.Valid THEN
    OnExecute := INT#10;
ELSE
    OnExecute := INT#0;
END_IF;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: PrimeFillState is a concrete implementation in the Builder and State example. *)
FUNCTION_BLOCK PrimeFillState IMPLEMENTS IFillingState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
    Recipe : FillRecipe;
END_VAR
OnExecute := INT#20;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#10;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: FillState is a concrete implementation in the Builder and State example. *)
FUNCTION_BLOCK FillState IMPLEMENTS IFillingState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
    Recipe : FillRecipe;
END_VAR
IF FillDone THEN
    OnExecute := INT#30;
ELSE
    OnExecute := INT#20;
END_IF;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#20;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: CheckWeightState is a concrete implementation in the Builder and State example. *)
FUNCTION_BLOCK CheckWeightState IMPLEMENTS IFillingState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
    Recipe : FillRecipe;
END_VAR
IF (WeightG >= Recipe.LowWeightG) AND (WeightG <= Recipe.HighWeightG) THEN
    OnExecute := INT#100;
ELSE
    OnExecute := INT#40;
END_IF;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#30;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RejectState is a concrete implementation in the Builder and State example. *)
FUNCTION_BLOCK RejectState IMPLEMENTS IFillingState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
    Recipe : FillRecipe;
END_VAR
(* Restart-friendly terminal state: if a new vial is started in the
   same scan, jump to Prime so the next vial doesn't lose the start
   pulse waiting in Idle. *)
IF StartCommand AND VialPresent AND Recipe.Valid THEN
    OnExecute := INT#10;
ELSE
    OnExecute := INT#0;
END_IF;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#40;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: CompleteState is a concrete implementation in the Builder and State example. *)
FUNCTION_BLOCK CompleteState IMPLEMENTS IFillingState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
    Recipe : FillRecipe;
END_VAR
(* Same restart-friendly behavior as Reject: a fresh start pulse
   advances directly to Prime without going through Idle first. *)
IF StartCommand AND VialPresent AND Recipe.Valid THEN
    OnExecute := INT#10;
ELSE
    OnExecute := INT#0;
END_IF;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#100;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: FaultFillState is a concrete implementation in the Builder and State example. *)
FUNCTION_BLOCK FaultFillState IMPLEMENTS IFillingState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
    Recipe : FillRecipe;
END_VAR
OnExecute := INT#90;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#90;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: FillingController owns the collaborating objects used by this Builder and State example. *)
FUNCTION_BLOCK FillingController
VAR
    Idle : IdleFillState;
    Prime : PrimeFillState;
    Filling : FillState;
    CheckWeight : CheckWeightState;
    Reject : RejectState;
    Complete : CompleteState;
    Fault : FaultFillState;
    Current : IFillingState;
    CurrentStateIdValue : INT;
    RejectCounter : DwordCounter;
    RecipeValue : FillRecipe;
END_VAR

METHOD PUBLIC Initialize
Current := Idle;
CurrentStateIdValue := INT#0;
RejectCounter.Initialize();
RejectCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PUBLIC LoadRecipe
VAR_INPUT
    Recipe : FillRecipe;
END_VAR
RecipeValue := Recipe;
END_METHOD

METHOD PUBLIC ResolveState : IFillingState
VAR_INPUT
    NextId : INT;
END_VAR
IF NextId = INT#10 THEN
    ResolveState := Prime;
ELSIF NextId = INT#20 THEN
    ResolveState := Filling;
ELSIF NextId = INT#30 THEN
    ResolveState := CheckWeight;
ELSIF NextId = INT#40 THEN
    ResolveState := Reject;
ELSIF NextId = INT#100 THEN
    ResolveState := Complete;
ELSIF NextId = INT#90 THEN
    ResolveState := Fault;
ELSE
    ResolveState := Idle;
END_IF;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
END_VAR
VAR
    NextId : INT;
END_VAR
NextId := Current.OnExecute(StartCommand := StartCommand, VialPresent := VialPresent, FillDone := FillDone, WeightG := WeightG, Recipe := RecipeValue);
IF NextId <> CurrentStateIdValue THEN
    Current := ResolveState(NextId := NextId);
    CurrentStateIdValue := NextId;
    IF NextId = INT#40 THEN
        (* COUNT_DR is edge-triggered; drop UP edge first so consecutive
           rejects across vials each register a fresh rising edge. *)
        RejectCounter.Update(Up := FALSE, Down := FALSE);
        RejectCounter.Update(Up := TRUE, Down := FALSE);
    END_IF;
END_IF;
Update := CurrentStateIdValue;
END_METHOD

PUBLIC PROPERTY CurrentStateId : INT
GET
    CurrentStateId := CurrentStateIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RejectCount : DWORD
GET
    RejectCount := RejectCounter.Count;
END_GET
END_PROPERTY

PUBLIC PROPERTY RecipeValid : BOOL
GET
    RecipeValid := RecipeValue.Valid;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Builder : FillRecipeBuilder;
    Filler : FillingController;
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightRaw : INT;
    Recipe : FillRecipe;
    PumpRunOut : BOOL;
    RejectOut : BOOL;
END_VAR

Recipe := Builder.SetVolume(VolumeMl := REAL#10.0).SetPumpSpeed(PumpSpeedPct := REAL#45.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
Filler.Initialize();
Filler.LoadRecipe(Recipe := Recipe);
Filler.Update(StartCommand := StartCommand, VialPresent := VialPresent, FillDone := FillDone, WeightG := INT_TO_REAL(WeightRaw) / REAL#10.0);
PumpRunOut := Filler.CurrentStateId = INT#20;
RejectOut := Filler.CurrentStateId = INT#40;
END_PROGRAM
