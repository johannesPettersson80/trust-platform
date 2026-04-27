(* Composition role: TemperatureZoneController owns the collaborating objects used by this Composition example. *)
FUNCTION_BLOCK TemperatureZoneController
VAR
    Filter : Pt1Filter;
    Controller : PidController;
    AlarmSwitch : HysteresisSwitch;
    Limits : RealRange;
    AlarmLimits : RealRange;
    Gains : PidGains;
    CommandValue : REAL;
    AlarmValue : BOOL;
    FilteredTemperatureValue : REAL;
END_VAR

METHOD PUBLIC Initialize
Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
AlarmLimits.Low := REAL#5.0;
AlarmLimits.High := REAL#30.0;
Gains.Kp := REAL#1.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
Filter.Initialize();
Filter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
Controller.Initialize();
Controller.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
AlarmSwitch.Initialize();
AlarmSwitch.SetLimits(Limits := AlarmLimits);
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    RawTemperature : REAL;
    TargetTemperature : REAL;
    ManualOutput : REAL;
END_VAR
FilteredTemperatureValue := Filter.Update(Sample := RawTemperature);
Controller.SetManual(Manual := TRUE, ManualInput := ManualOutput);
CommandValue := Controller.Update(Actual := FilteredTemperatureValue, Target := TargetTemperature);
(* HysteresisSwitch.Update returns Q (high-side trip only). For an
   out-of-window alarm we trip when the filtered value falls below
   AlarmLimits.Low OR rises above AlarmLimits.High. The switch is
   still updated so its internal state stays current. *)
AlarmSwitch.Update(MeasuredValue := FilteredTemperatureValue);
AlarmValue := (FilteredTemperatureValue < AlarmLimits.Low) OR (FilteredTemperatureValue > AlarmLimits.High);
Update := CommandValue;
END_METHOD

PUBLIC PROPERTY Command : REAL
GET
    Command := CommandValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Alarm : BOOL
GET
    Alarm := AlarmValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FilteredTemperature : REAL
GET
    FilteredTemperature := FilteredTemperatureValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    ZoneA : TemperatureZoneController;
    ZoneB : TemperatureZoneController;
    ZoneACommand : REAL;
    ZoneBCommand : REAL;
    AnyAlarm : BOOL;
END_VAR

ZoneA.Initialize();
ZoneB.Initialize();
ZoneACommand := ZoneA.Update(RawTemperature := REAL#21.0, TargetTemperature := REAL#23.0, ManualOutput := REAL#45.0);
ZoneBCommand := ZoneB.Update(RawTemperature := REAL#18.0, TargetTemperature := REAL#22.0, ManualOutput := REAL#55.0);
AnyAlarm := ZoneA.Alarm OR ZoneB.Alarm;
END_PROGRAM
