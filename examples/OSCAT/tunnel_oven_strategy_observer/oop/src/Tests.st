TEST_PROGRAM OvenStrategiesProduceDifferentCommands
VAR
    Oven : TunnelOvenController;
    BreadHeat : REAL;
    PizzaHeat : REAL;
END_VAR

Oven.Initialize();
BreadHeat := Oven.Update(ProductId := INT#1, ZoneTemp := REAL#180.0, Overtemp := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Oven.ActiveProfileId);
PizzaHeat := Oven.Update(ProductId := INT#2, ZoneTemp := REAL#180.0, Overtemp := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Oven.ActiveProfileId);
ASSERT_TRUE(IN := PizzaHeat > BreadHeat);
ASSERT_NEAR(EXPECTED := REAL#55.0, ACTUAL := Oven.ConveyorSpeed, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM OvenObserverPublishesOvertempEvent
VAR
    Oven : TunnelOvenController;
    Heat : REAL;
END_VAR

Oven.Initialize();
Heat := Oven.Update(ProductId := INT#2, ZoneTemp := REAL#260.0, Overtemp := TRUE);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Heat, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Oven.QualityEventCount);
ASSERT_TRUE(IN := Oven.MqttPublishReady);
END_TEST_PROGRAM

(* Multi-event observer + strategy test: drives the same oven through
   normal -> overtemp -> recovered with a profile switch. Catches bugs
   where QualityEventCount stops growing after the first event or
   where the strategy slot fails to re-evaluate when ProductId changes
   after an overtemp recovery. *)
TEST_PROGRAM OvenAccumulatesEventsAndSwitchesProfileAcrossScans
VAR
    Oven : TunnelOvenController;
    InitialEvents : INT;
END_VAR

Oven.Initialize();

(* Scan 1: normal pizza bake *)
Oven.Update(ProductId := INT#2, ZoneTemp := REAL#180.0, Overtemp := FALSE);
InitialEvents := Oven.QualityEventCount;

(* Scan 2: overtemp event - count must grow *)
Oven.Update(ProductId := INT#2, ZoneTemp := REAL#260.0, Overtemp := TRUE);
ASSERT_TRUE(IN := Oven.QualityEventCount > InitialEvents);

(* Scan 3: recovered, switched to bread profile *)
Oven.Update(ProductId := INT#1, ZoneTemp := REAL#180.0, Overtemp := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Oven.ActiveProfileId);
END_TEST_PROGRAM

(* Multi-call regression: every overtemp on a single scan increments the
   historian counter exactly once. Catches bugs where the observer fires
   twice (e.g., once per profile member access) or zero times (e.g.,
   if the bus publishes to a stale subscriber slot). *)
TEST_PROGRAM OvenObserverIncrementsExactlyOncePerOvertempScan
VAR
    Oven : TunnelOvenController;
    I : INT;
    ExpectedCount : INT;
END_VAR

Oven.Initialize();
ExpectedCount := INT#0;
FOR I := 1 TO 5 DO
    Oven.Update(ProductId := INT#2, ZoneTemp := REAL#260.0, Overtemp := TRUE);
    ExpectedCount := ExpectedCount + INT#1;
    ASSERT_EQUAL(EXPECTED := ExpectedCount, ACTUAL := Oven.QualityEventCount);
END_FOR;
END_TEST_PROGRAM

(* Boundary test: well below setpoint, the LIMIT clamps the bread
   heat command to 100. (Pt1Filter is wall-clock based so chasing high
   inputs from a cold first sample is not stable in microsecond test
   cycles; we only check the cold-clamp here.) *)
TEST_PROGRAM OvenBreadProfileColdClampsHeatCommandHigh
VAR
    Oven : TunnelOvenController;
    Heat : REAL;
END_VAR

Oven.Initialize();
Heat := Oven.Update(ProductId := INT#1, ZoneTemp := REAL#20.0, Overtemp := FALSE);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Heat, DELTA := REAL#0.5);
END_TEST_PROGRAM

(* Per-FB isolation: directly drive a PizzaProfile object and verify it
   reports profile id 2 and conveyor speed 55 without a controller. *)
TEST_PROGRAM OvenPizzaProfileReportsExpectedConveyorAndId
VAR
    Pizza : PizzaProfile;
    Speed : REAL;
END_VAR

Pizza.Initialize();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Pizza.ProfileId);
Speed := Pizza.ConveyorSpeed();
ASSERT_NEAR(EXPECTED := REAL#55.0, ACTUAL := Speed, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Per-FB isolation: drive an OvenHistorianSubscriber on its own and
   confirm that EventCount accumulates one per OnOvenEvent call. *)
TEST_PROGRAM OvenHistorianSubscriberCountsAllEvents
VAR
    Sub : OvenHistorianSubscriber;
END_VAR

Sub.OnOvenEvent(EventClass := BYTE#3, EventCode := WORD#16#A601, ProfileId := INT#1);
Sub.OnOvenEvent(EventClass := BYTE#3, EventCode := WORD#16#A601, ProfileId := INT#2);
Sub.OnOvenEvent(EventClass := BYTE#1, EventCode := WORD#16#B001, ProfileId := INT#2);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Sub.EventCount);
END_TEST_PROGRAM

(* Edge input: ProductId outside {1, 2} must default to bread (id 1)
   so a stale or wild input never selects an undefined strategy. *)
TEST_PROGRAM OvenUnknownProductIdDefaultsToBread
VAR
    Oven : TunnelOvenController;
END_VAR

Oven.Initialize();
Oven.Update(ProductId := INT#42, ZoneTemp := REAL#180.0, Overtemp := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Oven.ActiveProfileId);
ASSERT_NEAR(EXPECTED := REAL#35.0, ACTUAL := Oven.ConveyorSpeed, DELTA := REAL#0.001);
Oven.Update(ProductId := INT#-1, ZoneTemp := REAL#180.0, Overtemp := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Oven.ActiveProfileId);
END_TEST_PROGRAM
