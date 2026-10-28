// Simulation only: the disturbance and the field faults the Simulation page switches. The plant
// model and the couplings in simulation.toml read them. On a real plant, delete this program, its
// page and simulation.toml.
PROGRAM Simulation
VAR
    (* How far the room's window is open, in %: the room loses heat through it. *)
    WindowOpening AT %ML0 : LREAL;
    LowWater AT %QX4.0 : BOOL;
    PumpOverload AT %QX4.1 : BOOL;
    FanFault AT %QX4.2 : BOOL;
    PositionerFault AT %QX4.3 : BOOL;
    TankSensorFault AT %QX4.4 : BOOL;
    RoomSensorFault AT %QX4.5 : BOOL;
END_VAR
    WindowOpening := LIMIT(0.0, WindowOpening, 100.0);
END_PROGRAM
