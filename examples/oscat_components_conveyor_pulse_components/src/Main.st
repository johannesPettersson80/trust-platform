PROGRAM Main
VAR
    BeltPulse : PulseGenerator;
    PulseObject : IPulseGenerator;
    ConveyorEnabled : BOOL := FALSE;
    PulseCommand : BOOL;
END_VAR

PulseObject := BeltPulse;
PulseObject.Initialize();
PulseObject.Configure(HighTime := T#500ms, LowTime := T#500ms);
PulseObject.SetEnabled(Enabled := ConveyorEnabled);
PulseCommand := PulseObject.Update();
END_PROGRAM
