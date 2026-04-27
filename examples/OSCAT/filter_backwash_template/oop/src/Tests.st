TEST_PROGRAM BackwashTemplateUsesMediaSpecificHooks
VAR
    Sand : SandFilterBackwash;
    Carbon : CarbonFilterBackwash;
    SandResult : BackwashResult;
    CarbonResult : BackwashResult;
END_VAR

Sand.Initialize();
Carbon.Initialize();
SandResult := Sand.RunTemplate(ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
CarbonResult := Carbon.RunTemplate(ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
ASSERT_TRUE(IN := SandResult.ReadyForService);
ASSERT_FALSE(IN := CarbonResult.ReadyForService);
ASSERT_EQUAL(EXPECTED := INT#420, ACTUAL := CarbonResult.RequiredSeconds);
END_TEST_PROGRAM

(* Template progression test: drives the same media filter through
   under-time, on-time, and turbidity-fail conditions. Catches bugs
   where the template's hook results cache from a prior run-through. *)
TEST_PROGRAM BackwashTemplateRunsRepeatedlyOnSameInstance
VAR
    Sand : SandFilterBackwash;
    Result1 : BackwashResult;
    Result2 : BackwashResult;
    Result3 : BackwashResult;
END_VAR

Sand.Initialize();

(* Below required time -> not ready *)
Result1 := Sand.RunTemplate(ElapsedSeconds := INT#100, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
ASSERT_FALSE(IN := Result1.ReadyForService);

(* Past required time + acceptable turbidity -> ready *)
Result2 := Sand.RunTemplate(ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
ASSERT_TRUE(IN := Result2.ReadyForService);

(* Past time but turbidity too high -> not ready (rinse limit failed) *)
Result3 := Sand.RunTemplate(ElapsedSeconds := INT#360, TurbidityNtu := REAL#5.0, FlowOk := TRUE);
ASSERT_FALSE(IN := Result3.ReadyForService);
END_TEST_PROGRAM

(* Step boundary regression: each step transition must occur at the
   exact documented elapsed-time boundary. The ladder uses < (not <=)
   in two places, so 60 is past drain and 300 is past backwash for
   sand. *)
TEST_PROGRAM BackwashStepBoundariesAreExact
VAR
    Sand : SandFilterBackwash;
    Result : BackwashResult;
END_VAR

Sand.Initialize();

(* Just before drain ends: step = 10 *)
Result := Sand.RunTemplate(ElapsedSeconds := INT#59, TurbidityNtu := REAL#1.0, FlowOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Result.LastStep);
(* Exactly at the drain boundary (60): step = 20 (backwash) *)
Result := Sand.RunTemplate(ElapsedSeconds := INT#60, TurbidityNtu := REAL#1.0, FlowOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Result.LastStep);
(* Just before backwash ends: step = 20 *)
Result := Sand.RunTemplate(ElapsedSeconds := INT#299, TurbidityNtu := REAL#1.0, FlowOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Result.LastStep);
(* Exactly at backwash boundary (300): step = 100 (ready, since turbidity is good) *)
Result := Sand.RunTemplate(ElapsedSeconds := INT#300, TurbidityNtu := REAL#1.0, FlowOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Result.LastStep);
ASSERT_TRUE(IN := Result.ReadyForService);
END_TEST_PROGRAM

(* Flow loss is the highest-priority alarm: it overrides all other
   step decisions, regardless of elapsed time or turbidity. *)
TEST_PROGRAM BackwashFlowLossOverridesAllOtherSteps
VAR
    Sand : SandFilterBackwash;
    Result : BackwashResult;
END_VAR

Sand.Initialize();

(* Even if everything is perfect for ready-for-service, FlowOk = FALSE alarms *)
Result := Sand.RunTemplate(ElapsedSeconds := INT#600, TurbidityNtu := REAL#0.5, FlowOk := FALSE);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Result.LastStep);
ASSERT_FALSE(IN := Result.ReadyForService);

(* During drain phase too *)
Result := Sand.RunTemplate(ElapsedSeconds := INT#10, TurbidityNtu := REAL#10.0, FlowOk := FALSE);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Result.LastStep);
END_TEST_PROGRAM

(* Per-media isolation: SandFilterBackwash and CarbonFilterBackwash
   instances must not share any state. The template hook overrides
   bind per-instance. *)
TEST_PROGRAM BackwashSandAndCarbonAreIsolated
VAR
    Sand : SandFilterBackwash;
    Carbon : CarbonFilterBackwash;
    SandResult : BackwashResult;
    CarbonResult : BackwashResult;
END_VAR

Sand.Initialize();
Carbon.Initialize();

(* Same elapsed time and turbidity hit different verdicts because
   the hook values differ *)
SandResult := Sand.RunTemplate(ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
CarbonResult := Carbon.RunTemplate(ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE);

(* Sand: needs 300s, has 360s, turbidity 1.5 < 2.0 limit -> ready *)
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := SandResult.LastStep);
ASSERT_EQUAL(EXPECTED := INT#300, ACTUAL := SandResult.RequiredSeconds);
(* Carbon: needs 420s, has 360s -> still backwashing *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := CarbonResult.LastStep);
ASSERT_EQUAL(EXPECTED := INT#420, ACTUAL := CarbonResult.RequiredSeconds);
END_TEST_PROGRAM

(* BackwashController routes correctly between media. Test that
   FilterType = 1 dispatches to Sand and FilterType = 2 to Carbon. *)
TEST_PROGRAM BackwashControllerRoutesByFilterType
VAR
    Backwash : BackwashController;
    SandResult : BackwashResult;
    CarbonResult : BackwashResult;
END_VAR

Backwash.Initialize();

SandResult := Backwash.Update(FilterType := INT#1, ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#300, ACTUAL := SandResult.RequiredSeconds);
ASSERT_TRUE(IN := SandResult.ReadyForService);

CarbonResult := Backwash.Update(FilterType := INT#2, ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#420, ACTUAL := CarbonResult.RequiredSeconds);
ASSERT_FALSE(IN := CarbonResult.ReadyForService);

(* Properties expose the latest result *)
ASSERT_EQUAL(EXPECTED := INT#420, ACTUAL := Backwash.LastRequiredSeconds);
END_TEST_PROGRAM

(* Carbon filter rinse threshold is stricter (1.0 NTU vs 2.0 NTU). A
   turbidity reading that passes for sand must fail for carbon. *)
TEST_PROGRAM BackwashCarbonRinseLimitIsStricter
VAR
    Sand : SandFilterBackwash;
    Carbon : CarbonFilterBackwash;
    SandResult : BackwashResult;
    CarbonResult : BackwashResult;
END_VAR

Sand.Initialize();
Carbon.Initialize();

(* Both past required time, turbidity = 1.5 (between sand 2.0 and carbon 1.0) *)
SandResult := Sand.RunTemplate(ElapsedSeconds := INT#600, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
CarbonResult := Carbon.RunTemplate(ElapsedSeconds := INT#600, TurbidityNtu := REAL#1.5, FlowOk := TRUE);

ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := SandResult.LastStep);
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := CarbonResult.LastStep);
END_TEST_PROGRAM
