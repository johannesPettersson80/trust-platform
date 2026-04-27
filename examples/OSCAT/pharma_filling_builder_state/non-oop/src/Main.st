FUNCTION_BLOCK ClassicFillingController
VAR
    CurrentStateValue : INT;
    RejectCounter : DwordCounter;
    RecipeValidValue : BOOL;
END_VAR

METHOD PUBLIC ConfigureRecipe : BOOL
VAR_INPUT
    VolumeMl : REAL;
    PumpSpeedPct : REAL;
    LowWeightG : REAL;
    HighWeightG : REAL;
END_VAR
RecipeValidValue := (VolumeMl > REAL#0.0) AND (VolumeMl <= REAL#100.0) AND (PumpSpeedPct > REAL#0.0) AND (PumpSpeedPct <= REAL#100.0) AND (HighWeightG > LowWeightG);
ConfigureRecipe := RecipeValidValue;
END_METHOD

METHOD PUBLIC Initialize
CurrentStateValue := INT#0;
RejectCounter.Initialize();
RejectCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    VialPresent : BOOL;
    FillDone : BOOL;
    WeightG : REAL;
    LowWeightG : REAL;
    HighWeightG : REAL;
END_VAR
CASE CurrentStateValue OF
    INT#0:
        IF StartCommand AND VialPresent AND RecipeValidValue THEN
            CurrentStateValue := INT#10;
        END_IF;
    INT#10:
        CurrentStateValue := INT#20;
    INT#20:
        IF FillDone THEN
            CurrentStateValue := INT#30;
        END_IF;
    INT#30:
        IF (WeightG >= LowWeightG) AND (WeightG <= HighWeightG) THEN
            CurrentStateValue := INT#100;
        ELSE
            CurrentStateValue := INT#40;
            RejectCounter.Update(Up := TRUE, Down := FALSE);
        END_IF;
END_CASE;
Update := CurrentStateValue;
END_METHOD

PUBLIC PROPERTY RejectCount : DWORD
GET
    RejectCount := RejectCounter.Count;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Filler : ClassicFillingController;
    StateId : INT;
END_VAR
Filler.Initialize();
Filler.ConfigureRecipe(VolumeMl := REAL#10.0, PumpSpeedPct := REAL#45.0, LowWeightG := REAL#9.8, HighWeightG := REAL#10.2);
StateId := Filler.Update(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, LowWeightG := REAL#9.8, HighWeightG := REAL#10.2);
END_PROGRAM
