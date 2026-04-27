TEST_PROGRAM ColdStorageAlarmOopDetectWarmChamber
VAR
    Converter : UnitConverter;
    DoorAlarm : HysteresisSwitch;
    ClassicAlarm : HYST;
    AlarmLimits : RealRange;
END_VAR

Converter.Initialize();
AlarmLimits.Low := REAL#0.0 - REAL#30.0;
AlarmLimits.High := REAL#0.0 - REAL#18.0;
DoorAlarm.Initialize();
DoorAlarm.SetLimits(Limits := AlarmLimits);
ClassicAlarm(
    In := REAL#0.0 - REAL#16.0,
    HIGH_LIMIT := REAL#0.0 - REAL#18.0,
    LOW_LIMIT := REAL#0.0 - REAL#30.0
);

ASSERT_NEAR(EXPECTED := REAL#257.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#0.0 - REAL#16.0), DELTA := REAL#0.01);
ASSERT_TRUE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#16.0) = ClassicAlarm.Q);
END_TEST_PROGRAM

(* Multi-temperature sequence test: drives the same alarm switch
   through warm -> in-window -> frozen readings. Verifies Q transits
   instead of latching after the first warm reading. *)
TEST_PROGRAM ColdStorageAlarmTracksChamberTemperatureChanges
VAR
    DoorAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#0.0 - REAL#30.0;
AlarmLimits.High := REAL#0.0 - REAL#18.0;
DoorAlarm.Initialize();
DoorAlarm.SetLimits(Limits := AlarmLimits);

(* Warm chamber (-16 above HIGH -18) -> Q := TRUE *)
ASSERT_TRUE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#16.0));

(* Inside hysteresis window -> Q retains, Window TRUE *)
DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#22.0);
ASSERT_TRUE(IN := DoorAlarm.Window);

(* Frozen chamber (-35 below LOW -30) -> Q := FALSE *)
ASSERT_FALSE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#35.0));
END_TEST_PROGRAM

(* Hysteresis band: once Q latches TRUE, an in-band reading must hold Q.
   The deadband is the whole point of the FB — verify it does not snap
   back to FALSE the moment the value re-enters the window. *)
TEST_PROGRAM ColdStorageAlarmHoldsThroughHysteresisBand
VAR
    DoorAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#0.0 - REAL#30.0;
AlarmLimits.High := REAL#0.0 - REAL#18.0;
DoorAlarm.Initialize();
DoorAlarm.SetLimits(Limits := AlarmLimits);

(* Cross HighLimit -> Q := TRUE *)
ASSERT_TRUE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#10.0));
(* Drop into window (-18 .. -30): Q must hold TRUE *)
ASSERT_TRUE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#22.0));
ASSERT_TRUE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#28.0));
(* Cross LowLimit -> Q clears *)
ASSERT_FALSE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#31.0));
(* Re-enter window from below: Q must stay FALSE *)
ASSERT_FALSE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#25.0));
END_TEST_PROGRAM

(* Boundary: exact threshold values. The FB uses strict less-than for
   LowLimit and strict greater-than for HighLimit, so a reading exactly
   at HighLimit must NOT latch. *)
TEST_PROGRAM ColdStorageAlarmBoundaryAtExactLimits
VAR
    DoorAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#0.0 - REAL#30.0;
AlarmLimits.High := REAL#0.0 - REAL#18.0;
DoorAlarm.Initialize();
DoorAlarm.SetLimits(Limits := AlarmLimits);

(* Exactly at HighLimit (-18): not above, so Q stays FALSE (initial) *)
ASSERT_FALSE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#18.0));
ASSERT_TRUE(IN := DoorAlarm.Window);
(* Just above HighLimit (-17.99): Q latches TRUE *)
ASSERT_TRUE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#17.99));
(* Exactly at LowLimit (-30): not below, so Q stays TRUE *)
ASSERT_TRUE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#30.0));
(* Just below LowLimit (-30.01): Q clears *)
ASSERT_FALSE(IN := DoorAlarm.Update(MeasuredValue := REAL#0.0 - REAL#30.01));
END_TEST_PROGRAM

(* Two chambers side by side: per-FB isolation. Each chamber's
   deadband state must not leak into the other. *)
TEST_PROGRAM ColdStorageTwoChambersAreIsolated
VAR
    Chamber1 : HysteresisSwitch;
    Chamber2 : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#0.0 - REAL#30.0;
AlarmLimits.High := REAL#0.0 - REAL#18.0;
Chamber1.Initialize();
Chamber2.Initialize();
Chamber1.SetLimits(Limits := AlarmLimits);
Chamber2.SetLimits(Limits := AlarmLimits);

(* Chamber1 warms up; Chamber2 stays frozen *)
ASSERT_TRUE(IN := Chamber1.Update(MeasuredValue := REAL#0.0 - REAL#10.0));
ASSERT_FALSE(IN := Chamber2.Update(MeasuredValue := REAL#0.0 - REAL#35.0));
(* States are independent: Chamber1 in-window stays TRUE, Chamber2 stays FALSE *)
ASSERT_TRUE(IN := Chamber1.Update(MeasuredValue := REAL#0.0 - REAL#22.0));
ASSERT_FALSE(IN := Chamber2.Update(MeasuredValue := REAL#0.0 - REAL#22.0));
END_TEST_PROGRAM

(* SetLimits with inverted range raises Error. Commissioning slip
   guard — must not silently degrade to default limits. *)
TEST_PROGRAM ColdStorageInvertedLimitsRaiseError
VAR
    DoorAlarm : HysteresisSwitch;
    BadLimits : RealRange;
END_VAR

BadLimits.Low := REAL#0.0 - REAL#10.0;
BadLimits.High := REAL#0.0 - REAL#30.0;
DoorAlarm.Initialize();
ASSERT_FALSE(IN := DoorAlarm.Error);
DoorAlarm.SetLimits(Limits := BadLimits);
ASSERT_TRUE(IN := DoorAlarm.Error);
END_TEST_PROGRAM
