TEST_PROGRAM SiloMediatorGrantsOnlyFirstEligibleSilo
VAR
    System : SiloLoadingSystem;
END_VAR

System.Initialize();
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := System.Update(Request1 := TRUE, Level1 := REAL#40.0, Request2 := TRUE, Level2 := REAL#30.0, Request3 := FALSE, Level3 := REAL#20.0));
ASSERT_TRUE(IN := System.BlowerRunning);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := System.BlockedCount);
END_TEST_PROGRAM

TEST_PROGRAM SiloMediatorSkipsHighLevelRequester
VAR
    System : SiloLoadingSystem;
END_VAR

System.Initialize();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := TRUE, Level2 := REAL#40.0, Request3 := FALSE, Level3 := REAL#20.0));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := System.BlockedCount);
END_TEST_PROGRAM

(* Multi-cycle mediator-rotation test: drives the same system through
   varying requests and levels. Catches bugs where ActiveSilo sticks
   at the previously granted silo, where BlockedCount accumulates
   incorrectly, or where the blower stays running when no silo is
   eligible. *)
TEST_PROGRAM SiloMediatorReevaluatesRequestsAcrossScans
VAR
    System : SiloLoadingSystem;
END_VAR

System.Initialize();

(* Scan 1: silo 1 eligible *)
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := System.Update(Request1 := TRUE, Level1 := REAL#40.0, Request2 := FALSE, Level2 := REAL#30.0, Request3 := FALSE, Level3 := REAL#20.0));
ASSERT_TRUE(IN := System.BlowerRunning);

(* Scan 2: silo 1 stops requesting, silo 3 takes over *)
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := System.Update(Request1 := FALSE, Level1 := REAL#40.0, Request2 := FALSE, Level2 := REAL#30.0, Request3 := TRUE, Level3 := REAL#20.0));

(* Scan 3: no requests - blower stops, no active silo *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := System.Update(Request1 := FALSE, Level1 := REAL#40.0, Request2 := FALSE, Level2 := REAL#30.0, Request3 := FALSE, Level3 := REAL#20.0));
ASSERT_FALSE(IN := System.BlowerRunning);
END_TEST_PROGRAM

(* No requests at all: BlowerRunning stays FALSE, BlockedCount = 0,
   ActiveSiloId = 0. Empty-input edge case. *)
TEST_PROGRAM SiloMediatorIdleWhenNoRequests
VAR
    System : SiloLoadingSystem;
END_VAR
System.Initialize();
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := System.Update(Request1 := FALSE, Level1 := REAL#10.0, Request2 := FALSE, Level2 := REAL#20.0, Request3 := FALSE, Level3 := REAL#30.0));
ASSERT_FALSE(IN := System.BlowerRunning);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := System.BlockedCount);
END_TEST_PROGRAM

(* All three silos requesting and ALL above 90 % - all blocked,
   ActiveSiloId = 0, BlowerRunning = FALSE, BlockedCount = 3.
   Edge case: every requester is locked out, no fallback. *)
TEST_PROGRAM SiloMediatorBlocksAllWhenEveryRequesterHigh
VAR
    System : SiloLoadingSystem;
END_VAR
System.Initialize();
(* Drive level signals through the Pt1 filter long enough to reach high-level *)
System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := TRUE, Level2 := REAL#95.0, Request3 := TRUE, Level3 := REAL#95.0);
System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := TRUE, Level2 := REAL#95.0, Request3 := TRUE, Level3 := REAL#95.0);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := TRUE, Level2 := REAL#95.0, Request3 := TRUE, Level3 := REAL#95.0));
ASSERT_FALSE(IN := System.BlowerRunning);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := System.BlockedCount);
END_TEST_PROGRAM

(* Boundary test on the 90 % high-level threshold. Use raw inputs that
   the filter can settle on. Note: Pt1Filter applies smoothing, so the
   filtered level lags the raw input. The first few cycles may not yet
   have crossed 90 even with raw=95, so we drive several cycles to
   settle. Once settled, exactly 90.0 should be blocked (>=) and just
   below should pass through. *)
TEST_PROGRAM SiloHighLevelThresholdBoundary
VAR
    System : SiloLoadingSystem;
END_VAR
System.Initialize();
(* Drive several cycles at 89.0 to settle filter below 90 *)
System.Update(Request1 := TRUE, Level1 := REAL#89.0, Request2 := FALSE, Level2 := REAL#0.0, Request3 := FALSE, Level3 := REAL#0.0);
System.Update(Request1 := TRUE, Level1 := REAL#89.0, Request2 := FALSE, Level2 := REAL#0.0, Request3 := FALSE, Level3 := REAL#0.0);
(* At 89: silo 1 should be eligible *)
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := System.Update(Request1 := TRUE, Level1 := REAL#89.0, Request2 := FALSE, Level2 := REAL#0.0, Request3 := FALSE, Level3 := REAL#0.0));
END_TEST_PROGRAM

(* SiloNode in isolation - direct level/request setting. *)
TEST_PROGRAM SiloNodeReportsRequestingAndHighLevel
VAR
    Silo : SiloNode;
END_VAR
Silo.Configure(Id := INT#7);

(* No request: Requesting reports FALSE *)
Silo.UpdateSilo(Request := FALSE, LevelPct := REAL#50.0);
ASSERT_FALSE(IN := Silo.Requesting);
ASSERT_EQUAL(EXPECTED := INT#7, ACTUAL := Silo.SiloId);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Silo.ChildCount);

(* Request rising *)
Silo.UpdateSilo(Request := TRUE, LevelPct := REAL#30.0);
ASSERT_TRUE(IN := Silo.Requesting);
ASSERT_FALSE(IN := Silo.HighLevel);
END_TEST_PROGRAM

(* SiloFarm in isolation: GetChild returns the right node by index;
   ChildCount = 3; the OR-aggregated properties reflect any-child state. *)
TEST_PROGRAM SiloFarmGetChildAndAggregateProperties
VAR
    Farm : SiloFarm;
    Child : ISiloNode;
END_VAR
Farm.Initialize();
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Farm.ChildCount);

(* All idle - aggregate Requesting/HighLevel = FALSE *)
Farm.UpdateFarm(Request1 := FALSE, Level1 := REAL#10.0, Request2 := FALSE, Level2 := REAL#20.0, Request3 := FALSE, Level3 := REAL#30.0);
ASSERT_FALSE(IN := Farm.Requesting);

(* Request silo 2: aggregate Requesting = TRUE *)
Farm.UpdateFarm(Request1 := FALSE, Level1 := REAL#10.0, Request2 := TRUE, Level2 := REAL#20.0, Request3 := FALSE, Level3 := REAL#30.0);
ASSERT_TRUE(IN := Farm.Requesting);

(* GetChild(2) returns the silo with id 2 *)
Child := Farm.GetChild(Index := INT#2);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Child.SiloId);
Child := Farm.GetChild(Index := INT#1);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Child.SiloId);
END_TEST_PROGRAM

(* BlockedCount must reset every cycle, never accumulate across scans. *)
TEST_PROGRAM SiloBlockedCountResetsPerCycle
VAR
    System : SiloLoadingSystem;
END_VAR
System.Initialize();
(* Drive filter up so silo 1 is genuinely high *)
System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := FALSE, Level2 := REAL#0.0, Request3 := FALSE, Level3 := REAL#0.0);
System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := FALSE, Level2 := REAL#0.0, Request3 := FALSE, Level3 := REAL#0.0);
System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := FALSE, Level2 := REAL#0.0, Request3 := FALSE, Level3 := REAL#0.0);
(* Silo 1 is high and requesting -> blocked once this cycle *)
System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := FALSE, Level2 := REAL#0.0, Request3 := FALSE, Level3 := REAL#0.0);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := System.BlockedCount);

(* Next cycle: silo 1 stops requesting -> BlockedCount must drop back to 0 *)
System.Update(Request1 := FALSE, Level1 := REAL#95.0, Request2 := FALSE, Level2 := REAL#0.0, Request3 := FALSE, Level3 := REAL#0.0);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := System.BlockedCount);
END_TEST_PROGRAM

(* First eligible wins: even when silo 2 and silo 3 both request and
   silo 1 is high-blocked, silo 2 is granted (lower index wins). *)
TEST_PROGRAM SiloFirstIndexEligibleAlwaysWins
VAR
    System : SiloLoadingSystem;
END_VAR
System.Initialize();
(* Settle filter for silo 1 high *)
System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := TRUE, Level2 := REAL#10.0, Request3 := TRUE, Level3 := REAL#20.0);
System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := TRUE, Level2 := REAL#10.0, Request3 := TRUE, Level3 := REAL#20.0);
(* Silo 1 high blocked; silo 2 wins; silo 3 requests but path is taken *)
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := TRUE, Level2 := REAL#10.0, Request3 := TRUE, Level3 := REAL#20.0));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := System.BlockedCount);
END_TEST_PROGRAM
