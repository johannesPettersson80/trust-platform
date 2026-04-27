(* Pattern role: IHoistDrive is the contract boundary for the Adapter and State example. *)
INTERFACE IHoistDrive
    PROPERTY IsRunning : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY HasFault : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY FaultCode : DWORD
    GET
    END_GET
    END_PROPERTY

    METHOD Run
    VAR_INPUT
        SpeedPct : REAL;
    END_VAR
    END_METHOD

    METHOD Stop
    END_METHOD
END_INTERFACE

(* Pattern role: AbbHoistAdapter is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK AbbHoistAdapter IMPLEMENTS IHoistDrive
VAR
    StatusWordValue : WORD;
    FaultRawValue : WORD;
    RunningValue : BOOL;
    FaultValue : BOOL;
END_VAR

METHOD PUBLIC ConfigureRaw
VAR_INPUT
    StatusWord : WORD;
    FaultRaw : WORD;
END_VAR
StatusWordValue := StatusWord;
FaultRawValue := FaultRaw;
RunningValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0004) <> DWORD#0;
FaultValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0008) <> DWORD#0;
END_METHOD

METHOD PUBLIC Run
VAR_INPUT
    SpeedPct : REAL;
END_VAR
RunningValue := SpeedPct > REAL#0.0;
END_METHOD

METHOD PUBLIC Stop
RunningValue := FALSE;
END_METHOD

PUBLIC PROPERTY IsRunning : BOOL
GET
    IsRunning := RunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HasFault : BOOL
GET
    HasFault := FaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FaultCode : DWORD
GET
    FaultCode := DWORD#16#A0000000 OR WORD_TO_DWORD(FaultRawValue);
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: SiemensTravelAdapter is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK SiemensTravelAdapter IMPLEMENTS IHoistDrive
VAR
    StatusWordValue : WORD;
    FaultRawValue : WORD;
    RunningValue : BOOL;
    FaultValue : BOOL;
END_VAR

METHOD PUBLIC ConfigureRaw
VAR_INPUT
    StatusWord : WORD;
    FaultRaw : WORD;
END_VAR
StatusWordValue := StatusWord;
FaultRawValue := FaultRaw;
RunningValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0400) <> DWORD#0;
FaultValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0008) <> DWORD#0;
END_METHOD

METHOD PUBLIC Run
VAR_INPUT
    SpeedPct : REAL;
END_VAR
RunningValue := SpeedPct > REAL#0.0;
END_METHOD

METHOD PUBLIC Stop
RunningValue := FALSE;
END_METHOD

PUBLIC PROPERTY IsRunning : BOOL
GET
    IsRunning := RunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HasFault : BOOL
GET
    HasFault := FaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FaultCode : DWORD
GET
    FaultCode := DWORD#16#50000000 OR WORD_TO_DWORD(FaultRawValue);
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: ICraneState is the contract boundary for the Adapter and State example. *)
INTERFACE ICraneState
    PROPERTY StateId : INT
    GET
    END_GET
    END_PROPERTY

    METHOD OnExecute : INT
    VAR_INPUT
        LiftCommand : BOOL;
        TravelCommand : BOOL;
        LowerCommand : BOOL;
        UpperLimit : BOOL;
        Overload : BOOL;
        DriveFault : BOOL;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: ParkedState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK ParkedState IMPLEMENTS ICraneState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    LiftCommand : BOOL;
    TravelCommand : BOOL;
    LowerCommand : BOOL;
    UpperLimit : BOOL;
    Overload : BOOL;
    DriveFault : BOOL;
END_VAR
IF DriveFault OR Overload THEN
    OnExecute := INT#90;
ELSIF LiftCommand AND NOT UpperLimit THEN
    OnExecute := INT#10;
ELSIF TravelCommand THEN
    OnExecute := INT#20;
ELSIF LowerCommand THEN
    OnExecute := INT#30;
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

(* Pattern role: LiftingState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK LiftingState IMPLEMENTS ICraneState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    LiftCommand : BOOL;
    TravelCommand : BOOL;
    LowerCommand : BOOL;
    UpperLimit : BOOL;
    Overload : BOOL;
    DriveFault : BOOL;
END_VAR
IF DriveFault OR Overload OR UpperLimit THEN
    OnExecute := INT#90;
ELSIF NOT LiftCommand THEN
    (* Lift command released: forward to the next active command in
       a single scan rather than via Parked, so callers see one-step
       transitions when switching motions on the same instance. *)
    IF TravelCommand THEN
        OnExecute := INT#20;
    ELSIF LowerCommand THEN
        OnExecute := INT#30;
    ELSE
        OnExecute := INT#0;
    END_IF;
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

(* Pattern role: TravellingState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK TravellingState IMPLEMENTS ICraneState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    LiftCommand : BOOL;
    TravelCommand : BOOL;
    LowerCommand : BOOL;
    UpperLimit : BOOL;
    Overload : BOOL;
    DriveFault : BOOL;
END_VAR
IF DriveFault OR Overload THEN
    OnExecute := INT#90;
ELSIF NOT TravelCommand THEN
    (* Travel released: forward to next active command in one scan. *)
    IF LiftCommand AND NOT UpperLimit THEN
        OnExecute := INT#10;
    ELSIF LowerCommand THEN
        OnExecute := INT#30;
    ELSE
        OnExecute := INT#0;
    END_IF;
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

(* Pattern role: LoweringState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK LoweringState IMPLEMENTS ICraneState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    LiftCommand : BOOL;
    TravelCommand : BOOL;
    LowerCommand : BOOL;
    UpperLimit : BOOL;
    Overload : BOOL;
    DriveFault : BOOL;
END_VAR
IF DriveFault OR Overload THEN
    OnExecute := INT#90;
ELSIF NOT LowerCommand THEN
    (* Lower released: forward to next active command in one scan. *)
    IF LiftCommand AND NOT UpperLimit THEN
        OnExecute := INT#10;
    ELSIF TravelCommand THEN
        OnExecute := INT#20;
    ELSE
        OnExecute := INT#0;
    END_IF;
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

(* Pattern role: CraneFaultState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK CraneFaultState IMPLEMENTS ICraneState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    LiftCommand : BOOL;
    TravelCommand : BOOL;
    LowerCommand : BOOL;
    UpperLimit : BOOL;
    Overload : BOOL;
    DriveFault : BOOL;
END_VAR
OnExecute := INT#90;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#90;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: CraneController owns the collaborating objects used by this Adapter and State example. *)
FUNCTION_BLOCK CraneController
VAR
    Hoist : AbbHoistAdapter;
    Travel : SiemensTravelAdapter;
    HoistDrive : IHoistDrive;
    TravelDrive : IHoistDrive;
    Parked : ParkedState;
    Lifting : LiftingState;
    Travelling : TravellingState;
    Lowering : LoweringState;
    Fault : CraneFaultState;
    Current : ICraneState;
    CurrentStateIdValue : INT;
    NormalizedFaultCodeValue : DWORD;
END_VAR

METHOD PUBLIC Initialize
HoistDrive := Hoist;
TravelDrive := Travel;
Current := Parked;
CurrentStateIdValue := INT#0;
END_METHOD

METHOD PUBLIC InjectDriveStatus
VAR_INPUT
    HoistStatus : WORD;
    TravelStatus : WORD;
END_VAR
Hoist.ConfigureRaw(StatusWord := HoistStatus, FaultRaw := WORD#16#0044);
Travel.ConfigureRaw(StatusWord := TravelStatus, FaultRaw := WORD#16#0055);
END_METHOD

METHOD PUBLIC ResolveState : ICraneState
VAR_INPUT
    StateId : INT;
END_VAR
IF StateId = INT#10 THEN
    ResolveState := Lifting;
ELSIF StateId = INT#20 THEN
    ResolveState := Travelling;
ELSIF StateId = INT#30 THEN
    ResolveState := Lowering;
ELSIF StateId = INT#90 THEN
    ResolveState := Fault;
ELSE
    ResolveState := Parked;
END_IF;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    LiftCommand : BOOL;
    TravelCommand : BOOL;
    LowerCommand : BOOL;
    UpperLimit : BOOL;
    Overload : BOOL;
END_VAR
VAR
    DriveFault : BOOL;
    NextId : INT;
END_VAR
DriveFault := HoistDrive.HasFault OR TravelDrive.HasFault;
IF HoistDrive.HasFault THEN
    NormalizedFaultCodeValue := HoistDrive.FaultCode;
ELSIF TravelDrive.HasFault THEN
    NormalizedFaultCodeValue := TravelDrive.FaultCode;
END_IF;
NextId := Current.OnExecute(LiftCommand := LiftCommand, TravelCommand := TravelCommand, LowerCommand := LowerCommand, UpperLimit := UpperLimit, Overload := Overload, DriveFault := DriveFault);
IF NextId <> CurrentStateIdValue THEN
    Current := ResolveState(StateId := NextId);
    CurrentStateIdValue := NextId;
END_IF;
IF CurrentStateIdValue = INT#10 THEN
    HoistDrive.Run(SpeedPct := REAL#40.0);
ELSIF CurrentStateIdValue = INT#20 THEN
    TravelDrive.Run(SpeedPct := REAL#35.0);
ELSE
    HoistDrive.Stop();
    TravelDrive.Stop();
END_IF;
Update := CurrentStateIdValue;
END_METHOD

PUBLIC PROPERTY CurrentStateId : INT
GET
    CurrentStateId := CurrentStateIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY NormalizedFaultCode : DWORD
GET
    NormalizedFaultCode := NormalizedFaultCodeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HoistRunning : BOOL
GET
    HoistRunning := HoistDrive.IsRunning;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Crane : CraneController;
    LiftCommand : BOOL;
    TravelCommand : BOOL;
    LowerCommand : BOOL;
    UpperLimit : BOOL;
    Overload : BOOL;
    HoistEnableOut : BOOL;
    TravelEnableOut : BOOL;
    AlarmOut : BOOL;
END_VAR

Crane.Initialize();
Crane.Update(LiftCommand := LiftCommand, TravelCommand := TravelCommand, LowerCommand := LowerCommand, UpperLimit := UpperLimit, Overload := Overload);
HoistEnableOut := Crane.CurrentStateId = INT#10;
TravelEnableOut := Crane.CurrentStateId = INT#20;
AlarmOut := Crane.CurrentStateId = INT#90;
END_PROGRAM
