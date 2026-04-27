FUNCTION_BLOCK ClassicTemperatureConditioning
VAR
    Filter : Pt1Filter;
    LastGoodValue : REAL;
    HasLastGood : BOOL;
    RejectedCountValue : INT;
    QualityValue : BYTE;
    ClampFaultValue : BOOL;
    ConditionedValueValue : REAL;
END_VAR

METHOD PUBLIC Initialize
Filter.Initialize();
Filter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
LastGoodValue := REAL#0.0;
HasLastGood := FALSE;
RejectedCountValue := INT#0;
QualityValue := BYTE#0;
ClampFaultValue := FALSE;
ConditionedValueValue := REAL#0.0;
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
VAR
    VotedValue : REAL;
    VotedQuality : BYTE;
    ClampedValue : REAL;
    ClampedQuality : BYTE;
END_VAR
IF (Quality1 = BYTE#2) AND (Quality2 = BYTE#2) THEN
    VotedValue := (Input1 + Input2) / REAL#2.0;
    VotedQuality := BYTE#2;
ELSIF (Quality1 = BYTE#2) AND (Quality3 = BYTE#2) THEN
    VotedValue := (Input1 + Input3) / REAL#2.0;
    VotedQuality := BYTE#2;
ELSIF (Quality2 = BYTE#2) AND (Quality3 = BYTE#2) THEN
    VotedValue := (Input2 + Input3) / REAL#2.0;
    VotedQuality := BYTE#2;
ELSE
    VotedQuality := BYTE#0;
END_IF;

ClampFaultValue := FALSE;
IF VotedValue < REAL#0.0 THEN
    ClampedValue := REAL#0.0;
    ClampedQuality := BYTE#0;
    ClampFaultValue := TRUE;
ELSIF VotedValue > REAL#400.0 THEN
    ClampedValue := REAL#400.0;
    ClampedQuality := BYTE#0;
    ClampFaultValue := TRUE;
ELSE
    ClampedValue := VotedValue;
    ClampedQuality := VotedQuality;
END_IF;

IF ClampedQuality <> BYTE#2 THEN
    ConditionedValueValue := ClampedValue;
    QualityValue := ClampedQuality;
ELSIF HasLastGood AND (ABS(ClampedValue - LastGoodValue) > REAL#25.0) THEN
    ConditionedValueValue := LastGoodValue;
    QualityValue := BYTE#1;
    RejectedCountValue := RejectedCountValue + INT#1;
ELSE
    LastGoodValue := ClampedValue;
    HasLastGood := TRUE;
    ConditionedValueValue := Filter.Update(Sample := ClampedValue);
    QualityValue := BYTE#2;
END_IF;
Update := ConditionedValueValue;
END_METHOD

PUBLIC PROPERTY ConditionedValue : REAL
GET
    ConditionedValue := ConditionedValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Quality : BYTE
GET
    Quality := QualityValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RejectedCount : INT
GET
    RejectedCount := RejectedCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ClampFault : BOOL
GET
    ClampFault := ClampFaultValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Conditioning : ClassicTemperatureConditioning;
    Value : REAL;
END_VAR
Conditioning.Initialize();
Value := Conditioning.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#101.0, Quality2 := BYTE#2, Input3 := REAL#100.5, Quality3 := BYTE#2);
END_PROGRAM
