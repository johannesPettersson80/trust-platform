TEST_PROGRAM DosingSchedulerEnqueuesCommandObject
VAR
    Scheduler : DosingScheduler;
    Chlorine : ChlorineDoseCommand;
END_VAR

Scheduler.Initialize();
ASSERT_TRUE(IN := Scheduler.Enqueue(Cmd := Chlorine));
ASSERT_TRUE(IN := Scheduler.HasPendingCommand);
END_TEST_PROGRAM

TEST_PROGRAM DosingSchedulerExecutesQueuedCommandWithMemento
VAR
    Scheduler : DosingScheduler;
    Chlorine : ChlorineDoseCommand;
END_VAR

Scheduler.Initialize();
Scheduler.Enqueue(Cmd := Chlorine);
Scheduler.ExecuteNext(CalibrationValid := TRUE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#7.2);
ASSERT_TRUE(IN := Scheduler.LastCanExecute);
ASSERT_TRUE(IN := Scheduler.LastExecuteResult);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Scheduler.ExecutedCount);
ASSERT_EQUAL(EXPECTED := DWORD#16#C001, ACTUAL := Scheduler.LastCommandId);
ASSERT_NEAR(EXPECTED := REAL#7.2, ACTUAL := Scheduler.LastBeforePH, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM DosingSchedulerRejectsBadCalibrationBeforeExecute
VAR
    Scheduler : DosingScheduler;
    Alum : AlumDoseCommand;
END_VAR

Scheduler.Initialize();
ASSERT_TRUE(IN := Scheduler.Enqueue(Cmd := Alum));
ASSERT_FALSE(IN := Scheduler.ExecuteNext(CalibrationValid := FALSE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#7.0));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Scheduler.ExecutedCount);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Scheduler.RejectedCount);
END_TEST_PROGRAM

(* Multi-command audit-trail test: drives different command types
   through one Scheduler. Catches bugs where ExecutedCount/RejectedCount
   reset between calls or LastCommandId leaks state when commands of
   different types are processed in sequence. *)
TEST_PROGRAM DosingSchedulerAccumulatesAuditCountsAcrossCommandTypes
VAR
    Scheduler : DosingScheduler;
    Chlorine : ChlorineDoseCommand;
    Alum : AlumDoseCommand;
    AntiScalant : AntiScalantDoseCommand;
END_VAR

Scheduler.Initialize();

(* Command 1: chlorine, valid calibration, executes *)
Scheduler.Enqueue(Cmd := Chlorine);
ASSERT_TRUE(IN := Scheduler.ExecuteNext(CalibrationValid := TRUE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#7.2));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Scheduler.ExecutedCount);
ASSERT_EQUAL(EXPECTED := DWORD#16#C001, ACTUAL := Scheduler.LastCommandId);

(* Command 2: alum, invalid calibration, rejected; counters separate *)
Scheduler.Enqueue(Cmd := Alum);
ASSERT_FALSE(IN := Scheduler.ExecuteNext(CalibrationValid := FALSE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#7.2));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Scheduler.ExecutedCount);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Scheduler.RejectedCount);

(* Command 3: anti-scalant, valid calibration; LastCommandId reflects
   the latest executed command, not a previously rejected one *)
Scheduler.Enqueue(Cmd := AntiScalant);
ASSERT_TRUE(IN := Scheduler.ExecuteNext(CalibrationValid := TRUE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#7.2));
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Scheduler.ExecutedCount);
ASSERT_EQUAL(EXPECTED := DWORD#16#C003, ACTUAL := Scheduler.LastCommandId);
END_TEST_PROGRAM

(* Each command exposes its own CommandId via the IDosingCommand interface. *)
TEST_PROGRAM DosingCommandIdsAreVendorDistinct
VAR
    Chlorine : ChlorineDoseCommand;
    Alum : AlumDoseCommand;
    AntiScalant : AntiScalantDoseCommand;
END_VAR
ASSERT_EQUAL(EXPECTED := DWORD#16#C001, ACTUAL := Chlorine.CommandId);
ASSERT_EQUAL(EXPECTED := DWORD#16#C002, ACTUAL := Alum.CommandId);
ASSERT_EQUAL(EXPECTED := DWORD#16#C003, ACTUAL := AntiScalant.CommandId);
END_TEST_PROGRAM

(* Each command's CanExecute checks its own permissive (calibration valid,
   tank above per-command minimum, pH window safe). The Scheduler maps
   CurrentPH -> SafePH ([4,9] window) before calling CanExecute, so direct
   tests pass SafePH := TRUE/FALSE. *)
TEST_PROGRAM ChlorineCommandCanExecutePermissiveCheck
VAR
    Chlorine : ChlorineDoseCommand;
END_VAR
(* Valid calibration + tank above 120 ml threshold + pH safe *)
ASSERT_TRUE(IN := Chlorine.CanExecute(CalibrationValid := TRUE, TankLevelMl := REAL#500.0, SafePH := TRUE));

(* Calibration invalid *)
ASSERT_FALSE(IN := Chlorine.CanExecute(CalibrationValid := FALSE, TankLevelMl := REAL#500.0, SafePH := TRUE));

(* Tank empty (below 120 ml threshold) *)
ASSERT_FALSE(IN := Chlorine.CanExecute(CalibrationValid := TRUE, TankLevelMl := REAL#0.0, SafePH := TRUE));

(* pH out of safe window *)
ASSERT_FALSE(IN := Chlorine.CanExecute(CalibrationValid := TRUE, TankLevelMl := REAL#500.0, SafePH := FALSE));

(* Tank exactly at 120 ml threshold (inclusive) *)
ASSERT_TRUE(IN := Chlorine.CanExecute(CalibrationValid := TRUE, TankLevelMl := REAL#120.0, SafePH := TRUE));

(* Tank just below 120 ml *)
ASSERT_FALSE(IN := Chlorine.CanExecute(CalibrationValid := TRUE, TankLevelMl := REAL#119.99, SafePH := TRUE));
END_TEST_PROGRAM

(* Empty queue: ExecuteNext returns FALSE without throwing. *)
TEST_PROGRAM DosingSchedulerEmptyQueueExecuteNextReturnsFalse
VAR
    Scheduler : DosingScheduler;
END_VAR
Scheduler.Initialize();
ASSERT_FALSE(IN := Scheduler.HasPendingCommand);
ASSERT_FALSE(IN := Scheduler.ExecuteNext(CalibrationValid := TRUE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#7.0));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Scheduler.ExecutedCount);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Scheduler.RejectedCount);
END_TEST_PROGRAM

(* Memento captures pH before execution. *)
TEST_PROGRAM DosingSchedulerMementoRecordsBeforePh
VAR
    Scheduler : DosingScheduler;
    Chlorine : ChlorineDoseCommand;
END_VAR
Scheduler.Initialize();
Scheduler.Enqueue(Cmd := Chlorine);
Scheduler.ExecuteNext(CalibrationValid := TRUE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#6.5);
ASSERT_NEAR(EXPECTED := REAL#6.5, ACTUAL := Scheduler.LastBeforePH, DELTA := REAL#0.001);

(* Different pH on second command - memento captures the new value *)
Scheduler.Enqueue(Cmd := Chlorine);
Scheduler.ExecuteNext(CalibrationValid := TRUE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#8.0);
ASSERT_NEAR(EXPECTED := REAL#8.0, ACTUAL := Scheduler.LastBeforePH, DELTA := REAL#0.001);
END_TEST_PROGRAM
