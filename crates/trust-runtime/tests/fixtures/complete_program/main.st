PROGRAM MainProg
VAR
    motor : DemoImpl.Motor;
    sensor : DemoTypes.Sensor;
    values : DemoTypes.ValueArray;
    acc : DemoLib.FB_Accumulator;
    sum : DINT := 0;
    total : DINT := 0;
    sensor_size : DINT := 0;
    sensor_type_size : DINT := 0;
    values_size : DINT := 0;
    color : Color := Color#Red;
    swap_a : DINT := 10;
    swap_b : DINT := 20;
    ok : BOOL := FALSE;
END_VAR

ok := motor.Start(100);
motor.Speed := 100;
ok := motor.Stop();

sensor.id := 1;
sensor.label := 'Main';
sensor.active := TRUE;
sensor.value := REAL#12.5;
sensor_size := SIZEOF(sensor);
sensor_type_size := SIZEOF(DemoTypes.Sensor);

values[0] := DINT#1;
values[1] := DINT#2;
values[2] := DINT#3;
values[3] := DINT#4;
values_size := SIZEOF(values);

sum := DemoLib.Add(A := values[0], B := values[1]);
acc(In := sum);
total := acc.Sum;
ok := DemoLib.SwapDint(swap_a, swap_b);

IF ok THEN
    total := total + swap_a + swap_b;
    color := Color#Green;
END_IF;
END_PROGRAM
