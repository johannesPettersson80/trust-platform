(* The water heater of the example rust_service. The service `price` writes price_setpoint from the
   electricity price; the program takes it only while the service's heartbeat is fresh, and its own
   safe setpoint otherwise. The scan never waits for the service. *)
PROGRAM HeaterControl
VAR_EXTERNAL
    price_heartbeat : UDINT;        (* the service's heartbeat, which the runtime counts up *)
END_VAR
VAR
    price_setpoint AT %MD0 : REAL;  (* the setpoint the service writes *)
    target AT %MD4 : REAL;          (* the setpoint in use *)
    service_ok AT %MX8.0 : BOOL;    (* TRUE while the service's heartbeat is fresh *)
    temperature AT %ID0 : REAL;     (* the tank's temperature, a simulated input *)
    heater AT %QX0.0 : BOOL;        (* the heating element *)
    fresh : SERVICE_FRESH;
END_VAR
VAR CONSTANT
    SAFE_SETPOINT : REAL := 55.0;   (* the setpoint while the service is not fresh *)
    HYSTERESIS : REAL := 2.0;
END_VAR
fresh(HEARTBEAT := price_heartbeat, TIMEOUT := T#2s, FRESH => service_ok);
IF service_ok THEN
    target := price_setpoint;
ELSE
    target := SAFE_SETPOINT;
END_IF;
IF temperature < target - HYSTERESIS THEN
    heater := TRUE;
ELSIF temperature >= target THEN
    heater := FALSE;
END_IF;
END_PROGRAM
