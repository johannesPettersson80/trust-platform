FUNCTION_BLOCK ClassicPalletizer
VAR
    CurrentStateValue : INT;
    CompletedCounter : DwordCounter;
    LastCommandIdValue : INT;
END_VAR

METHOD PUBLIC Initialize
CurrentStateValue := INT#0;
CompletedCounter.Initialize();
CompletedCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    BoxPresent : BOOL;
    PalletReady : BOOL;
    RobotHealthy : BOOL;
    PickVacuumOk : BOOL;
END_VAR
CASE CurrentStateValue OF
    INT#0:
        IF BoxPresent AND PalletReady THEN
            CurrentStateValue := INT#10;
        END_IF;
    INT#10:
        LastCommandIdValue := INT#1;
        IF RobotHealthy AND PickVacuumOk THEN
            CurrentStateValue := INT#20;
        ELSE
            CurrentStateValue := INT#40;
        END_IF;
    INT#20:
        LastCommandIdValue := INT#2;
        IF RobotHealthy THEN
            CurrentStateValue := INT#30;
        ELSE
            CurrentStateValue := INT#90;
        END_IF;
    INT#30:
        LastCommandIdValue := INT#3;
        CurrentStateValue := INT#100;
        CompletedCounter.Update(Up := TRUE, Down := FALSE);
END_CASE;
Update := CurrentStateValue;
END_METHOD

PUBLIC PROPERTY CompletedPallets : DWORD
GET
    CompletedPallets := CompletedCounter.Count;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCommandId : INT
GET
    LastCommandId := LastCommandIdValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Cell : ClassicPalletizer;
    StateId : INT;
END_VAR
Cell.Initialize();
StateId := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
END_PROGRAM
