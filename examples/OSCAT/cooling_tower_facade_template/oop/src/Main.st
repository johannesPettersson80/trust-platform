(* Record role: TowerResult is a deterministic PLC-side record for tests and runtime readers. *)
TYPE TowerResult :
STRUCT
    FanStage : INT;
    BasinHeater : BOOL;
    BleedValve : BOOL;
    SeasonId : INT;
END_STRUCT
END_TYPE

(* Composition role: BaseTowerSeasonTemplate owns the collaborating objects used by this Facade and Template Method example. *)
FUNCTION_BLOCK BaseTowerSeasonTemplate
METHOD PUBLIC SeasonId : INT
SeasonId := INT#0;
END_METHOD
METHOD PUBLIC FanThreshold : REAL
FanThreshold := REAL#28.0;
END_METHOD
METHOD PUBLIC FreezeThreshold : REAL
FreezeThreshold := REAL#2.0;
END_METHOD
METHOD PUBLIC ConductivityLimit : REAL
ConductivityLimit := REAL#1800.0;
END_METHOD
METHOD PUBLIC RunTemplate : TowerResult
VAR_INPUT
    WaterTempC : REAL;
    OutdoorTempC : REAL;
    ConductivityUs : REAL;
END_VAR
VAR
    Result : TowerResult;
END_VAR
Result.SeasonId := SeasonId();
IF WaterTempC > (FanThreshold() + REAL#5.0) THEN
    Result.FanStage := INT#2;
ELSIF WaterTempC > FanThreshold() THEN
    Result.FanStage := INT#1;
ELSE
    Result.FanStage := INT#0;
END_IF;
Result.BasinHeater := OutdoorTempC < FreezeThreshold();
Result.BleedValve := ConductivityUs > ConductivityLimit();
RunTemplate := Result;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: SummerTowerTemplate customizes inherited behavior in the Facade and Template Method example. *)
FUNCTION_BLOCK SummerTowerTemplate EXTENDS BaseTowerSeasonTemplate
METHOD PUBLIC SeasonId : INT
SeasonId := INT#1;
END_METHOD
METHOD PUBLIC FanThreshold : REAL
FanThreshold := REAL#26.0;
END_METHOD
METHOD PUBLIC FreezeThreshold : REAL
FreezeThreshold := REAL#-10.0;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: WinterTowerTemplate customizes inherited behavior in the Facade and Template Method example. *)
FUNCTION_BLOCK WinterTowerTemplate EXTENDS BaseTowerSeasonTemplate
METHOD PUBLIC SeasonId : INT
SeasonId := INT#2;
END_METHOD
METHOD PUBLIC FanThreshold : REAL
FanThreshold := REAL#32.0;
END_METHOD
METHOD PUBLIC FreezeThreshold : REAL
FreezeThreshold := REAL#4.0;
END_METHOD
END_FUNCTION_BLOCK

(* Composition role: CoolingTowerCell owns the collaborating objects used by this Facade and Template Method example. *)
FUNCTION_BLOCK CoolingTowerCell
VAR
    Summer : SummerTowerTemplate;
    Winter : WinterTowerTemplate;
    LastResult : TowerResult;
    RuntimeMeter : OntimeMeter;
END_VAR
METHOD PUBLIC Initialize
RuntimeMeter.Initialize();
END_METHOD
METHOD PUBLIC Update : TowerResult
VAR_INPUT
    SeasonRequest : INT;
    WaterTempC : REAL;
    OutdoorTempC : REAL;
    ConductivityUs : REAL;
END_VAR
IF SeasonRequest = INT#2 THEN
    LastResult := Winter.RunTemplate(WaterTempC := WaterTempC, OutdoorTempC := OutdoorTempC, ConductivityUs := ConductivityUs);
ELSE
    LastResult := Summer.RunTemplate(WaterTempC := WaterTempC, OutdoorTempC := OutdoorTempC, ConductivityUs := ConductivityUs);
END_IF;
RuntimeMeter.Update(Input := LastResult.FanStage > INT#0);
Update := LastResult;
END_METHOD
PUBLIC PROPERTY FanStage : INT
GET
    FanStage := LastResult.FanStage;
END_GET
END_PROPERTY
PUBLIC PROPERTY BasinHeater : BOOL
GET
    BasinHeater := LastResult.BasinHeater;
END_GET
END_PROPERTY
PUBLIC PROPERTY BleedValve : BOOL
GET
    BleedValve := LastResult.BleedValve;
END_GET
END_PROPERTY
PUBLIC PROPERTY SeasonId : INT
GET
    SeasonId := LastResult.SeasonId;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Tower : CoolingTowerCell;
    SeasonRequest : INT;
    WaterTempRaw : INT;
    OutdoorTempRaw : INT;
    ConductivityRaw : INT;
    FanStageRaw : INT;
    BasinHeaterOut : BOOL;
    BleedValveOut : BOOL;
END_VAR
Tower.Initialize();
Tower.Update(SeasonRequest := SeasonRequest, WaterTempC := INT_TO_REAL(WaterTempRaw), OutdoorTempC := INT_TO_REAL(OutdoorTempRaw), ConductivityUs := INT_TO_REAL(ConductivityRaw));
FanStageRaw := Tower.FanStage;
BasinHeaterOut := Tower.BasinHeater;
BleedValveOut := Tower.BleedValve;
END_PROGRAM
