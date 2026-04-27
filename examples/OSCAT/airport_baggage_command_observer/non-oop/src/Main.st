FUNCTION_BLOCK ClassicBaggageDiverter
VAR
    LastCommandIdValue : INT;
    EventCountValue : INT;
END_VAR
METHOD PUBLIC ProcessBag : INT
VAR_INPUT
    BagId : INT;
    ScannerResult : INT;
END_VAR
IF ScannerResult = INT#1 THEN
    LastCommandIdValue := INT#1;
ELSIF ScannerResult = INT#2 THEN
    LastCommandIdValue := INT#2;
ELSE
    LastCommandIdValue := INT#3;
END_IF;
EventCountValue := EventCountValue + INT#1;
ProcessBag := LastCommandIdValue;
END_METHOD
PUBLIC PROPERTY EventCount : INT
GET
    EventCount := EventCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Diverter : ClassicBaggageDiverter;
    Command : INT;
END_VAR
Command := Diverter.ProcessBag(BagId := INT#42, ScannerResult := INT#2);
END_PROGRAM
