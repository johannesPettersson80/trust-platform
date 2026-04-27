(* Pattern role: IInfeedConveyor is the contract boundary for the Mediator example. *)
INTERFACE IInfeedConveyor
    PROPERTY LaneId : INT
    GET
    END_GET
    END_PROPERTY
    PROPERTY Requesting : BOOL
    GET
    END_GET
    END_PROPERTY
    PROPERTY Jammed : BOOL
    GET
    END_GET
    END_PROPERTY
    PROPERTY Granted : BOOL
    GET
    END_GET
    END_PROPERTY
    METHOD UpdateLane
    VAR_INPUT
        Request : BOOL;
        Jam : BOOL;
    END_VAR
    END_METHOD
    METHOD Grant
    VAR_INPUT
        Enable : BOOL;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: InfeedConveyor is a concrete implementation in the Mediator example. *)
FUNCTION_BLOCK InfeedConveyor IMPLEMENTS IInfeedConveyor
VAR
    LaneIdValue : INT;
    RequestingValue : BOOL;
    JammedValue : BOOL;
    GrantedValue : BOOL;
END_VAR
METHOD PUBLIC Configure
VAR_INPUT
    LaneId : INT;
END_VAR
LaneIdValue := LaneId;
END_METHOD
METHOD PUBLIC UpdateLane
VAR_INPUT
    Request : BOOL;
    Jam : BOOL;
END_VAR
RequestingValue := Request;
JammedValue := Jam;
END_METHOD
METHOD PUBLIC Grant
VAR_INPUT
    Enable : BOOL;
END_VAR
GrantedValue := Enable;
END_METHOD
PUBLIC PROPERTY LaneId : INT
GET
    LaneId := LaneIdValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY Requesting : BOOL
GET
    Requesting := RequestingValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY Jammed : BOOL
GET
    Jammed := JammedValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY Granted : BOOL
GET
    Granted := GrantedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: MergeMediator owns the collaborating objects used by this Mediator example. *)
FUNCTION_BLOCK MergeMediator
VAR
    Lane1 : InfeedConveyor;
    Lane2 : InfeedConveyor;
    Lane3 : InfeedConveyor;
    ActiveLaneIdValue : INT;
    GrantedCounter : DwordCounter;
    JamCountValue : INT;
END_VAR
METHOD PUBLIC Initialize
Lane1.Configure(LaneId := INT#1);
Lane2.Configure(LaneId := INT#2);
Lane3.Configure(LaneId := INT#3);
GrantedCounter.Initialize();
GrantedCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD
METHOD PRIVATE TryGrant : BOOL
VAR_INPUT
    Lane : IInfeedConveyor;
    DownstreamReady : BOOL;
END_VAR
IF (ActiveLaneIdValue = INT#0) AND DownstreamReady AND Lane.Requesting AND NOT Lane.Jammed THEN
    ActiveLaneIdValue := Lane.LaneId;
    Lane.Grant(Enable := TRUE);
    GrantedCounter.Update(Up := TRUE, Down := FALSE);
    TryGrant := TRUE;
ELSE
    Lane.Grant(Enable := FALSE);
    TryGrant := FALSE;
END_IF;
IF Lane.Jammed THEN
    JamCountValue := JamCountValue + INT#1;
END_IF;
END_METHOD
METHOD PUBLIC Update : INT
VAR_INPUT
    Lane1Request : BOOL;
    Lane1Jam : BOOL;
    Lane2Request : BOOL;
    Lane2Jam : BOOL;
    Lane3Request : BOOL;
    Lane3Jam : BOOL;
    DownstreamReady : BOOL;
END_VAR
ActiveLaneIdValue := INT#0;
JamCountValue := INT#0;
(* DwordCounter wraps COUNT_DR (edge-triggered: UP AND NOT LastUp).
   Drop the UP edge first so consecutive scans with grants each emit
   a fresh rising edge and actually increment GrantedCount. *)
GrantedCounter.Update(Up := FALSE, Down := FALSE);
Lane1.UpdateLane(Request := Lane1Request, Jam := Lane1Jam);
Lane2.UpdateLane(Request := Lane2Request, Jam := Lane2Jam);
Lane3.UpdateLane(Request := Lane3Request, Jam := Lane3Jam);
TryGrant(Lane := Lane2, DownstreamReady := DownstreamReady);
TryGrant(Lane := Lane1, DownstreamReady := DownstreamReady);
TryGrant(Lane := Lane3, DownstreamReady := DownstreamReady);
Update := ActiveLaneIdValue;
END_METHOD
PUBLIC PROPERTY ActiveLaneId : INT
GET
    ActiveLaneId := ActiveLaneIdValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY GrantedCount : DWORD
GET
    GrantedCount := GrantedCounter.Count;
END_GET
END_PROPERTY
PUBLIC PROPERTY JamCount : INT
GET
    JamCount := JamCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Merge : MergeMediator;
    Lane1Request : BOOL;
    Lane2Request : BOOL;
    Lane3Request : BOOL;
    DownstreamReady : BOOL;
    Lane1GrantOut : BOOL;
    Lane2GrantOut : BOOL;
    Lane3GrantOut : BOOL;
END_VAR
Merge.Initialize();
Merge.Update(Lane1Request := Lane1Request, Lane1Jam := FALSE, Lane2Request := Lane2Request, Lane2Jam := FALSE, Lane3Request := Lane3Request, Lane3Jam := FALSE, DownstreamReady := DownstreamReady);
Lane1GrantOut := Merge.ActiveLaneId = INT#1;
Lane2GrantOut := Merge.ActiveLaneId = INT#2;
Lane3GrantOut := Merge.ActiveLaneId = INT#3;
END_PROGRAM
