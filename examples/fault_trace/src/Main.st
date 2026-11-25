// A dosing pump's controller. The water needs 1,200 units of chlorine a minute, and the pump gives
// them over its strokes, so each stroke must give 1,200 divided by the strokes a minute that the
// stroke sensor counts. The division has no guard: when the sensor's wire breaks and it reads 0,
// the program divides by zero and faults.
VAR_GLOBAL
    // The pump's strokes a minute, from its stroke sensor.
    StrokeRate AT %IW0 : INT;
    // The dose each stroke gives, to the pump's dosing setpoint.
    DosePerStroke AT %QW0 : INT;
END_VAR

PROGRAM Main
VAR
    // The chlorine the water needs each minute.
    DosePerMinute : INT := 1200;
END_VAR
DosePerStroke := DosePerMinute / StrokeRate;
END_PROGRAM
