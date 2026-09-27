(* @trustlsp:runtime-ignore — run with trust-dev test, not as a PLC instance. *)
TEST_PROGRAM Lesson2SingleThresholdTruthTable
ASSERT_FALSE(IN := ColdStorageCoolingRequest(
    Enabled := TRUE,
    ProcessValue := LREAL#1.9,
    Setpoint := LREAL#2.0
));
ASSERT_TRUE(IN := ColdStorageCoolingRequest(
    Enabled := TRUE,
    ProcessValue := LREAL#2.0,
    Setpoint := LREAL#2.0
));
ASSERT_TRUE(IN := ColdStorageCoolingRequest(
    Enabled := TRUE,
    ProcessValue := LREAL#2.1,
    Setpoint := LREAL#2.0
));
ASSERT_FALSE(IN := ColdStorageCoolingRequest(
    Enabled := FALSE,
    ProcessValue := LREAL#2.1,
    Setpoint := LREAL#2.0
));
END_TEST_PROGRAM
