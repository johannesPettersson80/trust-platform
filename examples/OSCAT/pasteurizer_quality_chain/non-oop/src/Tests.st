TEST_PROGRAM ClassicPasteurizerBranchesOnRecipeAndAlarmInline
VAR
    Pasteurizer : ClassicPasteurizer;
    Result : ClassicPasteurizerResult;
END_VAR

Result := Pasteurizer.Update(RecipeId := INT#2, ProductTemp := REAL#73.0, FlowRate := REAL#120.0, HoldSeconds := INT#15, OperatorAck := FALSE, SupervisorOnline := FALSE);
ASSERT_FALSE(IN := Result.ForwardDivert);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Result.LastHandlerId);
ASSERT_TRUE(IN := Result.BatchAborted);
END_TEST_PROGRAM
