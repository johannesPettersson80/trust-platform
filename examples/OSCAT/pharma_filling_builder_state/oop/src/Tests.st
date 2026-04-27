TEST_PROGRAM FillRecipeBuilderRejectsInvalidRecipe
VAR
    Builder : FillRecipeBuilder;
    Recipe : FillRecipe;
END_VAR

Recipe := Builder.SetVolume(VolumeMl := REAL#0.0).SetPumpSpeed(PumpSpeedPct := REAL#50.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
ASSERT_FALSE(IN := Recipe.Valid);
END_TEST_PROGRAM

TEST_PROGRAM FillingStateObjectsRejectBadWeight
VAR
    Builder : FillRecipeBuilder;
    Recipe : FillRecipe;
    Filler : FillingController;
END_VAR

Recipe := Builder.SetVolume(VolumeMl := REAL#10.0).SetPumpSpeed(PumpSpeedPct := REAL#45.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
Filler.Initialize();
Filler.LoadRecipe(Recipe := Recipe);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Filler.Update(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0));
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0));
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := TRUE, WeightG := REAL#0.0));
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#10.5));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Filler.RejectCount);
END_TEST_PROGRAM

(* Two-vial sequence test: drives one Filler through two complete
   fill cycles (one good, one bad). Catches bugs where reject counts
   leak between vials or where the state machine sticks at Reject
   after the first bad vial. *)
TEST_PROGRAM FillerProcessesMultipleVialsOnOneInstance
VAR
    Builder : FillRecipeBuilder;
    Recipe : FillRecipe;
    Filler : FillingController;
END_VAR

Recipe := Builder.SetVolume(VolumeMl := REAL#10.0).SetPumpSpeed(PumpSpeedPct := REAL#45.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
Filler.Initialize();
Filler.LoadRecipe(Recipe := Recipe);

(* Vial 1: in-window weight (10.0 g) - should not increment reject *)
Filler.Update(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0);
Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0);
Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := TRUE, WeightG := REAL#0.0);
Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#10.0);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := Filler.RejectCount);

(* Vial 2: out-of-window weight (11 g) - reject increments *)
Filler.Update(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0);
Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0);
Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := TRUE, WeightG := REAL#0.0);
Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#11.0);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Filler.RejectCount);
END_TEST_PROGRAM

(* Builder boundary tests: each validation rule trips correctly.
   Volume must be in (0, 100], pump speed in (0, 100], window
   non-empty (High > Low). *)
TEST_PROGRAM FillRecipeBuilderBoundaryValidation
VAR
    Builder : FillRecipeBuilder;
    Recipe : FillRecipe;
END_VAR

(* Volume too high - invalid *)
Builder.Reset();
Recipe := Builder.SetVolume(VolumeMl := REAL#150.0).SetPumpSpeed(PumpSpeedPct := REAL#50.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
ASSERT_FALSE(IN := Recipe.Valid);

(* Pump speed too high - invalid *)
Builder.Reset();
Recipe := Builder.SetVolume(VolumeMl := REAL#10.0).SetPumpSpeed(PumpSpeedPct := REAL#120.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
ASSERT_FALSE(IN := Recipe.Valid);

(* Weight window inverted - invalid *)
Builder.Reset();
Recipe := Builder.SetVolume(VolumeMl := REAL#10.0).SetPumpSpeed(PumpSpeedPct := REAL#50.0).SetWeightWindow(LowWeightG := REAL#10.2, HighWeightG := REAL#9.8).Build();
ASSERT_FALSE(IN := Recipe.Valid);

(* Boundary upper edges: VolumeMl = 100, PumpSpeedPct = 100 are valid *)
Builder.Reset();
Recipe := Builder.SetVolume(VolumeMl := REAL#100.0).SetPumpSpeed(PumpSpeedPct := REAL#100.0).SetWeightWindow(LowWeightG := REAL#99.0, HighWeightG := REAL#101.0).Build();
ASSERT_TRUE(IN := Recipe.Valid);

(* Boundary zero: pump speed = 0 is invalid *)
Builder.Reset();
Recipe := Builder.SetVolume(VolumeMl := REAL#10.0).SetPumpSpeed(PumpSpeedPct := REAL#0.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
ASSERT_FALSE(IN := Recipe.Valid);
END_TEST_PROGRAM

(* Recipe gating: Filler must refuse to leave Idle when the loaded
   recipe is invalid, regardless of StartCommand or VialPresent. *)
TEST_PROGRAM FillerHoldsAtIdleWhenRecipeInvalid
VAR
    Builder : FillRecipeBuilder;
    Recipe : FillRecipe;
    Filler : FillingController;
END_VAR

(* Build an invalid recipe (volume = 0) *)
Recipe := Builder.SetVolume(VolumeMl := REAL#0.0).SetPumpSpeed(PumpSpeedPct := REAL#45.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
Filler.Initialize();
Filler.LoadRecipe(Recipe := Recipe);
ASSERT_FALSE(IN := Filler.RecipeValid);

(* All inputs healthy but recipe invalid: must hold at Idle *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Filler.Update(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Filler.Update(StartCommand := TRUE, VialPresent := TRUE, FillDone := TRUE, WeightG := REAL#10.0));

(* Reload with a valid recipe: now starts on next StartCommand *)
Recipe := Builder.SetVolume(VolumeMl := REAL#10.0).SetPumpSpeed(PumpSpeedPct := REAL#45.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
Filler.LoadRecipe(Recipe := Recipe);
ASSERT_TRUE(IN := Filler.RecipeValid);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Filler.Update(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0));
END_TEST_PROGRAM

(* Per-state isolation: each state FB returns its own id and computes
   transitions independent of any controller state. The state objects
   are not stateful - they read inputs and Recipe each call. *)
TEST_PROGRAM FillingStatesAreStatelessOracles
VAR
    Idle : IdleFillState;
    Prime : PrimeFillState;
    Fill : FillState;
    CheckWeight : CheckWeightState;
    Reject : RejectState;
    Complete : CompleteState;
    Recipe : FillRecipe;
END_VAR

(* StateId reflects the state's own number *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Idle.StateId);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Prime.StateId);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Fill.StateId);
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := CheckWeight.StateId);
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Reject.StateId);
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Complete.StateId);

(* Make a valid recipe *)
Recipe.VolumeMl := REAL#10.0;
Recipe.PumpSpeedPct := REAL#45.0;
Recipe.LowWeightG := REAL#9.8;
Recipe.HighWeightG := REAL#10.2;
Recipe.Valid := TRUE;

(* Idle: with start + vial + valid recipe, advances to Prime *)
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Idle.OnExecute(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, Recipe := Recipe));
(* Prime: always advances to Fill *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Prime.OnExecute(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, Recipe := Recipe));
(* Fill: stays on no FillDone, advances on FillDone *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Fill.OnExecute(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, Recipe := Recipe));
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Fill.OnExecute(StartCommand := FALSE, VialPresent := TRUE, FillDone := TRUE, WeightG := REAL#0.0, Recipe := Recipe));
(* CheckWeight: in window -> Complete; out of window -> Reject *)
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := CheckWeight.OnExecute(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#10.0, Recipe := Recipe));
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := CheckWeight.OnExecute(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#11.0, Recipe := Recipe));

(* Reject and Complete are restart-friendly: a fresh start advances to Prime *)
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Reject.OnExecute(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, Recipe := Recipe));
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Complete.OnExecute(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, Recipe := Recipe));
(* Without start: both return to Idle *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Reject.OnExecute(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, Recipe := Recipe));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Complete.OnExecute(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, Recipe := Recipe));
END_TEST_PROGRAM

(* Builder reuse: after Build, calling Reset gives a fresh recipe
   without leaking the previous values. *)
TEST_PROGRAM FillRecipeBuilderResetClearsState
VAR
    Builder : FillRecipeBuilder;
    Recipe1 : FillRecipe;
    Recipe2 : FillRecipe;
END_VAR

Recipe1 := Builder.SetVolume(VolumeMl := REAL#10.0).SetPumpSpeed(PumpSpeedPct := REAL#45.0).SetWeightWindow(LowWeightG := REAL#9.8, HighWeightG := REAL#10.2).Build();
ASSERT_TRUE(IN := Recipe1.Valid);

(* Reset wipes the internal recipe; Build without setters yields
   invalid (volume = 0, pump = 0) *)
Builder.Reset();
Recipe2 := Builder.Build();
ASSERT_FALSE(IN := Recipe2.Valid);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Recipe2.VolumeMl, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Recipe2.PumpSpeedPct, DELTA := REAL#0.001);

(* Original recipe1 is unchanged - the builder returned a copy *)
ASSERT_TRUE(IN := Recipe1.Valid);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := Recipe1.VolumeMl, DELTA := REAL#0.001);
END_TEST_PROGRAM
