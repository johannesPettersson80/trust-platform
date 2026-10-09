// Tank T-101: the actual Control.PID holds the level of a gravity-drained tank by its inlet
// valve, and the Alarm library's alarms follow the process. Each alarm belongs to the equipment
// it is declared in: the vessel's level alarms and the inlet pump's overload trip.
USING Control;
USING Alarm;

// Our own detection: the level rises faster than it should. It extends the Alarm library's
// base, so it gets the whole lifecycle, the records and the display.
FUNCTION_BLOCK LevelRiseAlarm EXTENDS Alarm.Alarm
VAR_INPUT
    Level : LREAL;
    // The fastest rise that is still normal, in m/s.
    MaximumRise : LREAL := 0.05;
    // The task's cycle, in s.
    CycleTime : LREAL := 0.1;
END_VAR
VAR
    Previous : LREAL;
    Started : BOOL;
END_VAR
    IF NOT Started THEN
        Previous := Level;
        Started := TRUE;
    END_IF;
    Evaluate(Condition := (Level - Previous) / CycleTime > MaximumRise);
    Previous := Level;
END_FUNCTION_BLOCK

// The vessel: its level and its level alarms. The high-high level is a trip cause; it latches
// until the trip is reset and takes part in the first out of the trips.
FUNCTION_BLOCK TankVessel
VAR_INPUT
    Level : LREAL;
    // The high and low alarm limits in m; the trip is 0.3 m above the high limit.
    HighLimit : LREAL := 2.5;
    LowLimit : LREAL := 0.3;
    // The controller holds the level: a fast rise is then its own doing, not the operator's.
    Auto : BOOL;
    Trips : REF_TO FirstOutGroup;
    Reset : BOOL;
END_VAR
VAR_OUTPUT
    // The high-high level holds, or is latched: the pump must trip.
    TripLevel : BOOL;
END_VAR
VAR RETAIN
    High : LimitAlarm;
    HighHigh : LimitAlarm;
    Low : LimitAlarm;
    Rising : LevelRiseAlarm;
END_VAR
    High(Value := Level, Limit := HighLimit, Deadband := 0.05,
        Message := 'T-101 level high', Priority := AlarmPriority#Medium);
    HighHigh(Value := Level, Limit := HighLimit + 0.3, Deadband := 0.05, Latching := TRUE, Group := Trips,
        Message := 'T-101 level high-high: inlet pump tripped', Priority := AlarmPriority#High);
    Low(Value := Level, Limit := LowLimit, Deadband := 0.05, Direction := LimitDirection#Low,
        Message := 'T-101 level low', Priority := AlarmPriority#Medium);
    // Suppressed by design in Auto: only an operator driving the valve by hand needs the warning.
    Rising(Level := Level, OnDelay := T#2s, Suppressed := Auto,
        Message := 'T-101 level rising fast', Priority := AlarmPriority#Low);
    IF Reset THEN
        HighHigh.Reset();
    END_IF;
    TripLevel := HighHigh.Active;
END_FUNCTION_BLOCK

// The inlet pump P-101: it runs when asked unless tripped. A high-high level or a motor
// overload trips it; it stays tripped until the trip is reset with both causes gone.
FUNCTION_BLOCK InletPump
VAR_INPUT
    Run : BOOL;
    HighHigh : BOOL;
    Overload : BOOL;
    Reset : BOOL;
    Trips : REF_TO FirstOutGroup;
END_VAR
VAR_OUTPUT
    Running : BOOL;
    Tripped : BOOL;
END_VAR
VAR RETAIN
    OverloadTrip : DigitalAlarm;
END_VAR
    OverloadTrip(Condition := Overload, Latching := TRUE, Group := Trips,
        Message := 'P-101 motor overload: pump tripped', Priority := AlarmPriority#High);
    IF Reset THEN
        OverloadTrip.Reset();
    END_IF;
    IF HighHigh OR OverloadTrip.Active THEN
        Tripped := TRUE;
    ELSIF Reset THEN
        Tripped := FALSE;
    END_IF;
    Running := Run AND NOT Tripped;
END_FUNCTION_BLOCK

PROGRAM TankLevelControl
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#50.0,
        IntegralGain := LREAL#2.0,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-50.0,
        RateMaximum := LREAL#50.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#Combined
    );
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL;
    // What the operator sets: the controller's mode, its setpoint in m, and the valve opening in
    // % while in Manual.
    Mode : PIDMode := PIDMode#Auto;
    Setpoint : LREAL := 1.5;
    ManualOpening : LREAL := 50.0;
    // The level from LT-101 in m, the opening of the inlet valve FV-101 in %, and the inflow
    // that reaches the tank in m³/s: 0.05 m³/s at a fully open valve while the pump runs.
    Level : LREAL := 1.5;
    ValveOpening : LREAL;
    Inflow : LREAL;
    PumpRun : BOOL := TRUE;
    TripReset : BOOL;
    // The level alarm limits in m, for the vessel's alarms and the trend's bands.
    HighLimit : LREAL := 2.5;
    LowLimit : LREAL := 0.3;
    // For the engineer: a simulated motor overload, to try the trip.
    OverloadTest : BOOL;
END_VAR
VAR RETAIN
    Vessel : TankVessel;
    Pump : InletPump;
    Trips : FirstOutGroup;
END_VAR
    // A trip reset clears the latched trip causes whose conditions are gone, then the first out.
    Vessel(Level := Level, HighLimit := HighLimit, LowLimit := LowLimit, Auto := Mode = PIDMode#Auto,
        Trips := REF(Trips), Reset := TripReset);
    Pump(Run := PumpRun, HighHigh := Vessel.TripLevel, Overload := OverloadTest,
        Reset := TripReset, Trips := REF(Trips));
    IF TripReset THEN
        Trips.Reset();
    END_IF;

    Controller.Configure(Parameters := Parameters);
    IF NOT Initialized THEN
        InitialState.Setpoint := Setpoint;
        InitialState.ProcessValue := Level;
        InitialState.NominalOutput := ManualOpening;
        InitialState.SaturatedOutput := ManualOpening;
        Controller.Initialize(State := InitialState);
        Initialized := TRUE;
    END_IF;
    Input.Setpoint := Setpoint;
    Input.ProcessValue := Level;
    Input.ManualOutput := ManualOpening;
    Input.CycleTime := LREAL#0.1;
    // While the pump is stopped the controller tracks a closed valve, so it returns without a bump.
    IF Pump.Running THEN
        Input.Mode := Mode;
    ELSE
        Input.Mode := PIDMode#Track;
        Input.TrackingOutput := LREAL#0.0;
    END_IF;
    ValveOpening := Controller.Update(Input := Input);
    IF Pump.Running THEN
        Inflow := ValveOpening / LREAL#100.0 * LREAL#0.05;
    ELSE
        Inflow := LREAL#0.0;
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

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Tank WITH Fast : TankLevelControl;
    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
VAR_CONFIG
    Tank.Level AT %IL0 : LREAL;
    Tank.Inflow AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
