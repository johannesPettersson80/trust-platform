TEST_PROGRAM DistrictMediatorBalancesHealthyRemoteDemand
VAR
    Network : DemandMediator;
END_VAR

Network.Initialize();
ASSERT_EQUAL(EXPECTED := REAL#90.0, ACTUAL := Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#5, EastDemandM3H := REAL#10.0, EastQualityGood := TRUE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := REAL#45.0, ACTUAL := Network.TotalDemandM3H);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Network.RemoteStaleCount);
END_TEST_PROGRAM

TEST_PROGRAM DistrictProxyDropsStaleRemoteDemand
VAR
    Network : DemandMediator;
END_VAR

Network.Initialize();
ASSERT_EQUAL(EXPECTED := REAL#40.0, ACTUAL := Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#45, EastDemandM3H := REAL#10.0, EastQualityGood := FALSE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := REAL#20.0, ACTUAL := Network.TotalDemandM3H);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Network.RemoteStaleCount);
END_TEST_PROGRAM

TEST_PROGRAM DistrictMediatorTripsOnLocalSuctionLoss
VAR
    Network : DemandMediator;
END_VAR

Network.Initialize();
ASSERT_EQUAL(EXPECTED := REAL#0.0, ACTUAL := Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#5, EastDemandM3H := REAL#10.0, EastQualityGood := TRUE, EastAgeSeconds := INT#5, SuctionHealthy := FALSE));
ASSERT_TRUE(IN := Network.ClassAAlarm);
END_TEST_PROGRAM

(* Multi-scan proxy-aging test: drives the same network through
   healthy, stale, and recovered remote states. Catches bugs where
   RemoteStaleCount stays sticky after recovery, or where a stale
   remote's last-known demand keeps contributing to TotalDemandM3H. *)
TEST_PROGRAM DistrictProxyTracksFreshAndStaleStateAcrossScans
VAR
    Network : DemandMediator;
END_VAR

Network.Initialize();

(* Scan 1: both remotes fresh *)
Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#5, EastDemandM3H := REAL#10.0, EastQualityGood := TRUE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Network.RemoteStaleCount);

(* Scan 2: north stale, east bad-quality *)
Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#45, EastDemandM3H := REAL#10.0, EastQualityGood := FALSE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Network.RemoteStaleCount);

(* Scan 3: both remotes recover - stale count must reset to zero *)
Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#5, EastDemandM3H := REAL#10.0, EastQualityGood := TRUE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Network.RemoteStaleCount);
ASSERT_EQUAL(EXPECTED := REAL#45.0, ACTUAL := Network.TotalDemandM3H);
END_TEST_PROGRAM

(* Boundary: exact age threshold. The proxy uses AgeSeconds > 30 as
   the staleness rule, so 30 is fresh, 31 is stale. *)
TEST_PROGRAM DistrictProxyAgeBoundaryAtExactThirty
VAR
    Network : DemandMediator;
END_VAR

Network.Initialize();

(* Age = 30 (exact threshold, still fresh by > rule) *)
Network.Update(LocalDemandM3H := REAL#10.0, NorthDemandM3H := REAL#5.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#30, EastDemandM3H := REAL#0.0, EastQualityGood := TRUE, EastAgeSeconds := INT#0, SuctionHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Network.RemoteStaleCount);
ASSERT_EQUAL(EXPECTED := REAL#15.0, ACTUAL := Network.TotalDemandM3H);

(* Age = 31 (over threshold, stale) *)
Network.Update(LocalDemandM3H := REAL#10.0, NorthDemandM3H := REAL#5.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#31, EastDemandM3H := REAL#0.0, EastQualityGood := TRUE, EastAgeSeconds := INT#0, SuctionHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Network.RemoteStaleCount);
ASSERT_EQUAL(EXPECTED := REAL#10.0, ACTUAL := Network.TotalDemandM3H);
END_TEST_PROGRAM

(* RemoteStationProxy in isolation: tests the staleness rule
   directly on a single proxy, verifies the IsStale property tracks
   the rule (NOT QualityGood OR AgeSeconds > 30). *)
TEST_PROGRAM DistrictRemoteProxyStalenessRuleInIsolation
VAR
    Remote : RemoteStationProxy;
END_VAR

Remote.Configure(StationId := INT#42);
ASSERT_EQUAL(EXPECTED := INT#42, ACTUAL := Remote.StationId);

(* Healthy: fresh demand, good quality *)
Remote.UpdateProxy(DemandM3H := REAL#7.5, QualityGoodInput := TRUE, AgeSeconds := INT#10);
ASSERT_FALSE(IN := Remote.IsStale);
ASSERT_TRUE(IN := Remote.QualityGood);
ASSERT_NEAR(EXPECTED := REAL#7.5, ACTUAL := Remote.DemandM3H, DELTA := REAL#0.001);

(* Aged out *)
Remote.UpdateProxy(DemandM3H := REAL#7.5, QualityGoodInput := TRUE, AgeSeconds := INT#100);
ASSERT_TRUE(IN := Remote.IsStale);

(* Bad quality even at age 0 *)
Remote.UpdateProxy(DemandM3H := REAL#7.5, QualityGoodInput := FALSE, AgeSeconds := INT#0);
ASSERT_TRUE(IN := Remote.IsStale);

(* Recovered *)
Remote.UpdateProxy(DemandM3H := REAL#5.0, QualityGoodInput := TRUE, AgeSeconds := INT#5);
ASSERT_FALSE(IN := Remote.IsStale);
END_TEST_PROGRAM

(* LocalStationProxy is never stale by design - the local PLC's own
   scan tags do not have a freshness contract. *)
TEST_PROGRAM DistrictLocalProxyIsNeverStale
VAR
    Local : LocalStationProxy;
END_VAR

(* Exercising UpdateProxy with bad quality and stale age must not flip IsStale *)
Local.UpdateProxy(DemandM3H := REAL#42.0, QualityGoodInput := FALSE, AgeSeconds := INT#9999);
ASSERT_FALSE(IN := Local.IsStale);
ASSERT_TRUE(IN := Local.QualityGood);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Local.StationId);
ASSERT_NEAR(EXPECTED := REAL#42.0, ACTUAL := Local.DemandM3H, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Suction trip overrides aggregated demand: even with all stations
   healthy, ClassA alarm forces PumpSpeed = 0. *)
TEST_PROGRAM DistrictSuctionTripOverridesHealthyAggregate
VAR
    Network : DemandMediator;
END_VAR

Network.Initialize();

(* All healthy except suction *)
ASSERT_EQUAL(EXPECTED := REAL#0.0, ACTUAL := Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#5, EastDemandM3H := REAL#10.0, EastQualityGood := TRUE, EastAgeSeconds := INT#5, SuctionHealthy := FALSE));
ASSERT_TRUE(IN := Network.ClassAAlarm);
(* TotalDemand should still aggregate (mediator computes both) *)
ASSERT_EQUAL(EXPECTED := REAL#45.0, ACTUAL := Network.TotalDemandM3H);

(* Recovery: suction comes back, command resumes *)
ASSERT_EQUAL(EXPECTED := REAL#90.0, ACTUAL := Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#5, EastDemandM3H := REAL#10.0, EastQualityGood := TRUE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE));
ASSERT_FALSE(IN := Network.ClassAAlarm);
END_TEST_PROGRAM

(* Pump speed clamps at 100% even with very large total demand. *)
TEST_PROGRAM DistrictPumpSpeedSaturatesAtHundredPercent
VAR
    Network : DemandMediator;
END_VAR

Network.Initialize();

(* Total demand 200 m3/h * 2 = 400; clamped to 100 *)
ASSERT_EQUAL(EXPECTED := REAL#100.0, ACTUAL := Network.Update(LocalDemandM3H := REAL#100.0, NorthDemandM3H := REAL#50.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#5, EastDemandM3H := REAL#50.0, EastQualityGood := TRUE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := REAL#200.0, ACTUAL := Network.TotalDemandM3H);
END_TEST_PROGRAM
