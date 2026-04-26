TEST_PROGRAM TankControlWithMockIo
VAR
    Dut : TankControl;
END_VAR

(* Low level -> fill tank fast. *)
Dut(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#200);
ASSERT_TRUE(IN := Dut.FillValveCmd);
ASSERT_FALSE(IN := Dut.DrainValveCmd);
ASSERT_EQUAL(EXPECTED := INT#800, ACTUAL := Dut.PumpSpeedCmd);

(* High level -> drain tank. *)
Dut(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#850);
ASSERT_FALSE(IN := Dut.FillValveCmd);
ASSERT_TRUE(IN := Dut.DrainValveCmd);
ASSERT_EQUAL(EXPECTED := INT#600, ACTUAL := Dut.PumpSpeedCmd);

(* Stop must dominate all other behavior. *)
Dut(StartCmd := TRUE, StopCmd := TRUE, LevelRaw := INT#100);
ASSERT_FALSE(IN := Dut.FillValveCmd);
ASSERT_FALSE(IN := Dut.DrainValveCmd);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Dut.PumpSpeedCmd);
END_TEST_PROGRAM

TEST_FUNCTION_BLOCK TankControlBand
VAR
    Dut : TankControl;
END_VAR

(* Mid band -> hold state with nominal pump speed. *)
Dut(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#500);
ASSERT_FALSE(IN := Dut.FillValveCmd);
ASSERT_FALSE(IN := Dut.DrainValveCmd);
ASSERT_EQUAL(EXPECTED := INT#500, ACTUAL := Dut.PumpSpeedCmd);
END_TEST_FUNCTION_BLOCK
