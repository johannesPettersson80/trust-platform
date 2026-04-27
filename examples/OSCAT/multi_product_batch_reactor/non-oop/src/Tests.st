TEST_PROGRAM BatchReactorClassicRunsRecipeBranchesAndRejectsInvalidRecipe
VAR
    Reactor : BatchReactorClassic;
END_VAR

Reactor.Initialize();
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#800, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE));
ASSERT_TRUE(IN := Reactor.BatchComplete);

Reactor.Reset();
ASSERT_EQUAL(EXPECTED := INT#900, ACTUAL := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#99, ProductTemp := REAL#25.0, PH := REAL#7.0, HoldComplete := FALSE, SampleOk := FALSE));
ASSERT_TRUE(IN := Reactor.AlarmActive);
ASSERT_EQUAL(EXPECTED := DWORD#16#B001, ACTUAL := Reactor.AlarmCode);
END_TEST_PROGRAM
