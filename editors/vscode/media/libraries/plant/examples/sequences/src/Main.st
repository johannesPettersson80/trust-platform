(* Two tanks, each filled by its own FillSequence; one FB_SequenceManager shows both and can stop
   them together. Valves and mixers are Plant devices commanded in Auto by the sequences. *)
USING Plant;

PROGRAM Main
VAR
    Level1Signal : LREAL;
    Level2Signal : LREAL;
    Inlet1Out : BOOL;
    Outlet1Out : BOOL;
    Inlet2Out : BOOL;
    Outlet2Out : BOOL;
    Mixer1Out : BOOL;
    Mixer2Out : BOOL;

    Level1 : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 3.0);
    Level2 : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 3.0);
    Inlet1 : Plant.FB_OnOffValve := (HasOpenFeedback := FALSE, HasClosedFeedback := FALSE);
    Outlet1 : Plant.FB_OnOffValve := (HasOpenFeedback := FALSE, HasClosedFeedback := FALSE);
    Inlet2 : Plant.FB_OnOffValve := (HasOpenFeedback := FALSE, HasClosedFeedback := FALSE);
    Outlet2 : Plant.FB_OnOffValve := (HasOpenFeedback := FALSE, HasClosedFeedback := FALSE);
    Mixer1 : Plant.FB_Agitator := (HasFeedback := FALSE);
    Mixer2 : Plant.FB_Agitator := (HasFeedback := FALSE);
    Fill1 : FillSequence;
    Fill2 : FillSequence := (TargetLevel := 2.5);
    Overview : Plant.FB_SequenceManager;
END_VAR
    Level1(Raw := Level1Signal);
    Level2(Raw := Level2Signal);
    Fill1(Level := Level1.Value);
    Fill2(Level := Level2.Value);
    Overview(Sequence1 := Fill1, Sequence2 := Fill2);
    Inlet1(AutoOpen := Fill1.OpenInlet);
    Outlet1(AutoOpen := Fill1.OpenOutlet);
    Mixer1(AutoRun := Fill1.RunMixer);
    Inlet2(AutoOpen := Fill2.OpenInlet);
    Outlet2(AutoOpen := Fill2.OpenOutlet);
    Mixer2(AutoRun := Fill2.RunMixer);
    Inlet1Out := Inlet1.Solenoid;
    Outlet1Out := Outlet1.Solenoid;
    Mixer1Out := Mixer1.RunCommand;
    Inlet2Out := Inlet2.Solenoid;
    Outlet2Out := Outlet2.Solenoid;
    Mixer2Out := Mixer2.RunCommand;
END_PROGRAM
