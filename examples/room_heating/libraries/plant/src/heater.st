(* An electric heater (specification trust-plant-library §6.3): run control from FB_RunDevice for
   the main contactor, with the over-temperature switch as its trip, plus a power demand. A heater
   starts at 0 %: the operator's power is 0 % after a restart and after every stop. *)
NAMESPACE Plant

FUNCTION_BLOCK FB_Heater EXTENDS FB_RunDevice IMPLEMENTS IPowerDevice
VAR_INPUT
    (* The program's power in %, followed in Auto while running. *)
    AutoPower : LREAL;
END_VAR
VAR_OUTPUT NON_RETAIN
    (* The power in % for a thyristor controller. *)
    PowerOutput : LREAL;
    (* The same power, time-proportioned over PulsePeriod, for a solid-state relay. *)
    PulseOutput : BOOL;
    (* The operator's power in %. *)
    ManualPower : LREAL;
END_VAR
VAR PUBLIC RETAIN
    MaxPower : LREAL := 100.0;
    PulsePeriod : TIME := T#10s;
END_VAR
VAR NON_RETAIN
    WasCommanded : BOOL;
    Phase : TIME;
END_VAR
    ExecuteRun();
    ExecutePower();

METHOD PROTECTED ExecutePower
VAR
    High : LREAL;
    Wanted : LREAL;
    Period : LREAL;
END_VAR
    High := MAX(MaxPower, 0.0);
    IF RunCommand THEN
        IF Mode = E_Mode#Auto THEN
            Wanted := AutoPower;
            IF NOT F_Finite(Wanted) THEN
                Wanted := PowerOutput;
            END_IF;
            (* Bumpless: Manual continues at the power Auto had. *)
            ManualPower := LIMIT(0.0, Wanted, High);
        ELSE
            Wanted := ManualPower;
        END_IF;
        PowerOutput := LIMIT(0.0, Wanted, High);
    ELSE
        PowerOutput := 0.0;
        IF WasCommanded THEN
            ManualPower := 0.0;
        END_IF;
    END_IF;
    WasCommanded := RunCommand;

    (* Time proportioning: on for PowerOutput % of each period. *)
    Period := F_Seconds(PulsePeriod);
    IF RunCommand AND Period > 0.0 THEN
        Phase := Phase + Clock.Elapsed;
        IF Phase >= PulsePeriod THEN
            Phase := T#0s;
        END_IF;
        PulseOutput := F_Seconds(Phase) < Period * PowerOutput / 100.0;
    ELSE
        Phase := T#0s;
        PulseOutput := FALSE;
    END_IF;
END_METHOD

(* Accepted in Manual, also while stopped, so a power can be chosen before the start. *)
METHOD PUBLIC SetPower : BOOL
VAR_INPUT
    Value : LREAL;
END_VAR
    SetPower := AcceptsCommands() AND F_Finite(Value);
    IF SetPower THEN
        ManualPower := LIMIT(0.0, Value, MAX(MaxPower, 0.0));
    END_IF;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
