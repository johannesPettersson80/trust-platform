(* Record role: PasteurizerResult is a deterministic PLC-side record for tests and runtime readers. *)
TYPE PasteurizerResult :
STRUCT
    ForwardDivert : BOOL;
    DivertToRecirc : BOOL;
    RequiredTemperature : REAL;
    RequiredHoldSeconds : INT;
    SequenceStep : INT;
END_STRUCT
END_TYPE

(* Record role: QualityAlarmEvent is a deterministic PLC-side record for tests and runtime readers. *)
TYPE QualityAlarmEvent :
STRUCT
    TemperatureC : REAL;
    FlowRateLpm : REAL;
    RequiredTemperatureC : REAL;
    OperatorAck : BOOL;
    SupervisorOnline : BOOL;
END_STRUCT
END_TYPE

(* Pattern role: IQualityAlarmHandler is the contract boundary for the Chain of Responsibility and Template Method example. *)
INTERFACE IQualityAlarmHandler
    METHOD SetNext
    VAR_INPUT
        NextHandler : IQualityAlarmHandler;
    END_VAR
    END_METHOD

    METHOD Handle : BOOL
    VAR_INPUT
        Event : QualityAlarmEvent;
    END_VAR
    END_METHOD

    PROPERTY HandlerId : INT
    GET
    END_GET
    END_PROPERTY
END_INTERFACE

(* Composition role: BasePasteurizerTemplate owns the collaborating objects used by this Chain of Responsibility and Template Method example. *)
FUNCTION_BLOCK BasePasteurizerTemplate
VAR
    TempFilter : Pt2Filter;
    DivertCounter : DwordCounter;
    LastResult : PasteurizerResult;
END_VAR

METHOD PUBLIC Initialize
TempFilter.Initialize();
TempFilter.Configure(TimeConstant := T#1s, Damping := REAL#1.0, Gain := REAL#1.0);
DivertCounter.Initialize();
DivertCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PUBLIC RequiredTemperature : REAL
RequiredTemperature := REAL#72.0;
END_METHOD

METHOD PUBLIC RequiredHoldSeconds : INT
RequiredHoldSeconds := INT#15;
END_METHOD

METHOD PUBLIC Preheat : BOOL
VAR_INPUT
    ProductPresent : BOOL;
END_VAR
Preheat := ProductPresent;
END_METHOD

METHOD PUBLIC Heat : BOOL
VAR_INPUT
    ProductTemp : REAL;
END_VAR
Heat := ProductTemp >= RequiredTemperature();
END_METHOD

METHOD PUBLIC Hold : BOOL
VAR_INPUT
    HoldSeconds : INT;
END_VAR
Hold := HoldSeconds >= RequiredHoldSeconds();
END_METHOD

METHOD PUBLIC Chill : BOOL
VAR_INPUT
    FlowRate : REAL;
END_VAR
Chill := FlowRate >= REAL#100.0;
END_METHOD

METHOD PUBLIC RouteProduct : BOOL
VAR_INPUT
    HeatOk : BOOL;
    HoldOk : BOOL;
    ChillOk : BOOL;
END_VAR
RouteProduct := HeatOk AND HoldOk AND ChillOk;
END_METHOD

METHOD PUBLIC RunTemplate : PasteurizerResult
VAR_INPUT
    ProductPresent : BOOL;
    ProductTemp : REAL;
    FlowRate : REAL;
    HoldSeconds : INT;
END_VAR
VAR
    HeatOk : BOOL;
    HoldOk : BOOL;
    ChillOk : BOOL;
END_VAR
LastResult.RequiredTemperature := RequiredTemperature();
LastResult.RequiredHoldSeconds := RequiredHoldSeconds();
(* SequenceStep legend: 10=preheat wait, 20=heat, 40=recirculate/divert, 50=forward product. *)
IF NOT Preheat(ProductPresent := ProductPresent) THEN
    LastResult.SequenceStep := INT#10;
    LastResult.ForwardDivert := FALSE;
ELSIF NOT Heat(ProductTemp := ProductTemp) THEN
    LastResult.SequenceStep := INT#20;
    LastResult.ForwardDivert := FALSE;
ELSE
    HeatOk := TRUE;
    HoldOk := Hold(HoldSeconds := HoldSeconds);
    ChillOk := Chill(FlowRate := FlowRate);
    LastResult.ForwardDivert := RouteProduct(HeatOk := HeatOk, HoldOk := HoldOk, ChillOk := ChillOk);
    IF LastResult.ForwardDivert THEN
        LastResult.SequenceStep := INT#50;
    ELSE
        LastResult.SequenceStep := INT#40;
    END_IF;
END_IF;
LastResult.DivertToRecirc := NOT LastResult.ForwardDivert;
(* DwordCounter wraps COUNT_DR (edge-triggered: UP AND NOT LastUp).
   Drop the UP edge first so consecutive divert batches each generate
   a fresh rising edge and actually increment. *)
DivertCounter.Update(Up := FALSE, Down := FALSE);
IF LastResult.DivertToRecirc THEN
    DivertCounter.Update(Up := TRUE, Down := FALSE);
END_IF;
RunTemplate := LastResult;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: MilkPasteurizer customizes inherited behavior in the Chain of Responsibility and Template Method example. *)
FUNCTION_BLOCK MilkPasteurizer EXTENDS BasePasteurizerTemplate
METHOD PUBLIC RequiredTemperature : REAL
RequiredTemperature := REAL#72.0;
END_METHOD

METHOD PUBLIC RequiredHoldSeconds : INT
RequiredHoldSeconds := INT#15;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: CreamPasteurizer customizes inherited behavior in the Chain of Responsibility and Template Method example. *)
FUNCTION_BLOCK CreamPasteurizer EXTENDS BasePasteurizerTemplate
METHOD PUBLIC RequiredTemperature : REAL
RequiredTemperature := REAL#75.0;
END_METHOD

METHOD PUBLIC RequiredHoldSeconds : INT
RequiredHoldSeconds := INT#20;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: LocalRecoveryHandler is a concrete implementation in the Chain of Responsibility and Template Method example. *)
FUNCTION_BLOCK LocalRecoveryHandler IMPLEMENTS IQualityAlarmHandler
VAR
    Next : IQualityAlarmHandler;
    HandlerIdValue : INT := INT#1;
    RecoveredValue : BOOL;
END_VAR

METHOD PUBLIC SetNext
VAR_INPUT
    NextHandler : IQualityAlarmHandler;
END_VAR
Next := NextHandler;
END_METHOD

METHOD PUBLIC Handle : BOOL
VAR_INPUT
    Event : QualityAlarmEvent;
END_VAR
(* Reset on entry so the flag reflects only the current event. *)
RecoveredValue := FALSE;
IF (Event.TemperatureC >= (Event.RequiredTemperatureC - REAL#1.0)) AND (Event.FlowRateLpm >= REAL#100.0) THEN
    RecoveredValue := TRUE;
    Handle := TRUE;
ELSE
    Handle := Next.Handle(Event := Event);
END_IF;
END_METHOD

METHOD PUBLIC ResetFlag
RecoveredValue := FALSE;
END_METHOD

PUBLIC PROPERTY HandlerId : INT
GET
    HandlerId := HandlerIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Recovered : BOOL
GET
    Recovered := RecoveredValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: OperatorAcknowledgeHandler is a concrete implementation in the Chain of Responsibility and Template Method example. *)
FUNCTION_BLOCK OperatorAcknowledgeHandler IMPLEMENTS IQualityAlarmHandler
VAR
    Next : IQualityAlarmHandler;
    HandlerIdValue : INT := INT#2;
    AcknowledgedValue : BOOL;
END_VAR

METHOD PUBLIC SetNext
VAR_INPUT
    NextHandler : IQualityAlarmHandler;
END_VAR
Next := NextHandler;
END_METHOD

METHOD PUBLIC Handle : BOOL
VAR_INPUT
    Event : QualityAlarmEvent;
END_VAR
(* Reset on entry so the flag reflects only the current event. *)
AcknowledgedValue := FALSE;
IF Event.OperatorAck THEN
    AcknowledgedValue := TRUE;
    Handle := TRUE;
ELSE
    Handle := Next.Handle(Event := Event);
END_IF;
END_METHOD

METHOD PUBLIC ResetFlag
AcknowledgedValue := FALSE;
END_METHOD

PUBLIC PROPERTY HandlerId : INT
GET
    HandlerId := HandlerIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Acknowledged : BOOL
GET
    Acknowledged := AcknowledgedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: SupervisorEscalateHandler is a concrete implementation in the Chain of Responsibility and Template Method example. *)
FUNCTION_BLOCK SupervisorEscalateHandler IMPLEMENTS IQualityAlarmHandler
VAR
    Next : IQualityAlarmHandler;
    HandlerIdValue : INT := INT#3;
    EscalatedValue : BOOL;
END_VAR

METHOD PUBLIC SetNext
VAR_INPUT
    NextHandler : IQualityAlarmHandler;
END_VAR
Next := NextHandler;
END_METHOD

METHOD PUBLIC Handle : BOOL
VAR_INPUT
    Event : QualityAlarmEvent;
END_VAR
(* Reset on entry so the flag reflects only the current event. *)
EscalatedValue := FALSE;
IF Event.SupervisorOnline THEN
    EscalatedValue := TRUE;
    Handle := TRUE;
ELSE
    Handle := Next.Handle(Event := Event);
END_IF;
END_METHOD

METHOD PUBLIC ResetFlag
EscalatedValue := FALSE;
END_METHOD

PUBLIC PROPERTY HandlerId : INT
GET
    HandlerId := HandlerIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Escalated : BOOL
GET
    Escalated := EscalatedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: BatchAbortHandler is a concrete implementation in the Chain of Responsibility and Template Method example. *)
FUNCTION_BLOCK BatchAbortHandler IMPLEMENTS IQualityAlarmHandler
VAR
    HandlerIdValue : INT := INT#4;
    AbortedValue : BOOL;
END_VAR

METHOD PUBLIC SetNext
VAR_INPUT
    NextHandler : IQualityAlarmHandler;
END_VAR
END_METHOD

METHOD PUBLIC Handle : BOOL
VAR_INPUT
    Event : QualityAlarmEvent;
END_VAR
(* Last handler in chain: always handles. Flag is set fresh each call. *)
AbortedValue := TRUE;
Handle := TRUE;
END_METHOD

METHOD PUBLIC ResetFlag
AbortedValue := FALSE;
END_METHOD

PUBLIC PROPERTY HandlerId : INT
GET
    HandlerId := HandlerIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Aborted : BOOL
GET
    Aborted := AbortedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: PasteurizerController owns the collaborating objects used by this Chain of Responsibility and Template Method example. *)
FUNCTION_BLOCK PasteurizerController
VAR
    Milk : MilkPasteurizer;
    Cream : CreamPasteurizer;
    Local : LocalRecoveryHandler;
    Operator : OperatorAcknowledgeHandler;
    Supervisor : SupervisorEscalateHandler;
    Abort : BatchAbortHandler;
    AlarmChain : IQualityAlarmHandler;
    LastResultValue : PasteurizerResult;
    LastHandlerIdValue : INT;
END_VAR

METHOD PUBLIC Initialize
Milk.Initialize();
Cream.Initialize();
Local.SetNext(NextHandler := Operator);
Operator.SetNext(NextHandler := Supervisor);
Supervisor.SetNext(NextHandler := Abort);
AlarmChain := Local;
END_METHOD

METHOD PUBLIC Update : PasteurizerResult
VAR_INPUT
    RecipeId : INT;
    ProductTemp : REAL;
    FlowRate : REAL;
    HoldSeconds : INT;
    OperatorAck : BOOL;
    SupervisorOnline : BOOL;
END_VAR
VAR
    Event : QualityAlarmEvent;
END_VAR
(* Clear cross-batch sticky state so LastHandlerId reflects only the current batch. *)
LastHandlerIdValue := INT#0;
Local.ResetFlag();
Operator.ResetFlag();
Supervisor.ResetFlag();
Abort.ResetFlag();
IF RecipeId = INT#2 THEN
    LastResultValue := Cream.RunTemplate(ProductPresent := TRUE, ProductTemp := ProductTemp, FlowRate := FlowRate, HoldSeconds := HoldSeconds);
ELSE
    LastResultValue := Milk.RunTemplate(ProductPresent := TRUE, ProductTemp := ProductTemp, FlowRate := FlowRate, HoldSeconds := HoldSeconds);
END_IF;

IF LastResultValue.DivertToRecirc THEN
    Event.TemperatureC := ProductTemp;
    Event.FlowRateLpm := FlowRate;
    Event.RequiredTemperatureC := LastResultValue.RequiredTemperature;
    Event.OperatorAck := OperatorAck;
    Event.SupervisorOnline := SupervisorOnline;
    AlarmChain.Handle(Event := Event);
    IF Abort.Aborted THEN
        LastHandlerIdValue := Abort.HandlerId;
    ELSIF Supervisor.Escalated THEN
        LastHandlerIdValue := Supervisor.HandlerId;
    ELSIF Operator.Acknowledged THEN
        LastHandlerIdValue := Operator.HandlerId;
    ELSIF Local.Recovered THEN
        LastHandlerIdValue := Local.HandlerId;
    END_IF;
END_IF;
Update := LastResultValue;
END_METHOD

PUBLIC PROPERTY ForwardDivert : BOOL
GET
    ForwardDivert := LastResultValue.ForwardDivert;
END_GET
END_PROPERTY

PUBLIC PROPERTY BatchAborted : BOOL
GET
    BatchAborted := Abort.Aborted;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastHandlerId : INT
GET
    LastHandlerId := LastHandlerIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RequiredTemperature : REAL
GET
    RequiredTemperature := LastResultValue.RequiredTemperature;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Pasteurizer : PasteurizerController;
    RecipeId : INT;
    ProductTempRaw : INT;
    FlowRateRaw : INT;
    OperatorAck : BOOL;
    SupervisorOnline : BOOL;
    Result : PasteurizerResult;
    ForwardValveOut : BOOL;
    DivertValveOut : BOOL;
    AlarmOut : BOOL;
END_VAR

Pasteurizer.Initialize();
Result := Pasteurizer.Update(RecipeId := RecipeId, ProductTemp := INT_TO_REAL(ProductTempRaw), FlowRate := INT_TO_REAL(FlowRateRaw), HoldSeconds := INT#15, OperatorAck := OperatorAck, SupervisorOnline := SupervisorOnline);
ForwardValveOut := Result.ForwardDivert;
DivertValveOut := Result.DivertToRecirc;
AlarmOut := Result.DivertToRecirc;
END_PROGRAM
