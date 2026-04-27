TEST_PROGRAM AhuClassicUsesModeBranches
VAR
    Ahu : AhuClassic;
    EcoCommand : REAL;
    ComfortCommand : REAL;
    FrostCommand : REAL;
END_VAR

Ahu.Initialize();
EcoCommand := Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Ahu.ActiveMode);
ComfortCommand := Ahu.Update(ModeRequest := INT#2, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Ahu.ActiveMode);
ASSERT_TRUE(IN := ComfortCommand > EcoCommand);
FrostCommand := Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#-8.0, SupplyTemp := REAL#1.0, SupplySetpoint := REAL#3.0, SmokeClear := TRUE, FreezeStatClear := FALSE, SupplyFanHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Ahu.ActiveMode);
ASSERT_TRUE(IN := FrostCommand >= REAL#20.0);
END_TEST_PROGRAM
