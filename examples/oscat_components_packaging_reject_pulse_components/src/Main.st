PROGRAM Main
VAR
    RejectPulse : PulseGenerator;
    PulseObject : IPulseGenerator;
    DefectDetected : BOOL := FALSE;
    RejectSolenoid : BOOL;
END_VAR

PulseObject := RejectPulse;
PulseObject.Initialize();
PulseObject.Configure(HighTime := T#150ms, LowTime := T#500ms);
PulseObject.SetEnabled(Enabled := DefectDetected);
RejectSolenoid := PulseObject.Update();
END_PROGRAM
