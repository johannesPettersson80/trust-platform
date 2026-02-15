TEST_PROGRAM TEST_TANK_CONTROL_WITH_MOCK_IO
VAR
    DUT : FB_TANK_CONTROL;
END_VAR

(* Low level -> fill tank fast. *)
DUT(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#200);
ASSERT_TRUE(IN := DUT.FillValveCmd);
ASSERT_FALSE(IN := DUT.DrainValveCmd);
ASSERT_EQUAL(EXPECTED := INT#800, ACTUAL := DUT.PumpSpeedCmd);

(* High level -> drain tank. *)
DUT(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#850);
ASSERT_FALSE(IN := DUT.FillValveCmd);
ASSERT_TRUE(IN := DUT.DrainValveCmd);
ASSERT_EQUAL(EXPECTED := INT#600, ACTUAL := DUT.PumpSpeedCmd);

(* Stop must dominate all other behavior. *)
DUT(StartCmd := TRUE, StopCmd := TRUE, LevelRaw := INT#100);
ASSERT_FALSE(IN := DUT.FillValveCmd);
ASSERT_FALSE(IN := DUT.DrainValveCmd);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := DUT.PumpSpeedCmd);
END_TEST_PROGRAM

TEST_FUNCTION_BLOCK TEST_TANK_CONTROL_BAND
VAR
    DUT : FB_TANK_CONTROL;
END_VAR

(* Mid band -> hold state with nominal pump speed. *)
DUT(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#500);
ASSERT_FALSE(IN := DUT.FillValveCmd);
ASSERT_FALSE(IN := DUT.DrainValveCmd);
ASSERT_EQUAL(EXPECTED := INT#500, ACTUAL := DUT.PumpSpeedCmd);
END_TEST_FUNCTION_BLOCK
