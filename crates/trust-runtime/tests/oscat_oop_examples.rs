use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const EXAMPLE_TEST_TIMEOUT: Duration = Duration::from_secs(120);
const EXAMPLE_TEST_PROGRESS_INTERVAL: Duration = Duration::from_secs(10);
const OSCAT_AGGREGATE_TRIGGER_EXAMPLE: &str = "airport_baggage_command_observer";
const OSCAT_AGGREGATE_TRIGGER_NAMESPACE: &str = "OSCAT_airport_baggage_command_observer_oop";

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExampleGateEvent {
    Started {
        index: usize,
        total: usize,
        project: PathBuf,
    },
    Passed {
        index: usize,
        total: usize,
        project: PathBuf,
    },
    Failed {
        index: usize,
        total: usize,
        project: PathBuf,
        message: String,
    },
}

impl ExampleGateEvent {
    fn log_line(&self) -> String {
        match self {
            ExampleGateEvent::Started {
                index,
                total,
                project,
            } => format!(
                "[oscat examples] starting {index}/{total}: {}",
                project.display()
            ),
            ExampleGateEvent::Passed {
                index,
                total,
                project,
            } => format!(
                "[oscat examples] passed {index}/{total}: {}",
                project.display()
            ),
            ExampleGateEvent::Failed {
                index,
                total,
                project,
                ..
            } => format!(
                "[oscat examples] failed {index}/{total}: {}",
                project.display()
            ),
        }
    }
}

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

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("src"))
            .unwrap_or_else(|err| panic!("create temp project {}: {err}", path.display()));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn source_line_starts_with_keyword(line: &str, keyword: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case(keyword))
}

fn source_without_configuration_blocks(source: &str) -> String {
    let mut output = String::new();
    let mut skipping_configuration = false;
    for line in source.lines() {
        if source_line_starts_with_keyword(line, "CONFIGURATION") {
            skipping_configuration = true;
            continue;
        }
        if skipping_configuration {
            if source_line_starts_with_keyword(line, "END_CONFIGURATION") {
                skipping_configuration = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn dependency_manifest_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn aggregate_dependency_manifest(workspace_root: &Path) -> String {
    let oscat_path = workspace_root.join("libraries").join("oscat");
    let oscat_oop_path = oscat_path.join("oop");
    format!(
        r#"[project]
include_paths = ["src"]
stdlib = "iec"

[dependencies]
OSCAT = {{ path = "{}", version = "0.1.0" }}
OscatOop = {{ path = "{}", version = "0.1.0" }}
"#,
        dependency_manifest_path(&oscat_path),
        dependency_manifest_path(&oscat_oop_path)
    )
}

fn write_oscat_namespace_aggregate_project(slug: &str, namespace: &str) -> TempProject {
    let temp = TempProject::new(slug);
    let workspace_root = examples_root()
        .parent()
        .expect("examples dir has workspace parent")
        .to_path_buf();
    let manifest = aggregate_dependency_manifest(&workspace_root);
    std::fs::write(temp.path().join("trust-lsp.toml"), manifest)
        .unwrap_or_else(|err| panic!("write aggregate manifest: {err}"));

    let source_dir = example_oop_path(slug).join("src");
    let mut source_files = std::fs::read_dir(&source_dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", source_dir.display()))
        .map(|entry| entry.expect("read OSCAT aggregate source entry").path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("st"))
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| !name.eq_ignore_ascii_case("Configuration.st"))
        })
        .collect::<Vec<_>>();
    source_files.sort();

    let mut aggregate = format!("NAMESPACE {namespace}\nUSING {namespace};\n");
    for source_file in source_files {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|err| panic!("read {}: {err}", source_file.display()));
        aggregate.push_str(&source_without_configuration_blocks(&source));
        aggregate.push('\n');
    }
    aggregate.push_str("END_NAMESPACE\n");
    std::fs::write(temp.path().join("src").join("Aggregate.st"), aggregate)
        .unwrap_or_else(|err| panic!("write aggregate source: {err}"));

    temp
}

fn example_child_started_line(child_id: u32, project: &Path) -> String {
    format!(
        "[oscat examples] child pid={child_id} command=trust-runtime test --project {} timeout={}s",
        project.display(),
        EXAMPLE_TEST_TIMEOUT.as_secs()
    )
}

fn example_child_progress_line(child_id: u32, project: &Path, elapsed: Duration) -> String {
    format!(
        "[oscat examples] child pid={child_id} still running elapsed={}s project={}",
        elapsed.as_secs(),
        project.display()
    )
}

fn example_child_timeout_line(child_id: u32, project: &Path, elapsed: Duration) -> String {
    format!(
        "[oscat examples] child pid={child_id} timed out reason=timeout elapsed={}s timeout={}s project={}",
        elapsed.as_secs(),
        EXAMPLE_TEST_TIMEOUT.as_secs(),
        project.display()
    )
}

fn run_example_st_tests_at(project: &Path) -> Result<(), String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["test", "--project"])
        .arg(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run trust-runtime test");

    let started = Instant::now();
    let child_id = child.id();
    let mut next_progress = EXAMPLE_TEST_PROGRESS_INTERVAL;
    eprintln!("{}", example_child_started_line(child_id, project));
    loop {
        if child
            .try_wait()
            .expect("poll trust-runtime example test")
            .is_some()
        {
            let output = child
                .wait_with_output()
                .expect("collect trust-runtime example test output");
            let elapsed = started.elapsed();
            if output.status.success() {
                eprintln!(
                    "[oscat examples] child pid={child_id} completed status={} elapsed={}ms project={}",
                    output.status,
                    elapsed.as_millis(),
                    project.display()
                );
                return Ok(());
            }

            return Err(format!(
                "expected ST example tests to pass at {}\nchild pid: {}\nelapsed: {}ms\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
                project.display(),
                child_id,
                elapsed.as_millis(),
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        if started.elapsed() >= EXAMPLE_TEST_TIMEOUT {
            let elapsed = started.elapsed();
            let timeout_line = example_child_timeout_line(child_id, project, elapsed);
            eprintln!("{timeout_line}");
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("collect timed-out trust-runtime example test output");
            return Err(format!(
                "{timeout_line}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let elapsed = started.elapsed();
        if elapsed >= next_progress {
            eprintln!(
                "{}",
                example_child_progress_line(child_id, project, elapsed)
            );
            next_progress += EXAMPLE_TEST_PROGRESS_INTERVAL;
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn assert_example_project_tests_pass_with<RunProject, Report>(
    projects: &[PathBuf],
    mut run_project: RunProject,
    mut report: Report,
) where
    RunProject: FnMut(&Path) -> Result<(), String>,
    Report: FnMut(ExampleGateEvent),
{
    assert!(
        !projects.is_empty(),
        "expected at least one OSCAT example project"
    );
    let mut failures = Vec::new();
    let total = projects.len();

    for (offset, project) in projects.iter().enumerate() {
        let index = offset + 1;
        report(ExampleGateEvent::Started {
            index,
            total,
            project: project.clone(),
        });
        match run_project(project) {
            Ok(()) => report(ExampleGateEvent::Passed {
                index,
                total,
                project: project.clone(),
            }),
            Err(message) => {
                report(ExampleGateEvent::Failed {
                    index,
                    total,
                    project: project.clone(),
                    message: message.clone(),
                });
                failures.push(message);
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} OSCAT OOP example project(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn assert_example_project_tests_pass(projects: &[PathBuf]) {
    assert_example_project_tests_pass_with(projects, run_example_st_tests_at, |event| {
        eprintln!("{}", event.log_line());
    });
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
fn oscat_example_gate_reports_active_project_before_running_child() {
    use std::cell::RefCell;

    let projects = vec![
        PathBuf::from("/tmp/oscat-example-a"),
        PathBuf::from("/tmp/oscat-example-b"),
    ];
    let events = RefCell::new(Vec::new());

    assert_example_project_tests_pass_with(
        &projects,
        |project| {
            let expected_index = if project == projects[0] { 1 } else { 2 };
            assert_eq!(
                events.borrow().last(),
                Some(&ExampleGateEvent::Started {
                    index: expected_index,
                    total: projects.len(),
                    project: project.to_path_buf(),
                }),
                "OSCAT gate must report the active project before running the child command"
            );
            Ok(())
        },
        |event| events.borrow_mut().push(event),
    );

    assert_eq!(
        events.into_inner(),
        vec![
            ExampleGateEvent::Started {
                index: 1,
                total: 2,
                project: projects[0].clone(),
            },
            ExampleGateEvent::Passed {
                index: 1,
                total: 2,
                project: projects[0].clone(),
            },
            ExampleGateEvent::Started {
                index: 2,
                total: 2,
                project: projects[1].clone(),
            },
            ExampleGateEvent::Passed {
                index: 2,
                total: 2,
                project: projects[1].clone(),
            },
        ],
    );
}

#[test]
fn oscat_example_child_lines_include_pid_project_and_elapsed_context() {
    let project = PathBuf::from("/tmp/oscat-example-a");

    assert_eq!(
        example_child_started_line(42, &project),
        format!(
            "[oscat examples] child pid=42 command=trust-runtime test --project {} timeout={}s",
            project.display(),
            EXAMPLE_TEST_TIMEOUT.as_secs()
        )
    );
    assert_eq!(
        example_child_progress_line(42, &project, Duration::from_secs(31)),
        format!(
            "[oscat examples] child pid=42 still running elapsed=31s project={}",
            project.display()
        )
    );
    assert_eq!(
        example_child_timeout_line(42, &project, Duration::from_secs(121)),
        format!(
            "[oscat examples] child pid=42 timed out reason=timeout elapsed=121s timeout={}s project={}",
            EXAMPLE_TEST_TIMEOUT.as_secs(),
            project.display()
        )
    );
}

#[test]
fn oscat_airport_baggage_namespace_aggregate_trigger_passes() {
    let aggregate = write_oscat_namespace_aggregate_project(
        OSCAT_AGGREGATE_TRIGGER_EXAMPLE,
        OSCAT_AGGREGATE_TRIGGER_NAMESPACE,
    );
    run_example_st_tests_at(aggregate.path()).unwrap_or_else(|message| panic!("{message}"));
}

#[test]
fn oscat_aggregate_manifest_uses_toml_safe_dependency_paths() {
    let workspace_root = PathBuf::from(r"C:\Users\runneradmin\work\trust-platform");
    let manifest = aggregate_dependency_manifest(&workspace_root);
    let parsed: toml::Value =
        toml::from_str(&manifest).expect("aggregate dependency manifest must parse as TOML");

    assert_eq!(
        parsed["dependencies"]["OSCAT"]["path"].as_str(),
        Some("C:/Users/runneradmin/work/trust-platform/libraries/oscat")
    );
    assert_eq!(
        parsed["dependencies"]["OscatOop"]["path"].as_str(),
        Some("C:/Users/runneradmin/work/trust-platform/libraries/oscat/oop")
    );
}

#[test]
#[ignore = "expensive OSCAT gate runs all 98 paired example projects through trust-runtime CLI"]
fn oscat_oop_example_st_unit_tests_pass() {
    let projects = oscat_example_projects();
    assert_example_project_tests_pass(&projects);
}

#[test]
fn oscat_oop_examples_contain_claimed_pattern_structures() {
    for (name, needles) in STRUCTURAL_EXPECTATIONS {
        assert_pattern_structure(name, needles);
    }
}
