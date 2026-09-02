(* BuildingMpcForecast contains the reviewed outdoor-temperature, occupancy,
   and tariff values for the three future one-minute prediction intervals. *)
TYPE BuildingMpcForecast :
STRUCT
    OutsideTemperature : ARRAY[0..2] OF LREAL;
    Zone1OccupancyGain : ARRAY[0..2] OF LREAL;
    Zone2OccupancyGain : ARRAY[0..2] OF LREAL;
    Tariff : ARRAY[0..2] OF LREAL;
END_STRUCT
END_TYPE

(* BuildingMpcRequest is one atomic controller request: measured room state,
   setpoints, previous applied heat, qualified model correction, constraints,
   objective weights, and the forecast used by the optimizer. *)
TYPE BuildingMpcRequest :
STRUCT
    Enable : BOOL;
    UpstreamValid : BOOL;
    Zone1Temperature : LREAL;
    Zone2Temperature : LREAL;
    Zone1Setpoint : LREAL;
    Zone2Setpoint : LREAL;
    ComfortMinimum : LREAL;
    ComfortMaximum : LREAL;
    TemperatureWeight : LREAL;
    EnergyWeight : LREAL;
    MoveWeight : LREAL;
    PreviousZone1Heat : LREAL;
    PreviousZone2Heat : LREAL;
    OutsideCurrent : LREAL;
    Occupancy1Current : LREAL;
    Occupancy2Current : LREAL;
    EstimatedBias1 : LREAL;
    EstimatedBias2 : LREAL;
    Forecast : BuildingMpcForecast;
END_STRUCT
END_TYPE

(* BuildingMpcResult separates solver completion and feasibility from the first
   requested heater move. Main decides whether and how that request may reach
   the physical actuators. *)
TYPE BuildingMpcResult :
STRUCT
    ConfigurationValid : BOOL;
    SolverComplete : BOOL;
    SolverFeasible : BOOL;
    Infeasible : BOOL;
    CandidateCount : DINT;
    FeasibleCandidateCount : DINT;
    BestObjective : LREAL;
    FirstZone1Heat : LREAL;
    FirstZone2Heat : LREAL;
END_STRUCT
END_TYPE
