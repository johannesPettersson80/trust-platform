TEST_PROGRAM CoolingTowerFacadeUsesSummerTemplate
VAR
    Tower : CoolingTowerCell;
    Result : TowerResult;
END_VAR
Tower.Initialize();
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#32.0, OutdoorTempC := REAL#20.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Result.SeasonId);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Result.FanStage);
END_TEST_PROGRAM

TEST_PROGRAM CoolingTowerWinterTemplateEnablesFreezeProtection
VAR
    Tower : CoolingTowerCell;
    Result : TowerResult;
END_VAR
Tower.Initialize();
Result := Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#20.0, OutdoorTempC := REAL#0.0, ConductivityUs := REAL#2000.0);
ASSERT_TRUE(IN := Result.BasinHeater);
ASSERT_TRUE(IN := Result.BleedValve);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Result.SeasonId);
END_TEST_PROGRAM

(* Season-switching integration test: drives the same Tower from
   summer to winter and back. Catches bugs where template state caches
   between seasons (e.g., basin heater still on in summer because the
   winter template's state was not cleared). *)
TEST_PROGRAM CoolingTowerSwitchesBetweenSummerAndWinterTemplates
VAR
    Tower : CoolingTowerCell;
    SummerResult : TowerResult;
    WinterResult : TowerResult;
    SummerAgain : TowerResult;
END_VAR

Tower.Initialize();

(* Summer: hot water, fans on, no basin heater *)
SummerResult := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#32.0, OutdoorTempC := REAL#25.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := SummerResult.SeasonId);
ASSERT_FALSE(IN := SummerResult.BasinHeater);

(* Winter: cold outdoor, basin heater + bleed *)
WinterResult := Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#20.0, OutdoorTempC := REAL#0.0, ConductivityUs := REAL#2000.0);
ASSERT_TRUE(IN := WinterResult.BasinHeater);

(* Back to summer: basin heater must turn off (freeze protect specific
   to winter template), not stick from prior winter scan *)
SummerAgain := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#32.0, OutdoorTempC := REAL#25.0, ConductivityUs := REAL#1000.0);
ASSERT_FALSE(IN := SummerAgain.BasinHeater);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := SummerAgain.SeasonId);
END_TEST_PROGRAM

(* Fan stage boundaries, summer template (FanThreshold = 26 C):
   exactly at, just above, just below 26 + 5 = 31 must fall on the
   correct stage boundary. *)
TEST_PROGRAM CoolingTowerSummerFanStageBoundaries
VAR
    Tower : CoolingTowerCell;
    Result : TowerResult;
END_VAR
Tower.Initialize();

(* Below FanThreshold (26.0): stage 0 *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#25.9, OutdoorTempC := REAL#20.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Result.FanStage);

(* Exactly at FanThreshold (26.0): stage 0 (strict greater-than) *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#26.0, OutdoorTempC := REAL#20.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Result.FanStage);

(* Just above FanThreshold (26.0): stage 1 *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#26.1, OutdoorTempC := REAL#20.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Result.FanStage);

(* Below upper threshold (26 + 5 = 31): still stage 1 *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#31.0, OutdoorTempC := REAL#20.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Result.FanStage);

(* Just above upper threshold: stage 2 *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#31.1, OutdoorTempC := REAL#20.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Result.FanStage);
END_TEST_PROGRAM

(* Winter template: FanThreshold = 32 C, FreezeThreshold = 4 C.
   Verifies winter sets a higher fan threshold and warmer freeze line. *)
TEST_PROGRAM CoolingTowerWinterFanStageBoundaries
VAR
    Tower : CoolingTowerCell;
    Result : TowerResult;
END_VAR
Tower.Initialize();

(* WaterTemp 31.9 C with winter (threshold 32): stage 0. Same temp in
   summer would be stage 2 (above 26+5=31). *)
Result := Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#31.9, OutdoorTempC := REAL#5.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Result.FanStage);

(* Just above 32: stage 1 in winter *)
Result := Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#32.1, OutdoorTempC := REAL#5.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Result.FanStage);

(* Above 32 + 5 = 37: stage 2 *)
Result := Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#37.5, OutdoorTempC := REAL#5.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Result.FanStage);
END_TEST_PROGRAM

(* Freeze threshold boundary: winter freeze line is 4 C; summer is
   -10 C. Outdoor at 3.9 C must trip basin heater in winter but not
   summer; outdoor at 4.0 C must NOT trip heater (strict less-than). *)
TEST_PROGRAM CoolingTowerFreezeThresholdBoundaries
VAR
    Tower : CoolingTowerCell;
    Result : TowerResult;
END_VAR
Tower.Initialize();

(* Winter at exactly 4.0 C: heater stays off (strict less-than) *)
Result := Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#20.0, OutdoorTempC := REAL#4.0, ConductivityUs := REAL#1000.0);
ASSERT_FALSE(IN := Result.BasinHeater);

(* Winter at 3.9 C: heater on *)
Result := Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#20.0, OutdoorTempC := REAL#3.9, ConductivityUs := REAL#1000.0);
ASSERT_TRUE(IN := Result.BasinHeater);

(* Summer at 0.0 C (below winter threshold): heater off because
   summer freeze line is -10 *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#20.0, OutdoorTempC := REAL#0.0, ConductivityUs := REAL#1000.0);
ASSERT_FALSE(IN := Result.BasinHeater);

(* Summer at -10.0 C: still off (strict less-than) *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#20.0, OutdoorTempC := REAL#-10.0, ConductivityUs := REAL#1000.0);
ASSERT_FALSE(IN := Result.BasinHeater);

(* Summer at -10.1 C: heater on *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#20.0, OutdoorTempC := REAL#-10.1, ConductivityUs := REAL#1000.0);
ASSERT_TRUE(IN := Result.BasinHeater);
END_TEST_PROGRAM

(* Conductivity bleed boundary: 1800 uS/cm threshold. Strict greater-than. *)
TEST_PROGRAM CoolingTowerConductivityBleedBoundary
VAR
    Tower : CoolingTowerCell;
    Result : TowerResult;
END_VAR
Tower.Initialize();

(* At 1800: not bleeding *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#20.0, OutdoorTempC := REAL#15.0, ConductivityUs := REAL#1800.0);
ASSERT_FALSE(IN := Result.BleedValve);

(* Just above 1800: bleed opens *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#20.0, OutdoorTempC := REAL#15.0, ConductivityUs := REAL#1800.1);
ASSERT_TRUE(IN := Result.BleedValve);

(* Below 1800: bleed closes again on the same instance *)
Result := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#20.0, OutdoorTempC := REAL#15.0, ConductivityUs := REAL#1799.9);
ASSERT_FALSE(IN := Result.BleedValve);
END_TEST_PROGRAM

(* Per-template isolation: SummerTowerTemplate alone (without the
   facade) returns the right thresholds. Tests inheritance hook
   overrides directly. *)
TEST_PROGRAM CoolingTowerSummerTemplateIsolatedHookOverrides
VAR
    Summer : SummerTowerTemplate;
    Result : TowerResult;
END_VAR
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Summer.SeasonId());
ASSERT_EQUAL(EXPECTED := REAL#26.0, ACTUAL := Summer.FanThreshold());
ASSERT_EQUAL(EXPECTED := REAL#-10.0, ACTUAL := Summer.FreezeThreshold());
ASSERT_EQUAL(EXPECTED := REAL#1800.0, ACTUAL := Summer.ConductivityLimit());

Result := Summer.RunTemplate(WaterTempC := REAL#30.0, OutdoorTempC := REAL#-15.0, ConductivityUs := REAL#1700.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Result.SeasonId);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Result.FanStage);
ASSERT_TRUE(IN := Result.BasinHeater);
ASSERT_FALSE(IN := Result.BleedValve);
END_TEST_PROGRAM

(* Per-template isolation: WinterTowerTemplate alone. *)
TEST_PROGRAM CoolingTowerWinterTemplateIsolatedHookOverrides
VAR
    Winter : WinterTowerTemplate;
    Result : TowerResult;
END_VAR
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Winter.SeasonId());
ASSERT_EQUAL(EXPECTED := REAL#32.0, ACTUAL := Winter.FanThreshold());
ASSERT_EQUAL(EXPECTED := REAL#4.0, ACTUAL := Winter.FreezeThreshold());

(* Winter at marginal water temp (33) and warm outdoor: stage 1, no heat *)
Result := Winter.RunTemplate(WaterTempC := REAL#33.0, OutdoorTempC := REAL#10.0, ConductivityUs := REAL#1500.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Result.FanStage);
ASSERT_FALSE(IN := Result.BasinHeater);
ASSERT_FALSE(IN := Result.BleedValve);
END_TEST_PROGRAM

(* Multi-cycle facade: the same Tower scanned 5 times across changing
   inputs. Snapshot properties must reflect the most recent Update. *)
TEST_PROGRAM CoolingTowerFacadePropertiesReflectLastUpdate
VAR
    Tower : CoolingTowerCell;
    I : INT;
END_VAR
Tower.Initialize();

(* First scan: summer hot *)
Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#32.0, OutdoorTempC := REAL#25.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Tower.FanStage);
ASSERT_FALSE(IN := Tower.BasinHeater);

(* Second scan: same season, cooler *)
Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#25.0, OutdoorTempC := REAL#15.0, ConductivityUs := REAL#1000.0);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Tower.FanStage);

(* Third scan: high conductivity *)
Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#25.0, OutdoorTempC := REAL#15.0, ConductivityUs := REAL#2500.0);
ASSERT_TRUE(IN := Tower.BleedValve);

(* Fourth scan: switch to winter, freeze *)
Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#15.0, OutdoorTempC := REAL#-5.0, ConductivityUs := REAL#500.0);
ASSERT_TRUE(IN := Tower.BasinHeater);
ASSERT_FALSE(IN := Tower.BleedValve);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Tower.SeasonId);

(* Fifth scan: winter warming up *)
Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#20.0, OutdoorTempC := REAL#10.0, ConductivityUs := REAL#500.0);
ASSERT_FALSE(IN := Tower.BasinHeater);
END_TEST_PROGRAM
