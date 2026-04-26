PROGRAM RecipeManager
VAR
    SelectedRecipe : INT := 1;
    ApplySelection : BOOL := TRUE;
    ActiveTargetTemperature : REAL := 0.0;
    ActiveMixTime : TIME := T#0s;
    ActiveBatchSize : INT := 0;
END_VAR

IF ApplySelection THEN
    CASE SelectedRecipe OF
        1:
            ActiveTargetTemperature := 65.0;
            ActiveMixTime := T#30s;
            ActiveBatchSize := 10;
        2:
            ActiveTargetTemperature := 72.5;
            ActiveMixTime := T#45s;
            ActiveBatchSize := 14;
        3:
            ActiveTargetTemperature := 80.0;
            ActiveMixTime := T#60s;
            ActiveBatchSize := 18;
        ELSE
            ActiveTargetTemperature := 60.0;
            ActiveMixTime := T#20s;
            ActiveBatchSize := 8;
    END_CASE;
END_IF;
END_PROGRAM
