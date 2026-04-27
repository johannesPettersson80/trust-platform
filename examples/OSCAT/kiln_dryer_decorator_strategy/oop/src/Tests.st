TEST_PROGRAM KilnDecoratorClampsBadMoisture
VAR
    Dryer : KilnDryerController;
    Heat : REAL;
END_VAR
Dryer.Initialize();
Heat := Dryer.Update(ProductId := INT#1, RawMoisturePct := REAL#120.0);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Dryer.MoistureQuality);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Dryer.ConditionedMoisture, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM KilnStrategiesSelectDifferentFanSpeed
VAR
    Dryer : KilnDryerController;
    SoftFan : REAL;
    HardFan : REAL;
END_VAR
Dryer.Initialize();
Dryer.Update(ProductId := INT#1, RawMoisturePct := REAL#20.0);
SoftFan := Dryer.FanSpeed;
Dryer.Update(ProductId := INT#2, RawMoisturePct := REAL#20.0);
HardFan := Dryer.FanSpeed;
ASSERT_TRUE(IN := SoftFan > HardFan);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Dryer.ActiveProfileId);
END_TEST_PROGRAM

(* Decorator + strategy combined-flow test: drives a sequence of bad
   inputs (clamped) -> good inputs (passes through) -> profile switch
   on the same instance. Catches bugs where the decorator's quality
   flag stays bad after a clean reading, or where strategy selection
   doesn't re-evaluate when ProductId changes. *)
TEST_PROGRAM KilnDecoratorAndStrategyReevaluateAcrossScans
VAR
    Dryer : KilnDryerController;
END_VAR

Dryer.Initialize();

(* Bad input - quality clamped *)
Dryer.Update(ProductId := INT#1, RawMoisturePct := REAL#120.0);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Dryer.MoistureQuality);

(* Recover - quality must report good *)
Dryer.Update(ProductId := INT#1, RawMoisturePct := REAL#22.0);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Dryer.MoistureQuality);

(* Switch product (Strategy change) on recovered signal *)
Dryer.Update(ProductId := INT#2, RawMoisturePct := REAL#22.0);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Dryer.ActiveProfileId);

(* Switch back to product 1 - profile must follow *)
Dryer.Update(ProductId := INT#1, RawMoisturePct := REAL#22.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Dryer.ActiveProfileId);
END_TEST_PROGRAM

(* Boundary test: clamp must trip on the negative side too. A negative
   moisture reading is physically impossible and must not be passed
   through. *)
TEST_PROGRAM KilnDecoratorClampsNegativeMoisture
VAR
    Dryer : KilnDryerController;
END_VAR
Dryer.Initialize();
Dryer.Update(ProductId := INT#1, RawMoisturePct := REAL#-5.0);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Dryer.MoistureQuality);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Dryer.ConditionedMoisture, DELTA := REAL#0.001);
(* Heat command must be zero because (clamped 0 - 12 setpoint) * gain
   limits to zero on the lower bound *)
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Dryer.HeatCommand, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Strategy edge inputs: each strategy maps the moisture-vs-target
   curve to a heat command. Below target, heat is zero; above target,
   it grows linearly until the per-strategy ceiling. *)
TEST_PROGRAM KilnStrategiesRespectTargetAndCeilings
VAR
    Soft : SoftwoodStrategy;
    Hard : HardwoodStrategy;
END_VAR

(* Softwood target 12, gain 3, ceiling 100 *)
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Soft.HeatCommand(MoisturePct := REAL#10.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Soft.HeatCommand(MoisturePct := REAL#12.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#15.0, ACTUAL := Soft.HeatCommand(MoisturePct := REAL#17.0), DELTA := REAL#0.001);
(* Saturation: above 12 + 100/3 = 45.33 the ceiling clips *)
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Soft.HeatCommand(MoisturePct := REAL#80.0), DELTA := REAL#0.001);

(* Hardwood target 8, gain 2, ceiling 80 *)
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Hard.HeatCommand(MoisturePct := REAL#5.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Hard.HeatCommand(MoisturePct := REAL#8.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := Hard.HeatCommand(MoisturePct := REAL#13.0), DELTA := REAL#0.001);
(* Hardwood ceiling: above 8 + 80/2 = 48 the ceiling clips *)
ASSERT_NEAR(EXPECTED := REAL#80.0, ACTUAL := Hard.HeatCommand(MoisturePct := REAL#60.0), DELTA := REAL#0.001);

(* Per-product fan speed *)
ASSERT_NEAR(EXPECTED := REAL#45.0, ACTUAL := Soft.FanSpeed(MoisturePct := REAL#20.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#30.0, ACTUAL := Hard.FanSpeed(MoisturePct := REAL#20.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Decorator chain isolation: two RawMoistureInput sources can each
   feed an independent clamp without bleeding. The pattern requires
   one wrap per source; two wraps must be independent. *)
TEST_PROGRAM KilnDecoratorChainsAreIndependent
VAR
    SourceA : RawMoistureInput;
    SourceB : RawMoistureInput;
    ClampA : MoistureClampDecorator;
    ClampB : MoistureClampDecorator;
END_VAR

ClampA.Wrap(Source := SourceA);
ClampB.Wrap(Source := SourceB);

SourceA.SetRaw(MoisturePct := REAL#150.0);
SourceB.SetRaw(MoisturePct := REAL#22.0);
ClampA.Update();
ClampB.Update();

(* A is clamped, B is in range *)
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := ClampA.Value, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := ClampA.Quality);
ASSERT_NEAR(EXPECTED := REAL#22.0, ACTUAL := ClampB.Value, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := ClampB.Quality);

(* Drive A back into range and B out: the flags must follow each
   independently *)
SourceA.SetRaw(MoisturePct := REAL#30.0);
SourceB.SetRaw(MoisturePct := REAL#-10.0);
ClampA.Update();
ClampB.Update();
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := ClampA.Quality);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := ClampB.Quality);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := ClampB.Value, DELTA := REAL#0.001);
END_TEST_PROGRAM
