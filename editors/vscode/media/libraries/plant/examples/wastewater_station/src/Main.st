(* Wastewater lift station: a level transmitter in the wet well, three variable-speed pumps in a
   staged group, and a level loop, the real Control.PID in ordinary code, whose output is both the
   pumps' speed and the group's demand: a higher level starts more pumps and runs them faster.
   Negative gains make the loop pump harder as the level rises. *)
USING Plant;
USING Control;

PROGRAM Main
VAR
    (* Hardware signals, mapped to I/O in Configuration.st. *)
    LevelSignal : LREAL;
    FlowSignal : LREAL;
    Pump1Running : BOOL;
    Pump2Running : BOOL;
    Pump3Running : BOOL;
    Pump1Trip : BOOL;
    Pump2Trip : BOOL;
    Pump3Trip : BOOL;
    Pump1Run : BOOL;
    Pump2Run : BOOL;
    Pump3Run : BOOL;
    Pump1Speed : LREAL;
    Pump2Speed : LREAL;
    Pump3Speed : LREAL;

    WellLevel : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 5.0, Deadband := 0.05,
        HighHighEnabled := TRUE, HighHighLimit := 4.5, HighEnabled := TRUE, HighLimit := 4.0,
        LowEnabled := TRUE, LowLimit := 0.5);
    Outflow : Plant.FB_FlowInput := (Unit := 'm³/h', RangeHigh := 500.0);
    (* The level loop: the PID, its input and start records, and whether it has started. *)
    LevelControl : Control.PID;
    LevelControl_Input : Control.PIDInput;
    LevelControl_Start : Control.PIDInitialState;
    LevelControl_Initialized : BOOL;
    Pump1 : Plant.FB_VariableSpeedPump := (MinSpeed := 40.0);
    Pump2 : Plant.FB_VariableSpeedPump := (MinSpeed := 40.0);
    Pump3 : Plant.FB_VariableSpeedPump := (MinSpeed := 40.0);
    Pumps : Plant.FB_StagedGroup := (Hysteresis := 10.0, MinRunTime := T#2m, MinStopTime := T#1m);
    Demand : LREAL;
END_VAR
VAR RETAIN
    (* What the engineer and the operator set, and the output: kept through a restart. *)
    LevelControl_Parameters : Control.PIDParameters := (
        ProportionalGain := -40.0,
        IntegralGain := -2.0,
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
    LevelControl_Manual : BOOL := FALSE;
    LevelControl_Setpoint : LREAL := 2.0;
    LevelControl_ManualOutput : LREAL;
    LevelControl_Output : LREAL;
END_VAR
    WellLevel(Raw := LevelSignal);
    Outflow(Raw := FlowSignal);
    (* The level loop, as Create PLC code writes a loop: Control.PID holds WellLevel at its setpoint. *)
    LevelControl.Configure(Parameters := LevelControl_Parameters);
    LevelControl_Input.Setpoint := LevelControl_Setpoint;
    LevelControl_Input.ManualOutput := LevelControl_ManualOutput;
    LevelControl_Input.CycleTime := 0.1;
    IF WellLevel.GetQuality() = E_Quality#Good THEN
        LevelControl_Input.ProcessValue := WellLevel.GetValue();
        IF NOT LevelControl_Initialized THEN
            (* It starts from the measurement and the output as they are: no bump. *)
            LevelControl_Start.Setpoint := LevelControl_Setpoint;
            LevelControl_Start.ProcessValue := LevelControl_Input.ProcessValue;
            LevelControl_Start.NominalOutput := LevelControl_Output;
            LevelControl_Start.SaturatedOutput := LevelControl_Output;
            LevelControl.Initialize(State := LevelControl_Start);
            LevelControl_Initialized := TRUE;
        END_IF;
        IF LevelControl_Manual THEN
            LevelControl_Input.Mode := PIDMode#Manual;
        ELSE
            LevelControl_Input.Mode := PIDMode#Auto;
        END_IF;
    ELSE
        (* A bad measurement holds the output; the loop starts again from the next good one. *)
        LevelControl_Input.Mode := PIDMode#Manual;
        LevelControl_Initialized := FALSE;
    END_IF;
    LevelControl_Output := LevelControl.Update(Input := LevelControl_Input);
    IF LevelControl_Input.Mode = PIDMode#Auto THEN
        (* Manual takes over from the output as it is. *)
        LevelControl_ManualOutput := LevelControl_Output;
    END_IF;

    (* Below 0.8 m the well is pumped down: every pump stops. *)
    IF WellLevel.Value < 0.8 THEN
        Demand := 0.0;
    ELSE
        Demand := LevelControl_Output;
    END_IF;
    Pumps(Demand := Demand, Member1 := Pump1, Member2 := Pump2, Member3 := Pump3);
    Pump1(AutoSpeed := LevelControl_Output, RunningFeedback := Pump1Running, Trip := Pump1Trip);
    Pump2(AutoSpeed := LevelControl_Output, RunningFeedback := Pump2Running, Trip := Pump2Trip);
    Pump3(AutoSpeed := LevelControl_Output, RunningFeedback := Pump3Running, Trip := Pump3Trip);
    Pump1Run := Pump1.RunCommand;
    Pump2Run := Pump2.RunCommand;
    Pump3Run := Pump3.RunCommand;
    Pump1Speed := Pump1.SpeedSetpoint;
    Pump2Speed := Pump2.SpeedSetpoint;
    Pump3Speed := Pump3.SpeedSetpoint;
END_PROGRAM
