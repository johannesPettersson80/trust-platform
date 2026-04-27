(* Pattern role: ISignalSource is the contract boundary for the Decorator example. *)
INTERFACE ISignalSource
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

(* Pattern role: IPump is the contract boundary for the Decorator example. *)
INTERFACE IPump
    PROPERTY IsRunning : BOOL
    GET
    END_GET
    END_PROPERTY

    METHOD Run
    VAR_INPUT
        SpeedReference : REAL;
    END_VAR
    END_METHOD

    METHOD Stop
    END_METHOD
END_INTERFACE

(* Pattern role: RealPressureSource is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK RealPressureSource IMPLEMENTS ISignalSource
VAR
    ValueValue : REAL;
    QualityValue : BYTE := BYTE#2;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    PressureBar : REAL;
    QualityCode : BYTE;
END_VAR
ValueValue := PressureBar;
QualityValue := QualityCode;
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

(* Pattern role: SimulatedPressureDecorator is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK SimulatedPressureDecorator IMPLEMENTS ISignalSource
VAR
    RealSourceRef : ISignalSource;
    SineGen : SineSignalGenerator;
    UseSimulationValue : BOOL;
    ValueValue : REAL;
    QualityValue : BYTE;
END_VAR

METHOD PUBLIC Wrap
VAR_INPUT
    RealSource : ISignalSource;
    UseSimulation : BOOL;
END_VAR
RealSourceRef := RealSource;
UseSimulationValue := UseSimulation;
SineGen.Initialize();
SineGen.Configure(Period := T#10s, Amplitude := REAL#0.5, Offset := REAL#4.5, Delay := REAL#0.0);
END_METHOD

METHOD PUBLIC SetSimulation
VAR_INPUT
    Enabled : BOOL;
END_VAR
UseSimulationValue := Enabled;
END_METHOD

METHOD PUBLIC Update
RealSourceRef.Update();
IF UseSimulationValue THEN
    ValueValue := SineGen.Update();
    QualityValue := BYTE#2;
ELSE
    ValueValue := RealSourceRef.Value;
    QualityValue := RealSourceRef.Quality;
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

PUBLIC PROPERTY UsingSimulation : BOOL
GET
    UsingSimulation := UseSimulationValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RealPumpDrive is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK RealPumpDrive IMPLEMENTS IPump
VAR
    RunningValue : BOOL;
    LastSpeedValue : REAL;
END_VAR

METHOD PUBLIC Run
VAR_INPUT
    SpeedReference : REAL;
END_VAR
LastSpeedValue := SpeedReference;
RunningValue := SpeedReference > REAL#0.0;
END_METHOD

METHOD PUBLIC Stop
LastSpeedValue := REAL#0.0;
RunningValue := FALSE;
END_METHOD

PUBLIC PROPERTY IsRunning : BOOL
GET
    IsRunning := RunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastSpeed : REAL
GET
    LastSpeed := LastSpeedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: CommissioningOutputDriver is a concrete implementation in the Decorator example. *)
FUNCTION_BLOCK CommissioningOutputDriver IMPLEMENTS IPump
VAR
    RealPump : IPump;
    LoopbackOnlyValue : BOOL;
    LastCommandedSpeedValue : REAL;
END_VAR

METHOD PUBLIC Wrap
VAR_INPUT
    Pump : IPump;
    LoopbackOnly : BOOL;
END_VAR
RealPump := Pump;
LoopbackOnlyValue := LoopbackOnly;
END_METHOD

METHOD PUBLIC SetLoopback
VAR_INPUT
    Enabled : BOOL;
END_VAR
LoopbackOnlyValue := Enabled;
END_METHOD

METHOD PUBLIC Run
VAR_INPUT
    SpeedReference : REAL;
END_VAR
LastCommandedSpeedValue := SpeedReference;
IF NOT LoopbackOnlyValue THEN
    RealPump.Run(SpeedReference := SpeedReference);
END_IF;
END_METHOD

METHOD PUBLIC Stop
LastCommandedSpeedValue := REAL#0.0;
IF NOT LoopbackOnlyValue THEN
    RealPump.Stop();
END_IF;
END_METHOD

PUBLIC PROPERTY IsRunning : BOOL
GET
    IF LoopbackOnlyValue THEN
        IsRunning := LastCommandedSpeedValue > REAL#0.0;
    ELSE
        IsRunning := RealPump.IsRunning;
    END_IF;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCommandedSpeed : REAL
GET
    LastCommandedSpeed := LastCommandedSpeedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: BoosterCommissioningCell owns the collaborating objects used by this Decorator example. *)
FUNCTION_BLOCK BoosterCommissioningCell
VAR
    RealPressure : RealPressureSource;
    PressureDecorator : SimulatedPressureDecorator;
    RealPump : RealPumpDrive;
    PumpDecorator : CommissioningOutputDriver;
    Pressure : ISignalSource;
    Pump : IPump;
END_VAR

METHOD PUBLIC Initialize
RealPressure.Configure(PressureBar := REAL#4.0, QualityCode := BYTE#2);
PressureDecorator.Wrap(RealSource := RealPressure, UseSimulation := FALSE);
PumpDecorator.Wrap(Pump := RealPump, LoopbackOnly := FALSE);
Pressure := PressureDecorator;
Pump := PumpDecorator;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    RealPressureBar : REAL;
    CommissioningMode : BOOL;
    LoopbackOnly : BOOL;
    PressureSetpoint : REAL;
END_VAR
VAR
    SpeedCommand : REAL;
END_VAR
RealPressure.Configure(PressureBar := RealPressureBar, QualityCode := BYTE#2);
PressureDecorator.SetSimulation(Enabled := CommissioningMode);
PumpDecorator.SetLoopback(Enabled := LoopbackOnly);
Pressure.Update();
IF Pressure.Value < PressureSetpoint THEN
    SpeedCommand := REAL#50.0;
    Pump.Run(SpeedReference := SpeedCommand);
ELSE
    Pump.Stop();
END_IF;
Update := SpeedCommand;
END_METHOD

PUBLIC PROPERTY PressureValue : REAL
GET
    PressureValue := Pressure.Value;
END_GET
END_PROPERTY

PUBLIC PROPERTY PressureQuality : BYTE
GET
    PressureQuality := Pressure.Quality;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCommandedSpeed : REAL
GET
    LastCommandedSpeed := PumpDecorator.LastCommandedSpeed;
END_GET
END_PROPERTY

PUBLIC PROPERTY RealPumpRunning : BOOL
GET
    RealPumpRunning := RealPump.IsRunning;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Cell : BoosterCommissioningCell;
    CommissioningMode : BOOL;
    LoopbackOnly : BOOL;
    RealPressureRaw : INT;
    PumpSpeedRaw : INT;
    TestLampOut : BOOL;
END_VAR

Cell.Initialize();
PumpSpeedRaw := REAL_TO_INT(Cell.Update(RealPressureBar := INT_TO_REAL(RealPressureRaw) / REAL#10.0, CommissioningMode := CommissioningMode, LoopbackOnly := LoopbackOnly, PressureSetpoint := REAL#4.5));
TestLampOut := CommissioningMode;
END_PROGRAM
