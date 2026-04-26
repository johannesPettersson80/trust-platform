PROGRAM PidLoop
VAR
    Setpoint : REAL := 50.0;
    ProcessValue : REAL := 40.0;
    Kp : REAL := 1.2;
    Ki : REAL := 0.1;
    Kd : REAL := 0.05;
    SampleTime : REAL := 0.1;
    IntegralTerm : REAL := 0.0;
    PreviousError : REAL := 0.0;
    ErrorValue : REAL := 0.0;
    Derivative : REAL := 0.0;
    ControlOutput : REAL := 0.0;
END_VAR

ErrorValue := Setpoint - ProcessValue;
IntegralTerm := IntegralTerm + (ErrorValue * Ki * SampleTime);
Derivative := (ErrorValue - PreviousError) / SampleTime;
ControlOutput := (Kp * ErrorValue) + IntegralTerm + (Kd * Derivative);
PreviousError := ErrorValue;
ProcessValue := ProcessValue + (ControlOutput * 0.02);
END_PROGRAM
