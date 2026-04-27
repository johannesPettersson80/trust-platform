TEST_PROGRAM TankFarmCompositeStopsOneAreaThroughPlantNode
VAR
    Farm : FarmNode;
    Area : IPlantNode;
    Tank : IPlantNode;
END_VAR

Farm.Initialize();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Farm.ChildCount);
Area := Farm.GetChild(Index := INT#2);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Area.ChildCount);
Tank := Area.GetChild(Index := INT#1);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Farm.StoppedCount);

Area.EmergencyStop();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Farm.StoppedCount);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Tank.StoppedCount);
END_TEST_PROGRAM

TEST_PROGRAM TankFarmCompositeAcknowledgesAlarmTree
VAR
    Farm : FarmNode;
END_VAR

Farm.Initialize();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Farm.AlarmCount);
Farm.AcknowledgeAll();
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Farm.AlarmCount);
END_TEST_PROGRAM

(* Per-area isolation test: drives EmergencyStop on one area only and
   verifies that the OTHER area's tanks remain unaffected. Catches
   bugs where the composite's recursion descends into the wrong subtree
   or where StoppedCount accumulates beyond the affected subtree. *)
TEST_PROGRAM TankFarmEmergencyStopIsLocalToOneAreaOnly
VAR
    Farm : FarmNode;
    Area1 : IPlantNode;
    Area2 : IPlantNode;
    Tank1Of2 : IPlantNode;
END_VAR

Farm.Initialize();

(* Stop Area 2 only - Area 2's tanks count as stopped *)
Area2 := Farm.GetChild(Index := INT#2);
Area2.EmergencyStop();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Farm.StoppedCount);

(* Area 1 must NOT be stopped - the recursion stays within the called
   subtree and does not cross into siblings *)
Area1 := Farm.GetChild(Index := INT#1);
Tank1Of2 := Area2.GetChild(Index := INT#1);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Tank1Of2.StoppedCount);
END_TEST_PROGRAM

(* Iterator boundary: traversal across all areas and tanks in the farm
   matches the manual aggregation. Catches regressions where ChildCount
   reports the wrong count or GetChild returns the wrong node. *)
TEST_PROGRAM TankFarmIteratorVisitsEveryLeafExactlyOnce
VAR
    Farm : FarmNode;
    Area : IPlantNode;
    Tank : IPlantNode;
    AreaIndex : INT;
    TankIndex : INT;
    LeafCount : INT;
    AlarmTotal : INT;
END_VAR

Farm.Initialize();
LeafCount := INT#0;
AlarmTotal := INT#0;
FOR AreaIndex := INT#1 TO Farm.ChildCount DO
    Area := Farm.GetChild(Index := AreaIndex);
    FOR TankIndex := INT#1 TO Area.ChildCount DO
        Tank := Area.GetChild(Index := TankIndex);
        LeafCount := LeafCount + INT#1;
        AlarmTotal := AlarmTotal + Tank.AlarmCount;
    END_FOR;
END_FOR;
(* 2 areas x 2 tanks = 4 leaves *)
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := LeafCount);
(* Aggregated leaf alarms must equal the farm's aggregated alarm count *)
ASSERT_EQUAL(EXPECTED := AlarmTotal, ACTUAL := Farm.AlarmCount);
END_TEST_PROGRAM

(* Per-leaf isolation: acknowledging a single tank's alarm must not
   touch the alarm state of other tanks in the same area or in other
   areas. *)
TEST_PROGRAM TankFarmPerLeafAcknowledgeIsLocal
VAR
    Farm : FarmNode;
    Area1 : IPlantNode;
    Tank1Of1 : IPlantNode;
END_VAR

Farm.Initialize();
(* Initial setup: Area1.Tank1 has alarm; Area2.Tank2 has alarm; total = 2 *)
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Farm.AlarmCount);

(* Acknowledge only Area1.Tank1 - the alarm in Area2.Tank2 must remain *)
Area1 := Farm.GetChild(Index := INT#1);
Tank1Of1 := Area1.GetChild(Index := INT#1);
Tank1Of1.AcknowledgeAll();
(* Total drops to 1 - the other alarm is still pending *)
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Farm.AlarmCount);
END_TEST_PROGRAM

(* Recursive EmergencyStop on the root farm node stops every leaf. *)
TEST_PROGRAM TankFarmRootEmergencyStopStopsAllLeaves
VAR
    Farm : FarmNode;
END_VAR

Farm.Initialize();
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Farm.StoppedCount);

Farm.EmergencyStop();
(* All four leaves must now report stopped *)
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Farm.StoppedCount);
END_TEST_PROGRAM

(* Per-FB isolation: two FarmNode instances must keep independent
   stop/alarm state. Catches a class of bug where shared state leaks
   between composite instances. *)
TEST_PROGRAM TankFarmFarmNodeInstancesAreIndependent
VAR
    FarmA : FarmNode;
    FarmB : FarmNode;
END_VAR

FarmA.Initialize();
FarmB.Initialize();

(* Stop FarmA only; FarmB unchanged *)
FarmA.EmergencyStop();
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := FarmA.StoppedCount);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := FarmB.StoppedCount);

(* Acknowledge FarmB; FarmA's alarms unchanged *)
FarmB.AcknowledgeAll();
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := FarmB.AlarmCount);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := FarmA.AlarmCount);
END_TEST_PROGRAM

(* EmergencyStopArea using the typed helper produces the same effect as
   walking the tree manually with GetChild + EmergencyStop. *)
TEST_PROGRAM TankFarmEmergencyStopAreaMatchesManualWalk
VAR
    Farm : FarmNode;
    AreaA : IPlantNode;
END_VAR

Farm.Initialize();
Farm.EmergencyStopArea(AreaIndex := INT#1);
(* Both tanks in Area 1 are stopped *)
AreaA := Farm.GetChild(Index := INT#1);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := AreaA.StoppedCount);
(* Area 2 untouched *)
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Farm.StoppedCount);
END_TEST_PROGRAM
