TEST_PROGRAM TemperatureZoneComponentComposesFilterControllerAndAlarm
VAR
    Zone : TemperatureZoneController;
END_VAR

Zone.Initialize();
ASSERT_NEAR(EXPECTED := REAL#45.0, ACTUAL := Zone.Update(RawTemperature := REAL#21.0, TargetTemperature := REAL#23.0, ManualOutput := REAL#45.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#21.0, ACTUAL := Zone.FilteredTemperature, DELTA := REAL#0.001);
ASSERT_FALSE(IN := Zone.Alarm);
END_TEST_PROGRAM

(* Multi-instance independence test: drives two zones with different
   inputs in the same scan. Catches bugs where zone state leaks across
   instances (e.g., a static or shared internal accumulator). The
   composition pattern's whole point is that each TemperatureZone is
   an independent instance with its own filter/PID/alarm. *)
TEST_PROGRAM TemperatureZoneTwoInstancesAreIndependent
VAR
    ZoneA : TemperatureZoneController;
    ZoneB : TemperatureZoneController;
    ZoneACommand : REAL;
    ZoneBCommand : REAL;
END_VAR

ZoneA.Initialize();
ZoneB.Initialize();

(* Different raw temperatures drive different filtered values *)
ZoneACommand := ZoneA.Update(RawTemperature := REAL#21.0, TargetTemperature := REAL#23.0, ManualOutput := REAL#45.0);
ZoneBCommand := ZoneB.Update(RawTemperature := REAL#3.0, TargetTemperature := REAL#22.0, ManualOutput := REAL#80.0);

(* Each zone reports its own filtered temperature *)
ASSERT_NEAR(EXPECTED := REAL#21.0, ACTUAL := ZoneA.FilteredTemperature, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := ZoneB.FilteredTemperature, DELTA := REAL#0.001);

(* ZoneB's measured value (3 C) is below the alarm low limit (5 C);
   ZoneA's (21 C) is in the safe window. Alarm states differ. *)
ASSERT_FALSE(IN := ZoneA.Alarm);
ASSERT_TRUE(IN := ZoneB.Alarm);

(* Each zone reports its own manual output *)
ASSERT_NEAR(EXPECTED := REAL#45.0, ACTUAL := ZoneACommand, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#80.0, ACTUAL := ZoneBCommand, DELTA := REAL#0.001);
END_TEST_PROGRAM
