(* Crusher lubrication: two lubrication pumps in a duty/standby group, an oil pressure transmitter,
   an oil level permissive and the lubrication sequence. The pumps run in Auto from the group; an
   operator may take one into Manual from its panel, and the group then uses the other. *)
USING Plant;

PROGRAM Main
VAR
    (* Hardware signals, mapped to I/O in Configuration.st. *)
    StartButton : BOOL;
    StopButton : BOOL;
    OilLevelOk : BOOL;
    OilFlow : BOOL;
    PressureSignal : LREAL;
    MainPumpRunning : BOOL;
    StandbyPumpRunning : BOOL;
    MainPumpTrip : BOOL;
    StandbyPumpTrip : BOOL;
    MainPumpRun : BOOL;
    StandbyPumpRun : BOOL;
    CrusherPermit : BOOL;

    OilPressure : Plant.FB_PressureInput := (Unit := 'bar', RangeHigh := 10.0,
        LowEnabled := TRUE, LowLimit := 1.5, Deadband := 0.1);
    OilLevel : Plant.FB_Permissives;
    MainPump : Plant.FB_Pump;
    StandbyPump : Plant.FB_Pump;
    Pumps : Plant.FB_DutyStandbyGroup := (StartDelay := T#500ms);
    Lubrication : LubricationSequence;
    Alarms : Plant.FB_AlarmManager;
    StartEdge : R_TRIG;
    StopEdge : R_TRIG;
END_VAR
    (* Measurements and conditions first. *)
    OilPressure(Raw := PressureSignal);
    OilLevel(Condition1 := OilLevelOk, Text1 := 'Oil level low');

    (* The push buttons start and stop the sequence; an operator can do the same from its panel. *)
    StartEdge(CLK := StartButton);
    StopEdge(CLK := StopButton);
    IF StartEdge.Q THEN
        Lubrication.Start();
    END_IF;
    IF StopEdge.Q THEN
        Lubrication.Stop();
    END_IF;
    Lubrication(PumpRunning := MainPump.Running OR StandbyPump.Running, OilFlow := OilFlow,
        Pressure := OilPressure.Value);

    (* The group runs one pump while the sequence asks for lubrication; the pumps need oil. *)
    Pumps(Run := Lubrication.RunPumps, Member1 := MainPump, Member2 := StandbyPump);
    MainPump(RunningFeedback := MainPumpRunning, Trip := MainPumpTrip,
        Permissive := OilLevel.Healthy, PermissiveText := OilLevel.Text);
    StandbyPump(RunningFeedback := StandbyPumpRunning, Trip := StandbyPumpTrip,
        Permissive := OilLevel.Healthy, PermissiveText := OilLevel.Text);
    MainPumpRun := MainPump.RunCommand;
    StandbyPumpRun := StandbyPump.RunCommand;
    CrusherPermit := Lubrication.CrusherPermit;

    Alarms(Source1 := MainPump, Source2 := StandbyPump, Source3 := OilPressure, Source4 := Pumps);
END_PROGRAM
