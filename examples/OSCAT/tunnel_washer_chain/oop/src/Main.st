(* Record role: WasherQualityEvent is a deterministic PLC-side record for tests and runtime readers. *)
TYPE WasherQualityEvent :
STRUCT
    SoilLevel : REAL;
    Conductivity : REAL;
    RinseConductivity : REAL;
    OperatorAvailable : BOOL;
END_STRUCT
END_TYPE

(* Pattern role: IWasherQualityHandler is the contract boundary for the Chain of Responsibility example. *)
INTERFACE IWasherQualityHandler
    PROPERTY HandlerId : INT
    GET
    END_GET
    END_PROPERTY

    METHOD SetNext
    VAR_INPUT
        NextHandler : IWasherQualityHandler;
    END_VAR
    END_METHOD

    METHOD Handle : BOOL
    VAR_INPUT
        Event : WasherQualityEvent;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: AutoDoseHandler is a concrete implementation in the Chain of Responsibility example. *)
FUNCTION_BLOCK AutoDoseHandler IMPLEMENTS IWasherQualityHandler
VAR
    Next : IWasherQualityHandler;
    HandledValue : BOOL;
END_VAR

METHOD PUBLIC SetNext
VAR_INPUT
    NextHandler : IWasherQualityHandler;
END_VAR
Next := NextHandler;
END_METHOD

METHOD PUBLIC Handle : BOOL
VAR_INPUT
    Event : WasherQualityEvent;
END_VAR
(* Reset on entry so HandledValue reflects only the current event. *)
HandledValue := FALSE;
IF (Event.SoilLevel > REAL#70.0) AND (Event.Conductivity < REAL#3.0) THEN
    HandledValue := TRUE;
    Handle := TRUE;
ELSE
    Handle := Next.Handle(Event := Event);
END_IF;
END_METHOD

PUBLIC PROPERTY HandlerId : INT
GET
    HandlerId := INT#1;
END_GET
END_PROPERTY

PUBLIC PROPERTY Handled : BOOL
GET
    Handled := HandledValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: ExtendRinseHandler is a concrete implementation in the Chain of Responsibility example. *)
FUNCTION_BLOCK ExtendRinseHandler IMPLEMENTS IWasherQualityHandler
VAR
    Next : IWasherQualityHandler;
    HandledValue : BOOL;
END_VAR

METHOD PUBLIC SetNext
VAR_INPUT
    NextHandler : IWasherQualityHandler;
END_VAR
Next := NextHandler;
END_METHOD

METHOD PUBLIC Handle : BOOL
VAR_INPUT
    Event : WasherQualityEvent;
END_VAR
(* Reset on entry so HandledValue reflects only the current event. *)
HandledValue := FALSE;
IF Event.RinseConductivity > REAL#1.5 THEN
    HandledValue := TRUE;
    Handle := TRUE;
ELSE
    Handle := Next.Handle(Event := Event);
END_IF;
END_METHOD

PUBLIC PROPERTY HandlerId : INT
GET
    HandlerId := INT#2;
END_GET
END_PROPERTY

PUBLIC PROPERTY Handled : BOOL
GET
    Handled := HandledValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: OperatorHoldHandler is a concrete implementation in the Chain of Responsibility example. *)
FUNCTION_BLOCK OperatorHoldHandler IMPLEMENTS IWasherQualityHandler
VAR
    Next : IWasherQualityHandler;
    HandledValue : BOOL;
END_VAR

METHOD PUBLIC SetNext
VAR_INPUT
    NextHandler : IWasherQualityHandler;
END_VAR
Next := NextHandler;
END_METHOD

METHOD PUBLIC Handle : BOOL
VAR_INPUT
    Event : WasherQualityEvent;
END_VAR
(* Reset on entry so HandledValue reflects only the current event. *)
HandledValue := FALSE;
IF Event.OperatorAvailable THEN
    HandledValue := TRUE;
    Handle := TRUE;
ELSE
    Handle := Next.Handle(Event := Event);
END_IF;
END_METHOD

PUBLIC PROPERTY HandlerId : INT
GET
    HandlerId := INT#3;
END_GET
END_PROPERTY

PUBLIC PROPERTY Handled : BOOL
GET
    Handled := HandledValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RejectBatchHandler is a concrete implementation in the Chain of Responsibility example. *)
FUNCTION_BLOCK RejectBatchHandler IMPLEMENTS IWasherQualityHandler
VAR
    HandledValue : BOOL;
END_VAR

METHOD PUBLIC SetNext
VAR_INPUT
    NextHandler : IWasherQualityHandler;
END_VAR
END_METHOD

METHOD PUBLIC Handle : BOOL
VAR_INPUT
    Event : WasherQualityEvent;
END_VAR
(* Last handler in the chain: always handles. Reset on entry so the
   flag reflects only this event. *)
HandledValue := TRUE;
Handle := TRUE;
END_METHOD

PUBLIC PROPERTY HandlerId : INT
GET
    HandlerId := INT#4;
END_GET
END_PROPERTY

PUBLIC PROPERTY Handled : BOOL
GET
    Handled := HandledValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: TunnelWasherQualityController owns the collaborating objects used by this Chain of Responsibility example. *)
FUNCTION_BLOCK TunnelWasherQualityController
VAR
    AutoDose : AutoDoseHandler;
    ExtendRinse : ExtendRinseHandler;
    OperatorHold : OperatorHoldHandler;
    RejectBatch : RejectBatchHandler;
    Chain : IWasherQualityHandler;
    AutoDoseCounter : DwordCounter;
    RejectCounter : DwordCounter;
    LastHandlerIdValue : INT;
END_VAR

METHOD PUBLIC Initialize
AutoDose.SetNext(NextHandler := ExtendRinse);
ExtendRinse.SetNext(NextHandler := OperatorHold);
OperatorHold.SetNext(NextHandler := RejectBatch);
Chain := AutoDose;
AutoDoseCounter.Initialize();
AutoDoseCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
RejectCounter.Initialize();
RejectCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PUBLIC Evaluate : INT
VAR_INPUT
    SoilLevel : REAL;
    Conductivity : REAL;
    RinseConductivity : REAL;
    OperatorAvailable : BOOL;
END_VAR
VAR
    Event : WasherQualityEvent;
END_VAR
Event.SoilLevel := SoilLevel;
Event.Conductivity := Conductivity;
Event.RinseConductivity := RinseConductivity;
Event.OperatorAvailable := OperatorAvailable;
(* DwordCounter wraps COUNT_DR which is edge-triggered (UP AND NOT
   LastUp). Drop the UP edge first so consecutive same-class events
   each generate a fresh rising edge and actually increment. *)
AutoDoseCounter.Update(Up := FALSE, Down := FALSE);
RejectCounter.Update(Up := FALSE, Down := FALSE);
Chain.Handle(Event := Event);
IF AutoDose.Handled THEN
    LastHandlerIdValue := AutoDose.HandlerId;
    AutoDoseCounter.Update(Up := TRUE, Down := FALSE);
ELSIF ExtendRinse.Handled THEN
    LastHandlerIdValue := ExtendRinse.HandlerId;
ELSIF OperatorHold.Handled THEN
    LastHandlerIdValue := OperatorHold.HandlerId;
ELSE
    LastHandlerIdValue := RejectBatch.HandlerId;
    RejectCounter.Update(Up := TRUE, Down := FALSE);
END_IF;
Evaluate := LastHandlerIdValue;
END_METHOD

PUBLIC PROPERTY LastHandlerId : INT
GET
    LastHandlerId := LastHandlerIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RejectCount : DWORD
GET
    RejectCount := RejectCounter.Count;
END_GET
END_PROPERTY

PUBLIC PROPERTY AutoDoseCount : DWORD
GET
    AutoDoseCount := AutoDoseCounter.Count;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Washer : TunnelWasherQualityController;
    SoilLevelRaw : INT;
    ConductivityRaw : INT;
    OperatorAvailable : BOOL;
    DosePumpOut : BOOL;
    RejectOut : BOOL;
END_VAR

Washer.Initialize();
Washer.Evaluate(SoilLevel := INT_TO_REAL(SoilLevelRaw), Conductivity := INT_TO_REAL(ConductivityRaw) / REAL#10.0, RinseConductivity := REAL#0.8, OperatorAvailable := OperatorAvailable);
DosePumpOut := Washer.LastHandlerId = INT#1;
RejectOut := Washer.LastHandlerId = INT#4;
END_PROGRAM
