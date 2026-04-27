(* Pattern role: ISiloNode is the contract boundary for the Composite and Mediator example. *)
INTERFACE ISiloNode
    PROPERTY SiloId : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY ChildCount : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY Requesting : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY HighLevel : BOOL
    GET
    END_GET
    END_PROPERTY

    METHOD GetChild : ISiloNode
    VAR_INPUT
        Index : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: SiloNode is a concrete implementation in the Composite and Mediator example. *)
FUNCTION_BLOCK SiloNode IMPLEMENTS ISiloNode
VAR
    SiloIdValue : INT;
    RequestingValue : BOOL;
    HighLevelValue : BOOL;
    LevelFilter : Pt1Filter;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    Id : INT;
END_VAR
SiloIdValue := Id;
LevelFilter.Initialize();
LevelFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
END_METHOD

METHOD PUBLIC UpdateSilo
VAR_INPUT
    Request : BOOL;
    LevelPct : REAL;
END_VAR
RequestingValue := Request;
HighLevelValue := LevelFilter.Update(Sample := LevelPct) >= REAL#90.0;
END_METHOD

METHOD PUBLIC GetChild : ISiloNode
VAR_INPUT
    Index : INT;
END_VAR
GetChild := THIS;
END_METHOD

PUBLIC PROPERTY SiloId : INT
GET
    SiloId := SiloIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#0;
END_GET
END_PROPERTY

PUBLIC PROPERTY Requesting : BOOL
GET
    Requesting := RequestingValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HighLevel : BOOL
GET
    HighLevel := HighLevelValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: SiloFarm is a concrete implementation in the Composite and Mediator example. *)
FUNCTION_BLOCK SiloFarm IMPLEMENTS ISiloNode
VAR
    Silo1 : SiloNode;
    Silo2 : SiloNode;
    Silo3 : SiloNode;
END_VAR

METHOD PUBLIC Initialize
Silo1.Configure(Id := INT#1);
Silo2.Configure(Id := INT#2);
Silo3.Configure(Id := INT#3);
END_METHOD

METHOD PUBLIC UpdateFarm
VAR_INPUT
    Request1 : BOOL;
    Level1 : REAL;
    Request2 : BOOL;
    Level2 : REAL;
    Request3 : BOOL;
    Level3 : REAL;
END_VAR
Silo1.UpdateSilo(Request := Request1, LevelPct := Level1);
Silo2.UpdateSilo(Request := Request2, LevelPct := Level2);
Silo3.UpdateSilo(Request := Request3, LevelPct := Level3);
END_METHOD

METHOD PUBLIC GetChild : ISiloNode
VAR_INPUT
    Index : INT;
END_VAR
IF Index = INT#1 THEN
    GetChild := Silo1;
ELSIF Index = INT#2 THEN
    GetChild := Silo2;
ELSE
    GetChild := Silo3;
END_IF;
END_METHOD

PUBLIC PROPERTY SiloId : INT
GET
    SiloId := INT#100;
END_GET
END_PROPERTY

PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#3;
END_GET
END_PROPERTY

PUBLIC PROPERTY Requesting : BOOL
GET
    Requesting := Silo1.Requesting OR Silo2.Requesting OR Silo3.Requesting;
END_GET
END_PROPERTY

PUBLIC PROPERTY HighLevel : BOOL
GET
    HighLevel := Silo1.HighLevel OR Silo2.HighLevel OR Silo3.HighLevel;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: TransferPathMediator owns the collaborating objects used by this Composite and Mediator example. *)
FUNCTION_BLOCK TransferPathMediator
VAR
    ActiveSilo : ISiloNode;
    ActiveSiloIdValue : INT;
    BlowerRunningValue : BOOL;
    BlockedCountValue : INT;
END_VAR

METHOD PUBLIC Grant : INT
VAR_INPUT
    Farm : SiloFarm;
END_VAR
VAR
    Candidate : ISiloNode;
    Index : INT;
END_VAR
ActiveSiloIdValue := INT#0;
BlowerRunningValue := FALSE;
BlockedCountValue := INT#0;
FOR Index := INT#1 TO Farm.ChildCount DO
    Candidate := Farm.GetChild(Index := Index);
    IF Candidate.Requesting AND Candidate.HighLevel THEN
        BlockedCountValue := BlockedCountValue + INT#1;
    ELSIF Candidate.Requesting AND (ActiveSiloIdValue = INT#0) THEN
        ActiveSilo := Candidate;
        ActiveSiloIdValue := Candidate.SiloId;
        BlowerRunningValue := TRUE;
    END_IF;
END_FOR;
Grant := ActiveSiloIdValue;
END_METHOD

PUBLIC PROPERTY ActiveSiloId : INT
GET
    ActiveSiloId := ActiveSiloIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY BlowerRunning : BOOL
GET
    BlowerRunning := BlowerRunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY BlockedCount : INT
GET
    BlockedCount := BlockedCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: SiloLoadingSystem owns the collaborating objects used by this Composite and Mediator example. *)
FUNCTION_BLOCK SiloLoadingSystem
VAR
    Farm : SiloFarm;
    Mediator : TransferPathMediator;
END_VAR

METHOD PUBLIC Initialize
Farm.Initialize();
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    Request1 : BOOL;
    Level1 : REAL;
    Request2 : BOOL;
    Level2 : REAL;
    Request3 : BOOL;
    Level3 : REAL;
END_VAR
Farm.UpdateFarm(Request1 := Request1, Level1 := Level1, Request2 := Request2, Level2 := Level2, Request3 := Request3, Level3 := Level3);
Update := Mediator.Grant(Farm := Farm);
END_METHOD

PUBLIC PROPERTY ActiveSiloId : INT
GET
    ActiveSiloId := Mediator.ActiveSiloId;
END_GET
END_PROPERTY

PUBLIC PROPERTY BlowerRunning : BOOL
GET
    BlowerRunning := Mediator.BlowerRunning;
END_GET
END_PROPERTY

PUBLIC PROPERTY BlockedCount : INT
GET
    BlockedCount := Mediator.BlockedCount;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    System : SiloLoadingSystem;
    Silo1Request : BOOL;
    Silo2Request : BOOL;
    Silo3Request : BOOL;
    Silo1LevelRaw : INT;
    Silo2LevelRaw : INT;
    Silo3LevelRaw : INT;
    BlowerOut : BOOL;
    Diverter1Out : BOOL;
    Diverter2Out : BOOL;
    Diverter3Out : BOOL;
END_VAR

System.Initialize();
System.Update(Request1 := Silo1Request, Level1 := INT_TO_REAL(Silo1LevelRaw), Request2 := Silo2Request, Level2 := INT_TO_REAL(Silo2LevelRaw), Request3 := Silo3Request, Level3 := INT_TO_REAL(Silo3LevelRaw));
BlowerOut := System.BlowerRunning;
Diverter1Out := System.ActiveSiloId = INT#1;
Diverter2Out := System.ActiveSiloId = INT#2;
Diverter3Out := System.ActiveSiloId = INT#3;
END_PROGRAM
