// Tank T-101 with the Plant library: inlet pump P-101 feeds the tank through control valve FV-101,
// level transmitter LT-101 measures it, and level controller LIC-101, the real Control.PID in
// ordinary code as Create PLC code writes a loop, sets the valve; it starts in Auto and holds
// 1.5 m. Each symbol of the overview opens its block's panel.
USING Plant;
USING Control;

PROGRAM Transfer
VAR
    P101 : Plant.FB_Pump := (HasFeedback := FALSE);
    FV101 : Plant.FB_ControlValve;
    // The simulated level arrives in metres, so the transmitter's raw range is 0-3 m too.
    LT101 : Plant.FB_LevelInput := (Unit := 'm', RawLow := 0.0, RawHigh := 3.0, RangeHigh := 3.0,
        BadBelow := -0.5, BadAbove := 3.5, Deadband := 0.05,
        HighHighEnabled := TRUE, HighHighLimit := 2.8, HighEnabled := TRUE, HighLimit := 2.5,
        LowEnabled := TRUE, LowLimit := 0.3, LowLowEnabled := TRUE, LowLowLimit := 0.1);
    T101 : Plant.FB_Tank := (Height := 3.0, Capacity := 0.6);
    // LIC-101: the PID, its input and start records, and whether it has started.
    LIC101 : Control.PID;
    LIC101_Input : Control.PIDInput;
    LIC101_Start : Control.PIDInitialState;
    LIC101_Initialized : BOOL;
    // The level from the simulated tank in m, and the inflow that reaches it in m³/s: 0.05 m³/s
    // through a fully open valve while the pump runs.
    Level : LREAL := 1.5;
    Inflow : LREAL;
END_VAR
VAR RETAIN
    // What the engineer and the operator set, and the output: kept through a restart.
    LIC101_Parameters : Control.PIDParameters := (
        ProportionalGain := 50.0,
        IntegralGain := 2.0,
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
    LIC101_Manual : BOOL := FALSE;
    LIC101_Setpoint : LREAL := 1.5;
    LIC101_ManualOutput : LREAL;
    LIC101_Output : LREAL;
END_VAR
    LT101(Raw := Level);
    T101(LevelInstrument := LT101);
    // The pump runs in Auto unless the level is high-high.
    P101(AutoRun := TRUE, Interlock := LT101.HighHigh, InterlockText := 'T-101 level high-high');
    // LIC-101: Control.PID holds LT101 at LIC101_Setpoint.
    LIC101.Configure(Parameters := LIC101_Parameters);
    LIC101_Input.Setpoint := LIC101_Setpoint;
    LIC101_Input.ManualOutput := LIC101_ManualOutput;
    LIC101_Input.CycleTime := 0.1;
    IF LT101.GetQuality() = E_Quality#Good THEN
        LIC101_Input.ProcessValue := LT101.GetValue();
        IF NOT LIC101_Initialized THEN
            // It starts from the measurement and the output as they are: no bump.
            LIC101_Start.Setpoint := LIC101_Setpoint;
            LIC101_Start.ProcessValue := LIC101_Input.ProcessValue;
            LIC101_Start.NominalOutput := LIC101_Output;
            LIC101_Start.SaturatedOutput := LIC101_Output;
            LIC101.Initialize(State := LIC101_Start);
            LIC101_Initialized := TRUE;
        END_IF;
        IF LIC101_Manual THEN
            LIC101_Input.Mode := PIDMode#Manual;
        ELSE
            LIC101_Input.Mode := PIDMode#Auto;
        END_IF;
    ELSE
        // A bad measurement holds the output; the loop starts again from the next good one.
        LIC101_Input.Mode := PIDMode#Manual;
        LIC101_Initialized := FALSE;
    END_IF;
    // This project's own addition: while the pump stands the loop tracks a closed valve, so it
    // returns without a bump.
    IF NOT P101.Running THEN
        LIC101_Input.Mode := PIDMode#Track;
        LIC101_Input.TrackingOutput := 0.0;
    END_IF;
    LIC101_Output := LIC101.Update(Input := LIC101_Input);
    IF LIC101_Input.Mode = PIDMode#Auto THEN
        // Manual takes over from the output as it is.
        LIC101_ManualOutput := LIC101_Output;
    END_IF;
    FV101(AutoPosition := LIC101_Output);
    IF P101.Running THEN
        Inflow := FV101.Position / 100.0 * 0.05;
    ELSE
        Inflow := 0.0;
    END_IF;
END_PROGRAM

// The screens: after the task that calls Hmi.Display, the runtime copies the values they show,
// applies their commands and reads the alarms.
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM

CONFIGURATION EquipmentProject
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Transfer;
    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
VAR_CONFIG
    Main.Level AT %IL0 : LREAL;
    Main.Inflow AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
