TEST_PROGRAM BatchReactorFactorySelectsDifferentSequencers
VAR
    Factory : SequencerFactory;
    Sequencer : IBatchSequencer;
END_VAR

Factory.Initialize();
Sequencer := Factory.Build(RecipeId := INT#1);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Sequencer.NameId);
Sequencer := Factory.Build(RecipeId := INT#2);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Sequencer.NameId);
Sequencer := Factory.Build(RecipeId := INT#3);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Sequencer.NameId);
END_TEST_PROGRAM

TEST_PROGRAM BatchReactorCompletesValidRecipeAndRejectsInvalidRecipe
VAR
    Reactor : BatchReactor;
    Snapshot : BatchSnapshot;
END_VAR

Reactor.Initialize();
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#800, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
Snapshot := Reactor.Snapshot();
ASSERT_TRUE(IN := Snapshot.BatchComplete);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Snapshot.RecipeId);
ASSERT_EQUAL(EXPECTED := INT#140, ACTUAL := Snapshot.SequenceStep);

Reactor.Reset();
ASSERT_EQUAL(EXPECTED := INT#900, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#99, ProductTemp := REAL#25.0, PH := REAL#7.0, HoldComplete := FALSE, SampleOk := FALSE));
Snapshot := Reactor.Snapshot();
ASSERT_TRUE(IN := Snapshot.AlarmActive);
ASSERT_EQUAL(EXPECTED := DWORD#16#B001, ACTUAL := Snapshot.AlarmCode);
END_TEST_PROGRAM

(* Recipe-switching regression test: drives the same Reactor instance
   through all three recipes back-to-back. Catches bugs where Factory
   dispatch is one-shot or where the Reactor caches the wrong sequencer
   after Reset. The Snapshot.RecipeId must reflect the recipe driven
   in the most recent Update, not a previously cached value. *)
TEST_PROGRAM BatchReactorRoutesEachRecipeThroughItsOwnSequencer
VAR
    Reactor : BatchReactor;
    Snapshot : BatchSnapshot;
END_VAR

Reactor.Initialize();

(* Recipe 1 — drive once, snapshot must report recipe 1 *)
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE);
Snapshot := Reactor.Snapshot();
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Snapshot.RecipeId);

Reactor.Reset();

(* Recipe 2 — same instance, different recipe *)
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#2, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE);
Snapshot := Reactor.Snapshot();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Snapshot.RecipeId);

Reactor.Reset();

(* Recipe 3 — third recipe through the same Reactor *)
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#3, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE);
Snapshot := Reactor.Snapshot();
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Snapshot.RecipeId);
END_TEST_PROGRAM

(* Lid not closed: rejects start, raises alarm. *)
TEST_PROGRAM BatchReactorRejectsStartWithLidOpen
VAR
    Reactor : BatchReactor;
    Snapshot : BatchSnapshot;
END_VAR
Reactor.Initialize();
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := FALSE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#25.0, PH := REAL#7.0, HoldComplete := FALSE, SampleOk := FALSE);
Snapshot := Reactor.Snapshot();
ASSERT_TRUE(IN := Snapshot.AlarmActive);
END_TEST_PROGRAM

(* E-stop loss aborts. *)
TEST_PROGRAM BatchReactorEStopLossAborts
VAR
    Reactor : BatchReactor;
    Snapshot : BatchSnapshot;
END_VAR
Reactor.Initialize();
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := FALSE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#25.0, PH := REAL#7.0, HoldComplete := FALSE, SampleOk := FALSE);
Snapshot := Reactor.Snapshot();
ASSERT_TRUE(IN := Snapshot.AlarmActive);
END_TEST_PROGRAM

(* Pressure relief breach aborts. *)
TEST_PROGRAM BatchReactorPressureReliefBreachAborts
VAR
    Reactor : BatchReactor;
    Snapshot : BatchSnapshot;
END_VAR
Reactor.Initialize();
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := FALSE, RecipeId := INT#1, ProductTemp := REAL#25.0, PH := REAL#7.0, HoldComplete := FALSE, SampleOk := FALSE);
Snapshot := Reactor.Snapshot();
ASSERT_TRUE(IN := Snapshot.AlarmActive);
END_TEST_PROGRAM

(* AbortCommand triggers immediate abort regardless of permissives. *)
TEST_PROGRAM BatchReactorAbortCommandImmediatelyAborts
VAR
    Reactor : BatchReactor;
    Snapshot : BatchSnapshot;
END_VAR
Reactor.Initialize();
(* Drive to mid-batch *)
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE);
(* Now abort *)
Reactor.Update(StartCommand := TRUE, AbortCommand := TRUE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE);
Snapshot := Reactor.Snapshot();
ASSERT_TRUE(IN := Snapshot.AlarmActive);
END_TEST_PROGRAM

(* Reset clears alarm and allows new batch. *)
TEST_PROGRAM BatchReactorResetClearsAlarmAndAllowsNewBatch
VAR
    Reactor : BatchReactor;
    Snapshot : BatchSnapshot;
END_VAR
Reactor.Initialize();
(* Trigger an alarm with bad recipe *)
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#99, ProductTemp := REAL#25.0, PH := REAL#7.0, HoldComplete := FALSE, SampleOk := FALSE);
Snapshot := Reactor.Snapshot();
ASSERT_TRUE(IN := Snapshot.AlarmActive);

(* Reset and verify alarm cleared *)
Reactor.Reset();
Snapshot := Reactor.Snapshot();
ASSERT_FALSE(IN := Snapshot.AlarmActive);

(* Now a valid recipe works *)
Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE);
Snapshot := Reactor.Snapshot();
ASSERT_FALSE(IN := Snapshot.AlarmActive);
END_TEST_PROGRAM
