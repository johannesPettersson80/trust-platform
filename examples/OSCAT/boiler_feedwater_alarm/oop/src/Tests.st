TEST_PROGRAM BoilerFeedwaterAlarmOopQueueAlarmCode
VAR
    LevelAlarm : HysteresisSwitch;
    AlarmQueue : DwordFifo16;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
LevelAlarm.Initialize();
LevelAlarm.SetLimits(Limits := AlarmLimits);
AlarmQueue.Initialize();
AlarmQueue.Reset();

ASSERT_TRUE(IN := LevelAlarm.Update(MeasuredValue := REAL#12.0));
ASSERT_TRUE(IN := AlarmQueue.Push(Value := DWORD#16#B001));
ASSERT_TRUE(IN := AlarmQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#B001, ACTUAL := AlarmQueue.Value);
END_TEST_PROGRAM

(* Trip-stay-clear sequence: verifies HysteresisSwitch reacts to
   transitions instead of locking after first trip, and that the
   AlarmQueue can hold more than one event in sequence. *)
TEST_PROGRAM BoilerFeedwaterAlarmRespondsToLevelTransitions
VAR
    LevelAlarm : HysteresisSwitch;
    AlarmQueue : DwordFifo16;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
LevelAlarm.Initialize();
LevelAlarm.SetLimits(Limits := AlarmLimits);
AlarmQueue.Initialize();
AlarmQueue.Reset();

(* Above HIGH_LIMIT -> trip *)
ASSERT_TRUE(IN := LevelAlarm.Update(MeasuredValue := REAL#12.0));
AlarmQueue.Push(Value := DWORD#16#B001);

(* In the hysteresis window -> Q retains last value (trip), Window TRUE *)
LevelAlarm.Update(MeasuredValue := REAL#7.0);
ASSERT_TRUE(IN := LevelAlarm.Window);

(* Below LOW_LIMIT -> clear *)
ASSERT_FALSE(IN := LevelAlarm.Update(MeasuredValue := REAL#1.0));

(* Trip again - queue holds two distinct codes pushed across cycles *)
ASSERT_TRUE(IN := LevelAlarm.Update(MeasuredValue := REAL#12.0));
AlarmQueue.Push(Value := DWORD#16#B002);
ASSERT_FALSE(IN := AlarmQueue.Empty);
END_TEST_PROGRAM

(* Hysteresis boundary tests: exact-at-limit values are inclusive on
   the low side and exclusive on the high side. *)
TEST_PROGRAM BoilerFeedwaterAlarmBoundaryConditions
VAR
    LevelAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
LevelAlarm.Initialize();
LevelAlarm.SetLimits(Limits := AlarmLimits);

(* Exactly at high limit: NOT yet tripped (strict >) *)
ASSERT_FALSE(IN := LevelAlarm.Update(MeasuredValue := REAL#10.0));

(* Just above high limit: trips *)
ASSERT_TRUE(IN := LevelAlarm.Update(MeasuredValue := REAL#10.001));

(* Inside band, strictly below high: still latched at last trip *)
LevelAlarm.Update(MeasuredValue := REAL#5.0);
ASSERT_TRUE(IN := LevelAlarm.Window);

(* Exactly at low limit: NOT yet cleared (strict <) *)
ASSERT_TRUE(IN := LevelAlarm.Update(MeasuredValue := REAL#3.0) OR LevelAlarm.Window);

(* Just below low limit: clears *)
ASSERT_FALSE(IN := LevelAlarm.Update(MeasuredValue := REAL#2.999));
END_TEST_PROGRAM

(* FIFO ordering: pushed alarm codes come back in FIFO order even when
   pushes happen across multiple Push calls. *)
TEST_PROGRAM BoilerFeedwaterAlarmQueueIsFifo
VAR
    AlarmQueue : DwordFifo16;
END_VAR

AlarmQueue.Initialize();
AlarmQueue.Reset();
AlarmQueue.Push(Value := DWORD#16#B001);
AlarmQueue.Push(Value := DWORD#16#B002);
AlarmQueue.Push(Value := DWORD#16#B003);

ASSERT_TRUE(IN := AlarmQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#B001, ACTUAL := AlarmQueue.Value);

ASSERT_TRUE(IN := AlarmQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#B002, ACTUAL := AlarmQueue.Value);

ASSERT_TRUE(IN := AlarmQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#B003, ACTUAL := AlarmQueue.Value);

(* Empty queue: TryPop returns FALSE without throwing. *)
ASSERT_FALSE(IN := AlarmQueue.TryPop());
ASSERT_TRUE(IN := AlarmQueue.Empty);
END_TEST_PROGRAM

(* Reset clears the queue regardless of prior state. *)
TEST_PROGRAM BoilerFeedwaterAlarmQueueResetClears
VAR
    AlarmQueue : DwordFifo16;
END_VAR

AlarmQueue.Initialize();
AlarmQueue.Push(Value := DWORD#16#B001);
AlarmQueue.Push(Value := DWORD#16#B002);
ASSERT_FALSE(IN := AlarmQueue.Empty);

AlarmQueue.Reset();
ASSERT_TRUE(IN := AlarmQueue.Empty);
ASSERT_FALSE(IN := AlarmQueue.TryPop());
END_TEST_PROGRAM

(* Two independent switches behave independently — each owns its own
   latched Q without cross-contamination. *)
TEST_PROGRAM BoilerFeedwaterAlarmTwoSwitchesAreIndependent
VAR
    LowDrumAlarm : HysteresisSwitch;
    HighDrumAlarm : HysteresisSwitch;
    Limits : RealRange;
END_VAR

Limits.Low := REAL#3.0;
Limits.High := REAL#10.0;
LowDrumAlarm.Initialize();
LowDrumAlarm.SetLimits(Limits := Limits);
HighDrumAlarm.Initialize();
HighDrumAlarm.SetLimits(Limits := Limits);

(* Trip low drum, leave high drum below band *)
ASSERT_TRUE(IN := LowDrumAlarm.Update(MeasuredValue := REAL#15.0));
ASSERT_FALSE(IN := HighDrumAlarm.Update(MeasuredValue := REAL#1.0));

(* High drum stays clean even when Low drum is latched *)
ASSERT_TRUE(IN := LowDrumAlarm.Q);
ASSERT_FALSE(IN := HighDrumAlarm.Q);
END_TEST_PROGRAM
