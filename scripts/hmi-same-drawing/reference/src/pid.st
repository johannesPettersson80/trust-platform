// The real Control.PID for the same-drawing check (specification 39, §19.4, §18 "1A done when"):
// in Auto with setpoint and process value at 60 and its state started at 60, the controller's
// members hold the design samples the editor draws.

USING Control;

PROGRAM PidLoop
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#0.8,
        IntegralGain := LREAL#0.2,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    InitialState : Control.PIDInitialState := (
        Setpoint := LREAL#60.0,
        SetpointRate := LREAL#0.0,
        ProcessValue := LREAL#60.0,
        ProcessValueRate := LREAL#0.0,
        FeedForward := LREAL#0.0,
        NominalOutput := LREAL#60.0,
        SaturatedOutput := LREAL#60.0);
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    Output : LREAL;
END_VAR
Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;
Input.Setpoint := LREAL#60.0;
Input.ProcessValue := LREAL#60.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.1;
Output := Controller.Update(Input := Input);
END_PROGRAM
