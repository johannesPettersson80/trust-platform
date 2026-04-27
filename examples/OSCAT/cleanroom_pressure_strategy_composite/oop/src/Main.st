(* Pattern role: IRoomNode is the contract boundary for the Strategy and Composite example. *)
INTERFACE IRoomNode
    PROPERTY ChildCount : INT
    GET
    END_GET
    END_PROPERTY
    PROPERTY PressureError : REAL
    GET
    END_GET
    END_PROPERTY
    PROPERTY AlarmCount : INT
    GET
    END_GET
    END_PROPERTY
    METHOD GetChild : IRoomNode
    VAR_INPUT
        Index : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: IPressureStrategy is the contract boundary for the Strategy and Composite example. *)
INTERFACE IPressureStrategy
    PROPERTY ModeId : INT
    GET
    END_GET
    END_PROPERTY
    METHOD TargetPressure : REAL
    VAR_INPUT
        BasePressurePa : REAL;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: NormalPressureStrategy is a concrete implementation in the Strategy and Composite example. *)
FUNCTION_BLOCK NormalPressureStrategy IMPLEMENTS IPressureStrategy
METHOD PUBLIC TargetPressure : REAL
VAR_INPUT
    BasePressurePa : REAL;
END_VAR
TargetPressure := BasePressurePa;
END_METHOD
PUBLIC PROPERTY ModeId : INT
GET
    ModeId := INT#1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: CleaningPressureStrategy is a concrete implementation in the Strategy and Composite example. *)
FUNCTION_BLOCK CleaningPressureStrategy IMPLEMENTS IPressureStrategy
METHOD PUBLIC TargetPressure : REAL
VAR_INPUT
    BasePressurePa : REAL;
END_VAR
TargetPressure := BasePressurePa + REAL#5.0;
END_METHOD
PUBLIC PROPERTY ModeId : INT
GET
    ModeId := INT#2;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: EmergencyPressureStrategy is a concrete implementation in the Strategy and Composite example. *)
FUNCTION_BLOCK EmergencyPressureStrategy IMPLEMENTS IPressureStrategy
METHOD PUBLIC TargetPressure : REAL
VAR_INPUT
    BasePressurePa : REAL;
END_VAR
TargetPressure := REAL#0.0;
END_METHOD
PUBLIC PROPERTY ModeId : INT
GET
    ModeId := INT#3;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RoomNode is a concrete implementation in the Strategy and Composite example. *)
FUNCTION_BLOCK RoomNode IMPLEMENTS IRoomNode
VAR
    ErrorValue : REAL;
    AlarmValue : BOOL;
END_VAR
METHOD PUBLIC UpdateRoom
VAR_INPUT
    ActualPressurePa : REAL;
    TargetPressurePa : REAL;
    DoorOpen : BOOL;
END_VAR
ErrorValue := TargetPressurePa - ActualPressurePa;
AlarmValue := (ABS(ErrorValue) > REAL#10.0) OR DoorOpen;
END_METHOD
METHOD PUBLIC GetChild : IRoomNode
VAR_INPUT
    Index : INT;
END_VAR
GetChild := THIS;
END_METHOD
PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#0;
END_GET
END_PROPERTY
PUBLIC PROPERTY PressureError : REAL
GET
    PressureError := ErrorValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY AlarmCount : INT
GET
    IF AlarmValue THEN AlarmCount := INT#1; ELSE AlarmCount := INT#0; END_IF;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RoomSuite is a concrete implementation in the Strategy and Composite example. *)
FUNCTION_BLOCK RoomSuite IMPLEMENTS IRoomNode
VAR
    Room1 : RoomNode;
    Room2 : RoomNode;
END_VAR
METHOD PUBLIC UpdateSuite
VAR_INPUT
    Room1Pressure : REAL;
    Room2Pressure : REAL;
    TargetPressure : REAL;
    DoorOpen : BOOL;
END_VAR
Room1.UpdateRoom(ActualPressurePa := Room1Pressure, TargetPressurePa := TargetPressure, DoorOpen := DoorOpen);
Room2.UpdateRoom(ActualPressurePa := Room2Pressure, TargetPressurePa := TargetPressure - REAL#5.0, DoorOpen := FALSE);
END_METHOD
METHOD PUBLIC GetChild : IRoomNode
VAR_INPUT
    Index : INT;
END_VAR
IF Index = INT#1 THEN GetChild := Room1; ELSE GetChild := Room2; END_IF;
END_METHOD
PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#2;
END_GET
END_PROPERTY
PUBLIC PROPERTY PressureError : REAL
GET
    PressureError := Room1.PressureError + Room2.PressureError;
END_GET
END_PROPERTY
PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := Room1.AlarmCount + Room2.AlarmCount;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: PressureCascadeController owns the collaborating objects used by this Strategy and Composite example. *)
FUNCTION_BLOCK PressureCascadeController
VAR
    Normal : NormalPressureStrategy;
    Cleaning : CleaningPressureStrategy;
    Emergency : EmergencyPressureStrategy;
    ActiveStrategy : IPressureStrategy;
    Suite : RoomSuite;
    ActiveModeIdValue : INT;
    SupplyDamperCommandValue : REAL;
END_VAR
METHOD PUBLIC Initialize
ActiveStrategy := Normal;
END_METHOD
METHOD PUBLIC Update : REAL
VAR_INPUT
    ModeRequest : INT;
    Room1Pressure : REAL;
    Room2Pressure : REAL;
    DoorOpen : BOOL;
END_VAR
VAR
    Target : REAL;
END_VAR
IF ModeRequest = INT#3 THEN
    ActiveStrategy := Emergency;
ELSIF ModeRequest = INT#2 THEN
    ActiveStrategy := Cleaning;
ELSE
    ActiveStrategy := Normal;
END_IF;
ActiveModeIdValue := ActiveStrategy.ModeId;
Target := ActiveStrategy.TargetPressure(BasePressurePa := REAL#15.0);
Suite.UpdateSuite(Room1Pressure := Room1Pressure, Room2Pressure := Room2Pressure, TargetPressure := Target, DoorOpen := DoorOpen);
SupplyDamperCommandValue := LIMIT(REAL#0.0, Suite.PressureError * REAL#2.0, REAL#100.0);
Update := SupplyDamperCommandValue;
END_METHOD
PUBLIC PROPERTY ActiveModeId : INT
GET
    ActiveModeId := ActiveModeIdValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY SupplyDamperCommand : REAL
GET
    SupplyDamperCommand := SupplyDamperCommandValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := Suite.AlarmCount;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Cascade : PressureCascadeController;
    ModeRequest : INT;
    Room1PressureRaw : INT;
    Room2PressureRaw : INT;
    DoorOpen : BOOL;
    SupplyDamperRaw : INT;
    AlarmOut : BOOL;
END_VAR
Cascade.Initialize();
SupplyDamperRaw := REAL_TO_INT(Cascade.Update(ModeRequest := ModeRequest, Room1Pressure := INT_TO_REAL(Room1PressureRaw), Room2Pressure := INT_TO_REAL(Room2PressureRaw), DoorOpen := DoorOpen));
AlarmOut := Cascade.AlarmCount > INT#0;
END_PROGRAM
