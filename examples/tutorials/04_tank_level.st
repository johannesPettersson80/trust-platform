PROGRAM TankLevel
VAR
    inlet_valve_cmd : BOOL := FALSE;
    outlet_valve_cmd : BOOL := TRUE;
    level_percent : REAL := 45.0;
    low_setpoint : REAL := 30.0;
    high_setpoint : REAL := 70.0;
END_VAR

IF level_percent <= low_setpoint THEN
    inlet_valve_cmd := TRUE;
    outlet_valve_cmd := FALSE;
ELSIF level_percent >= high_setpoint THEN
    inlet_valve_cmd := FALSE;
    outlet_valve_cmd := TRUE;
END_IF;

IF inlet_valve_cmd THEN
    level_percent := level_percent + 2.5;
END_IF;

IF outlet_valve_cmd THEN
    level_percent := level_percent - 1.5;
END_IF;
END_PROGRAM
