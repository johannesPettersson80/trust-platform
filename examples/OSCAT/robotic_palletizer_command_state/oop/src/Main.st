(* Pattern role: IRobotCommand is the contract boundary for the Command and State example. *)
INTERFACE IRobotCommand
    PROPERTY CommandId : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY Done : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Failed : BOOL
    GET
    END_GET
    END_PROPERTY

    METHOD Execute : BOOL
    VAR_INPUT
        RobotHealthy : BOOL;
        PickVacuumOk : BOOL;
    END_VAR
    END_METHOD

    METHOD Reset
    END_METHOD
END_INTERFACE

(* Pattern role: PickCommand is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK PickCommand IMPLEMENTS IRobotCommand
VAR
    DoneValue : BOOL;
    FailedValue : BOOL;
END_VAR

METHOD PUBLIC Execute : BOOL
VAR_INPUT
    RobotHealthy : BOOL;
    PickVacuumOk : BOOL;
END_VAR
DoneValue := RobotHealthy AND PickVacuumOk;
FailedValue := NOT DoneValue;
Execute := DoneValue;
END_METHOD

METHOD PUBLIC Reset
DoneValue := FALSE;
FailedValue := FALSE;
END_METHOD

PUBLIC PROPERTY CommandId : INT
GET
    CommandId := INT#1;
END_GET
END_PROPERTY

PUBLIC PROPERTY Done : BOOL
GET
    Done := DoneValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Failed : BOOL
GET
    Failed := FailedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: PlaceCommand is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK PlaceCommand IMPLEMENTS IRobotCommand
VAR
    DoneValue : BOOL;
    FailedValue : BOOL;
END_VAR

METHOD PUBLIC Execute : BOOL
VAR_INPUT
    RobotHealthy : BOOL;
    PickVacuumOk : BOOL;
END_VAR
DoneValue := RobotHealthy;
FailedValue := NOT DoneValue;
Execute := DoneValue;
END_METHOD

METHOD PUBLIC Reset
DoneValue := FALSE;
FailedValue := FALSE;
END_METHOD

PUBLIC PROPERTY CommandId : INT
GET
    CommandId := INT#2;
END_GET
END_PROPERTY

PUBLIC PROPERTY Done : BOOL
GET
    Done := DoneValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Failed : BOOL
GET
    Failed := FailedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: IndexPalletCommand is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK IndexPalletCommand IMPLEMENTS IRobotCommand
VAR
    DoneValue : BOOL;
END_VAR

METHOD PUBLIC Execute : BOOL
VAR_INPUT
    RobotHealthy : BOOL;
    PickVacuumOk : BOOL;
END_VAR
DoneValue := TRUE;
Execute := TRUE;
END_METHOD

METHOD PUBLIC Reset
DoneValue := FALSE;
END_METHOD

PUBLIC PROPERTY CommandId : INT
GET
    CommandId := INT#3;
END_GET
END_PROPERTY

PUBLIC PROPERTY Done : BOOL
GET
    Done := DoneValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Failed : BOOL
GET
    Failed := FALSE;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RejectBoxCommand is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK RejectBoxCommand IMPLEMENTS IRobotCommand
VAR
    DoneValue : BOOL;
END_VAR

METHOD PUBLIC Execute : BOOL
VAR_INPUT
    RobotHealthy : BOOL;
    PickVacuumOk : BOOL;
END_VAR
DoneValue := TRUE;
Execute := TRUE;
END_METHOD

METHOD PUBLIC Reset
DoneValue := FALSE;
END_METHOD

PUBLIC PROPERTY CommandId : INT
GET
    CommandId := INT#4;
END_GET
END_PROPERTY

PUBLIC PROPERTY Done : BOOL
GET
    Done := DoneValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Failed : BOOL
GET
    Failed := FALSE;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: IPalletizerState is the contract boundary for the Command and State example. *)
INTERFACE IPalletizerState
    PROPERTY StateId : INT
    GET
    END_GET
    END_PROPERTY

    METHOD OnExecute : INT
    VAR_INPUT
        BoxPresent : BOOL;
        PalletReady : BOOL;
        LastCommandDone : BOOL;
        LastCommandFailed : BOOL;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: WaitBoxState is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK WaitBoxState IMPLEMENTS IPalletizerState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    LastCommandDone : BOOL;
    LastCommandFailed : BOOL;
END_VAR
IF BoxPresent AND PalletReady THEN
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

(* Pattern role: PickState is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK PickState IMPLEMENTS IPalletizerState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    LastCommandDone : BOOL;
    LastCommandFailed : BOOL;
END_VAR
IF LastCommandFailed THEN
    OnExecute := INT#40;
ELSIF LastCommandDone THEN
    OnExecute := INT#20;
ELSE
    OnExecute := INT#10;
END_IF;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#10;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: PlaceState is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK PlaceState IMPLEMENTS IPalletizerState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    LastCommandDone : BOOL;
    LastCommandFailed : BOOL;
END_VAR
IF LastCommandFailed THEN
    OnExecute := INT#90;
ELSIF LastCommandDone THEN
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

(* Pattern role: IndexState is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK IndexState IMPLEMENTS IPalletizerState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    LastCommandDone : BOOL;
    LastCommandFailed : BOOL;
END_VAR
IF LastCommandDone THEN
    OnExecute := INT#100;
ELSE
    OnExecute := INT#30;
END_IF;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#30;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RejectState is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK RejectState IMPLEMENTS IPalletizerState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    LastCommandDone : BOOL;
    LastCommandFailed : BOOL;
END_VAR
IF LastCommandDone THEN
    OnExecute := INT#0;
ELSE
    OnExecute := INT#40;
END_IF;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#40;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: FaultState is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK FaultState IMPLEMENTS IPalletizerState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    LastCommandDone : BOOL;
    LastCommandFailed : BOOL;
END_VAR
OnExecute := INT#90;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#90;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: CompletePalletState is a concrete implementation in the Command and State example. *)
FUNCTION_BLOCK CompletePalletState IMPLEMENTS IPalletizerState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    LastCommandDone : BOOL;
    LastCommandFailed : BOOL;
END_VAR
(* Restart-friendly terminal state: if a new box and pallet are ready
   in the same scan, advance straight into Pick so the operator does
   not lose the trigger waiting in WaitBox. *)
IF BoxPresent AND PalletReady THEN
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

(* Composition role: PalletizerController owns the collaborating objects used by this Command and State example. *)
FUNCTION_BLOCK PalletizerController
VAR
    WaitBox : WaitBoxState;
    Pick : PickState;
    Place : PlaceState;
    IndexPallet : IndexState;
    Reject : RejectState;
    Fault : FaultState;
    Complete : CompletePalletState;
    PickCmd : PickCommand;
    PlaceCmd : PlaceCommand;
    IndexCmd : IndexPalletCommand;
    RejectCmd : RejectBoxCommand;
    Current : IPalletizerState;
    CurrentCommand : IRobotCommand;
    CurrentStateIdValue : INT;
    LastCommandIdValue : INT;
    CompletedPalletsValue : DWORD;
    Counter : DwordCounter;
END_VAR

METHOD PUBLIC Initialize
Current := WaitBox;
CurrentStateIdValue := INT#0;
Counter.Initialize();
Counter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PUBLIC ResolveState : IPalletizerState
VAR_INPUT
    StateId : INT;
END_VAR
IF StateId = INT#10 THEN
    ResolveState := Pick;
ELSIF StateId = INT#20 THEN
    ResolveState := Place;
ELSIF StateId = INT#30 THEN
    ResolveState := IndexPallet;
ELSIF StateId = INT#40 THEN
    ResolveState := Reject;
ELSIF StateId = INT#90 THEN
    ResolveState := Fault;
ELSIF StateId = INT#100 THEN
    ResolveState := Complete;
ELSE
    ResolveState := WaitBox;
END_IF;
END_METHOD

METHOD PUBLIC CommandForState : IRobotCommand
VAR_INPUT
    StateId : INT;
END_VAR
IF StateId = INT#10 THEN
    CommandForState := PickCmd;
ELSIF StateId = INT#20 THEN
    CommandForState := PlaceCmd;
ELSIF StateId = INT#30 THEN
    CommandForState := IndexCmd;
ELSIF StateId = INT#40 THEN
    CommandForState := RejectCmd;
ELSE
    CommandForState := RejectCmd;
END_IF;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    RobotHealthy : BOOL;
    PickVacuumOk : BOOL;
END_VAR
VAR
    CommandDone : BOOL;
    CommandFailed : BOOL;
    NextStateId : INT;
END_VAR
IF CurrentStateIdValue <> INT#0 THEN
    CurrentCommand := CommandForState(StateId := CurrentStateIdValue);
    CommandDone := CurrentCommand.Execute(RobotHealthy := RobotHealthy, PickVacuumOk := PickVacuumOk);
    CommandFailed := CurrentCommand.Failed;
    LastCommandIdValue := CurrentCommand.CommandId;
END_IF;
NextStateId := Current.OnExecute(BoxPresent := BoxPresent, PalletReady := PalletReady, LastCommandDone := CommandDone, LastCommandFailed := CommandFailed);
IF NextStateId <> CurrentStateIdValue THEN
    Current := ResolveState(StateId := NextStateId);
    CurrentStateIdValue := NextStateId;
    IF NextStateId = INT#100 THEN
        (* COUNT_DR is edge-triggered; drop UP edge first so consecutive
           pallet completions each register a fresh rising edge. *)
        Counter.Update(Up := FALSE, Down := FALSE);
        CompletedPalletsValue := Counter.Update(Up := TRUE, Down := FALSE);
    END_IF;
END_IF;
Update := CurrentStateIdValue;
END_METHOD

PUBLIC PROPERTY CurrentStateId : INT
GET
    CurrentStateId := CurrentStateIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY CompletedPallets : DWORD
GET
    CompletedPallets := CompletedPalletsValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCommandId : INT
GET
    LastCommandId := LastCommandIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Faulted : BOOL
GET
    Faulted := CurrentStateIdValue = INT#90;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Cell : PalletizerController;
    BoxPresent : BOOL;
    PalletReady : BOOL;
    RobotHealthy : BOOL;
    PickVacuumOk : BOOL;
    RobotMoveOut : BOOL;
    RejectOut : BOOL;
END_VAR

Cell.Initialize();
Cell.Update(BoxPresent := BoxPresent, PalletReady := PalletReady, RobotHealthy := RobotHealthy, PickVacuumOk := PickVacuumOk);
RobotMoveOut := (Cell.CurrentStateId = INT#10) OR (Cell.CurrentStateId = INT#20);
RejectOut := Cell.CurrentStateId = INT#40;
END_PROGRAM
