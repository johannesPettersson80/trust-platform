TEST_PROGRAM oscat_basic_plc_clock_helpers_behave
VAR
FirstMs : DWORD;
SecondMs : DWORD;
ClockUs : DWORD;
END_VAR

FirstMs := T_PLC_MS();
SecondMs := T_PLC_MS();
ClockUs := T_PLC_US();

ASSERT_TRUE(IN := SecondMs >= FirstMs);
ASSERT_TRUE(IN := DWORD_TO_UDINT(ClockUs) >= DWORD_TO_UDINT(FirstMs) * UDINT#1000);
END_TEST_PROGRAM
