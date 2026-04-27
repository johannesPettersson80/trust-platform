(* Pattern role: IMoistureSource is the contract boundary for the Decorator and Strategy example. *)
INTERFACE IMoistureSource
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

(* Pattern role: IDryingStrategy is the contract boundary for the Decorator and Strategy example. *)
INTERFACE IDryingStrategy
    PROPERTY ProfileId : INT
    GET
    END_GET
    END_PROPERTY
    METHOD HeatCommand : REAL
    VAR_INPUT
        MoisturePct : REAL;
    END_VAR
    END_METHOD
    METHOD FanSpeed : REAL
    VAR_INPUT
        MoisturePct : REAL;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: RawMoistureInput is a concrete implementation in the Decorator and Strategy example. *)
FUNCTION_BLOCK RawMoistureInput IMPLEMENTS IMoistureSource
VAR
    ValueValue : REAL;
    QualityValue : BYTE := BYTE#2;
END_VAR
METHOD PUBLIC SetRaw
VAR_INPUT
    MoisturePct : REAL;
END_VAR
ValueValue := MoisturePct;
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
END_FUNCTION_BLOCK

(* Pattern role: MoistureClampDecorator is a concrete implementation in the Decorator and Strategy example. *)
FUNCTION_BLOCK MoistureClampDecorator IMPLEMENTS IMoistureSource
VAR
    Inner : IMoistureSource;
    ValueValue : REAL;
    QualityValue : BYTE;
END_VAR
METHOD PUBLIC Wrap
VAR_INPUT
    Source : IMoistureSource;
END_VAR
Inner := Source;
END_METHOD
METHOD PUBLIC Update
Inner.Update();
IF Inner.Value < REAL#0.0 THEN
    ValueValue := REAL#0.0;
    QualityValue := BYTE#0;
ELSIF Inner.Value > REAL#100.0 THEN
    ValueValue := REAL#100.0;
    QualityValue := BYTE#0;
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
END_FUNCTION_BLOCK

(* Pattern role: MoistureFilterDecorator is a concrete implementation in the Decorator and Strategy example. *)
FUNCTION_BLOCK MoistureFilterDecorator IMPLEMENTS IMoistureSource
VAR
    Inner : IMoistureSource;
    Filter : Pt1Filter;
    ValueValue : REAL;
    QualityValue : BYTE;
END_VAR
METHOD PUBLIC Wrap
VAR_INPUT
    Source : IMoistureSource;
END_VAR
Inner := Source;
Filter.Initialize();
Filter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
END_METHOD
METHOD PUBLIC Update
Inner.Update();
IF Inner.Quality = BYTE#2 THEN
    ValueValue := Filter.Update(Sample := Inner.Value);
ELSE
    ValueValue := Inner.Value;
END_IF;
QualityValue := Inner.Quality;
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
END_FUNCTION_BLOCK

(* Pattern role: SoftwoodStrategy is a concrete implementation in the Decorator and Strategy example. *)
FUNCTION_BLOCK SoftwoodStrategy IMPLEMENTS IDryingStrategy
METHOD PUBLIC HeatCommand : REAL
VAR_INPUT
    MoisturePct : REAL;
END_VAR
HeatCommand := LIMIT(REAL#0.0, (MoisturePct - REAL#12.0) * REAL#3.0, REAL#100.0);
END_METHOD
METHOD PUBLIC FanSpeed : REAL
VAR_INPUT
    MoisturePct : REAL;
END_VAR
FanSpeed := REAL#45.0;
END_METHOD
PUBLIC PROPERTY ProfileId : INT
GET
    ProfileId := INT#1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: HardwoodStrategy is a concrete implementation in the Decorator and Strategy example. *)
FUNCTION_BLOCK HardwoodStrategy IMPLEMENTS IDryingStrategy
METHOD PUBLIC HeatCommand : REAL
VAR_INPUT
    MoisturePct : REAL;
END_VAR
HeatCommand := LIMIT(REAL#0.0, (MoisturePct - REAL#8.0) * REAL#2.0, REAL#80.0);
END_METHOD
METHOD PUBLIC FanSpeed : REAL
VAR_INPUT
    MoisturePct : REAL;
END_VAR
FanSpeed := REAL#30.0;
END_METHOD
PUBLIC PROPERTY ProfileId : INT
GET
    ProfileId := INT#2;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: KilnDryerController owns the collaborating objects used by this Decorator and Strategy example. *)
FUNCTION_BLOCK KilnDryerController
VAR
    Raw : RawMoistureInput;
    Clamped : MoistureClampDecorator;
    Filtered : MoistureFilterDecorator;
    Moisture : IMoistureSource;
    Softwood : SoftwoodStrategy;
    Hardwood : HardwoodStrategy;
    ActiveStrategy : IDryingStrategy;
    ActiveProfileIdValue : INT;
    HeatCommandValue : REAL;
    FanSpeedValue : REAL;
END_VAR
METHOD PUBLIC Initialize
Clamped.Wrap(Source := Raw);
Filtered.Wrap(Source := Clamped);
Moisture := Filtered;
ActiveStrategy := Softwood;
END_METHOD
METHOD PUBLIC Update : REAL
VAR_INPUT
    ProductId : INT;
    RawMoisturePct : REAL;
END_VAR
Raw.SetRaw(MoisturePct := RawMoisturePct);
IF ProductId = INT#2 THEN ActiveStrategy := Hardwood; ELSE ActiveStrategy := Softwood; END_IF;
Moisture.Update();
ActiveProfileIdValue := ActiveStrategy.ProfileId;
HeatCommandValue := ActiveStrategy.HeatCommand(MoisturePct := Moisture.Value);
FanSpeedValue := ActiveStrategy.FanSpeed(MoisturePct := Moisture.Value);
Update := HeatCommandValue;
END_METHOD
PUBLIC PROPERTY ActiveProfileId : INT
GET
    ActiveProfileId := ActiveProfileIdValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY HeatCommand : REAL
GET
    HeatCommand := HeatCommandValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY FanSpeed : REAL
GET
    FanSpeed := FanSpeedValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY ConditionedMoisture : REAL
GET
    ConditionedMoisture := Moisture.Value;
END_GET
END_PROPERTY
PUBLIC PROPERTY MoistureQuality : BYTE
GET
    MoistureQuality := Moisture.Quality;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Dryer : KilnDryerController;
    ProductId : INT;
    MoistureRaw : INT;
    HeatCommandRaw : INT;
    FanSpeedRaw : INT;
END_VAR
Dryer.Initialize();
HeatCommandRaw := REAL_TO_INT(Dryer.Update(ProductId := ProductId, RawMoisturePct := INT_TO_REAL(MoistureRaw) / REAL#10.0));
FanSpeedRaw := REAL_TO_INT(Dryer.FanSpeed);
END_PROGRAM
