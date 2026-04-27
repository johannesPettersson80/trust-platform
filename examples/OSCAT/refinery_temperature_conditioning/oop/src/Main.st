(* Pattern role: ISignalSource is the contract boundary for the Decorator example. *)
INTERFACE ISignalSource EXTENDS IComponent
    PROPERTY Value : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Quality : BYTE
    GET
    END_GET
    END_PROPERTY

    METHOD Update
    END_METHOD
END_INTERFACE

(* Pattern role: RawAnalogInput is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK RawAnalogInput IMPLEMENTS ISignalSource
VAR
    ValueValue : REAL;
    QualityValue : BYTE;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    InputValue : REAL;
    InputQuality : BYTE;
END_VAR
ValueValue := InputValue;
QualityValue := InputQuality;
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
QualityValue := BYTE#2;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ValueValue := REAL#0.0;
QualityValue := BYTE#2;
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

METHOD PUBLIC Update
END_METHOD

PUBLIC PROPERTY Value : REAL
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Quality : BYTE
GET
    Quality := QualityValue;
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

(* Pattern role: VotingDecorator is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK VotingDecorator IMPLEMENTS ISignalSource
VAR
    Source1 : ISignalSource;
    Source2 : ISignalSource;
    Source3 : ISignalSource;
    ValueValue : REAL;
    QualityValue : BYTE;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
END_VAR

METHOD PUBLIC SetSources
VAR_INPUT
    First : ISignalSource;
    Second : ISignalSource;
    Third : ISignalSource;
END_VAR
Source1 := First;
Source2 := Second;
Source3 := Third;
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
QualityValue := BYTE#0;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ValueValue := REAL#0.0;
QualityValue := BYTE#0;
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

METHOD PUBLIC Update
Source1.Update();
Source2.Update();
Source3.Update();
IF (Source1.Quality = BYTE#2) AND (Source2.Quality = BYTE#2) THEN
    ValueValue := (Source1.Value + Source2.Value) / REAL#2.0;
    QualityValue := BYTE#2;
ELSIF (Source1.Quality = BYTE#2) AND (Source3.Quality = BYTE#2) THEN
    ValueValue := (Source1.Value + Source3.Value) / REAL#2.0;
    QualityValue := BYTE#2;
ELSIF (Source2.Quality = BYTE#2) AND (Source3.Quality = BYTE#2) THEN
    ValueValue := (Source2.Value + Source3.Value) / REAL#2.0;
    QualityValue := BYTE#2;
ELSE
    QualityValue := BYTE#0;
END_IF;
END_METHOD

PUBLIC PROPERTY Value : REAL
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Quality : BYTE
GET
    Quality := QualityValue;
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

(* Pattern role: RangeClampDecorator is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK RangeClampDecorator IMPLEMENTS ISignalSource
VAR
    Inner : ISignalSource;
    MinValue : REAL;
    MaxValue : REAL;
    ValueValue : REAL;
    QualityValue : BYTE;
    ClampFaultValue : BOOL;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
END_VAR

METHOD PUBLIC Wrap
VAR_INPUT
    InnerSource : ISignalSource;
    Minimum : REAL;
    Maximum : REAL;
END_VAR
Inner := InnerSource;
MinValue := Minimum;
MaxValue := Maximum;
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ValueValue := REAL#0.0;
QualityValue := BYTE#0;
ClampFaultValue := FALSE;
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

METHOD PUBLIC Update
Inner.Update();
ClampFaultValue := FALSE;
IF Inner.Value < MinValue THEN
    ValueValue := MinValue;
    QualityValue := BYTE#0;
    ClampFaultValue := TRUE;
ELSIF Inner.Value > MaxValue THEN
    ValueValue := MaxValue;
    QualityValue := BYTE#0;
    ClampFaultValue := TRUE;
ELSE
    ValueValue := Inner.Value;
    QualityValue := Inner.Quality;
END_IF;
END_METHOD

PUBLIC PROPERTY ClampFault : BOOL
GET
    ClampFault := ClampFaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Value : REAL
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Quality : BYTE
GET
    Quality := QualityValue;
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

(* Pattern role: SpikeRejectDecorator is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK SpikeRejectDecorator IMPLEMENTS ISignalSource
VAR
    Inner : ISignalSource;
    MaxStepValue : REAL;
    ValueValue : REAL;
    QualityValue : BYTE;
    LastGoodValue : REAL;
    HasLastGood : BOOL;
    RejectedCountValue : INT;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
END_VAR

METHOD PUBLIC Wrap
VAR_INPUT
    InnerSource : ISignalSource;
    MaxStep : REAL;
END_VAR
Inner := InnerSource;
MaxStepValue := MaxStep;
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ValueValue := REAL#0.0;
QualityValue := BYTE#0;
LastGoodValue := REAL#0.0;
HasLastGood := FALSE;
RejectedCountValue := INT#0;
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

METHOD PUBLIC Update
Inner.Update();
IF Inner.Quality <> BYTE#2 THEN
    ValueValue := Inner.Value;
    QualityValue := Inner.Quality;
ELSIF HasLastGood AND (ABS(Inner.Value - LastGoodValue) > MaxStepValue) THEN
    ValueValue := LastGoodValue;
    QualityValue := BYTE#1;
    RejectedCountValue := RejectedCountValue + INT#1;
ELSE
    ValueValue := Inner.Value;
    QualityValue := BYTE#2;
    LastGoodValue := Inner.Value;
    HasLastGood := TRUE;
END_IF;
END_METHOD

PUBLIC PROPERTY RejectedCount : INT
GET
    RejectedCount := RejectedCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Value : REAL
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Quality : BYTE
GET
    Quality := QualityValue;
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

(* Pattern role: Pt1SignalDecorator is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK Pt1SignalDecorator IMPLEMENTS ISignalSource
VAR
    Inner : ISignalSource;
    Filter : Pt1Filter;
    ValueValue : REAL;
    QualityValue : BYTE;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
END_VAR

METHOD PUBLIC Wrap
VAR_INPUT
    InnerSource : ISignalSource;
    TimeConstant : TIME;
END_VAR
Inner := InnerSource;
Filter.Initialize();
Filter.Configure(TimeConstant := TimeConstant, Gain := REAL#1.0);
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Filter.Initialize();
ClearError();
END_METHOD

METHOD PUBLIC Reset
Filter.Reset();
ValueValue := REAL#0.0;
QualityValue := BYTE#0;
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

METHOD PUBLIC Update
Inner.Update();
IF Inner.Quality = BYTE#2 THEN
    ValueValue := Filter.Update(Sample := Inner.Value);
    QualityValue := BYTE#2;
ELSE
    ValueValue := Inner.Value;
    QualityValue := Inner.Quality;
END_IF;
END_METHOD

PUBLIC PROPERTY Value : REAL
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Quality : BYTE
GET
    Quality := QualityValue;
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

(* Composition role: SignalConditioningPipeline owns the collaborating objects used by this Decorator example. *)
FUNCTION_BLOCK SignalConditioningPipeline
VAR
    Tc1 : RawAnalogInput;
    Tc2 : RawAnalogInput;
    Tc3 : RawAnalogInput;
    Voted : VotingDecorator;
    Clamped : RangeClampDecorator;
    SpikeRejected : SpikeRejectDecorator;
    Filtered : Pt1SignalDecorator;
    Conditioned : ISignalSource;
END_VAR

METHOD PUBLIC Initialize
Tc1.Initialize();
Tc2.Initialize();
Tc3.Initialize();
Voted.Initialize();
Clamped.Initialize();
SpikeRejected.Initialize();
Filtered.Initialize();
Voted.SetSources(First := Tc1, Second := Tc2, Third := Tc3);
Clamped.Wrap(InnerSource := Voted, Minimum := REAL#0.0, Maximum := REAL#400.0);
SpikeRejected.Wrap(InnerSource := Clamped, MaxStep := REAL#25.0);
Filtered.Wrap(InnerSource := SpikeRejected, TimeConstant := T#1s);
Conditioned := Filtered;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    Input1 : REAL;
    Quality1 : BYTE;
    Input2 : REAL;
    Quality2 : BYTE;
    Input3 : REAL;
    Quality3 : BYTE;
END_VAR
Tc1.Configure(InputValue := Input1, InputQuality := Quality1);
Tc2.Configure(InputValue := Input2, InputQuality := Quality2);
Tc3.Configure(InputValue := Input3, InputQuality := Quality3);
Conditioned.Update();
Update := Conditioned.Value;
END_METHOD

PUBLIC PROPERTY ConditionedValue : REAL
GET
    ConditionedValue := Conditioned.Value;
END_GET
END_PROPERTY

PUBLIC PROPERTY Quality : BYTE
GET
    Quality := Conditioned.Quality;
END_GET
END_PROPERTY

PUBLIC PROPERTY RejectedCount : INT
GET
    RejectedCount := SpikeRejected.RejectedCount;
END_GET
END_PROPERTY

PUBLIC PROPERTY ClampFault : BOOL
GET
    ClampFault := Clamped.ClampFault;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Conditioning : SignalConditioningPipeline;
    Tc1Raw : INT;
    Tc2Raw : INT;
    Tc3Raw : INT;
    ConditionedRaw : INT;
    GoodQualityOut : BOOL;
END_VAR

Conditioning.Initialize();
ConditionedRaw := REAL_TO_INT(Conditioning.Update(Input1 := INT_TO_REAL(Tc1Raw), Quality1 := BYTE#2, Input2 := INT_TO_REAL(Tc2Raw), Quality2 := BYTE#2, Input3 := INT_TO_REAL(Tc3Raw), Quality3 := BYTE#2));
GoodQualityOut := Conditioning.Quality = BYTE#2;
END_PROGRAM
