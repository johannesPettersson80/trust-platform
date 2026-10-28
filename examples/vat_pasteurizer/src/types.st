(* The vat pasteurizer's types: its batch states, its recipe, its alarms, the signals of its plant,
   the operator's commands and what it shows the operator. *)

TYPE VatState :
(
    Idle := 0,
    Filling := 1,
    Heating := 2,
    Holding := 3,
    Cooling := 4,
    Draining := 5,
    Done := 6,
    Stopped := 7,
    Aborted := 8
);
END_TYPE

(* One product's heat treatment: the vat is filled to FillLevel, heated to PasteurTemp, held there
   for HoldTime and cooled to CoolTemp. *)
TYPE Recipe :
STRUCT
    Name : STRING[16];
    FillLevel : REAL;
    PasteurTemp : REAL;
    HoldTime : TIME;
    CoolTemp : REAL;
END_STRUCT
END_TYPE

(* One alarm: its condition now, raised and not yet reset, acknowledged by the operator. *)
TYPE AlarmEntry :
STRUCT
    Active : BOOL;
    Latched : BOOL;
    Acknowledged : BOOL;
END_STRUCT
END_TYPE

(* The plant's signals as the I/O wiring gives them: transmitters in raw counts, where 0 is 4 mA and
   27648 is 20 mA, switches and contactor feedbacks, and the outputs to valves, motors, the heater
   and the horn. A real installation binds these to its I/O; the bench's simulation drives them. *)
TYPE FieldSignals :
STRUCT
    LevelRaw : INT;
    TempRaw : INT;
    LevelHighHigh : BOOL;
    EmergencyStopOk : BOOL;
    InletOpen : BOOL;
    OutletOpen : BOOL;
    CoolingOpen : BOOL;
    AgitatorRunning : BOOL;
    PumpRunning : BOOL;
    InletValve : BOOL;
    OutletValve : BOOL;
    CoolingValve : BOOL;
    Agitator : BOOL;
    Pump : BOOL;
    HeaterPower : INT;
    Horn : BOOL;
END_STRUCT
END_TYPE

(* The operator's commands from the panel: each acts on its rising edge. *)
TYPE OperatorCommands :
STRUCT
    Start : BOOL;
    Stop : BOOL;
    Abort : BOOL;
    Reset : BOOL;
    Acknowledge : BOOL;
    RecipeIndex : INT := 1;
END_STRUCT
END_TYPE

(* What the panel shows: the batch's state and values, and the texts the panel's slow task builds. *)
TYPE VatStatus :
STRUCT
    State : VatState;
    Recipe : STRING[16];
    Level : REAL;
    Temperature : REAL;
    Setpoint : REAL;
    HeaterPower : REAL;
    HoldRemaining : TIME;
    StepTime : TIME;
    Batches : UDINT;
    ActiveAlarms : INT;
    Unacknowledged : INT;
    FirstOut : INT;
    StateText : STRING[48];
    AlarmText : STRING[40];
    BatchReport : STRING[64];
END_STRUCT
END_TYPE
