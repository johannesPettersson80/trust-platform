(* Controllers (specification trust-plant-library §8). A PID loop is no block of this library: Create
   PLC code writes it as ordinary code around the Control library's PID (specification 39, §10.5).
   The thermostat-style on/off controller is a block. *)
NAMESPACE Plant

(* A thermostat-style controller (§8.3): Demand is TRUE below the setpoint for heating (above it
   for cooling), with Hysteresis around the setpoint. Wire Demand to a device's Auto input. *)
FUNCTION_BLOCK FB_OnOffController IMPLEMENTS ISetpoint
VAR_INPUT
    (* The measured value: an instrument by composition, else ProcessValue and its quality. *)
    Measurement : IMeasurement;
    ProcessValue : LREAL;
    ProcessValueGood : BOOL := TRUE;
END_VAR
VAR_OUTPUT RETAIN
    Setpoint : LREAL := 50.0;
END_VAR
VAR_OUTPUT NON_RETAIN
    Demand : BOOL;
    MeasuredValue : LREAL;
END_VAR
VAR PUBLIC RETAIN
    Hysteresis : LREAL := 1.0;
    (* TRUE: demand below the setpoint; FALSE (cooling): demand above it. *)
    Heating : BOOL := TRUE;
    SetpointMin : LREAL := 0.0;
    SetpointMax : LREAL := 100.0;
END_VAR
VAR_TEMP
    Half : LREAL;
    Measured : LREAL;
    Good : BOOL;
END_VAR
    Half := ABS(Hysteresis) / 2.0;
    IF Measurement <> NULL THEN
        Measured := Measurement.GetValue();
        Good := Measurement.GetQuality() = E_Quality#Good;
    ELSE
        Measured := ProcessValue;
        Good := ProcessValueGood;
    END_IF;
    IF Good AND F_Finite(Measured) THEN
        MeasuredValue := Measured;
    END_IF;
    IF NOT Good OR NOT F_Finite(Measured) THEN
        Demand := FALSE;
    ELSIF Heating THEN
        IF Measured < Setpoint - Half THEN
            Demand := TRUE;
        ELSIF Measured > Setpoint + Half THEN
            Demand := FALSE;
        END_IF;
    ELSE
        IF Measured > Setpoint + Half THEN
            Demand := TRUE;
        ELSIF Measured < Setpoint - Half THEN
            Demand := FALSE;
        END_IF;
    END_IF;

METHOD PUBLIC SetSetpoint : BOOL
VAR_INPUT
    Value : LREAL;
END_VAR
    SetSetpoint := F_Finite(Value);
    IF SetSetpoint THEN
        Setpoint := LIMIT(MIN(SetpointMin, SetpointMax), Value, MAX(SetpointMin, SetpointMax));
    END_IF;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
