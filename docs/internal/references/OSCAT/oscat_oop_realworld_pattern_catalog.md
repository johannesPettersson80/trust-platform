# OSCAT OOP Real-World Pattern Catalog

Status: implementation contract, not marketing copy.

This file is the acceptance checklist for the 27 industrial OSCAT OOP
pattern examples plus the two compact pattern showcases. The full example suite
also contains 20 compact component-composition showcases; those are intentionally
smaller but their READMEs still document process, boundary, scan sequence,
records, reuse, and classic-ST tradeoffs.

Every industrial catalog item must be implemented as two runnable projects:

- `examples/OSCAT/<slug>/non-oop`
- `examples/OSCAT/<slug>/oop`

The classic project shows normal Structured Text using direct logic and classic
OSCAT/component calls. The components project shows the named OOP pattern in the
actual ST structure. The shared README lives at
`examples/OSCAT/<slug>/README.md` and explains the process, pattern, reuse path,
and tradeoff for the pair. A README may only claim a pattern, communication
driver, or logging/historian behavior when the project contains
code/configuration that backs the claim.

## Non-Negotiable Acceptance Rules

- No generated real-world pattern catalog. These examples are hand-written.
- No generic state-machine scaffold may be reused as a pattern substitute.
- The OOP pattern must be visible in `src/Main.st`, not only in the README.
- The classic and OOP versions must solve the same process problem.
- Each README must explain:
  - the physical process;
  - the real customer/integrator problem;
  - the I/O and communication boundary;
  - the state machine or scan sequence;
  - alarm classes and logging/historian events;
  - the OOP pattern used;
  - how to use the pattern in another project;
  - why OOP helps;
  - when the pattern is overkill and classic ST is better;
  - exact validation commands.
- Communication claims must be backed by files:
  - Modbus/MQTT require `io.toml` and `AT %I/%Q/%M` boundary variables.
  - OPC UA requires `[runtime.opcua]` in `runtime.toml` and exposed variables.
  - EtherCAT requires `io.toml` EtherCAT configuration.
  - CSV/database logging is represented as deterministic PLC-side record-ready
    structs/flags; the README must state that persistence is runtime/HMI side.
- Tests must cover process behavior and pattern behavior. Passing tests must
  not merely prove "start -> state 10, fault -> state 90".
- Structural tests in Rust must verify that each OOP example contains the
  pattern constructs it claims, for example interface variables, concrete
  implementations, subscribers, adapters, explicit state objects, or command
  objects.

## Catalog

### 1. Multi-Product Batch Reactor

- Pattern: Factory + Template Method.
- Process: 2000 L reactor for acid neutralization, polymer batch, and base wash.
- Required OOP structure:
  - `IBatchSequencer` interface.
  - `AcidSequencer`, `PolymerSequencer`, and `BaseWashSequencer`.
  - `SequencerFactory.Build(RecipeId)` returning an `IBatchSequencer`.
  - Common reactor state machine calls `ActiveSequencer.ExecuteStep(...)`.
  - Common skeleton: prepare, charge, react, hold, sample, discharge, clean.
- Required classic structure:
  - One recipe `CASE` with repeated charge/react/hold/discharge branches.
- Components:
  - `PiController`, `PidController`, `PwmController`, `Pt1Filter`,
    `Pt2Filter`, `DerivativeFilter`, `HysteresisSwitch`, `Calibrator`,
    `DwordFifo32`, `CalendarClock`, `AutomationContext`.
- Comms/logging:
  - Modbus/MQTT/OPC UA backed by `io.toml` and `runtime.toml`.
  - PLC sets batch-complete and alarm-publish records.
- Tests:
  - recipe 1, 2, and 3 select different sequencer objects;
  - invalid recipe rejects start;
  - class A alarm aborts;
  - batch-complete record contains recipe id and final state.

### 2. HVAC Air Handling Unit

- Pattern: Strategy.
- Process: office AHU with Eco, Comfort, and FrostProtect modes.
- Required OOP structure:
  - `IAhuStrategy` interface.
  - `EcoStrategy`, `ComfortStrategy`, `FrostProtectStrategy`.
  - `ActiveStrategy : IAhuStrategy`.
  - Main scan calls `ActiveStrategy.Update(...)` once.
- Required classic structure:
  - Explicit `IF Mode = ... ELSIF ...` tuning and command branches.
- Components:
  - `PidController`, `Pt1Filter`, `HysteresisSwitch`, `OntimeMeter`,
    `CycleTimeMeter`, `CalendarClock`, `HolidayCalendar`, `DwordFifo16`.
- Comms/logging:
  - OPC UA for BMS mirror, MQTT snapshot, Modbus historian totals.
- Tests:
  - Eco and Comfort produce different commands for same input;
  - FrostProtect overrides scheduled setpoint;
  - smoke/freeze alarms force shutdown.

### 3. Water Booster Pump Station

- Pattern: Mediator + Observer.
- Process: three-pump potable-water booster with lead/lag/standby rotation.
- Required OOP structure:
  - `IPump` interface.
  - `DanfossPumpAdapter` or `PumpDrive` implementing `IPump`.
  - Three pump objects, not one generic pump.
  - `LeadLagMediator` chooses lead/lag/standby by health and runtime.
  - `IAlarmSubscriber` interface and `AlarmBus`.
  - At least two subscribers: historian and MQTT/SCADA mirror.
- Required classic structure:
  - Direct pump1/pump2/pump3 branching for selection and alarm handling.
- Components:
  - `PidController`, `Pt1Filter`, `HysteresisSwitch`, `OntimeMeter`,
    `CycleTimeMeter`, `DwordFifo32`, `CalendarClock`.
- Comms/logging:
  - Modbus VFD status boundary, MQTT event boundary, OPC UA status exposure.
- Tests:
  - lowest-runtime healthy pump becomes lead;
  - faulted lead fails over;
  - alarm publish reaches both subscribers;
  - dry-run permissive blocks pumps.

### 4. Tank Farm Transfer Skid

- Pattern: Composite + Iterator.
- Process: solvent tank farm with areas and tanks, area e-stop, snapshot all.
- Required OOP structure:
  - `IPlantNode` interface.
  - `TankNode` leaf.
  - `AreaNode` composite owning several tanks.
  - `FarmNode` composite owning areas.
  - `GetChild(Index)` and `ChildCount` iteration.
  - Recursive `EmergencyStop` and `AcknowledgeAll`.
- Required classic structure:
  - Repeated per-tank calls and hard-coded area operations.
- Components:
  - `Pt1Filter`, `HysteresisSwitch`, `DwordFifo16`, `CalendarClock`,
    `AutomationContext`.
- Comms/logging:
  - OPC UA hierarchy mirror and MQTT tank/alarm events.
- Tests:
  - area e-stop closes only that area's tanks;
  - snapshot iteration includes all tanks;
  - high-high and leak alarms aggregate to plant.

### 5. Refinery Temperature Conditioning Chain

- Pattern: Decorator.
- Process: redundant thermocouples feeding PID, alarm, historian, and OPC UA.
- Required OOP structure:
  - `ISignalSource` interface.
  - `RawAnalogInput`.
  - `VotingDecorator`.
  - `RangeClampDecorator`.
  - `SpikeRejectDecorator`.
  - `Pt1Decorator`.
  - `Conditioned : ISignalSource` assigned to final decorator.
- Required classic structure:
  - One procedural pipeline copied into scan logic.
- Components:
  - `Pt1Filter`, `Pt2Filter`, `DerivativeFilter`, `DelayLine16`,
    `DwordFifo16`, `HysteresisSwitch`, `OntimeMeter`.
- Comms/logging:
  - OPC UA exposes per-stage diagnostics, MQTT publishes conditioned value,
    Modbus exposes winner/status registers.
- Tests:
  - out-of-range raw input is clamped and quality downgraded;
  - spike rejection holds previous value;
  - downstream PID reads only `ISignalSource`.

### 6. Boiler Room Heating Plant

- Pattern: Facade + Observer.
- Process: two-boiler district-heating room with heat curve and pumps.
- Required OOP structure:
  - `BoilerStation` facade with `Start`, `Stop`, `Update`, `Snapshot`.
  - Internal `BoilerController` objects are not used by `Main`.
  - `AlarmBus` with historian and MQTT/OPC UA subscribers.
- Required classic structure:
  - `Main` wires boiler, pump, alarm, and snapshot logic directly.
- Components:
  - `Pt1Filter`, `HysteresisSwitch`, `OntimeMeter`, `DwordCounter`,
    `CalendarClock`, `HolidayCalendar`, `RtcClock`, `DwordFifo32`.
- Comms/logging:
  - Modbus boiler controller boundary, MQTT snapshot/alarm, OPC UA facade.
- Tests:
  - `Main` drives the room through `BoilerStation` only;
  - single boiler fault keeps station degraded;
  - both boilers offline raises class A alarm;
  - snapshot hides internals and exposes SCADA fields.

### 7. Pasteurizer Temperature Control

- Pattern: Chain of Responsibility + Template Method.
- Process: plate pasteurizer with legal hold temperature/time and divert valve.
- Required OOP structure:
  - `IQualityAlarmHandler` interface.
  - `LocalRecoveryHandler`, `OperatorAcknowledgeHandler`,
    `SupervisorEscalateHandler`, `BatchAbortHandler`.
  - Handlers wired with `Next` references.
  - Pasteurizer sequence skeleton uses step methods for preheat, heat, hold,
    chill, drain.
- Required classic structure:
  - Direct alarm escalation branches in one controller.
- Components:
  - `PidController`, `Pt2Filter`, `DerivativeFilter`, `HysteresisSwitch`,
    `DwordCounter`, `OntimeMeter`, `CycleTimeMeter`, `CalendarClock`.
- Comms/logging:
  - Modbus flow/positioner, MQTT batch quality, OPC UA KPI exposure.
- Tests:
  - recoverable quality event stops at local handler;
  - unrecoverable event reaches abort handler;
  - milk vs cream hold thresholds differ;
  - divert counter increments.

### 8. CIP Wash Skid

- Pattern: State objects.
- Process: dairy CIP skid with pre-rinse, caustic, rinse, acid, final rinse,
  dry, pause, abort, safe shutdown.
- Required OOP structure:
  - `ICipState` interface.
  - Separate `IdleState`, `PreRinseState`, `CausticWashState`,
    `IntermediateRinseState`, `AcidWashState`, `FinalRinseState`,
    `DryState`, `PausedState`, `AbortState`, `SafeShutdownState`.
  - `CipController.Current : ICipState`.
  - `OnEnter`, `OnExecute`, `OnExit` calls on transitions.
- Required classic structure:
  - `CASE Step OF` block with all actions inline.
- Components:
  - `Pt1Filter`, `PidController`, `OntimeMeter`, `DwordCounter`, `Latch`,
    `CalendarClock`.
- Comms/logging:
  - Modbus conductivity/flow, MQTT step transition records, OPC UA state node.
- Tests:
  - normal sequence enters at least three concrete state objects;
  - pause/resume preserves current state;
  - final rinse conductivity alarm routes to abort/safe shutdown.

### 9. Chemical Dosing Skid

- Pattern: Command + Memento.
- Process: water-treatment dosing of chlorine, alum, and anti-scalant.
- Required OOP structure:
  - `IDosingCommand` interface.
  - `ChlorineDoseCommand`, `AlumDoseCommand`, `AntiScalantDoseCommand`.
  - `DosingScheduler` queue and command pool.
  - `DosingMemento` captured before execution and written to audit log.
- Required classic structure:
  - Fixed command table and direct branch execution.
- Components:
  - `Calibrator`, `PwmController`, `Pt1Filter`, `DwordFifo32`,
    `DwordCounter`, `HysteresisSwitch`, `CalendarClock`.
- Comms/logging:
  - Modbus ORP/turbidity, MQTT command audit, OPC UA queue preview.
- Tests:
  - calibration overdue rejects command before execution;
  - three command types execute through one scheduler path;
  - audit memento captures before/after;
  - replay emits audit-ready flag.

### 10. Mixed-Vendor VFD Motor Cell

- Pattern: Adapter.
- Process: six conveyor motors using ABB, Danfoss, and Siemens drives.
- Required OOP structure:
  - `IMotorDrive` interface.
  - `AbbAcs580Adapter`, `DanfossFc302Adapter`, `SiemensG120Adapter`.
  - Main application iterates or calls motors through `IMotorDrive`.
  - Fault codes normalized to one `DWORD` space.
- Required classic structure:
  - Vendor-specific status word decoding mixed into conveyor logic.
- Components:
  - `OntimeMeter`, `HysteresisSwitch`, `Pt1Filter`, `DwordCounter`,
    `DwordFifo16`, `CalendarClock`.
- Comms/logging:
  - Modbus RTU VFD register boundary, MQTT status, OPC UA motor tree.
- Tests:
  - three raw vendor status words normalize to same running/fault semantics;
  - app logic uses `IMotorDrive`;
  - swapping a motor adapter does not change conveyor logic.

### 11. Cold Storage Plant

- Pattern: Composite + Observer + Mediator.
- Process: freezer/chiller rooms with shared compressor rack.
- Required OOP structure:
  - `ColdStorePlant`, `RoomCluster`, and `ColdRoom` composition.
  - Room event publication through `AlarmBus`.
  - `CompressorRackMediator` allocates capacity by room demand priority.
- Required classic structure:
  - Direct room and compressor branching in one controller.
- Components:
  - `Pt1Filter`, `HysteresisSwitch`, `OntimeMeter`, `CycleTimeMeter`,
    `Latch`, `DwordCounter`, `DwordStack32`, `DwordFifo32`,
    `CalendarClock`, `HolidayCalendar`.
- Comms/logging:
  - Modbus compressor controllers, MQTT room/defrost/energy events, OPC UA tree.
- Tests:
  - freezer demand wins over chiller demand under limited capacity;
  - room alarm is received by maintenance stack and MQTT subscriber;
  - adding a room only changes composition setup.

### 12. Water Booster Commissioning Mode

- Pattern: Decorator.
- Process: commissioning mode wraps the water booster station signals and pump
  outputs with simulation/loopback objects.
- Required OOP structure:
  - Same `ISignalSource`/`IPump` boundary as production booster.
  - `SimulatedPressureDecorator` wraps real pressure source.
  - `CommissioningPumpDecorator` wraps real pump.
  - Production mediator/controller uses the decorated objects unchanged.
- Required classic structure:
  - `IF CommissioningMode THEN` branches scattered through scan logic.
- Components:
  - `SineSignalGenerator`, `SquareSignalGenerator`, `RandomSignalGenerator`,
    `ByteRamp`, `WordRamp`, `DwordCounter`, `Latch`.
- Comms/logging:
  - MQTT/OPC UA disabled or inhibited in commissioning mode; hardwired test-mode
    output remains active.
- Tests:
  - production mode reads real signal;
  - commissioning mode reads simulated signal;
  - pump decorator records command without driving real output in loopback mode.

### 13. Pharmaceutical Filling Line

- Pattern: Builder + State.
- Process: sterile vial filling with recipe-specific fill volume, pump speed,
  stopper check, weight check, reject lane, and batch report.
- Required OOP structure:
  - `FillRecipeBuilder` validates recipe inputs and returns `FillRecipe`.
  - Explicit fill states for idle, prime, fill, check weight, reject, complete,
    and fault.
  - State logic uses the built recipe, not scattered raw settings.
- Required classic structure:
  - Recipe fields validated inline inside the step `CASE`.
- Components:
  - `Calibrator`, `PwmController`, `Pt1Filter`, `HysteresisSwitch`,
    `DwordCounter`, `DwordFifo32`, `CalendarClock`.
- Comms/logging:
  - MQTT electronic batch record; OPC UA recipe/status; optional Modbus scale.
- Tests:
  - invalid recipe cannot start;
  - valid recipe reaches fill state;
  - reject path increments reject counter;
  - batch record ready at complete.

### 14. Robotic Palletizer Cell

- Pattern: State + Command.
- Process: infeed, box present sensor, pick/place robot action, layer build,
  pallet index, reject/manual recovery.
- Required OOP structure:
  - `IRobotCommand` interface.
  - `PickCommand`, `PlaceCommand`, `IndexPalletCommand`, `RejectBoxCommand`.
  - Palletizer state machine executes queued/current command objects.
- Required classic structure:
  - Pick/place/index/reject branches inline in one step machine.
- Components:
  - `PulseGenerator`, `DwordFifo32`, `DwordCounter`, `HysteresisSwitch`,
    `OntimeMeter`, `CalendarClock`.
- Comms/logging:
  - MQTT command/fault events and OPC UA cell status.
- Tests:
  - command failure retries then faults;
  - successful pick/place advances layer count;
  - reject command records audit event.

### 15. Silo Loading System

- Pattern: Composite + Mediator.
- Process: multiple silos share one blower and diverter path.
- Required OOP structure:
  - `Silo` leaf objects under `SiloFarm`.
  - `BlowerPathMediator` grants the shared resource to only one silo.
  - Silo high-level protection remains local to the silo object.
- Required classic structure:
  - One `CASE TargetSilo` with repeated blower/diverter branches.
- Components:
  - `Pt1Filter`, `HysteresisSwitch`, `DwordFifo16`, `OntimeMeter`,
    `DwordCounter`, `CalendarClock`.
- Comms/logging:
  - Modbus weigh/level boundary, MQTT transfer report, OPC UA farm status.
- Tests:
  - two silos request loading, mediator grants one;
  - high-level silo is denied;
  - transfer-complete report identifies selected silo.

### 16. Tunnel Oven

- Pattern: Strategy + Observer.
- Process: multi-zone oven with product-specific heating profile, conveyor
  speed, over-temperature trip, quality and energy events.
- Required OOP structure:
  - `IOvenProfileStrategy` interface.
  - At least three profiles: bread, biscuit, cleaning/idle.
  - `OvenEventBus` observers for quality, alarm, and historian records.
- Required classic structure:
  - Product `IF/CASE` tuning in every zone.
- Components:
  - `PidController`, `Pt1Filter`, `HysteresisSwitch`, `OntimeMeter`,
    `DwordCounter`, `DwordFifo32`.
- Comms/logging:
  - MQTT quality/energy records and OPC UA oven status.
- Tests:
  - same measured temperature produces different command by profile;
  - over-temperature publishes alarm event;
  - historian receives batch-complete event.

### 17. Crane Hoist Load Handling Cell

- Pattern: State + Adapter.
- Process: hoist and travel motors move loads between zones with load sensor,
  upper/lower limits, travel limits, and safe zone interlocks.
- Required OOP structure:
  - `IDriveAdapter` interface.
  - Mixed drive adapters normalize status/faults.
  - State objects or explicit state FBs for parked, lifting, travelling,
    lowering, and fault.
- Required classic structure:
  - Raw drive status decoding inside hoist state logic.
- Components:
  - `Pt1Filter`, `HysteresisSwitch`, `SingleOutputDriver`, `OntimeMeter`,
    `DwordFifo16`, `CalendarClock`.
- Comms/logging:
  - Modbus drive boundary and OPC UA crane state.
- Tests:
  - overload blocks lift;
  - travel is denied unless hoist is above safe height;
  - two vendor adapters normalize fault status.

### 18. Water Treatment Filter Backwash

- Pattern: Template Method.
- Process: filter bed service, backwash, rinse, settle, return to service for
  sand and carbon filters.
- Required OOP structure:
  - Shared sequence controller calls configurable step methods.
  - `SandFilterBackwash` and `CarbonFilterBackwash` provide different timings
    and thresholds.
- Required classic structure:
  - Separate procedural step code for each filter type.
- Components:
  - `Pt1Filter`, `HysteresisSwitch`, `OntimeMeter`, `DwordCounter`,
    `DwordFifo32`, `CalendarClock`.
- Comms/logging:
  - Modbus turbidity/flow boundary, MQTT cycle report, OPC UA filter status.
- Tests:
  - sand and carbon choose different rinse/backwash timing;
  - high turbidity extends rinse;
  - cycle-complete record is produced.

### 19. Tunnel Washer / Industrial Laundry Line

- Pattern: Chain of Responsibility.
- Process: soil-level detection, detergent dose, wash temperature, rinse
  validation, and batch quality hold/reject.
- Required OOP structure:
  - Quality handlers: auto-correct dose, extend rinse, operator hold, reject.
  - Chain is wired explicitly and can be reordered by changing links.
- Required classic structure:
  - Escalation hard-coded in one `IF/ELSIF` ladder.
- Components:
  - `PwmController`, `Pt1Filter`, `HysteresisSwitch`, `DwordCounter`,
    `DwordFifo32`, `CalendarClock`.
- Comms/logging:
  - MQTT batch quality and OPC UA line status.
- Tests:
  - detergent under-dose is handled by auto-correct;
  - failed final rinse escalates to operator then reject;
  - quality record contains final handler id.

### 20. Battery Energy Storage Cabinet

- Pattern: Facade + Observer.
- Process: battery racks, inverter, HVAC, fire detection, grid command, and
  telemetry.
- Required OOP structure:
  - `EnergyStorageCabinet` facade.
  - Internal rack/inverter/HVAC objects hidden from `Main`.
  - Alarm observers for HMI, MQTT, and historian.
- Required classic structure:
  - `Main` directly coordinates rack, inverter, HVAC, and alarm outputs.
- Components:
  - `Pt1Filter`, `HysteresisSwitch`, `PidController`, `DwordFifo32`,
    `OntimeMeter`, `CalendarClock`.
- Comms/logging:
  - Modbus inverter/BMS boundary, MQTT snapshot/alarm, OPC UA cabinet node.
- Tests:
  - fire alarm trips facade and notifies observers;
  - grid command denied when rack temperature unsafe;
  - snapshot exposes one SCADA-friendly status.

### 21. Warehouse Conveyor Merge

- Pattern: Mediator.
- Process: three infeed conveyors merge into one trunk with priority,
  anti-collision, jam states, and throughput counters.
- Required OOP structure:
  - Independent `InfeedConveyor` objects.
  - `MergeMediator` grants merge token by priority/queue/clear-zone status.
  - Infeeds do not command each other directly.
- Required classic structure:
  - Cross-coupled infeed `IF` statements.
- Components:
  - `ShiftRegister8`, `PulseGenerator`, `HysteresisSwitch`, `DwordCounter`,
    `DwordFifo16`, `CalendarClock`.
- Comms/logging:
  - MQTT throughput/jam events and OPC UA merge status.
- Tests:
  - only one infeed receives merge token;
  - jam blocks all grants;
  - priority infeed wins under conflict.

### 22. Cleanroom Pressure Cascade

- Pattern: Strategy + Composite.
- Process: room hierarchy with pressure setpoints, door interlocks, AHU dampers,
  normal/cleaning/emergency modes.
- Required OOP structure:
  - `CleanroomArea` composite containing room objects.
  - `IPressureModeStrategy` for normal, cleaning, and emergency modes.
  - Alarm propagation from room to area.
- Required classic structure:
  - Repeated room pressure logic and mode branches.
- Components:
  - `PidController`, `Pt1Filter`, `HysteresisSwitch`, `DwordFifo32`,
    `CalendarClock`, `HolidayCalendar`.
- Comms/logging:
  - OPC UA area tree and MQTT alarm/snapshot.
- Tests:
  - emergency mode changes all room setpoints;
  - door-open alarm propagates to area;
  - composite snapshot includes child rooms.

### 23. Cooling Tower Cell

- Pattern: Facade + Template Method.
- Process: fan stages, basin heater, water temperature, conductivity bleed,
  freeze protection, summer/winter operation.
- Required OOP structure:
  - `CoolingTowerCell` facade.
  - Seasonal operating template: prepare, regulate, bleed, protect, report.
  - Summer and winter variants provide different protection logic.
- Required classic structure:
  - Seasonal branches inline throughout scan logic.
- Components:
  - `PidController`, `Pt1Filter`, `HysteresisSwitch`, `OntimeMeter`,
    `DwordCounter`, `DwordFifo32`, `CalendarClock`.
- Comms/logging:
  - MQTT energy/water snapshot and OPC UA tower status.
- Tests:
  - winter mode enables basin heater under freeze condition;
  - summer mode prioritizes fan staging;
  - facade snapshot hides internals.

### 24. Kiln/Dryer Moisture Control

- Pattern: Decorator + Strategy.
- Process: moisture sensor conditioning, temperature zones, fan speed, and
  product drying recipe.
- Required OOP structure:
  - Decorated moisture signal chain.
  - Drying profile strategy for softwood, hardwood, and standby.
  - Controller reads one conditioned signal and one active strategy.
- Required classic structure:
  - Sensor conditioning and product tuning copied into scan branches.
- Components:
  - `Pt1Filter`, `DerivativeFilter`, `HysteresisSwitch`,
    `SingleOutputDriver`, `OntimeMeter`, `DwordFifo32`.
- Comms/logging:
  - MQTT kiln batch snapshot and OPC UA current profile.
- Tests:
  - decorator clamps bad moisture input;
  - softwood/hardwood strategies command different fan speeds;
  - alarm event records high drying rate.

### 25. Airport Baggage Diverter

- Pattern: Command + Observer.
- Process: scanner result, bag tracking, diverter arms, reject spur, and fault
  recovery.
- Required OOP structure:
  - `IBaggageCommand` interface.
  - `DivertLeftCommand`, `DivertRightCommand`, `RejectBagCommand`.
  - `BaggageEventBus` with historian and MQTT subscribers.
  - Observers receive route, reject, and fault events.
- Required classic structure:
  - Route decisions and event logging inline in scanner branch.
- Components:
  - `ShiftRegister8`, `PulseGenerator`, `DwordFifo32`, `DwordCounter`,
    `HysteresisSwitch`, `CalendarClock`.
- Comms/logging:
  - MQTT audit events and OPC UA diverter status.
- Tests:
  - route command follows scanner destination;
  - reject command publishes observer event;
  - event counters prove the observer fan-out path executed.

### 26. Dairy Separator Skid

- Pattern: Adapter + State.
- Process: separator bowl speed, vibration, feed valve, discharge cycle, and CIP.
- Required OOP structure:
  - `ISeparatorDrive` interface.
  - VFD/sensor adapters hide raw protocol status.
  - State machine for idle, spin-up, production, discharge, CIP, fault.
- Required classic structure:
  - Raw drive/sensor status decoded inside the step machine.
- Components:
  - `Pt1Filter`, `DerivativeFilter`, `HysteresisSwitch`,
    `SingleOutputDriver`, `OntimeMeter`, `DwordFifo32`.
- Comms/logging:
  - Modbus VFD/vibration boundary, MQTT discharge/CIP events, OPC UA skid state.
- Tests:
  - high vibration trips to fault;
  - adapter normalizes ready/running/fault status;
  - production state opens the feed valve only after the bowl reaches speed.

### 27. District Pump Network

- Pattern: Proxy + Mediator.
- Process: local pump station balances demand with remote stations publishing
  demand and quality over MQTT.
- Required OOP structure:
  - `IStationProxy` interface.
  - `RemoteStationProxy` with stale/quality status.
  - `DemandMediator` balances local and remote demand.
- Required classic structure:
  - Raw remote values read directly in pump decision logic.
- Components:
  - `PidController`, `Pt1Filter`, `HysteresisSwitch`, `DwordFifo32`,
    `OntimeMeter`, `CalendarClock`.
- Comms/logging:
  - MQTT remote demand boundary, Modbus local pump boundary, OPC UA network view.
- Tests:
  - stale proxy is ignored and raises warning;
  - valid remote demand changes local demand allocation;
  - local suction loss forces a zero pump command.

## Compact Reference Showcases To Keep

These are smaller than the industrial catalog but are kept because they are
honest and useful for learning a single pattern quickly.

### Closed Loop Polymorphism

- Pattern: interface polymorphism.
- Projects:
  - `examples/OSCAT/closed_loop_polymorphism/non-oop`
  - `examples/OSCAT/closed_loop_polymorphism/oop`
- Required structure:
  - `Loop : IClosedLoopController`.
  - `PiLoop` and `PidLoop` concrete controllers.
  - One caller path through `Loop.Update(...)`.

### Temperature Zone Composition

- Pattern: composition and multi-instance fan-out.
- Projects:
  - `examples/OSCAT/temperature_zone_composition/non-oop`
  - `examples/OSCAT/temperature_zone_composition/oop`
- Required structure:
  - `TemperatureZoneController` owns filter, controller, and alarm.
  - Main owns at least two independent zone objects.
