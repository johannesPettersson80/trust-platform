(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    BeltPulse : PulseGenerator;
    ConveyorEnabled : BOOL := FALSE;
    PulseCommand : BOOL;
END_VAR

BeltPulse.Initialize();
BeltPulse.Configure(HighTime := T#500ms, LowTime := T#500ms);
BeltPulse.SetEnabled(Enabled := ConveyorEnabled);
PulseCommand := BeltPulse.Update();
END_PROGRAM
