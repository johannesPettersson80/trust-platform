USING Control;

PROGRAM Main
VAR
    HeaterController : Control.PID;
    HeaterParameters : Control.PIDParameters;
    HeaterInitialState : Control.PIDInitialState;
    HeaterInput : Control.PIDInput;
    TankController : Control.PID;
    TankParameters : Control.PIDParameters;
    TankInitialState : Control.PIDInitialState;
    TankInput : Control.PIDInput;
    TableController : Control.PID;
    TableParameters : Control.PIDParameters;
    TableInitialState : Control.PIDInitialState;
    TableInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ColdRoomSetpoint : LREAL := 4.0;
    HeaterSetpoint : LREAL := 20.0;
    TankSetpoint : LREAL := 50.0;
    TableSetpoint : LREAL := 0.0;
    ColdRoomTemperature : LREAL := 4.0;
    HeaterTemperature : LREAL := 20.0;
    TankLevel : LREAL := 50.0;
    TablePosition : LREAL := 0.0;
    TableVelocity : LREAL := 0.0;
    ColdRoomCompressor : LREAL := 0.0;
    HeaterPower : LREAL := 0.0;
    TankPump : LREAL := 20.0;
    TableTorque : LREAL := 0.0;
    ColdRoomStructure : INT := 1;
    HeaterStructure : INT := 3;
    TankStructure : INT := 2;
    TableStructure : INT := 5;
    CompressorOn : BOOL := FALSE;
    DwellScans : INT := 50;
END_VAR

IF NOT Initialized THEN
    (* The heater is self-regulating and needs zero load error: PI is sufficient. *)
    HeaterParameters.ProportionalGain := LREAL#8.0;
    HeaterParameters.IntegralGain := LREAL#1.0;
    HeaterParameters.DerivativeGain := LREAL#0.0;
    HeaterParameters.ProportionalSetpointWeight := LREAL#1.0;
    HeaterParameters.DerivativeSetpointWeight := LREAL#0.0;
    HeaterParameters.Bias := LREAL#0.0;
    HeaterParameters.OutputMinimum := LREAL#0.0;
    HeaterParameters.OutputMaximum := LREAL#100.0;
    HeaterParameters.RateMinimum := LREAL#-1000.0;
    HeaterParameters.RateMaximum := LREAL#1000.0;
    HeaterParameters.TrackingTime := LREAL#1.0;
    HeaterParameters.AntiWindup := PIDAntiWindup#Combined;
    HeaterController.Configure(Parameters := HeaterParameters);
    HeaterInitialState.Setpoint := HeaterTemperature;
    HeaterInitialState.ProcessValue := HeaterTemperature;
    HeaterInitialState.NominalOutput := LREAL#0.0;
    HeaterInitialState.SaturatedOutput := LREAL#0.0;
    HeaterController.Initialize(State := HeaterInitialState);

    (* The tank plant already integrates net flow. P plus the known equilibrium
       bias tracks this setpoint without adding a second integrator. *)
    TankParameters.ProportionalGain := LREAL#2.0;
    TankParameters.IntegralGain := LREAL#0.0;
    TankParameters.DerivativeGain := LREAL#0.0;
    TankParameters.ProportionalSetpointWeight := LREAL#1.0;
    TankParameters.DerivativeSetpointWeight := LREAL#0.0;
    TankParameters.Bias := LREAL#20.0;
    TankParameters.OutputMinimum := LREAL#0.0;
    TankParameters.OutputMaximum := LREAL#100.0;
    TankParameters.RateMinimum := LREAL#-1000.0;
    TankParameters.RateMaximum := LREAL#1000.0;
    TankParameters.TrackingTime := LREAL#1.0;
    TankParameters.AntiWindup := PIDAntiWindup#Combined;
    TankController.Configure(Parameters := TankParameters);
    TankInitialState.Setpoint := TankLevel;
    TankInitialState.ProcessValue := TankLevel;
    TankInitialState.NominalOutput := LREAL#20.0;
    TankInitialState.SaturatedOutput := LREAL#20.0;
    TankController.Initialize(State := TankInitialState);

    (* The loaded table needs P stiffness, D damping, and I load rejection. *)
    TableParameters.ProportionalGain := LREAL#8.0;
    TableParameters.IntegralGain := LREAL#2.0;
    TableParameters.DerivativeGain := LREAL#3.0;
    TableParameters.ProportionalSetpointWeight := LREAL#1.0;
    TableParameters.DerivativeSetpointWeight := LREAL#0.0;
    TableParameters.Bias := LREAL#0.0;
    TableParameters.OutputMinimum := LREAL#-10.0;
    TableParameters.OutputMaximum := LREAL#10.0;
    TableParameters.RateMinimum := LREAL#-1000.0;
    TableParameters.RateMaximum := LREAL#1000.0;
    TableParameters.TrackingTime := LREAL#0.2;
    TableParameters.AntiWindup := PIDAntiWindup#Combined;
    TableController.Configure(Parameters := TableParameters);
    TableInitialState.Setpoint := TablePosition;
    TableInitialState.ProcessValue := TablePosition;
    TableInitialState.NominalOutput := LREAL#0.0;
    TableInitialState.SaturatedOutput := LREAL#0.0;
    TableController.Initialize(State := TableInitialState);

    ColdRoomStructure := 1;
    HeaterStructure := 3;
    TankStructure := 2;
    TableStructure := 5;
    Initialized := TRUE;
END_IF;

(* One reviewed authority starts and restores all four selected structures.
   The project records engineering selections; it does not select structures
   automatically. *)
IF ExperimentEnabled THEN
    ColdRoomSetpoint := LREAL#2.0;
    HeaterSetpoint := LREAL#30.0;
    TankSetpoint := LREAL#60.0;
    TableSetpoint := LREAL#1.0;
    AutoMode := TRUE;
ELSIF ExperimentWasEnabled THEN
    ColdRoomSetpoint := LREAL#4.0;
    HeaterSetpoint := LREAL#20.0;
    TankSetpoint := LREAL#50.0;
    TableSetpoint := LREAL#0.0;
    AutoMode := FALSE;
    CompressorOn := FALSE;
    DwellScans := 50;
END_IF;

InputsValid := ColdRoomTemperature >= LREAL#-10.0
    AND ColdRoomTemperature <= LREAL#20.0
    AND HeaterTemperature >= LREAL#0.0 AND HeaterTemperature <= LREAL#60.0
    AND TankLevel >= LREAL#0.0 AND TankLevel <= LREAL#100.0
    AND TablePosition >= LREAL#-4.0 AND TablePosition <= LREAL#4.0
    AND TableVelocity >= LREAL#-20.0 AND TableVelocity <= LREAL#20.0;
IF SafetyAbort OR NOT InputsValid THEN
    AutoMode := FALSE;
    CompressorOn := FALSE;
END_IF;

(* A binary compressor cannot realize a continuous PID output. Hysteresis and
   five-second dwell implement the declared temperature-band requirement. *)
IF DwellScans < 50 THEN
    DwellScans := DwellScans + 1;
END_IF;
IF AutoMode AND (ColdRoomTemperature >= ColdRoomSetpoint + LREAL#0.5)
    AND NOT CompressorOn AND (DwellScans >= 50) THEN
    CompressorOn := TRUE;
    DwellScans := 0;
ELSIF (ColdRoomTemperature <= ColdRoomSetpoint - LREAL#0.5)
    AND CompressorOn AND (DwellScans >= 50) THEN
    CompressorOn := FALSE;
    DwellScans := 0;
END_IF;
IF CompressorOn THEN
    ColdRoomCompressor := LREAL#100.0;
ELSE
    ColdRoomCompressor := LREAL#0.0;
END_IF;

HeaterInput.Setpoint := HeaterSetpoint;
HeaterInput.ProcessValue := HeaterTemperature;
HeaterInput.FeedForward := LREAL#0.0;
IF AutoMode THEN
    HeaterInput.Mode := PIDMode#Auto;
ELSE
    HeaterInput.Mode := PIDMode#Manual;
    HeaterInput.ManualOutput := LREAL#0.0;
END_IF;
HeaterInput.CycleTime := LREAL#0.1;
HeaterPower := HeaterController.Update(Input := HeaterInput);

TankInput.Setpoint := TankSetpoint;
TankInput.ProcessValue := TankLevel;
TankInput.FeedForward := LREAL#0.0;
IF AutoMode THEN
    TankInput.Mode := PIDMode#Auto;
ELSE
    TankInput.Mode := PIDMode#Manual;
    TankInput.ManualOutput := LREAL#20.0;
END_IF;
TankInput.CycleTime := LREAL#0.1;
TankPump := TankController.Update(Input := TankInput);

TableInput.Setpoint := TableSetpoint;
TableInput.ProcessValue := TablePosition;
TableInput.FeedForward := LREAL#0.0;
IF AutoMode THEN
    TableInput.Mode := PIDMode#Auto;
ELSE
    TableInput.Mode := PIDMode#Manual;
    TableInput.ManualOutput := LREAL#0.0;
END_IF;
TableInput.CycleTime := LREAL#0.1;
TableTorque := TableController.Update(Input := TableInput);
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ColdRoomStructure AT %MW80 : INT;
    P1.HeaterStructure AT %MW82 : INT;
    P1.TankStructure AT %MW84 : INT;
    P1.TableStructure AT %MW86 : INT;
    P1.ExperimentEnabled AT %MX88.0 : BOOL;
    P1.AutoMode AT %MX88.1 : BOOL;
    P1.SafetyAbort AT %MX88.2 : BOOL;
    P1.InputsValid AT %MX88.3 : BOOL;
    P1.ColdRoomSetpoint AT %ML0 : LREAL;
    P1.HeaterSetpoint AT %ML8 : LREAL;
    P1.TankSetpoint AT %ML16 : LREAL;
    P1.TableSetpoint AT %ML24 : LREAL;
    P1.ColdRoomCompressor AT %QL0 : LREAL;
    P1.HeaterPower AT %QL8 : LREAL;
    P1.TankPump AT %QL16 : LREAL;
    P1.TableTorque AT %QL24 : LREAL;
    P1.ColdRoomTemperature AT %IL0 : LREAL;
    P1.HeaterTemperature AT %IL8 : LREAL;
    P1.TankLevel AT %IL16 : LREAL;
    P1.TablePosition AT %IL24 : LREAL;
    P1.TableVelocity AT %IL32 : LREAL;
END_VAR
END_CONFIGURATION
