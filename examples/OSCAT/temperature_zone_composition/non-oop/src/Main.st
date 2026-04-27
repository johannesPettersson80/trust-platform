PROGRAM Main
VAR
    ZoneAFilter : FT_PT1;
    ZoneBFilter : FT_PT1;
    ZoneAController : CTRL_PID;
    ZoneBController : CTRL_PID;
    ZoneAAlarm : HYST;
    ZoneBAlarm : HYST;
    ZoneACommand : REAL;
    ZoneBCommand : REAL;
    AnyAlarm : BOOL;
END_VAR

ZoneAFilter(in := REAL#21.0, T := T#1s, K := REAL#1.0);
ZoneAController(ACT := ZoneAFilter.out, SET_POINT := REAL#23.0, SUP := REAL#0.0, OFS := REAL#0.0, M_I := REAL#45.0, MAN := TRUE, RST := FALSE, KP := REAL#1.0, TN := REAL#1.0, TV := REAL#0.0, LL := REAL#0.0, LH := REAL#100.0);
ZoneAAlarm(In := ZoneAFilter.out, HIGH_LIMIT := REAL#30.0, LOW_LIMIT := REAL#5.0);
ZoneACommand := ZoneAController.Y;

ZoneBFilter(in := REAL#18.0, T := T#1s, K := REAL#1.0);
ZoneBController(ACT := ZoneBFilter.out, SET_POINT := REAL#22.0, SUP := REAL#0.0, OFS := REAL#0.0, M_I := REAL#55.0, MAN := TRUE, RST := FALSE, KP := REAL#1.0, TN := REAL#1.0, TV := REAL#0.0, LL := REAL#0.0, LH := REAL#100.0);
ZoneBAlarm(In := ZoneBFilter.out, HIGH_LIMIT := REAL#30.0, LOW_LIMIT := REAL#5.0);
ZoneBCommand := ZoneBController.Y;

AnyAlarm := ZoneAAlarm.Q OR ZoneBAlarm.Q;
END_PROGRAM
