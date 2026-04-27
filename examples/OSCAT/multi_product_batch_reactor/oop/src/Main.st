(* Record role: BatchSnapshot is a deterministic PLC-side record for tests and runtime readers. *)
TYPE BatchSnapshot :
STRUCT
    RecipeId : INT;
    State : INT;
    SequenceStep : INT;
    AlarmActive : BOOL;
    AlarmCode : DWORD;
    BatchComplete : BOOL;
END_STRUCT
END_TYPE

(* Pattern role: IBatchSequencer is the contract boundary for the Factory and Template Method example. *)
INTERFACE IBatchSequencer EXTENDS IComponent
    PROPERTY NameId : INT
    GET
    END_GET
    END_PROPERTY

    METHOD ExecuteStep : INT
    VAR_INPUT
        ProductTemp : REAL;
        PH : REAL;
        HoldComplete : BOOL;
        SampleOk : BOOL;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: AcidSequencer is a concrete implementation in the Factory and Template Method example. *)
FUNCTION_BLOCK AcidSequencer IMPLEMENTS IBatchSequencer
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    StepValue : INT;
    PhCalibrator : Calibrator;
    TempFilter : Pt2Filter;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Reset();
PhCalibrator.Initialize();
PhCalibrator.Configure(OffsetTarget := REAL#0.0, ScaleTarget := REAL#1.0);
TempFilter.Initialize();
TempFilter.Configure(TimeConstant := T#2s, Damping := REAL#1.0, Gain := REAL#1.0);
END_METHOD

METHOD PUBLIC Reset
StepValue := INT#0;
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

METHOD PUBLIC ExecuteStep : INT
VAR_INPUT
    ProductTemp : REAL;
    PH : REAL;
    HoldComplete : BOOL;
    SampleOk : BOOL;
END_VAR
VAR
    FilteredTemp : REAL;
    CalibratedPH : REAL;
END_VAR
FilteredTemp := TempFilter.Update(Sample := ProductTemp);
CalibratedPH := PhCalibrator.Update(Sample := PH, CalibrateOffset := FALSE, CalibrateScale := FALSE);
IF StepValue = INT#0 THEN
    StepValue := INT#110;
ELSIF StepValue = INT#110 THEN
    IF CalibratedPH < REAL#6.8 THEN
        StepValue := INT#120;
    END_IF;
ELSIF StepValue = INT#120 THEN
    IF HoldComplete AND FilteredTemp >= REAL#55.0 THEN
        StepValue := INT#130;
    END_IF;
ELSIF StepValue = INT#130 THEN
    IF SampleOk THEN
        StepValue := INT#140;
    END_IF;
END_IF;
ExecuteStep := StepValue;
END_METHOD

PUBLIC PROPERTY NameId : INT
GET
    NameId := INT#1;
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

(* Pattern role: PolymerSequencer is a concrete implementation in the Factory and Template Method example. *)
FUNCTION_BLOCK PolymerSequencer IMPLEMENTS IBatchSequencer
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    StepValue : INT;
    TempController : PidController;
    RateFilter : DerivativeFilter;
    Limits : RealRange;
    Gains : PidGains;
    CommandValue : REAL;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.2;
TempController.Initialize();
TempController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
RateFilter.Initialize();
RateFilter.Configure(Gain := REAL#1.0);
Reset();
END_METHOD

METHOD PUBLIC Reset
StepValue := INT#0;
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

METHOD PUBLIC ExecuteStep : INT
VAR_INPUT
    ProductTemp : REAL;
    PH : REAL;
    HoldComplete : BOOL;
    SampleOk : BOOL;
END_VAR
VAR
    TempRate : REAL;
END_VAR
TempRate := RateFilter.Update(Sample := ProductTemp);
CommandValue := TempController.Update(Actual := ProductTemp, Target := REAL#72.0);
IF StepValue = INT#0 THEN
    StepValue := INT#210;
ELSIF StepValue = INT#210 THEN
    IF CommandValue >= REAL#0.0 THEN
        StepValue := INT#220;
    END_IF;
ELSIF StepValue = INT#220 THEN
    IF HoldComplete AND TempRate >= REAL#0.0 THEN
        StepValue := INT#230;
    END_IF;
ELSIF StepValue = INT#230 THEN
    IF SampleOk AND PH >= REAL#6.5 THEN
        StepValue := INT#240;
    END_IF;
END_IF;
ExecuteStep := StepValue;
END_METHOD

PUBLIC PROPERTY NameId : INT
GET
    NameId := INT#2;
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

(* Pattern role: BaseWashSequencer is a concrete implementation in the Factory and Template Method example. *)
FUNCTION_BLOCK BaseWashSequencer IMPLEMENTS IBatchSequencer
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    StepValue : INT;
    AgitatorPwm : PwmController;
    WashFilter : Pt1Filter;
    Limits : RealRange;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
AgitatorPwm.Initialize();
AgitatorPwm.Configure(Frequency := REAL#2.0);
WashFilter.Initialize();
WashFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
Reset();
END_METHOD

METHOD PUBLIC Reset
StepValue := INT#0;
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

METHOD PUBLIC ExecuteStep : INT
VAR_INPUT
    ProductTemp : REAL;
    PH : REAL;
    HoldComplete : BOOL;
    SampleOk : BOOL;
END_VAR
VAR
    FilteredTemp : REAL;
    Pulse : BOOL;
END_VAR
FilteredTemp := WashFilter.Update(Sample := ProductTemp);
Pulse := AgitatorPwm.Update(ControlInput := REAL#0.5);
IF StepValue = INT#0 THEN
    StepValue := INT#310;
ELSIF StepValue = INT#310 THEN
    IF Pulse THEN
        StepValue := INT#320;
    END_IF;
ELSIF StepValue = INT#320 THEN
    IF HoldComplete AND FilteredTemp >= REAL#35.0 THEN
        StepValue := INT#330;
    END_IF;
ELSIF StepValue = INT#330 THEN
    IF SampleOk THEN
        StepValue := INT#340;
    END_IF;
END_IF;
ExecuteStep := StepValue;
END_METHOD

PUBLIC PROPERTY NameId : INT
GET
    NameId := INT#3;
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

(* Composition role: SequencerFactory owns the collaborating objects used by this Factory and Template Method example. *)
FUNCTION_BLOCK SequencerFactory
VAR
    Acid : AcidSequencer;
    Polymer : PolymerSequencer;
    BaseWash : BaseWashSequencer;
    InvalidRecipeValue : BOOL;
END_VAR

METHOD PUBLIC Initialize
Acid.Initialize();
Polymer.Initialize();
BaseWash.Initialize();
InvalidRecipeValue := FALSE;
END_METHOD

METHOD PUBLIC Build : IBatchSequencer
VAR_INPUT
    RecipeId : INT;
END_VAR
InvalidRecipeValue := FALSE;
CASE RecipeId OF
    INT#1:
        Build := Acid;
    INT#2:
        Build := Polymer;
    INT#3:
        Build := BaseWash;
ELSE
    InvalidRecipeValue := TRUE;
    Build := Acid;
END_CASE;
END_METHOD

PUBLIC PROPERTY InvalidRecipe : BOOL
GET
    InvalidRecipe := InvalidRecipeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: BatchReactor owns the collaborating objects used by this Factory and Template Method example. *)
FUNCTION_BLOCK BatchReactor
VAR
    Factory : SequencerFactory;
    ActiveSequencer : IBatchSequencer;
    SequencerSelected : BOOL;
    StateValue : INT;
    StepValue : INT;
    RecipeIdValue : INT;
    AlarmActiveValue : BOOL;
    AlarmCodeValue : DWORD;
    BatchCompleteValue : BOOL;
    AlarmLog : DwordFifo32;
    ProductTempAlarm : HysteresisSwitch;
    TempAlarmLimits : RealRange;
    Clock : CalendarClock;
    SnapshotValue : BatchSnapshot;
END_VAR

METHOD PUBLIC Initialize
Factory.Initialize();
AlarmLog.Initialize();
AlarmLog.Reset();
TempAlarmLimits.Low := REAL#0.0;
TempAlarmLimits.High := REAL#95.0;
ProductTempAlarm.Initialize();
ProductTempAlarm.SetLimits(Limits := TempAlarmLimits);
Clock.Initialize();
Clock.Configure(LocationLatitude := REAL#59.3293, LocationLongitude := REAL#18.0686, OffsetMinutes := INT#60, DstEnabled := TRUE, LanguageIndex := INT#1);
Reset();
END_METHOD

METHOD PUBLIC Reset
SequencerSelected := FALSE;
StateValue := INT#0;
StepValue := INT#0;
RecipeIdValue := INT#0;
AlarmActiveValue := FALSE;
AlarmCodeValue := DWORD#0;
BatchCompleteValue := FALSE;
AlarmLog.Reset();
END_METHOD

METHOD PRIVATE RaiseAlarm
VAR_INPUT
    Code : DWORD;
END_VAR
AlarmActiveValue := TRUE;
AlarmCodeValue := Code;
StateValue := INT#900;
IF AlarmLog.Push(Value := Code) THEN
END_IF;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    AbortCommand : BOOL;
    LidClosedA : BOOL;
    LidClosedB : BOOL;
    EStopHealthy : BOOL;
    PressureReliefIntact : BOOL;
    RecipeId : INT;
    ProductTemp : REAL;
    PH : REAL;
    HoldComplete : BOOL;
    SampleOk : BOOL;
END_VAR
IF AbortCommand OR NOT EStopHealthy OR NOT PressureReliefIntact THEN
    RaiseAlarm(Code := DWORD#16#A001);
ELSIF NOT (LidClosedA AND LidClosedB) AND (StateValue <> INT#0 OR StartCommand) THEN
    (* Alarm if either: in motion with lid breach (interlock loss) OR
       operator commands start with lid still open (start permissive). *)
    RaiseAlarm(Code := DWORD#16#A002);
ELSIF ProductTempAlarm.Update(MeasuredValue := ProductTemp) THEN
    RaiseAlarm(Code := DWORD#16#A003);
ELSE
    Clock.Update(Utc := DT#2026-04-26-10:00:00);
    CASE StateValue OF
        INT#0:
            IF StartCommand AND LidClosedA AND LidClosedB AND EStopHealthy AND PressureReliefIntact THEN
                ActiveSequencer := Factory.Build(RecipeId := RecipeId);
                IF Factory.InvalidRecipe THEN
                    RaiseAlarm(Code := DWORD#16#B001);
                ELSE
                    RecipeIdValue := RecipeId;
                    SequencerSelected := TRUE;
                    StateValue := INT#100;
                END_IF;
            END_IF;
        INT#100:
            StepValue := ActiveSequencer.ExecuteStep(ProductTemp := ProductTemp, PH := PH, HoldComplete := HoldComplete, SampleOk := SampleOk);
            IF StepValue >= INT#140 THEN
                StateValue := INT#800;
                BatchCompleteValue := TRUE;
            ELSIF StepValue >= INT#240 THEN
                StateValue := INT#800;
                BatchCompleteValue := TRUE;
            ELSIF StepValue >= INT#340 THEN
                StateValue := INT#800;
                BatchCompleteValue := TRUE;
            END_IF;
        INT#800:
            IF NOT StartCommand THEN
                StateValue := INT#0;
            END_IF;
    ELSE
        StateValue := INT#0;
    END_CASE;
END_IF;
Update := StateValue;
END_METHOD

METHOD PUBLIC Snapshot : BatchSnapshot
SnapshotValue.RecipeId := RecipeIdValue;
SnapshotValue.State := StateValue;
SnapshotValue.SequenceStep := StepValue;
SnapshotValue.AlarmActive := AlarmActiveValue;
SnapshotValue.AlarmCode := AlarmCodeValue;
SnapshotValue.BatchComplete := BatchCompleteValue;
Snapshot := SnapshotValue;
END_METHOD

PUBLIC PROPERTY State : INT
GET
    State := StateValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RecipeId : INT
GET
    RecipeId := RecipeIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmActive : BOOL
GET
    AlarmActive := AlarmActiveValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY BatchComplete : BOOL
GET
    BatchComplete := BatchCompleteValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Reactor : BatchReactor;
    StartCommand : BOOL;
    AbortCommand : BOOL;
    LidClosedA : BOOL;
    LidClosedB : BOOL;
    EStopHealthy : BOOL;
    PressureReliefIntact : BOOL;
    RecipeIdRaw : INT;
    ProductTempRaw : INT;
    PhRaw : INT;
    SteamValveRaw : INT;
    CoolingValveRaw : INT;
    AgitatorSpeedRaw : INT;
    BatchCompleteOut : BOOL;
    AlarmOut : BOOL;
    ProductTemp : REAL;
    PH : REAL;
    StateValue : INT;
END_VAR

ProductTemp := TO_REAL(ProductTempRaw) / REAL#10.0;
PH := TO_REAL(PhRaw) / REAL#100.0;
Reactor.Initialize();
StateValue := Reactor.Update(
    StartCommand := StartCommand,
    AbortCommand := AbortCommand,
    LidClosedA := LidClosedA,
    LidClosedB := LidClosedB,
    EStopHealthy := EStopHealthy,
    PressureReliefIntact := PressureReliefIntact,
    RecipeId := RecipeIdRaw,
    ProductTemp := ProductTemp,
    PH := PH,
    HoldComplete := TRUE,
    SampleOk := TRUE);
BatchCompleteOut := Reactor.BatchComplete;
AlarmOut := Reactor.AlarmActive;
END_PROGRAM
