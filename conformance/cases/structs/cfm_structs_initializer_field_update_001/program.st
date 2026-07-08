TYPE StepCfg :
STRUCT
    cyl : INT := INT#1;
    ext : BOOL := FALSE;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg := (cyl := INT#4, ext := TRUE);
    observed : INT := INT#0;
END_VAR

observed := cfg.cyl;
cfg.cyl := cfg.cyl + INT#1;
END_PROGRAM
