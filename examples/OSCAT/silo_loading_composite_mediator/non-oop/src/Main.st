FUNCTION_BLOCK ClassicSiloLoadingSystem
VAR
    ActiveSiloIdValue : INT;
    BlowerRunningValue : BOOL;
    BlockedCountValue : INT;
END_VAR

METHOD PUBLIC Update : INT
VAR_INPUT
    Request1 : BOOL;
    Level1 : REAL;
    Request2 : BOOL;
    Level2 : REAL;
    Request3 : BOOL;
    Level3 : REAL;
END_VAR
ActiveSiloIdValue := INT#0;
BlockedCountValue := INT#0;
IF Request1 AND (Level1 >= REAL#90.0) THEN
    BlockedCountValue := BlockedCountValue + INT#1;
ELSIF Request1 THEN
    ActiveSiloIdValue := INT#1;
END_IF;
IF Request2 AND (Level2 >= REAL#90.0) THEN
    BlockedCountValue := BlockedCountValue + INT#1;
ELSIF Request2 AND (ActiveSiloIdValue = INT#0) THEN
    ActiveSiloIdValue := INT#2;
END_IF;
IF Request3 AND (Level3 >= REAL#90.0) THEN
    BlockedCountValue := BlockedCountValue + INT#1;
ELSIF Request3 AND (ActiveSiloIdValue = INT#0) THEN
    ActiveSiloIdValue := INT#3;
END_IF;
BlowerRunningValue := ActiveSiloIdValue <> INT#0;
Update := ActiveSiloIdValue;
END_METHOD

PUBLIC PROPERTY BlockedCount : INT
GET
    BlockedCount := BlockedCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    System : ClassicSiloLoadingSystem;
    Active : INT;
END_VAR
Active := System.Update(Request1 := TRUE, Level1 := REAL#40.0, Request2 := TRUE, Level2 := REAL#30.0, Request3 := FALSE, Level3 := REAL#20.0);
END_PROGRAM
