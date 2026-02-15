(*
    Minimal ST program for StateChart hardware execution
    
    This program defines the I/O variables that will be controlled
    via the StateChart editor through the control endpoint.
*)

PROGRAM Main
VAR
    // Digital outputs mapped to EtherCAT EL2008
    DO0 : BOOL;
    DO1 : BOOL;
    DO2 : BOOL;
    DO3 : BOOL;
    DO4 : BOOL;
    DO5 : BOOL;
    DO6 : BOOL;
    DO7 : BOOL;
    
    // Heartbeat to show program is running
    heartbeat : BOOL;
END_VAR

// Toggle heartbeat every cycle
heartbeat := NOT heartbeat;

// Outputs are controlled via StateChart action mappings
// through the control endpoint, not by this program

END_PROGRAM
