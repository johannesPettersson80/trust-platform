TEST_PROGRAM ClassicTunnelOvenBranchesOnProduct
VAR
    Oven : ClassicTunnelOven;
    BreadHeat : REAL;
    PizzaHeat : REAL;
END_VAR

BreadHeat := Oven.Update(ProductId := INT#1, ZoneTemp := REAL#180.0, Overtemp := FALSE);
PizzaHeat := Oven.Update(ProductId := INT#2, ZoneTemp := REAL#180.0, Overtemp := FALSE);
ASSERT_TRUE(IN := PizzaHeat > BreadHeat);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Oven.ActiveProfileId);
END_TEST_PROGRAM
