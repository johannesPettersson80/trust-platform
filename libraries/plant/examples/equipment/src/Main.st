(* One instance of every Plant block, each called once per scan with ordinary I/O. Feedback is
   switched off where this example has no hardware for it, so every device can be operated from its
   panel; with hardware, wire the feedback inputs and leave HasFeedback at TRUE. *)
USING Plant;

PROGRAM Main
VAR
    (* Analog channels in mA and contacts; Configuration.st maps the ones a test bench uses. *)
    PressureSignal : LREAL := 12.0;
    LevelSignal : LREAL := 12.0;
    TemperatureSignal : LREAL := 12.0;
    FlowSignal : LREAL := 12.0;
    WeightSignal : LREAL := 4.0;
    LevelSwitchContact : BOOL;
    CountPulse : BOOL;

    Motor : Plant.FB_Motor := (HasFeedback := FALSE);
    Pump : Plant.FB_Pump := (HasFeedback := FALSE);
    Fan : Plant.FB_Fan := (HasFeedback := FALSE);
    Drive : Plant.FB_VariableSpeedMotor := (HasFeedback := FALSE);
    DosingPump : Plant.FB_DosingPump := (HasFeedback := FALSE, MaxSpeed := 80.0);
    Heater : Plant.FB_Heater := (HasFeedback := FALSE, MaxPower := 75.0);
    Reverser : Plant.FB_ReversingMotor := (HasFeedback := FALSE);
    TwoSpeed : Plant.FB_TwoSpeedMotor := (HasFeedback := FALSE);
    Valve : Plant.FB_OnOffValve := (HasOpenFeedback := FALSE, HasClosedFeedback := FALSE);
    FailOpenValve : Plant.FB_OnOffValve := (HasOpenFeedback := FALSE, HasClosedFeedback := FALSE, FailOpen := TRUE);
    MotorValve : Plant.FB_MotorValve;
    Cylinder : Plant.FB_Cylinder := (HasExtendedSensor := FALSE, HasRetractedSensor := FALSE);
    ControlValve : Plant.FB_ControlValve := (FailPosition := 0.0);
    Pressure : Plant.FB_PressureInput := (Unit := 'bar', RangeHigh := 16.0, HighEnabled := TRUE, HighLimit := 12.0);
    Level : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 4.0);
    Temperature : Plant.FB_TemperatureInput := (Unit := '°C', RangeLow := -20.0, RangeHigh := 120.0);
    Flow : Plant.FB_FlowInput := (Unit := 'm³/h', RangeHigh := 50.0);
    Scale : Plant.FB_WeighScale := (Unit := 'kg', RangeHigh := 500.0);
    LevelSwitch : Plant.FB_LevelSwitch := (Debounce := T#500ms);
    Counter : Plant.FB_Counter := (Preset := 100);
    Tank : Plant.FB_Tank := (Height := 4.0, Capacity := 12.0);
    Thermostat : Plant.FB_OnOffController := (Setpoint := 60.0);
    PumpTrips : Plant.FB_Interlocks;
    Group : Plant.FB_DutyStandbyGroup;
    StandbyPump : Plant.FB_Pump := (HasFeedback := FALSE);
    Alarms : Plant.FB_AlarmManager;
    (* A stand-in for the motor valve's travel: it reaches an end five seconds after it starts. *)
    OpenTravel : TON;
    CloseTravel : TON;
    AtOpen : BOOL;
    AtClosed : BOOL := TRUE;
END_VAR
    (* Instruments and the tank. *)
    Pressure(Raw := PressureSignal);
    Level(Raw := LevelSignal);
    Temperature(Raw := TemperatureSignal);
    Flow(Raw := FlowSignal);
    Scale(Raw := WeightSignal);
    LevelSwitch(Input := LevelSwitchContact);
    Counter(Pulse := CountPulse);
    Tank(LevelInstrument := Level);

    (* Controllers and conditions. *)
    Thermostat(Measurement := Temperature);
    PumpTrips(Condition1 := NOT Level.HighHigh, Text1 := 'Tank level high-high');

    (* Equipment. *)
    Group(Run := Level.Value > 1.0, Member1 := Pump, Member2 := StandbyPump);
    Motor();
    Pump(Interlock := PumpTrips.Active, InterlockText := PumpTrips.Text);
    StandbyPump(Interlock := PumpTrips.Active, InterlockText := PumpTrips.Text);
    Fan();
    Drive(AutoSpeed := 50.0);
    DosingPump();
    Heater(AutoRun := Thermostat.Demand, AutoPower := 100.0);
    Reverser();
    TwoSpeed();
    Valve();
    FailOpenValve();
    OpenTravel(IN := MotorValve.OpenCommand, PT := T#5s);
    CloseTravel(IN := MotorValve.CloseCommand, PT := T#5s);
    IF MotorValve.OpenCommand THEN
        AtClosed := FALSE;
    END_IF;
    IF MotorValve.CloseCommand THEN
        AtOpen := FALSE;
    END_IF;
    IF OpenTravel.Q THEN
        AtOpen := TRUE;
    END_IF;
    IF CloseTravel.Q THEN
        AtClosed := TRUE;
    END_IF;
    MotorValve(OpenLimit := AtOpen, ClosedLimit := AtClosed);
    Cylinder();
    ControlValve(AutoPosition := 50.0);

    Alarms(Source1 := Pump, Source2 := StandbyPump, Source3 := Pressure, Source4 := Level,
        Source5 := Temperature, Source6 := Flow, Source7 := Group, Source8 := ControlValve);
END_PROGRAM
