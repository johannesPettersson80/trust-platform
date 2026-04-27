(* Pattern role: IAhuStrategy is the contract boundary for the Strategy example. *)
INTERFACE IAhuStrategy EXTENDS IComponent
    PROPERTY ModeId : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY Output : REAL
    GET
    END_GET
    END_PROPERTY

    METHOD AcceptsCommand : BOOL
    VAR_INPUT
        SmokeClear : BOOL;
        FreezeStatClear : BOOL;
    END_VAR
    END_METHOD

    METHOD Update : REAL
    VAR_INPUT
        SupplyTemp : REAL;
        ScheduledSetpoint : REAL;
        OutdoorTemp : REAL;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: EcoStrategy is a concrete implementation in the Strategy example. *)
FUNCTION_BLOCK EcoStrategy IMPLEMENTS IAhuStrategy
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    Filter : Pt1Filter;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Filter.Initialize();
Filter.Configure(TimeConstant := T#5s, Gain := REAL#1.0);
ClearError();
END_METHOD

METHOD PUBLIC Reset
OutputValue := REAL#0.0;
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := WORD#0;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC AcceptsCommand : BOOL
VAR_INPUT
    SmokeClear : BOOL;
    FreezeStatClear : BOOL;
END_VAR
AcceptsCommand := SmokeClear AND FreezeStatClear;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    SupplyTemp : REAL;
    ScheduledSetpoint : REAL;
    OutdoorTemp : REAL;
END_VAR
VAR
    Error : REAL;
END_VAR
(* Filter is kept on the snapshot side for telemetry smoothing; the
   deadband and gain operate on the raw supply temp so the controller
   reacts to step changes within the same scan. *)
Filter.Update(Sample := SupplyTemp);
Error := ScheduledSetpoint - SupplyTemp;
IF ABS(Error) <= REAL#2.0 THEN
    OutputValue := REAL#0.0;
ELSE
    OutputValue := LIMIT(REAL#0.0, Error * REAL#8.0, REAL#100.0);
END_IF;
Update := OutputValue;
END_METHOD

PUBLIC PROPERTY ModeId : INT
GET
    ModeId := INT#1;
END_GET
END_PROPERTY

PUBLIC PROPERTY Output : REAL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: ComfortStrategy is a concrete implementation in the Strategy example. *)
FUNCTION_BLOCK ComfortStrategy IMPLEMENTS IAhuStrategy
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    Filter : Pt1Filter;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Filter.Initialize();
Filter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ClearError();
END_METHOD

METHOD PUBLIC Reset
OutputValue := REAL#0.0;
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := WORD#0;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC AcceptsCommand : BOOL
VAR_INPUT
    SmokeClear : BOOL;
    FreezeStatClear : BOOL;
END_VAR
AcceptsCommand := SmokeClear;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    SupplyTemp : REAL;
    ScheduledSetpoint : REAL;
    OutdoorTemp : REAL;
END_VAR
VAR
    Error : REAL;
END_VAR
(* Same approach as Eco: deadband/gain on raw supply, filter
   advanced for telemetry only. Strict inequality so Comfort reacts
   at the deadband boundary (tighter response than Eco). *)
Filter.Update(Sample := SupplyTemp);
Error := ScheduledSetpoint - SupplyTemp;
IF ABS(Error) < REAL#0.5 THEN
    OutputValue := REAL#0.0;
ELSE
    OutputValue := LIMIT(REAL#0.0, Error * REAL#20.0, REAL#100.0);
END_IF;
Update := OutputValue;
END_METHOD

PUBLIC PROPERTY ModeId : INT
GET
    ModeId := INT#2;
END_GET
END_PROPERTY

PUBLIC PROPERTY Output : REAL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: FrostProtectStrategy is a concrete implementation in the Strategy example. *)
FUNCTION_BLOCK FrostProtectStrategy IMPLEMENTS IAhuStrategy
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
OutputValue := REAL#0.0;
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := WORD#0;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC AcceptsCommand : BOOL
VAR_INPUT
    SmokeClear : BOOL;
    FreezeStatClear : BOOL;
END_VAR
AcceptsCommand := SmokeClear;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    SupplyTemp : REAL;
    ScheduledSetpoint : REAL;
    OutdoorTemp : REAL;
END_VAR
VAR
    FrostTarget : REAL;
END_VAR
FrostTarget := MAX(ScheduledSetpoint, REAL#5.0);
OutputValue := LIMIT(REAL#20.0, (FrostTarget - SupplyTemp) * REAL#30.0, REAL#100.0);
Update := OutputValue;
END_METHOD

PUBLIC PROPERTY ModeId : INT
GET
    ModeId := INT#3;
END_GET
END_PROPERTY

PUBLIC PROPERTY Output : REAL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: AhuController owns the collaborating objects used by this Strategy example. *)
FUNCTION_BLOCK AhuController
VAR
    Eco : EcoStrategy;
    Comfort : ComfortStrategy;
    Frost : FrostProtectStrategy;
    ActiveStrategy : IAhuStrategy;
    StateValue : INT;
    ActiveModeValue : INT;
    HeatingCommandValue : REAL;
    AlarmActiveValue : BOOL;
    AlarmCodeValue : DWORD;
    RuntimeFan : OntimeMeter;
    AlarmQueue : DwordFifo16;
END_VAR

METHOD PUBLIC Initialize
Eco.Initialize();
Comfort.Initialize();
Frost.Initialize();
RuntimeFan.Initialize();
RuntimeFan.Reset();
AlarmQueue.Initialize();
AlarmQueue.Reset();
StateValue := INT#0;
ActiveModeValue := INT#0;
HeatingCommandValue := REAL#0.0;
AlarmActiveValue := FALSE;
AlarmCodeValue := DWORD#0;
END_METHOD

METHOD PRIVATE RaiseAlarm
VAR_INPUT
    Code : DWORD;
END_VAR
AlarmActiveValue := TRUE;
AlarmCodeValue := Code;
StateValue := INT#900;
IF AlarmQueue.Push(Value := Code) THEN
END_IF;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    ModeRequest : INT;
    OutdoorTemp : REAL;
    SupplyTemp : REAL;
    SupplySetpoint : REAL;
    SmokeClear : BOOL;
    FreezeStatClear : BOOL;
    SupplyFanHealthy : BOOL;
END_VAR
IF NOT SmokeClear THEN
    RaiseAlarm(Code := DWORD#16#A100);
    HeatingCommandValue := REAL#0.0;
ELSE
    IF (OutdoorTemp < REAL#-5.0) OR NOT FreezeStatClear THEN
        ActiveStrategy := Frost;
    ELSIF ModeRequest = INT#1 THEN
        ActiveStrategy := Eco;
    ELSE
        ActiveStrategy := Comfort;
    END_IF;

    ActiveModeValue := ActiveStrategy.ModeId;
    IF NOT SupplyFanHealthy THEN
        RaiseAlarm(Code := DWORD#16#A101);
    ELSIF ActiveStrategy.AcceptsCommand(SmokeClear := SmokeClear, FreezeStatClear := FreezeStatClear) THEN
        StateValue := INT#200;
        HeatingCommandValue := ActiveStrategy.Update(SupplyTemp := SupplyTemp, ScheduledSetpoint := SupplySetpoint, OutdoorTemp := OutdoorTemp);
        RuntimeFan.Update(Input := TRUE);
    ELSE
        RaiseAlarm(Code := DWORD#16#B100);
    END_IF;
END_IF;
Update := HeatingCommandValue;
END_METHOD

PUBLIC PROPERTY State : INT
GET
    State := StateValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ActiveMode : INT
GET
    ActiveMode := ActiveModeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HeatingCommand : REAL
GET
    HeatingCommand := HeatingCommandValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmActive : BOOL
GET
    AlarmActive := AlarmActiveValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Ahu : AhuController;
    Initialized : BOOL;
    ModeRequest : INT;
    OutdoorTempRaw : INT;
    SupplyTempRaw : INT;
    SmokeClear : BOOL;
    FreezeStatClear : BOOL;
    SupplyFanHealthy : BOOL;
    HeatingValveRaw : INT;
    SupplyFanEnable : BOOL;
    FireAlarmRelay : BOOL;
    OutdoorTemp : REAL;
    SupplyTemp : REAL;
END_VAR

OutdoorTemp := TO_REAL(OutdoorTempRaw) / REAL#10.0;
SupplyTemp := TO_REAL(SupplyTempRaw) / REAL#10.0;
(* Initialize once on cold start; scanning Initialize every cycle would
   reset RuntimeFan, clear AlarmQueue, and force StateValue back to 0. *)
IF NOT Initialized THEN
    Ahu.Initialize();
    Initialized := TRUE;
END_IF;
HeatingValveRaw := TO_INT(Ahu.Update(ModeRequest := ModeRequest, OutdoorTemp := OutdoorTemp, SupplyTemp := SupplyTemp, SupplySetpoint := REAL#21.0, SmokeClear := SmokeClear, FreezeStatClear := FreezeStatClear, SupplyFanHealthy := SupplyFanHealthy) * REAL#10.0);
SupplyFanEnable := NOT Ahu.AlarmActive;
FireAlarmRelay := NOT SmokeClear;
END_PROGRAM
