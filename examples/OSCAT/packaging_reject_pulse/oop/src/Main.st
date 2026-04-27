(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    RejectPulse : PulseGenerator;
    DefectDetected : BOOL := FALSE;
    RejectSolenoid : BOOL;
END_VAR

RejectPulse.Initialize();
RejectPulse.Configure(HighTime := T#150ms, LowTime := T#500ms);
RejectPulse.SetEnabled(Enabled := DefectDetected);
RejectSolenoid := RejectPulse.Update();
END_PROGRAM
