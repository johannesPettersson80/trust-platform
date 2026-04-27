TEST_PROGRAM CleanroomStrategiesChangeTarget
VAR
    Cascade : PressureCascadeController;
    NormalCmd : REAL;
    CleaningCmd : REAL;
END_VAR
Cascade.Initialize();
NormalCmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
CleaningCmd := Cascade.Update(ModeRequest := INT#2, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
ASSERT_TRUE(IN := CleaningCmd > NormalCmd);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Cascade.ActiveModeId);
END_TEST_PROGRAM

TEST_PROGRAM CleanroomCompositeAggregatesRoomAlarms
VAR
    Cascade : PressureCascadeController;
    Cmd : REAL;
END_VAR
Cascade.Initialize();
Cmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#0.0, Room2Pressure := REAL#0.0, DoorOpen := TRUE);
ASSERT_TRUE(IN := Cascade.AlarmCount > INT#0);
END_TEST_PROGRAM

(* Mode-cycling integration test: drives the same Cascade through
   Normal -> Cleaning -> Emergency -> Normal mode requests. Catches
   bugs where the previous strategy's state leaks into the new mode,
   or where ActiveModeId fails to reflect the latest request. *)
TEST_PROGRAM CleanroomCascadeCyclesAllStrategiesOnOneInstance
VAR
    Cascade : PressureCascadeController;
    Cmd : REAL;
END_VAR

Cascade.Initialize();

(* Normal mode *)
Cmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cascade.ActiveModeId);

(* Cleaning mode *)
Cmd := Cascade.Update(ModeRequest := INT#2, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Cascade.ActiveModeId);

(* Emergency mode *)
Cmd := Cascade.Update(ModeRequest := INT#3, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Cascade.ActiveModeId);

(* Back to Normal — the strategy slot must reassign cleanly *)
Cmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cascade.ActiveModeId);
END_TEST_PROGRAM

(* Strategies in isolation: each FB returns the documented target.
   Catches a future regression where the ModeId getter and the
   TargetPressure delta drift apart. *)
TEST_PROGRAM CleanroomStrategiesInIsolationProduceCorrectTargets
VAR
    Normal : NormalPressureStrategy;
    Cleaning : CleaningPressureStrategy;
    Emergency : EmergencyPressureStrategy;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Normal.ModeId);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Cleaning.ModeId);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Emergency.ModeId);
ASSERT_NEAR(EXPECTED := REAL#15.0, ACTUAL := Normal.TargetPressure(BasePressurePa := REAL#15.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := Cleaning.TargetPressure(BasePressurePa := REAL#15.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Emergency.TargetPressure(BasePressurePa := REAL#15.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

(* RoomSuite composite walks both children. ChildCount and AlarmCount
   must aggregate from the two RoomNode children, not be hard-coded
   on the parent. *)
TEST_PROGRAM CleanroomCompositeAggregatesAcrossBothChildren
VAR
    Suite : RoomSuite;
END_VAR

(* Both rooms healthy: zero alarms *)
Suite.UpdateSuite(Room1Pressure := REAL#15.0, Room2Pressure := REAL#10.0, TargetPressure := REAL#15.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Suite.ChildCount);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Suite.AlarmCount);
(* Room1 fault only: AlarmCount = 1 *)
Suite.UpdateSuite(Room1Pressure := REAL#0.0, Room2Pressure := REAL#10.0, TargetPressure := REAL#15.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Suite.AlarmCount);
(* Both rooms faulted: AlarmCount = 2 *)
Suite.UpdateSuite(Room1Pressure := REAL#0.0, Room2Pressure := REAL#100.0, TargetPressure := REAL#15.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Suite.AlarmCount);
END_TEST_PROGRAM

(* Emergency strategy zeroes the target regardless of base pressure.
   The damper command must respond to the new target on the same scan,
   not lag a cycle. *)
TEST_PROGRAM CleanroomEmergencyDrivesZeroPressureOnSameCycle
VAR
    Cascade : PressureCascadeController;
    Cmd : REAL;
END_VAR

Cascade.Initialize();
(* Establish baseline at Normal mode *)
Cmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#15.0, Room2Pressure := REAL#10.0, DoorOpen := FALSE);
(* Switch to Emergency: target = 0; Room1 above target by 15 Pa, Room2 by 15 Pa *)
Cmd := Cascade.Update(ModeRequest := INT#3, Room1Pressure := REAL#15.0, Room2Pressure := REAL#10.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Cascade.ActiveModeId);
(* PressureError = (0 - 15) + ((0 - 5) - 10) = -30; clamped to 0 *)
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Cmd, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Damper command saturation: a far-from-target pressure clamps to 100,
   it does not overshoot. The LIMIT call inside Update is the gate. *)
TEST_PROGRAM CleanroomDamperCommandSaturatesAtHighEnd
VAR
    Cascade : PressureCascadeController;
    Cmd : REAL;
END_VAR

Cascade.Initialize();
(* Normal mode target = 15; both rooms read 0 -> error sum = 25, *2 = 50 *)
Cmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#0.0, Room2Pressure := REAL#0.0, DoorOpen := FALSE);
ASSERT_TRUE(IN := Cmd <= REAL#100.0);
(* Cleaning mode target = 20; both rooms read 0 -> error sum = 35, *2 = 70 *)
Cmd := Cascade.Update(ModeRequest := INT#2, Room1Pressure := REAL#0.0, Room2Pressure := REAL#0.0, DoorOpen := FALSE);
ASSERT_TRUE(IN := Cmd <= REAL#100.0);
ASSERT_TRUE(IN := Cmd >= REAL#0.0);
END_TEST_PROGRAM

(* DoorOpen on a healthy suite still triggers alarm in Room1 only.
   Room2 is intentionally not exposed to door state in this demo. *)
TEST_PROGRAM CleanroomDoorOpenAlarmsRoom1Only
VAR
    Cascade : PressureCascadeController;
    Cmd : REAL;
END_VAR

Cascade.Initialize();
(* Normal mode, both rooms exactly at target, door open *)
Cmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#15.0, Room2Pressure := REAL#10.0, DoorOpen := TRUE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cascade.AlarmCount);
(* Same pressures, door closed: no alarm *)
Cmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#15.0, Room2Pressure := REAL#10.0, DoorOpen := FALSE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cascade.AlarmCount);
END_TEST_PROGRAM
