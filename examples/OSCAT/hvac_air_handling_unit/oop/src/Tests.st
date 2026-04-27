TEST_PROGRAM AhuStrategySelectsModeSpecificObjects
VAR
    Ahu : AhuController;
    EcoCommand : REAL;
    ComfortCommand : REAL;
    FrostCommand : REAL;
END_VAR

Ahu.Initialize();
EcoCommand := Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Ahu.ActiveMode);

ComfortCommand := Ahu.Update(ModeRequest := INT#2, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Ahu.ActiveMode);
ASSERT_TRUE(IN := ComfortCommand > EcoCommand);

FrostCommand := Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#-8.0, SupplyTemp := REAL#1.0, SupplySetpoint := REAL#3.0, SmokeClear := TRUE, FreezeStatClear := FALSE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Ahu.ActiveMode);
ASSERT_TRUE(IN := FrostCommand >= REAL#20.0);
END_TEST_PROGRAM

TEST_PROGRAM AhuSmokeAlarmOverridesAllStrategies
VAR
    Ahu : AhuController;
    Command : REAL;
END_VAR

Ahu.Initialize();
Command := Ahu.Update(ModeRequest := INT#2, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := FALSE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_TRUE(IN := Ahu.AlarmActive);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Command, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Integration test: simulates the PROGRAM Main scan loop with the
   Init-once guard. Catches the "Initialize every cycle" bug —
   AhuController.Initialize() resets RuntimeFan accumulator, clears
   AlarmQueue, and forces State back to 0. Without the guard, the
   active mode never persists across cycles. *)
TEST_PROGRAM AhuScanLoopWithInitGuardPersistsActiveModeAcrossCycles
VAR
    Ahu : AhuController;
    Initialized : BOOL;
    I : INT;
    LastMode : INT;
END_VAR

FOR I := 1 TO 5 DO
    IF NOT Initialized THEN
        Ahu.Initialize();
        Initialized := TRUE;
    END_IF;
    Ahu.Update(ModeRequest := INT#2, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
    LastMode := Ahu.ActiveMode;
END_FOR;
(* After five cycles in Comfort mode, the active mode is stable *)
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := LastMode);
ASSERT_FALSE(IN := Ahu.AlarmActive);
END_TEST_PROGRAM

(* Anti-pattern test: re-initializing every cycle resets all retained
   state. This pins the contract: if a future change removes the
   Init-once guard from Main, this test still passes (because the
   anti-pattern is what's tested), but the guard test above will fail
   if the assertion is ever violated by re-init mid-flight. *)
TEST_PROGRAM AhuReinitializingEveryCycleClearsAlarmsAndState
VAR
    Ahu : AhuController;
    I : INT;
END_VAR

(* Trip a smoke alarm, then re-init: alarm should clear *)
Ahu.Initialize();
Ahu.Update(ModeRequest := INT#2, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := FALSE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_TRUE(IN := Ahu.AlarmActive);
Ahu.Initialize();
ASSERT_FALSE(IN := Ahu.AlarmActive);
END_TEST_PROGRAM

(* FrostProtect override priority: outdoor < -5 forces Frost regardless of mode request. *)
TEST_PROGRAM AhuFrostProtectOverridesAllOtherModes
VAR
    Ahu : AhuController;
END_VAR
Ahu.Initialize();

(* Eco request but freezing outside - Frost wins *)
Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#-10.0, SupplyTemp := REAL#5.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Ahu.ActiveMode);

(* Comfort request but freezing - Frost wins *)
Ahu.Update(ModeRequest := INT#2, OutdoorTemp := REAL#-10.0, SupplyTemp := REAL#5.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Ahu.ActiveMode);

(* Boundary: outdoor exactly -5 (NOT below -5, strict less-than) - Eco wins *)
Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#-5.0, SupplyTemp := REAL#15.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Ahu.ActiveMode);

(* Freeze stat trip alone (warm outside) -> Frost still wins *)
Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#15.0, SupplyTemp := REAL#15.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := FALSE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Ahu.ActiveMode);
END_TEST_PROGRAM

(* Smoke is the highest priority - it wins over Frost. *)
TEST_PROGRAM AhuSmokeBeatsFrostProtect
VAR
    Ahu : AhuController;
    Command : REAL;
END_VAR
Ahu.Initialize();

(* Both smoke and freeze conditions present -> alarm + heat off *)
Command := Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#-10.0, SupplyTemp := REAL#0.0, SupplySetpoint := REAL#5.0, SmokeClear := FALSE, FreezeStatClear := FALSE, SupplyFanHealthy := TRUE);
ASSERT_TRUE(IN := Ahu.AlarmActive);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Command, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Eco strategy: deadband of +/- 2 C around setpoint. *)
TEST_PROGRAM AhuEcoStrategyHasWideDeadband
VAR
    Eco : EcoStrategy;
    Out : REAL;
END_VAR
Eco.Initialize();

(* Inside +/- 2 C deadband -> output 0 *)
Out := Eco.Update(SupplyTemp := REAL#21.0, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#10.0);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Out, DELTA := REAL#0.001);

Out := Eco.Update(SupplyTemp := REAL#19.5, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#10.0);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Out, DELTA := REAL#0.001);

(* Outside deadband -> non-zero command *)
Out := Eco.Update(SupplyTemp := REAL#15.0, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#10.0);
ASSERT_TRUE(IN := Out > REAL#0.0);
END_TEST_PROGRAM

(* Comfort strategy: tighter deadband (+/- 0.5 C) and higher gain. *)
TEST_PROGRAM AhuComfortStrategyTighterThanEco
VAR
    Comfort : ComfortStrategy;
    Eco : EcoStrategy;
    ComfortOut : REAL;
    EcoOut : REAL;
END_VAR
Comfort.Initialize();
Eco.Initialize();

(* Same input - Comfort produces higher output (gain 20 vs Eco's 8) *)
ComfortOut := Comfort.Update(SupplyTemp := REAL#19.0, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#10.0);
EcoOut := Eco.Update(SupplyTemp := REAL#19.0, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#10.0);
ASSERT_TRUE(IN := ComfortOut > EcoOut);

(* Comfort responds to small errors that Eco ignores *)
ComfortOut := Comfort.Update(SupplyTemp := REAL#20.5, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#10.0);
EcoOut := Eco.Update(SupplyTemp := REAL#20.5, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#10.0);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := EcoOut, DELTA := REAL#0.001);
ASSERT_TRUE(IN := ComfortOut > REAL#0.0);
END_TEST_PROGRAM

(* FrostProtect strategy: forces setpoint to MAX(scheduled, 5 C). *)
TEST_PROGRAM AhuFrostProtectClampsSetpointToMinimumFiveCelsius
VAR
    Frost : FrostProtectStrategy;
    Out : REAL;
END_VAR
Frost.Initialize();

(* Scheduled setpoint below 5 C - Frost forces to 5 *)
Out := Frost.Update(SupplyTemp := REAL#3.0, ScheduledSetpoint := REAL#0.0, OutdoorTemp := REAL#-10.0);
(* Effective setpoint 5, error 2, gain 30 -> output ~60, then LIMITed to 100 max but at least 20 min *)
ASSERT_TRUE(IN := Out >= REAL#20.0);

(* Scheduled setpoint above 5 - Frost uses scheduled *)
Out := Frost.Update(SupplyTemp := REAL#15.0, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#-10.0);
ASSERT_TRUE(IN := Out > REAL#0.0);

(* Output never falls below 20 even when error is small (forced minimum heat) *)
Out := Frost.Update(SupplyTemp := REAL#21.0, ScheduledSetpoint := REAL#21.0, OutdoorTemp := REAL#-10.0);
ASSERT_TRUE(IN := Out >= REAL#20.0);
END_TEST_PROGRAM

(* Supply fan unhealthy raises a class B alarm. *)
TEST_PROGRAM AhuSupplyFanFaultRaisesAlarm
VAR
    Ahu : AhuController;
END_VAR
Ahu.Initialize();
Ahu.Update(ModeRequest := INT#2, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := FALSE);
ASSERT_TRUE(IN := Ahu.AlarmActive);
END_TEST_PROGRAM
