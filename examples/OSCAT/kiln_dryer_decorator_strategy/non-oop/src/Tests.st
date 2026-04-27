TEST_PROGRAM ClassicKilnDryerBranchesOnProduct
VAR
    Dryer : ClassicKilnDryer;
    SoftFan : REAL;
    HardFan : REAL;
END_VAR
Dryer.Update(ProductId := INT#1, RawMoisturePct := REAL#20.0);
SoftFan := Dryer.FanSpeed;
Dryer.Update(ProductId := INT#2, RawMoisturePct := REAL#20.0);
HardFan := Dryer.FanSpeed;
ASSERT_TRUE(IN := SoftFan > HardFan);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Dryer.ActiveProfileId);
END_TEST_PROGRAM
