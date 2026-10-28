// Room heating: hot water from tank T-101 heats a room through fan-coil E-101. TIC-101 holds the
// tank temperature with heater EH-101; TIC-102 holds the room temperature with three-way valve
// TV-101, which divides the circulation between the fan-coil and the bypass. HeatingSequence
// starts and stops the plant, and the protections hold equipment a fault endangers in its safe
// state. The I/O list is the plant's wiring; in simulation, simulation.toml drives the same signals.
USING Plant;
USING Alarm;
USING Control;

PROGRAM Main
VAR
    // I/O list. Digital inputs:
    LSL101_LowWater AT %IX0.0 : BOOL;       // T-101 low-water switch: TRUE while the water is below it
    FS101_Flow AT %IX0.1 : BOOL;            // flow switch: TRUE while water circulates
    P101_Running AT %IX0.2 : BOOL;          // pump contactor feedback
    P101_Overload AT %IX0.3 : BOOL;         // pump motor protection tripped
    F101_Running AT %IX0.4 : BOOL;          // fan drive running
    F101_Fault AT %IX0.5 : BOOL;            // fan drive fault
    EH101_Running AT %IX0.6 : BOOL;         // heater contactor feedback
    TV101_PositionerFault AT %IX0.7 : BOOL; // valve positioner alarm
    TT101_ChannelFault AT %IX1.0 : BOOL;    // analog module diagnosis of TT-101's channel
    TT102_ChannelFault AT %IX1.1 : BOOL;    // analog module diagnosis of TT-102's channel
    // Analog inputs:
    TT101_Signal AT %IL8 : LREAL;           // tank temperature, 4-20 mA for 0-100 °C
    TT102_Signal AT %IL16 : LREAL;          // room temperature, 4-20 mA for 0-50 °C
    TV101_Position AT %IL24 : LREAL;        // valve position feedback, % of the flow to the fan-coil
    // Digital outputs:
    EH101_Contactor AT %QX0.0 : BOOL;
    P101_Run AT %QX0.1 : BOOL;
    F101_Run AT %QX0.2 : BOOL;
    // Analog outputs:
    EH101_Power AT %QL8 : LREAL;            // heater thyristor controller, 0-100 %
    TV101_Demand AT %QL16 : LREAL;          // valve positioner, % to the fan-coil; 0 sends all water through the bypass
    F101_Speed AT %QL24 : LREAL;            // fan drive speed reference, 0-100 %
END_VAR
VAR
    (* truST plant: declarations sha256:2f293c91b2c357ce678d295e91892888cb1e5363b9b85b426d7e76ce4fc5b168 *)
    EH101 : Plant.FB_Heater;
    F101 : Plant.FB_VariableSpeedFan;
    FS101 : Plant.FB_DigitalInput;
    LSL101 : Plant.FB_LevelSwitch := (AlarmEnabled := TRUE, AlarmText := 'T-101 water low');
    P101 : Plant.FB_Pump;
    TT101 : Plant.FB_TemperatureInput := (
        HighHighEnabled := TRUE,
        HighHighLimit := 60.0,
        HighEnabled := TRUE,
        HighLimit := 55.0,
        Unit := '°C');
    TT102 : Plant.FB_TemperatureInput := (RangeHigh := 50.0, Unit := '°C');
    TV101 : Plant.FB_ControlValve := (DeviationTime := T#30s, HasFeedback := TRUE);
    Heating : HeatingSequence;
    HeaterProtection : Plant.FB_Protection;
    FanProtection : Plant.FB_Protection;
    PumpProtection : Plant.FB_Protection;
    ValveProtection : Plant.FB_Protection;
    TIC101 : Control.PID;
    TIC101_Input : Control.PIDInput;
    TIC101_Start : Control.PIDInitialState;
    TIC101_Initialized : BOOL;
    TIC102 : Control.PID;
    TIC102_Input : Control.PIDInput;
    TIC102_Start : Control.PIDInitialState;
    TIC102_Initialized : BOOL;
    (* end of truST plant declarations *)
END_VAR
VAR RETAIN
    (* truST plant: retained declarations sha256:e99945f6bf371c2d1f07a36c30fb024b66e0123914f5d525b8aa4293d3a24850 *)
    TIC101_Parameters : Control.PIDParameters := (
        ProportionalGain := 33.3,
        IntegralGain := 1.334,
        DerivativeGain := 0.0,
        ProportionalSetpointWeight := 1.0,
        DerivativeSetpointWeight := 0.0,
        Bias := 0.0,
        OutputMinimum := 0.0,
        OutputMaximum := 100.0,
        RateMinimum := -1000000000.0,
        RateMaximum := 1000000000.0,
        TrackingTime := 1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    TIC101_Manual : BOOL := TRUE;
    TIC101_Setpoint : LREAL := 45.0;
    TIC101_ManualOutput : LREAL;
    TIC101_Output : LREAL;
    TIC102_Parameters : Control.PIDParameters := (
        ProportionalGain := 28.6,
        IntegralGain := 1.36,
        DerivativeGain := 0.0,
        ProportionalSetpointWeight := 1.0,
        DerivativeSetpointWeight := 0.0,
        Bias := 0.0,
        OutputMinimum := 0.0,
        OutputMaximum := 100.0,
        RateMinimum := -1000000000.0,
        RateMaximum := 1000000000.0,
        TrackingTime := 1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    TIC102_Manual : BOOL := TRUE;
    TIC102_Setpoint : LREAL := 22.0;
    TIC102_ManualOutput : LREAL;
    TIC102_Output : LREAL;
    (* end of truST plant retained declarations *)
END_VAR
VAR
    HeatDemand : BOOL;
    WaterOk : BOOL;
    // Alarms no equipment raises itself.
    NoFlowAlarm : Alarm.DigitalAlarm;
    PositionerAlarm : Alarm.DigitalAlarm;
END_VAR
VAR RETAIN
    // FALSE until the very first start, which puts both loops in Auto; their setpoints start at
    // the design's (45 °C and 22 °C) and the operator's later changes are retained.
    Commissioned : BOOL;
    // The fan's speed while the room needs heat, in %.
    FanSpeed : LREAL := 70.0;
END_VAR
    // The very first start puts both loops in Auto; the operator's later changes are retained.
    IF NOT Commissioned THEN
        TIC101_Manual := FALSE;
        TIC102_Manual := FALSE;
        Commissioned := TRUE;
    END_IF;
    WaterOk := NOT LSL101.Value;
    // Protections: each condition is TRUE while it is healthy. They hold the equipment in its safe
    // state; they are process protections, not a certified safety function.
    HeaterProtection.Condition1 := WaterOk;
    HeaterProtection.Text1 := 'T-101 water low';
    HeaterProtection.Condition2 := NOT TT101.HighHigh;
    HeaterProtection.Text2 := 'T-101 temperature high-high';
    HeaterProtection.Condition3 := TT101.Quality = E_Quality#Good;
    HeaterProtection.Text3 := 'TT-101 signal bad';
    PumpProtection.Condition1 := WaterOk;
    PumpProtection.Text1 := 'T-101 water low';
    ValveProtection.Condition1 := NOT TV101_PositionerFault;
    ValveProtection.Text1 := 'TV-101 positioner fault';
    ValveProtection.Condition2 := TT102.Quality = E_Quality#Good;
    ValveProtection.Text2 := 'TT-102 signal bad';
    FanProtection.Condition1 := TT102.Quality = E_Quality#Good;
    FanProtection.Text1 := 'TT-102 signal bad';
    // The sequence starts and stops the plant.
    Heating.WaterOk := WaterOk;
    Heating.Flow := FS101.Value;
    Heating.TankTemperature := TT101.Value;
    P101.AutoRun := Heating.PumpRun;
    P101.Interlock := PumpProtection.Active;
    P101.InterlockText := PumpProtection.Text;
    EH101.AutoRun := Heating.HeaterRun;
    EH101.Interlock := HeaterProtection.Active;
    EH101.InterlockText := HeaterProtection.Text;
    TV101.Interlock := ValveProtection.Active;
    TV101.InterlockText := ValveProtection.Text;
    // The fan blows while the room needs heat: it starts below 0.5 K above the setpoint and stops
    // above 1.5 K above it.
    IF TT102.Value < TIC102_Setpoint + 0.5 THEN
        HeatDemand := TRUE;
    ELSIF TT102.Value > TIC102_Setpoint + 1.5 THEN
        HeatDemand := FALSE;
    END_IF;
    F101.AutoRun := Heating.RoomHeating AND HeatDemand;
    F101.AutoSpeed := FanSpeed;
    F101.Interlock := FanProtection.Active;
    F101.InterlockText := FanProtection.Text;
    NoFlowAlarm(Condition := P101.Running AND NOT FS101.Value, OnDelay := T#10s,
        Message := 'No circulation: P-101 runs but FS-101 sees no flow', Priority := AlarmPriority#High);
    PositionerAlarm(Condition := TV101_PositionerFault, Message := 'TV-101 positioner fault',
        Priority := AlarmPriority#Medium);
    (* truST plant: calls sha256:7fb4aa57e3e64e16ab32318683da07367d02fdd98bdaf65527390197e8e5f81f *)
    FS101(Input := FS101_Flow);
    LSL101(Input := LSL101_LowWater);
    TT101(Raw := TT101_Signal, SignalFault := TT101_ChannelFault);
    TT102(Raw := TT102_Signal, SignalFault := TT102_ChannelFault);
    FanProtection();
    HeaterProtection();
    Heating();
    PumpProtection();
    (* A PID loop: Control.PID holds its measurement at its setpoint. *)
    TIC101.Configure(Parameters := TIC101_Parameters);
    TIC101_Input.Setpoint := TIC101_Setpoint;
    TIC101_Input.ManualOutput := TIC101_ManualOutput;
    TIC101_Input.CycleTime := 0.1;
    IF TT101.GetQuality() = E_Quality#Good THEN
        TIC101_Input.ProcessValue := TT101.GetValue();
        IF NOT TIC101_Initialized THEN
            (* It starts from the measurement and the output as they are: no bump. *)
            TIC101_Start.Setpoint := TIC101_Setpoint;
            TIC101_Start.ProcessValue := TIC101_Input.ProcessValue;
            TIC101_Start.NominalOutput := TIC101_Output;
            TIC101_Start.SaturatedOutput := TIC101_Output;
            TIC101.Initialize(State := TIC101_Start);
            TIC101_Initialized := TRUE;
        END_IF;
        IF TIC101_Manual THEN
            TIC101_Input.Mode := PIDMode#Manual;
        ELSE
            TIC101_Input.Mode := PIDMode#Auto;
        END_IF;
    ELSE
        (* A bad measurement holds the output; the loop starts again from the next good one. *)
        TIC101_Input.Mode := PIDMode#Manual;
        TIC101_Initialized := FALSE;
    END_IF;
    TIC101_Output := TIC101.Update(Input := TIC101_Input);
    IF TIC101_Input.Mode = PIDMode#Auto THEN
        (* Manual takes over from the output as it is. *)
        TIC101_ManualOutput := TIC101_Output;
    END_IF;
    EH101.AutoPower := TIC101_Output;
    (* A PID loop: Control.PID holds its measurement at its setpoint. *)
    TIC102.Configure(Parameters := TIC102_Parameters);
    TIC102_Input.Setpoint := TIC102_Setpoint;
    TIC102_Input.ManualOutput := TIC102_ManualOutput;
    TIC102_Input.CycleTime := 0.1;
    IF TT102.GetQuality() = E_Quality#Good THEN
        TIC102_Input.ProcessValue := TT102.GetValue();
        IF NOT TIC102_Initialized THEN
            (* It starts from the measurement and the output as they are: no bump. *)
            TIC102_Start.Setpoint := TIC102_Setpoint;
            TIC102_Start.ProcessValue := TIC102_Input.ProcessValue;
            TIC102_Start.NominalOutput := TIC102_Output;
            TIC102_Start.SaturatedOutput := TIC102_Output;
            TIC102.Initialize(State := TIC102_Start);
            TIC102_Initialized := TRUE;
        END_IF;
        IF TIC102_Manual THEN
            TIC102_Input.Mode := PIDMode#Manual;
        ELSE
            TIC102_Input.Mode := PIDMode#Auto;
        END_IF;
    ELSE
        (* A bad measurement holds the output; the loop starts again from the next good one. *)
        TIC102_Input.Mode := PIDMode#Manual;
        TIC102_Initialized := FALSE;
    END_IF;
    TIC102_Output := TIC102.Update(Input := TIC102_Input);
    IF TIC102_Input.Mode = PIDMode#Auto THEN
        (* Manual takes over from the output as it is. *)
        TIC102_ManualOutput := TIC102_Output;
    END_IF;
    TV101.AutoPosition := TIC102_Output;
    ValveProtection();
    EH101(
        RunCommand => EH101_Contactor,
        RunningFeedback := EH101_Running,
        PowerOutput => EH101_Power);
    F101(
        RunCommand => F101_Run,
        RunningFeedback := F101_Running,
        Trip := F101_Fault,
        SpeedSetpoint => F101_Speed);
    P101(RunCommand => P101_Run, RunningFeedback := P101_Running, Trip := P101_Overload);
    TV101(PositionSetpoint => TV101_Demand, PositionFeedback := TV101_Position);
    (* end of truST plant calls *)
END_PROGRAM
