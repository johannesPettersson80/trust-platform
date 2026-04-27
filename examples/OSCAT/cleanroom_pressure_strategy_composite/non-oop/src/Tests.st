TEST_PROGRAM ClassicCleanroomBranchesOnMode
VAR
    Cascade : ClassicCleanroomCascade;
    NormalCmd : REAL;
    CleaningCmd : REAL;
END_VAR
NormalCmd := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
CleaningCmd := Cascade.Update(ModeRequest := INT#2, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
ASSERT_TRUE(IN := CleaningCmd > NormalCmd);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Cascade.ActiveModeId);
END_TEST_PROGRAM
