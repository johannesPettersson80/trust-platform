PROGRAM Main
VAR_EXTERNAL
    line1_temp : REAL;
    line1_ready : BOOL;
    line1_setpoint : REAL;
    line1_status : WORD;
    line1_temp_quality : ADS_QUALITY;
END_VAR

IF line1_ready THEN
    line1_setpoint := line1_temp;
ELSE
    line1_setpoint := REAL#0.0;
END_IF;
END_PROGRAM
