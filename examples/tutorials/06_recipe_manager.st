PROGRAM RecipeManager
VAR
    selected_recipe : INT := 1;
    apply_selection : BOOL := TRUE;
    active_target_temp : REAL := 0.0;
    active_mix_time : TIME := T#0s;
    active_batch_size : INT := 0;
END_VAR

IF apply_selection THEN
    CASE selected_recipe OF
        1:
            active_target_temp := 65.0;
            active_mix_time := T#30s;
            active_batch_size := 10;
        2:
            active_target_temp := 72.5;
            active_mix_time := T#45s;
            active_batch_size := 14;
        3:
            active_target_temp := 80.0;
            active_mix_time := T#60s;
            active_batch_size := 18;
        ELSE
            active_target_temp := 60.0;
            active_mix_time := T#20s;
            active_batch_size := 8;
    END_CASE;
END_IF;
END_PROGRAM
