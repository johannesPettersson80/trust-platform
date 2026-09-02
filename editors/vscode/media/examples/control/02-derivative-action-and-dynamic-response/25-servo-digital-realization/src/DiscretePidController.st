(* This lesson-specific discrete PID controller makes the sampled position-control algorithm visible. It retains the previous samples needed by the chosen discretization and is valid only for the positive sample interval and actuator limits supplied by the lesson. *)
FUNCTION_BLOCK LessonDiscretePID
VAR
    Initialized : BOOL := FALSE;
    PreviousError : LREAL := 0.0;
    PreviousPosition : LREAL := 0.0;
    PreviousPositionRate : LREAL := 0.0;
    IntegralTorque : LREAL := 0.0;
    DerivativeTorque : LREAL := 0.0;
    PreviousCalculatedTorque : LREAL := 0.0;
END_VAR

METHOD PUBLIC Update : LREAL
VAR_INPUT
    Setpoint : LREAL;
    ProcessValue : LREAL;
    CycleTime : LREAL;
    MethodSelection : INT;
    DelayEnabled : BOOL;
END_VAR
VAR
    Error : LREAL;
    PositionRate : LREAL;
    CalculatedTorque : LREAL;
END_VAR

IF NOT Initialized THEN
    PreviousError := Setpoint - ProcessValue;
    PreviousPosition := ProcessValue;
    PreviousPositionRate := LREAL#0.0;
    Initialized := TRUE;
END_IF;

Error := Setpoint - ProcessValue;
PositionRate := (ProcessValue - PreviousPosition) / CycleTime;

CASE MethodSelection OF
    0:
        (* Forward Euler uses values retained from the preceding update. *)
        IntegralTorque := IntegralTorque
            + LREAL#2.0 * CycleTime * PreviousError;
        DerivativeTorque := DerivativeTorque
            + CycleTime / LREAL#0.05
                * (LREAL#3.0 * PreviousPositionRate - DerivativeTorque);
    1:
        (* Backward Euler uses the current error and current position change. *)
        IntegralTorque := IntegralTorque + LREAL#2.0 * CycleTime * Error;
        DerivativeTorque := (
            DerivativeTorque
            + LREAL#3.0 / LREAL#0.05 * (ProcessValue - PreviousPosition)
        ) / (LREAL#1.0 + CycleTime / LREAL#0.05);
ELSE
    (* Tustin averages the integration interval and maps the filtered
       derivative with the trapezoidal substitution. *)
    IntegralTorque := IntegralTorque
        + LREAL#0.5 * LREAL#2.0 * CycleTime * (Error + PreviousError);
    DerivativeTorque := (
        (LREAL#2.0 * LREAL#0.05 - CycleTime) * DerivativeTorque
        + LREAL#2.0 * LREAL#3.0 * (ProcessValue - PreviousPosition)
    ) / (LREAL#2.0 * LREAL#0.05 + CycleTime);
END_CASE;

CalculatedTorque := LREAL#8.0 * Error + IntegralTorque - DerivativeTorque;
CalculatedTorque := MAX(MIN(CalculatedTorque, LREAL#10.0), LREAL#-10.0);
IF DelayEnabled THEN
    (* A computation delay is real retained state: this update calculates the
       torque that the following controller update will apply. *)
    Update := PreviousCalculatedTorque;
ELSE
    Update := CalculatedTorque;
END_IF;

PreviousCalculatedTorque := CalculatedTorque;
PreviousError := Error;
PreviousPosition := ProcessValue;
PreviousPositionRate := PositionRate;
END_METHOD
END_FUNCTION_BLOCK
