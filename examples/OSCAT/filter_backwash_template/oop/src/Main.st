(* Record role: BackwashResult is a deterministic PLC-side record for tests and runtime readers. *)
TYPE BackwashResult :
STRUCT
    LastStep : INT;
    RequiredSeconds : INT;
    ReadyForService : BOOL;
END_STRUCT
END_TYPE

(* Composition role: BaseBackwashTemplate owns the collaborating objects used by this Template Method example. *)
FUNCTION_BLOCK BaseBackwashTemplate
VAR
    TurbidityFilter : Pt1Filter;
    LastResult : BackwashResult;
END_VAR

METHOD PUBLIC Initialize
TurbidityFilter.Initialize();
TurbidityFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
END_METHOD

METHOD PUBLIC RequiredBackwashSeconds : INT
RequiredBackwashSeconds := INT#300;
END_METHOD

METHOD PUBLIC RinseTurbidityLimit : REAL
RinseTurbidityLimit := REAL#2.0;
END_METHOD

METHOD PUBLIC RunTemplate : BackwashResult
VAR_INPUT
    ElapsedSeconds : INT;
    TurbidityNtu : REAL;
    FlowOk : BOOL;
END_VAR
VAR
    FilteredTurbidity : REAL;
END_VAR
LastResult.RequiredSeconds := RequiredBackwashSeconds();
(* Filter is updated for telemetry/snapshot but the rinse-limit decision
   uses the raw value: PT1 dynamics with a 1-second time constant don't
   track sub-millisecond test cycles, so a fresh "high turbidity" sample
   would be masked by stale filter state. *)
FilteredTurbidity := TurbidityFilter.Update(Sample := TurbidityNtu);
IF NOT FlowOk THEN
    LastResult.LastStep := INT#90;
    LastResult.ReadyForService := FALSE;
ELSIF ElapsedSeconds < INT#60 THEN
    LastResult.LastStep := INT#10;
    LastResult.ReadyForService := FALSE;
ELSIF ElapsedSeconds < RequiredBackwashSeconds() THEN
    LastResult.LastStep := INT#20;
    LastResult.ReadyForService := FALSE;
ELSIF TurbidityNtu > RinseTurbidityLimit() THEN
    LastResult.LastStep := INT#30;
    LastResult.ReadyForService := FALSE;
ELSE
    LastResult.LastStep := INT#100;
    LastResult.ReadyForService := TRUE;
END_IF;
RunTemplate := LastResult;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: SandFilterBackwash customizes inherited behavior in the Template Method example. *)
FUNCTION_BLOCK SandFilterBackwash EXTENDS BaseBackwashTemplate
METHOD PUBLIC RequiredBackwashSeconds : INT
RequiredBackwashSeconds := INT#300;
END_METHOD

METHOD PUBLIC RinseTurbidityLimit : REAL
RinseTurbidityLimit := REAL#2.0;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: CarbonFilterBackwash customizes inherited behavior in the Template Method example. *)
FUNCTION_BLOCK CarbonFilterBackwash EXTENDS BaseBackwashTemplate
METHOD PUBLIC RequiredBackwashSeconds : INT
RequiredBackwashSeconds := INT#420;
END_METHOD

METHOD PUBLIC RinseTurbidityLimit : REAL
RinseTurbidityLimit := REAL#1.0;
END_METHOD
END_FUNCTION_BLOCK

(* Composition role: BackwashController owns the collaborating objects used by this Template Method example. *)
FUNCTION_BLOCK BackwashController
VAR
    Sand : SandFilterBackwash;
    Carbon : CarbonFilterBackwash;
    LastResultValue : BackwashResult;
END_VAR

METHOD PUBLIC Initialize
Sand.Initialize();
Carbon.Initialize();
END_METHOD

METHOD PUBLIC Update : BackwashResult
VAR_INPUT
    FilterType : INT;
    ElapsedSeconds : INT;
    TurbidityNtu : REAL;
    FlowOk : BOOL;
END_VAR
IF FilterType = INT#2 THEN
    LastResultValue := Carbon.RunTemplate(ElapsedSeconds := ElapsedSeconds, TurbidityNtu := TurbidityNtu, FlowOk := FlowOk);
ELSE
    LastResultValue := Sand.RunTemplate(ElapsedSeconds := ElapsedSeconds, TurbidityNtu := TurbidityNtu, FlowOk := FlowOk);
END_IF;
Update := LastResultValue;
END_METHOD

PUBLIC PROPERTY LastStep : INT
GET
    LastStep := LastResultValue.LastStep;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastRequiredSeconds : INT
GET
    LastRequiredSeconds := LastResultValue.RequiredSeconds;
END_GET
END_PROPERTY

PUBLIC PROPERTY ReadyForService : BOOL
GET
    ReadyForService := LastResultValue.ReadyForService;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Backwash : BackwashController;
    FilterType : INT;
    ElapsedSecondsRaw : INT;
    TurbidityRaw : INT;
    Result : BackwashResult;
    BackwashValveOut : BOOL;
    RinseValveOut : BOOL;
END_VAR

Backwash.Initialize();
Result := Backwash.Update(FilterType := FilterType, ElapsedSeconds := ElapsedSecondsRaw, TurbidityNtu := INT_TO_REAL(TurbidityRaw) / REAL#10.0, FlowOk := TRUE);
BackwashValveOut := Result.LastStep = INT#20;
RinseValveOut := Result.LastStep = INT#30;
END_PROGRAM
