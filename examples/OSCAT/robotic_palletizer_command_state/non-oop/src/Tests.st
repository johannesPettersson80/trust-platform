TEST_PROGRAM ClassicPalletizerCompletesInline
VAR
    Cell : ClassicPalletizer;
END_VAR

Cell.Initialize();
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Cell.CompletedPallets);
END_TEST_PROGRAM
