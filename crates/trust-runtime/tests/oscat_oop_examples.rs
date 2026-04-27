use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

const STRUCTURAL_EXPECTATIONS: &[(&str, &[&str])] = &[
    (
        "multi_product_batch_reactor",
        &[
            "INTERFACE IBatchSequencer",
            "FUNCTION_BLOCK AcidSequencer IMPLEMENTS IBatchSequencer",
            "FUNCTION_BLOCK PolymerSequencer IMPLEMENTS IBatchSequencer",
            "FUNCTION_BLOCK BaseWashSequencer IMPLEMENTS IBatchSequencer",
            "METHOD PUBLIC Build : IBatchSequencer",
            "ActiveSequencer : IBatchSequencer",
        ],
    ),
    (
        "hvac_air_handling_unit",
        &[
            "INTERFACE IAhuStrategy",
            "FUNCTION_BLOCK EcoStrategy IMPLEMENTS IAhuStrategy",
            "FUNCTION_BLOCK ComfortStrategy IMPLEMENTS IAhuStrategy",
            "FUNCTION_BLOCK FrostProtectStrategy IMPLEMENTS IAhuStrategy",
            "ActiveStrategy : IAhuStrategy",
        ],
    ),
    (
        "water_booster_pump_station",
        &[
            "INTERFACE IPump",
            "FUNCTION_BLOCK PumpDrive IMPLEMENTS IPump",
            "FUNCTION_BLOCK LeadLagMediator",
            "Lead : IPump",
            "INTERFACE IAlarmSubscriber",
            "FUNCTION_BLOCK AlarmBus",
            "HistorianAlarmSubscriber IMPLEMENTS IAlarmSubscriber",
            "MqttAlarmSubscriber IMPLEMENTS IAlarmSubscriber",
        ],
    ),
    (
        "tank_farm_transfer_skid",
        &[
            "INTERFACE IPlantNode",
            "FUNCTION_BLOCK TankNode IMPLEMENTS IPlantNode",
            "FUNCTION_BLOCK AreaNode IMPLEMENTS IPlantNode",
            "FUNCTION_BLOCK FarmNode IMPLEMENTS IPlantNode",
            "METHOD PUBLIC GetChild : IPlantNode",
            "Area : IPlantNode",
            "Tank : IPlantNode",
        ],
    ),
    (
        "refinery_temperature_conditioning",
        &[
            "INTERFACE ISignalSource",
            "FUNCTION_BLOCK RawAnalogInput IMPLEMENTS ISignalSource",
            "FUNCTION_BLOCK VotingDecorator IMPLEMENTS ISignalSource",
            "FUNCTION_BLOCK RangeClampDecorator IMPLEMENTS ISignalSource",
            "FUNCTION_BLOCK SpikeRejectDecorator IMPLEMENTS ISignalSource",
            "FUNCTION_BLOCK Pt1SignalDecorator IMPLEMENTS ISignalSource",
            "Conditioned : ISignalSource",
            "Clamped.Wrap(InnerSource := Voted",
            "SpikeRejected.Wrap(InnerSource := Clamped",
            "Filtered.Wrap(InnerSource := SpikeRejected",
        ],
    ),
    (
        "boiler_room_heating_plant",
        &[
            "TYPE BoilerStationStatus",
            "INTERFACE IBoilerAlarmSubscriber",
            "FUNCTION_BLOCK BoilerStation IMPLEMENTS IComponent",
            "METHOD PUBLIC Start",
            "METHOD PUBLIC Stop",
            "METHOD PUBLIC StationSnapshot : BoilerStationStatus",
            "FUNCTION_BLOCK BoilerAlarmBus",
            "METHOD PUBLIC Subscribe",
            "METHOD PUBLIC Publish",
            "HistorianAlarmSubscriber IMPLEMENTS IBoilerAlarmSubscriber",
            "MqttAlarmSubscriber IMPLEMENTS IBoilerAlarmSubscriber",
            "AlarmBus.Subscribe(Sub := Historian)",
            "AlarmBus.Subscribe(Sub := MqttPublisher)",
        ],
    ),
    (
        "pasteurizer_quality_chain",
        &[
            "INTERFACE IQualityAlarmHandler",
            "FUNCTION_BLOCK BasePasteurizerTemplate",
            "METHOD PUBLIC RunTemplate : PasteurizerResult",
            "FUNCTION_BLOCK MilkPasteurizer EXTENDS BasePasteurizerTemplate",
            "FUNCTION_BLOCK CreamPasteurizer EXTENDS BasePasteurizerTemplate",
            "FUNCTION_BLOCK LocalRecoveryHandler IMPLEMENTS IQualityAlarmHandler",
            "FUNCTION_BLOCK OperatorAcknowledgeHandler IMPLEMENTS IQualityAlarmHandler",
            "FUNCTION_BLOCK SupervisorEscalateHandler IMPLEMENTS IQualityAlarmHandler",
            "FUNCTION_BLOCK BatchAbortHandler IMPLEMENTS IQualityAlarmHandler",
            "Local.SetNext(NextHandler := Operator)",
            "Operator.SetNext(NextHandler := Supervisor)",
            "Supervisor.SetNext(NextHandler := Abort)",
            "AlarmChain : IQualityAlarmHandler",
        ],
    ),
    (
        "cip_wash_state",
        &[
            "INTERFACE ICipState",
            "FUNCTION_BLOCK IdleState IMPLEMENTS ICipState",
            "FUNCTION_BLOCK PreRinseState IMPLEMENTS ICipState",
            "FUNCTION_BLOCK CausticWashState IMPLEMENTS ICipState",
            "FUNCTION_BLOCK FinalRinseState IMPLEMENTS ICipState",
            "FUNCTION_BLOCK CipController",
            "Current : ICipState",
            "METHOD PUBLIC ResolveState : ICipState",
            "NextId := Current.OnExecute",
            "Current.OnExit()",
            "Current.OnEnter()",
        ],
    ),
    (
        "chemical_dosing_command",
        &[
            "TYPE DosingMemento",
            "INTERFACE IDosingCommand",
            "FUNCTION_BLOCK ChlorineDoseCommand IMPLEMENTS IDosingCommand",
            "FUNCTION_BLOCK AlumDoseCommand IMPLEMENTS IDosingCommand",
            "FUNCTION_BLOCK AntiScalantDoseCommand IMPLEMENTS IDosingCommand",
            "Pending : IDosingCommand",
            "METHOD PUBLIC Enqueue : BOOL",
            "METHOD PUBLIC ExecuteNext : BOOL",
            "METHOD PUBLIC CaptureMemento : DosingMemento",
            "LastMemento := Pending.CaptureMemento",
            "AuditLog : DwordFifo32",
        ],
    ),
    (
        "mixed_vendor_vfd_adapter",
        &[
            "INTERFACE IMotorDrive EXTENDS IComponent",
            "FUNCTION_BLOCK AbbAcs580Adapter IMPLEMENTS IMotorDrive",
            "FUNCTION_BLOCK DanfossFc302Adapter EXTENDS AbbAcs580Adapter",
            "FUNCTION_BLOCK SiemensG120Adapter EXTENDS AbbAcs580Adapter",
            "Drive1 : IMotorDrive",
            "Drive2 : IMotorDrive",
            "Drive3 : IMotorDrive",
            "METHOD PRIVATE ApplyDrive",
            "Drive.Command(SpeedReferenceHz := SpeedHz",
            "Drive.HasFault",
            "Drive.FaultCode",
        ],
    ),
    (
        "cold_storage_plant",
        &[
            "INTERFACE IColdNode",
            "FUNCTION_BLOCK RoomNode IMPLEMENTS IColdNode",
            "FUNCTION_BLOCK RoomCluster IMPLEMENTS IColdNode",
            "METHOD GetChild : IColdNode",
            "INTERFACE IColdAlarmSubscriber",
            "FUNCTION_BLOCK EnergyLossSubscriber IMPLEMENTS IColdAlarmSubscriber",
            "FUNCTION_BLOCK MaintenanceStackSubscriber IMPLEMENTS IColdAlarmSubscriber",
            "FUNCTION_BLOCK MqttColdAlarmSubscriber IMPLEMENTS IColdAlarmSubscriber",
            "FUNCTION_BLOCK ColdAlarmBus",
            "FUNCTION_BLOCK CompressorRackMediator",
            "AlarmBus.Subscribe",
            "Rack.Allocate",
        ],
    ),
    (
        "booster_commissioning_decorator",
        &[
            "INTERFACE ISignalSource",
            "FUNCTION_BLOCK RealPressureSource IMPLEMENTS ISignalSource",
            "FUNCTION_BLOCK SimulatedPressureDecorator IMPLEMENTS ISignalSource",
            "RealSourceRef : ISignalSource",
            "SineGen : SineSignalGenerator",
            "INTERFACE IPump",
            "FUNCTION_BLOCK RealPumpDrive IMPLEMENTS IPump",
            "FUNCTION_BLOCK CommissioningOutputDriver IMPLEMENTS IPump",
            "RealPump : IPump",
            "Pressure : ISignalSource",
            "Pump : IPump",
        ],
    ),
    (
        "pharma_filling_builder_state",
        &[
            "FUNCTION_BLOCK FillRecipeBuilder",
            "METHOD PUBLIC SetVolume : FillRecipeBuilder",
            "METHOD PUBLIC SetPumpSpeed : FillRecipeBuilder",
            "METHOD PUBLIC Build : FillRecipe",
            "INTERFACE IFillingState",
            "FUNCTION_BLOCK IdleFillState IMPLEMENTS IFillingState",
            "FUNCTION_BLOCK FillState IMPLEMENTS IFillingState",
            "FUNCTION_BLOCK CheckWeightState IMPLEMENTS IFillingState",
            "Current : IFillingState",
            "METHOD PUBLIC ResolveState : IFillingState",
        ],
    ),
    (
        "robotic_palletizer_command_state",
        &[
            "INTERFACE IRobotCommand",
            "FUNCTION_BLOCK PickCommand IMPLEMENTS IRobotCommand",
            "FUNCTION_BLOCK PlaceCommand IMPLEMENTS IRobotCommand",
            "FUNCTION_BLOCK IndexPalletCommand IMPLEMENTS IRobotCommand",
            "FUNCTION_BLOCK RejectBoxCommand IMPLEMENTS IRobotCommand",
            "INTERFACE IPalletizerState",
            "FUNCTION_BLOCK PickState IMPLEMENTS IPalletizerState",
            "FUNCTION_BLOCK PlaceState IMPLEMENTS IPalletizerState",
            "Current : IPalletizerState",
            "CurrentCommand : IRobotCommand",
            "METHOD PUBLIC CommandForState : IRobotCommand",
        ],
    ),
    (
        "silo_loading_composite_mediator",
        &[
            "INTERFACE ISiloNode",
            "FUNCTION_BLOCK SiloNode IMPLEMENTS ISiloNode",
            "FUNCTION_BLOCK SiloFarm IMPLEMENTS ISiloNode",
            "METHOD GetChild : ISiloNode",
            "FUNCTION_BLOCK TransferPathMediator",
            "ActiveSilo : ISiloNode",
            "Candidate : ISiloNode",
            "Candidate := Farm.GetChild",
            "Mediator.Grant",
        ],
    ),
    (
        "tunnel_oven_strategy_observer",
        &[
            "INTERFACE IOvenProfile",
            "FUNCTION_BLOCK BreadProfile IMPLEMENTS IOvenProfile",
            "FUNCTION_BLOCK PizzaProfile IMPLEMENTS IOvenProfile",
            "ActiveProfile : IOvenProfile",
            "ActiveProfile.HeatCommand",
            "INTERFACE IOvenEventSubscriber",
            "FUNCTION_BLOCK OvenHistorianSubscriber IMPLEMENTS IOvenEventSubscriber",
            "FUNCTION_BLOCK OvenMqttSubscriber IMPLEMENTS IOvenEventSubscriber",
            "FUNCTION_BLOCK OvenEventBus",
            "Bus.Subscribe",
            "Bus.Publish",
        ],
    ),
    (
        "crane_hoist_adapter_state",
        &[
            "INTERFACE IHoistDrive",
            "FUNCTION_BLOCK AbbHoistAdapter IMPLEMENTS IHoistDrive",
            "FUNCTION_BLOCK SiemensTravelAdapter IMPLEMENTS IHoistDrive",
            "HoistDrive : IHoistDrive",
            "TravelDrive : IHoistDrive",
            "INTERFACE ICraneState",
            "FUNCTION_BLOCK ParkedState IMPLEMENTS ICraneState",
            "FUNCTION_BLOCK LiftingState IMPLEMENTS ICraneState",
            "FUNCTION_BLOCK TravellingState IMPLEMENTS ICraneState",
            "Current : ICraneState",
            "METHOD PUBLIC ResolveState : ICraneState",
        ],
    ),
    (
        "filter_backwash_template",
        &[
            "FUNCTION_BLOCK BaseBackwashTemplate",
            "METHOD PUBLIC RunTemplate : BackwashResult",
            "METHOD PUBLIC RequiredBackwashSeconds : INT",
            "METHOD PUBLIC RinseTurbidityLimit : REAL",
            "FUNCTION_BLOCK SandFilterBackwash EXTENDS BaseBackwashTemplate",
            "FUNCTION_BLOCK CarbonFilterBackwash EXTENDS BaseBackwashTemplate",
            "Sand.RunTemplate",
            "Carbon.RunTemplate",
        ],
    ),
    (
        "tunnel_washer_chain",
        &[
            "INTERFACE IWasherQualityHandler",
            "FUNCTION_BLOCK AutoDoseHandler IMPLEMENTS IWasherQualityHandler",
            "FUNCTION_BLOCK ExtendRinseHandler IMPLEMENTS IWasherQualityHandler",
            "FUNCTION_BLOCK OperatorHoldHandler IMPLEMENTS IWasherQualityHandler",
            "FUNCTION_BLOCK RejectBatchHandler IMPLEMENTS IWasherQualityHandler",
            "AutoDose.SetNext(NextHandler := ExtendRinse)",
            "ExtendRinse.SetNext(NextHandler := OperatorHold)",
            "OperatorHold.SetNext(NextHandler := RejectBatch)",
            "Chain : IWasherQualityHandler",
        ],
    ),
    (
        "battery_energy_storage_facade",
        &[
            "FUNCTION_BLOCK EnergyStorageCabinet",
            "METHOD PUBLIC Enable",
            "METHOD PUBLIC Disable",
            "METHOD PUBLIC Snapshot : BessSnapshot",
            "INTERFACE IBessAlarmSubscriber",
            "FUNCTION_BLOCK BessHistorianSubscriber IMPLEMENTS IBessAlarmSubscriber",
            "FUNCTION_BLOCK BessMqttSubscriber IMPLEMENTS IBessAlarmSubscriber",
            "FUNCTION_BLOCK BessAlarmBus",
            "AlarmBus.Subscribe",
            "AlarmBus.Publish",
        ],
    ),
    (
        "warehouse_conveyor_merge_mediator",
        &[
            "INTERFACE IInfeedConveyor",
            "FUNCTION_BLOCK InfeedConveyor IMPLEMENTS IInfeedConveyor",
            "FUNCTION_BLOCK MergeMediator",
            "METHOD PRIVATE TryGrant",
            "Lane : IInfeedConveyor",
            "TryGrant(Lane := Lane2",
            "TryGrant(Lane := Lane1",
            "TryGrant(Lane := Lane3",
        ],
    ),
    (
        "cleanroom_pressure_strategy_composite",
        &[
            "INTERFACE IRoomNode",
            "FUNCTION_BLOCK RoomNode IMPLEMENTS IRoomNode",
            "FUNCTION_BLOCK RoomSuite IMPLEMENTS IRoomNode",
            "METHOD GetChild : IRoomNode",
            "INTERFACE IPressureStrategy",
            "FUNCTION_BLOCK NormalPressureStrategy IMPLEMENTS IPressureStrategy",
            "FUNCTION_BLOCK CleaningPressureStrategy IMPLEMENTS IPressureStrategy",
            "FUNCTION_BLOCK EmergencyPressureStrategy IMPLEMENTS IPressureStrategy",
            "ActiveStrategy : IPressureStrategy",
            "ActiveStrategy.TargetPressure",
        ],
    ),
    (
        "cooling_tower_facade_template",
        &[
            "FUNCTION_BLOCK CoolingTowerCell",
            "FUNCTION_BLOCK BaseTowerSeasonTemplate",
            "METHOD PUBLIC RunTemplate : TowerResult",
            "METHOD PUBLIC FanThreshold : REAL",
            "METHOD PUBLIC FreezeThreshold : REAL",
            "FUNCTION_BLOCK SummerTowerTemplate EXTENDS BaseTowerSeasonTemplate",
            "FUNCTION_BLOCK WinterTowerTemplate EXTENDS BaseTowerSeasonTemplate",
            "Summer.RunTemplate",
            "Winter.RunTemplate",
        ],
    ),
    (
        "kiln_dryer_decorator_strategy",
        &[
            "INTERFACE IMoistureSource",
            "FUNCTION_BLOCK RawMoistureInput IMPLEMENTS IMoistureSource",
            "FUNCTION_BLOCK MoistureClampDecorator IMPLEMENTS IMoistureSource",
            "FUNCTION_BLOCK MoistureFilterDecorator IMPLEMENTS IMoistureSource",
            "Moisture : IMoistureSource",
            "INTERFACE IDryingStrategy",
            "FUNCTION_BLOCK SoftwoodStrategy IMPLEMENTS IDryingStrategy",
            "FUNCTION_BLOCK HardwoodStrategy IMPLEMENTS IDryingStrategy",
            "ActiveStrategy : IDryingStrategy",
        ],
    ),
    (
        "airport_baggage_command_observer",
        &[
            "INTERFACE IBaggageCommand",
            "FUNCTION_BLOCK DivertLeftCommand IMPLEMENTS IBaggageCommand",
            "FUNCTION_BLOCK DivertRightCommand IMPLEMENTS IBaggageCommand",
            "FUNCTION_BLOCK RejectBagCommand IMPLEMENTS IBaggageCommand",
            "INTERFACE IBaggageEventSubscriber",
            "FUNCTION_BLOCK BaggageHistorianSubscriber IMPLEMENTS IBaggageEventSubscriber",
            "FUNCTION_BLOCK BaggageMqttSubscriber IMPLEMENTS IBaggageEventSubscriber",
            "FUNCTION_BLOCK BaggageEventBus",
            "CurrentCommand : IBaggageCommand",
            "METHOD PUBLIC SelectCommand : IBaggageCommand",
            "Bus.Publish",
        ],
    ),
    (
        "dairy_separator_adapter_state",
        &[
            "INTERFACE ISeparatorDrive",
            "FUNCTION_BLOCK AbbSeparatorDriveAdapter IMPLEMENTS ISeparatorDrive",
            "FUNCTION_BLOCK SiemensSeparatorDriveAdapter IMPLEMENTS ISeparatorDrive",
            "Drive : ISeparatorDrive",
            "INTERFACE ISeparatorState",
            "FUNCTION_BLOCK IdleState IMPLEMENTS ISeparatorState",
            "FUNCTION_BLOCK SpinUpState IMPLEMENTS ISeparatorState",
            "FUNCTION_BLOCK ProductionState IMPLEMENTS ISeparatorState",
            "FUNCTION_BLOCK DischargeState IMPLEMENTS ISeparatorState",
            "FUNCTION_BLOCK CipState IMPLEMENTS ISeparatorState",
            "FUNCTION_BLOCK FaultState IMPLEMENTS ISeparatorState",
            "Current : ISeparatorState",
            "METHOD PUBLIC ResolveState : ISeparatorState",
            "Drive.Decode",
        ],
    ),
    (
        "district_pump_network_proxy_mediator",
        &[
            "INTERFACE IStationProxy",
            "FUNCTION_BLOCK LocalStationProxy IMPLEMENTS IStationProxy",
            "FUNCTION_BLOCK RemoteStationProxy IMPLEMENTS IStationProxy",
            "FUNCTION_BLOCK DemandMediator",
            "METHOD PRIVATE AcceptProxy",
            "Station : IStationProxy",
            "AcceptProxy(Station := NorthRemote",
            "AcceptProxy(Station := EastRemote",
            "RemoteStaleCountValue",
        ],
    ),
    (
        "closed_loop_polymorphism",
        &[
            "Loop : IClosedLoopController",
            "PiLoop : PiController",
            "PidLoop : PidController",
        ],
    ),
    (
        "temperature_zone_composition",
        &[
            "FUNCTION_BLOCK TemperatureZoneController",
            "Filter : Pt1Filter",
            "Controller : PidController",
            "AlarmSwitch : HysteresisSwitch",
            "ZoneA : TemperatureZoneController",
            "ZoneB : TemperatureZoneController",
        ],
    ),
];

fn examples_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
}

fn oscat_examples_root() -> PathBuf {
    examples_root().join("OSCAT")
}

fn oscat_example_dirs() -> Vec<PathBuf> {
    let mut dirs = std::fs::read_dir(oscat_examples_root())
        .expect("read examples/OSCAT")
        .map(|entry| entry.expect("read OSCAT example entry"))
        .filter(|entry| entry.file_type().expect("OSCAT entry file type").is_dir())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs
}

fn oscat_example_projects() -> Vec<PathBuf> {
    let mut projects = Vec::new();
    for example_dir in oscat_example_dirs() {
        projects.push(example_dir.join("non-oop"));
        projects.push(example_dir.join("oop"));
    }
    projects
}

fn example_oop_path(slug: &str) -> PathBuf {
    oscat_examples_root().join(slug).join("oop")
}

fn run_example_st_tests_at(project: &Path) -> Result<(), String> {
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["test", "--project"])
        .arg(project)
        .output()
        .expect("run trust-runtime test");

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "expected ST example tests to pass at {}\nstdout:\n{}\nstderr:\n{}",
        project.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn assert_pattern_structure(name: &str, needles: &[&str]) {
    let main_st = example_oop_path(name).join("src").join("Main.st");
    let source = std::fs::read_to_string(&main_st)
        .unwrap_or_else(|err| panic!("read {}: {err}", main_st.display()));
    for needle in needles {
        assert!(
            source.contains(needle),
            "expected {name} to contain pattern marker {needle:?} in {}",
            main_st.display()
        );
    }
}

#[test]
fn oscat_examples_use_grouped_oop_non_oop_layout() {
    let root = oscat_examples_root();
    assert!(root.is_dir(), "expected {} to exist", root.display());

    let legacy_dirs = std::fs::read_dir(examples_root())
        .expect("read examples root")
        .map(|entry| entry.expect("read examples entry"))
        .filter(|entry| {
            entry
                .file_type()
                .expect("examples entry file type")
                .is_dir()
        })
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("oscat_components_")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert!(
        legacy_dirs.is_empty(),
        "OSCAT comparison projects must live under examples/OSCAT, found legacy dirs: {legacy_dirs:?}"
    );

    let example_dirs = oscat_example_dirs();
    assert_eq!(
        example_dirs.len(),
        49,
        "expected 49 paired OSCAT examples under {}",
        root.display()
    );

    for example_dir in example_dirs {
        let readme = example_dir.join("README.md");
        let readme_text = std::fs::read_to_string(&readme)
            .unwrap_or_else(|err| panic!("read {}: {err}", readme.display()));
        for marker in [
            "## Folder Layout",
            "## What This Example Teaches",
            "OOP pattern:",
            "## How The Pair Teaches OOP",
            "`non-oop/`",
            "`oop/`",
        ] {
            assert!(
                readme_text.contains(marker),
                "expected {} to contain {marker:?}",
                readme.display()
            );
        }

        for variant in ["non-oop", "oop"] {
            let project = example_dir.join(variant);
            assert!(
                project.join("trust-lsp.toml").is_file(),
                "expected {} to contain trust-lsp.toml",
                project.display()
            );
            assert!(
                project.join("src").join("Main.st").is_file(),
                "expected {} to contain src/Main.st",
                project.display()
            );
            assert!(
                project.join("src").join("Tests.st").is_file(),
                "expected {} to contain src/Tests.st",
                project.display()
            );
        }
    }
}

#[test]
fn oscat_oop_example_st_unit_tests_pass() {
    let projects = oscat_example_projects();
    let next = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(Mutex::new(Vec::new()));
    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(4)
        .clamp(1, 6)
        .min(projects.len());

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let next = Arc::clone(&next);
            let failures = Arc::clone(&failures);
            let projects = &projects;
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(project) = projects.get(index) else {
                    break;
                };
                if let Err(message) = run_example_st_tests_at(project) {
                    failures.lock().expect("failure lock").push(message);
                }
            });
        }
    });

    let failures = failures.lock().expect("failure lock");
    assert!(
        failures.is_empty(),
        "{} OSCAT OOP example project(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn oscat_oop_examples_contain_claimed_pattern_structures() {
    for (name, needles) in STRUCTURAL_EXPECTATIONS {
        assert_pattern_structure(name, needles);
    }
}
