PROGRAM PlantProgram
VAR
    Pump : PumpController;
    Cmd : PumpCommand;
    Status : PumpStatus;
    StartCmd : BOOL;
    SpeedRaw : INT;
    SpeedSet : REAL;
    HaltReq : BOOL;
END_VAR

Cmd.Enable := StartCmd AND NOT HaltReq;
SpeedSet := INT_TO_REAL(SpeedRaw);
Cmd.TargetSpeed := SpeedSet;

Pump(Command := Cmd);
Status := Pump.Status;
HaltReq := FALSE;
IF Status.State = PumpState#Fault THEN
    HaltReq := TRUE;
END_IF
END_PROGRAM
