use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};

#[path = "control_curriculum_motion_progression/evidence.rs"]
#[allow(dead_code)]
mod evidence;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

fn write_or_compare_curriculum_artifact(path: &Path, expected: &str) {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("curriculum artifact parent"))
            .expect("create curriculum artifact directory");
        std::fs::write(path, expected).expect("write curriculum artifact");
        return;
    }
    let actual = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read retained artifact {}: {error}", path.display()));
    assert_eq!(
        actual,
        expected,
        "retained artifact drift: {}",
        path.display()
    );
}

fn project_file(project: &str, relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/03-production-quality-digital-pid")
        .join(match project {
            "booster-pump-modes" => "27-booster-pump-modes",
            "booster-pump-bumpless-transfer" => "28-booster-pump-bumpless-transfer",
            "booster-pump-atomic-update" => "29-booster-pump-atomic-update",
            "booster-pump-invalid-data" => "30-booster-pump-invalid-data",
            _ => panic!("unknown production PID lesson: {project}"),
        })
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn temporary_config_path(project: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-production-{project}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn lesson_source() -> String {
    project_file("booster-pump-modes", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "InitializeRequest : BOOL :=",
            "InitializeRequest AT %IX64.0 : BOOL :=",
        )
        .replace(
            "UseMeasuredInitialization : BOOL :=",
            "UseMeasuredInitialization AT %IX64.1 : BOOL :=",
        )
        .replace(
            "RequireSettledFilter : BOOL :=",
            "RequireSettledFilter AT %IX64.2 : BOOL :=",
        )
        .replace("RequestedMode : INT :=", "RequestedMode AT %IW66 : INT :=")
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL0 : LREAL :=")
        .replace("RawPressure : LREAL :=", "RawPressure AT %IL8 : LREAL :=")
        .replace(
            "ActualValvePosition : LREAL :=",
            "ActualValvePosition AT %IL16 : LREAL :=",
        )
        .replace(
            "ManualOutput : LREAL :=",
            "ManualOutput AT %IL24 : LREAL :=",
        )
        .replace(
            "TrackingOutput : LREAL :=",
            "TrackingOutput AT %IL32 : LREAL :=",
        )
        .replace("ValveCommand : LREAL :=", "ValveCommand AT %QL0 : LREAL :=")
        .replace(
            "FilteredPressure : LREAL :=",
            "FilteredPressure AT %ML0 : LREAL :=",
        )
        .replace(
            "ControllerReady : BOOL :=",
            "ControllerReady AT %MX16.0 : BOOL :=",
        )
        .replace(
            "FilterSettled : BOOL :=",
            "FilterSettled AT %MX16.1 : BOOL :=",
        )
        .replace(
            "StartupBlocked : BOOL :=",
            "StartupBlocked AT %MX16.2 : BOOL :=",
        )
        .replace(
            "ActiveAuthority : INT :=",
            "ActiveAuthority AT %MW18 : INT :=",
        )
}

fn build_runtime() -> trust_runtime::Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("booster-pump-modes/src/Main.st", lesson_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped booster-pump initialization lesson")
}

fn write_lreal(runtime: &mut trust_runtime::Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).unwrap(),
            Value::LWord(value.to_bits()),
        )
        .expect("write LREAL");
}

fn write_bool(runtime: &mut trust_runtime::Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(&IoAddress::parse(address).unwrap(), Value::Bool(value))
        .expect("write BOOL");
}

fn write_int(runtime: &mut trust_runtime::Runtime, address: &str, value: i16) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).unwrap(),
            Value::Word(value as u16),
        )
        .expect("write INT");
}

fn read_lreal(runtime: &trust_runtime::Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .expect("read LREAL")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn read_bool(runtime: &trust_runtime::Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .expect("read BOOL")
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

fn read_int(runtime: &trust_runtime::Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .expect("read INT")
    {
        Value::Word(value) => value as i16,
        value => panic!("expected INT, got {value:?}"),
    }
}

fn set_startup_inputs(
    runtime: &mut trust_runtime::Runtime,
    actual_valve: f64,
    measured_initialization: bool,
    require_settled_filter: bool,
) {
    let equilibrium_pressure = 2.0 + 0.1 * actual_valve;
    write_lreal(runtime, "%IL0", equilibrium_pressure);
    write_lreal(runtime, "%IL8", equilibrium_pressure);
    write_lreal(runtime, "%IL16", actual_valve);
    write_lreal(runtime, "%IL24", actual_valve);
    write_lreal(runtime, "%IL32", actual_valve);
    write_bool(runtime, "%IX64.0", true);
    write_bool(runtime, "%IX64.1", measured_initialization);
    write_bool(runtime, "%IX64.2", require_settled_filter);
    write_int(runtime, "%IW66", 2);
}

#[test]
fn control_curriculum_booster_pump_initializes_from_actual_state_before_auto() {
    let mut run = String::from(
        "phase,time_s,sp_bar,pv_raw_bar,pv_filtered_bar,mv_actual_percent,mv_requested_percent,mv_applied_percent,requested_mode,active_authority,controller_ready,filter_settled,startup_blocked,limit_state,evidence_class\n",
    );
    let mut actual_valves = Vec::new();
    let mut measured_requests = Vec::new();
    let mut raw_pressures = Vec::new();
    let mut initialized_pressures = Vec::new();
    for actual_valve in [20.0, 40.0, 70.0] {
        let mut runtime = build_runtime();
        set_startup_inputs(&mut runtime, actual_valve, true, true);
        runtime
            .execute_cycle()
            .expect("initialize from measured pump state");
        write_bool(&mut runtime, "%IX64.0", false);
        for _ in 0..2 {
            runtime
                .execute_cycle()
                .expect("qualify settled pressure filter");
        }

        let valve = read_lreal(&runtime, "%QL0");
        let pressure = 2.0 + 0.1 * actual_valve;
        assert!((read_lreal(&runtime, "%ML0") - pressure).abs() <= 1.0e-12);
        assert!(
            (valve - actual_valve).abs() <= 1.0e-12,
            "measured-state initialization at {actual_valve}% returned {valve}%"
        );
        assert!(read_bool(&runtime, "%MX16.0"));
        assert!(read_bool(&runtime, "%MX16.1"));
        assert!(!read_bool(&runtime, "%MX16.2"));
        assert_eq!(read_int(&runtime, "%MW18"), 2);
        actual_valves.push(actual_valve);
        measured_requests.push(valve);
        raw_pressures.push(pressure);
        initialized_pressures.push(read_lreal(&runtime, "%ML0"));
        run.push_str(&format!(
            "measured_{:.0},0.300,{pressure:.12},{pressure:.12},{:.12},{actual_valve:.12},{valve:.12},{valve:.12},2,2,1,1,0,none,configured_simulation\n",
            actual_valve,
            read_lreal(&runtime, "%ML0"),
        ));
    }

    let mut zero = build_runtime();
    set_startup_inputs(&mut zero, 40.0, false, false);
    zero.execute_cycle()
        .expect("execute deliberate zero-state baseline");
    let zero_valve = read_lreal(&zero, "%QL0");
    let zero_filter = read_lreal(&zero, "%ML0");
    assert!(
        (zero_valve - 40.0).abs() >= 10.0,
        "zero-state baseline must expose a material first-output mismatch: {zero_valve}"
    );
    assert!(
        (zero_filter - 6.0).abs() >= 1.0,
        "zero-state filter must expose an artificial startup transient: {zero_filter}"
    );
    run.push_str(&format!(
        "zero_state,0.100,6.000000000000,6.000000000000,{zero_filter:.12},40.000000000000,{zero_valve:.12},{zero_valve:.12},2,2,1,0,0,rate,configured_simulation\n",
    ));

    let mut disabled = build_runtime();
    set_startup_inputs(&mut disabled, 40.0, true, true);
    write_int(&mut disabled, "%IW66", 0);
    disabled
        .execute_cycle()
        .expect("execute station-level Disabled state");
    assert!((read_lreal(&disabled, "%QL0") - 30.0).abs() <= 1.0e-12);
    assert_eq!(read_int(&disabled, "%MW18"), 0);
    run.push_str("disabled,0.100,6.000000000000,6.000000000000,6.000000000000,40.000000000000,30.000000000000,30.000000000000,0,0,1,0,0,safe_output,configured_simulation\n");

    let mut manual = build_runtime();
    set_startup_inputs(&mut manual, 40.0, true, false);
    write_int(&mut manual, "%IW66", 1);
    write_lreal(&mut manual, "%IL24", 45.0);
    manual
        .execute_cycle()
        .expect("execute explicit Manual authority");
    assert!((read_lreal(&manual, "%QL0") - 45.0).abs() <= 1.0e-12);
    assert_eq!(read_int(&manual, "%MW18"), 1);
    run.push_str("manual,0.100,6.000000000000,6.000000000000,6.000000000000,40.000000000000,45.000000000000,45.000000000000,1,1,1,0,0,none,configured_simulation\n");

    let mut track = build_runtime();
    set_startup_inputs(&mut track, 40.0, true, false);
    write_int(&mut track, "%IW66", 3);
    write_lreal(&mut track, "%IL32", 45.0);
    track
        .execute_cycle()
        .expect("execute explicit Track authority");
    assert!((read_lreal(&track, "%QL0") - 45.0).abs() <= 1.0e-12);
    assert_eq!(read_int(&track, "%MW18"), 3);
    run.push_str("track,0.100,6.000000000000,6.000000000000,6.000000000000,40.000000000000,45.000000000000,45.000000000000,3,3,1,0,0,none,configured_simulation\n");

    let mut simulated = build_runtime();
    let config_path = temporary_config_path("booster-pump-modes");
    let simulation_text = project_file("booster-pump-modes", "simulation.toml").replace(
        "feedback_targets = [\"%IL0\"]",
        "feedback_targets = [\"%IL8\"]",
    );
    std::fs::write(&config_path, simulation_text).expect("write pump simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load pump simulation"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut simulated)
        .unwrap();
    write_lreal(&mut simulated, "%IL0", 6.0);
    write_lreal(&mut simulated, "%IL16", 40.0);
    write_lreal(&mut simulated, "%IL24", 40.0);
    write_lreal(&mut simulated, "%IL32", 40.0);
    write_bool(&mut simulated, "%IX64.0", true);
    write_bool(&mut simulated, "%IX64.1", true);
    write_bool(&mut simulated, "%IX64.2", true);
    write_int(&mut simulated, "%IW66", 2);
    for cycle in 1..=100 {
        simulated
            .execute_cycle()
            .expect("execute booster-pump controller");
        write_bool(&mut simulated, "%IX64.0", false);
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &simulated)
            .expect("advance booster pump");
        simulation
            .apply_pre_cycle(now, &mut simulated)
            .expect("publish pump pressure");
        let requested = read_lreal(&simulated, "%QL0");
        let raw = read_lreal(&simulated, "%IL8");
        run.push_str(&format!(
            "simulation,{:.3},6.000000000000,{raw:.12},{:.12},40.000000000000,{requested:.12},{requested:.12},2,{},1,{},{},none,configured_simulation\n",
            cycle as f64 * 0.1,
            read_lreal(&simulated, "%ML0"),
            read_int(&simulated, "%MW18"),
            if read_bool(&simulated, "%MX16.1") { 1 } else { 0 },
            if read_bool(&simulated, "%MX16.2") { 1 } else { 0 },
        ));
    }
    let _ = std::fs::remove_file(config_path);
    let final_pressure = read_lreal(&simulated, "%IL8");
    let final_valve = read_lreal(&simulated, "%QL0");
    println!(
        "measured startup valve states=20/40/70%, zero-state valve/filter={zero_valve:.3}%/{zero_filter:.3}bar, simulated final pressure/valve={final_pressure:.3}bar/{final_valve:.3}%"
    );
    assert!((final_pressure - 6.0).abs() <= 0.05);
    assert!((final_valve - 40.0).abs() <= 0.5);

    let summary = format!(
        "case,actual_valve_percent,first_request_percent,filtered_pressure_bar,decision\nzero-state baseline,40.0,{zero_valve:.12},{zero_filter:.12},reject artificial startup state\nmeasured state,20.0,{:.12},{:.12},accept\nmeasured state,40.0,{:.12},{:.12},accept\nmeasured state,70.0,{:.12},{:.12},accept\nsimulated equilibrium,40.0,{final_valve:.12},{final_pressure:.12},finite bounded plant confirmation\n",
        measured_requests[0], initialized_pressures[0], measured_requests[1], initialized_pressures[1], measured_requests[2], initialized_pressures[2]
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../examples/control/03-production-quality-digital-pid/27-booster-pump-modes/figures",
    );
    write_or_compare_curriculum_artifact(&artifact_root.join("booster-pump-startup-run.csv"), &run);
    evidence::retain_sequence(
        &artifact_root,
        "booster-pump-startup",
        "Measured-state initialization prevents a startup bump",
        "Configured Simulation evidence · identical pump; actual startup state changes",
        &[
            evidence::Panel {
                title: "Actual and first requested valve position",
                unit: "%",
                minimum: 0.0,
                maximum: 80.0,
                series: vec![
                    evidence::Series {
                        name: "actual valve",
                        color: "#555555",
                        dashed: true,
                        values: &actual_valves,
                    },
                    evidence::Series {
                        name: "measured-state request",
                        color: "#0072b2",
                        dashed: false,
                        values: &measured_requests,
                    },
                ],
            },
            evidence::Panel {
                title: "Raw and initialized pressure",
                unit: "bar",
                minimum: 0.0,
                maximum: 10.0,
                series: vec![
                    evidence::Series {
                        name: "raw pressure",
                        color: "#555555",
                        dashed: true,
                        values: &raw_pressures,
                    },
                    evidence::Series {
                        name: "initialized filter",
                        color: "#009e73",
                        dashed: false,
                        values: &initialized_pressures,
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );
}

fn handover_runtime() -> trust_runtime::Runtime {
    let source = project_file("booster-pump-bumpless-transfer", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "InitializeRequest : BOOL :=",
            "InitializeRequest AT %IX64.0 : BOOL :=",
        )
        .replace(
            "RequestedAuthority : INT :=",
            "RequestedAuthority AT %IW66 : INT :=",
        )
        .replace("    Setpoint : LREAL :=", "    Setpoint AT %IL0 : LREAL :=")
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL8 : LREAL :=",
        )
        .replace(
            "ActualValvePosition : LREAL :=",
            "ActualValvePosition AT %IL16 : LREAL :=",
        )
        .replace(
            "ExternalOutput : LREAL :=",
            "ExternalOutput AT %IL24 : LREAL :=",
        )
        .replace(
            "ManualOutput : LREAL :=",
            "ManualOutput AT %IL32 : LREAL :=",
        )
        .replace(
            "TrackingOutput : LREAL :=",
            "TrackingOutput AT %IL40 : LREAL :=",
        )
        .replace("ValveCommand : LREAL :=", "ValveCommand AT %QL0 : LREAL :=");
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("booster-pump-bumpless-transfer/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped booster-pump handover lesson")
}

fn initialize_handover(runtime: &mut trust_runtime::Runtime, authority: i16) {
    write_lreal(runtime, "%IL0", 6.0);
    write_lreal(runtime, "%IL8", 6.0);
    write_lreal(runtime, "%IL16", 40.0);
    write_lreal(runtime, "%IL24", 40.0);
    write_lreal(runtime, "%IL32", 40.0);
    write_lreal(runtime, "%IL40", 40.0);
    write_bool(runtime, "%IX64.0", true);
    write_int(runtime, "%IW66", authority);
    runtime
        .execute_cycle()
        .expect("initialize handover controller");
    write_bool(runtime, "%IX64.0", false);
}

#[test]
fn control_curriculum_booster_pump_tracking_makes_authority_transfer_bumpless() {
    let mut run = String::from(
        "phase,time_s,sp_bar,pv_bar,mv_external_applied_percent,mv_track_percent,mv_requested_percent,mv_applied_percent,requested_authority,active_authority,transition_step_pp,tracking_quality,limit_state,evidence_class\n",
    );
    let mut tracked = handover_runtime();
    initialize_handover(&mut tracked, 3);
    write_lreal(&mut tracked, "%IL16", 60.0);
    write_lreal(&mut tracked, "%IL24", 60.0);
    write_lreal(&mut tracked, "%IL40", 60.0);
    tracked
        .execute_cycle()
        .expect("track external applied output");
    let last_tracked_output = read_lreal(&tracked, "%QL0");
    write_int(&mut tracked, "%IW66", 2);
    tracked
        .execute_cycle()
        .expect("transfer tracked controller to Auto");
    let tracked_auto = read_lreal(&tracked, "%QL0");
    let tracked_step = (tracked_auto - 60.0).abs();

    let mut untracked = handover_runtime();
    initialize_handover(&mut untracked, 4);
    write_lreal(&mut untracked, "%IL16", 60.0);
    write_lreal(&mut untracked, "%IL24", 60.0);
    untracked
        .execute_cycle()
        .expect("leave controller inactive without tracking");
    assert!((read_lreal(&untracked, "%QL0") - 60.0).abs() <= 1.0e-12);
    write_int(&mut untracked, "%IW66", 2);
    untracked
        .execute_cycle()
        .expect("transfer stale controller to Auto");
    let untracked_auto = read_lreal(&untracked, "%QL0");
    let untracked_step = (untracked_auto - 60.0).abs();

    let mut unaligned_manual = handover_runtime();
    initialize_handover(&mut unaligned_manual, 2);
    let automatic_output = read_lreal(&unaligned_manual, "%QL0");
    write_lreal(&mut unaligned_manual, "%IL32", 10.0);
    write_int(&mut unaligned_manual, "%IW66", 1);
    unaligned_manual
        .execute_cycle()
        .expect("transfer to unrelated Manual value");
    let unaligned_manual_output = read_lreal(&unaligned_manual, "%QL0");
    let unaligned_step = (unaligned_manual_output - automatic_output).abs();

    let mut aligned_manual = handover_runtime();
    initialize_handover(&mut aligned_manual, 2);
    let automatic_output = read_lreal(&aligned_manual, "%QL0");
    write_lreal(&mut aligned_manual, "%IL32", automatic_output);
    write_int(&mut aligned_manual, "%IW66", 1);
    aligned_manual
        .execute_cycle()
        .expect("transfer to aligned Manual value");
    let aligned_manual_output = read_lreal(&aligned_manual, "%QL0");
    let aligned_step = (aligned_manual_output - automatic_output).abs();

    let mut stale_tracking = handover_runtime();
    initialize_handover(&mut stale_tracking, 3);
    write_lreal(&mut stale_tracking, "%IL16", 60.0);
    write_lreal(&mut stale_tracking, "%IL24", 60.0);
    write_lreal(&mut stale_tracking, "%IL40", 40.0);
    stale_tracking
        .execute_cycle()
        .expect("apply deliberately stale tracking value");
    write_int(&mut stale_tracking, "%IW66", 2);
    stale_tracking
        .execute_cycle()
        .expect("expose infeasible stale-feedback handover");
    let stale_auto = read_lreal(&stale_tracking, "%QL0");
    let stale_step = (stale_auto - 60.0).abs();
    assert!(
        stale_step >= 20.0 - 1.0e-12,
        "stale applied-output evidence must make exact transfer infeasible"
    );

    let config_path = temporary_config_path("booster-pump-bumpless-transfer");
    let simulation_text = project_file("booster-pump-bumpless-transfer", "simulation.toml")
        .replace(
            "feedback_targets = [\"%IL0\"]",
            "feedback_targets = [\"%IL8\"]",
        );
    std::fs::write(&config_path, simulation_text).expect("write handover simulation config");
    let mut recovery_runtime = handover_runtime();
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load handover pump simulation"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut recovery_runtime)
        .expect("publish initial handover pressure");
    initialize_handover(&mut recovery_runtime, 3);
    write_lreal(&mut recovery_runtime, "%IL16", 60.0);
    write_lreal(&mut recovery_runtime, "%IL24", 60.0);
    write_lreal(&mut recovery_runtime, "%IL40", 60.0);
    recovery_runtime
        .execute_cycle()
        .expect("track applied output before recovery run");
    write_int(&mut recovery_runtime, "%IW66", 2);
    for cycle in 1..=100 {
        recovery_runtime
            .execute_cycle()
            .expect("execute post-transfer pressure recovery");
        let requested = read_lreal(&recovery_runtime, "%QL0");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &recovery_runtime)
            .expect("advance recovery pump");
        simulation
            .apply_pre_cycle(now, &mut recovery_runtime)
            .expect("publish recovery pressure");
        write_lreal(&mut recovery_runtime, "%IL16", requested);
        run.push_str(&format!(
            "recovery,{:.3},6.000000000000,{:.12},60.000000000000,60.000000000000,{requested:.12},{requested:.12},2,2,0.000000000000,good,none,configured_simulation\n",
            cycle as f64 * 0.1,
            read_lreal(&recovery_runtime, "%IL8"),
        ));
    }
    let recovery_pressure = read_lreal(&recovery_runtime, "%IL8");
    assert!(recovery_pressure.is_finite() && (0.0..=12.0).contains(&recovery_pressure));
    let _ = std::fs::remove_file(config_path);

    println!(
        "tracked last/Auto={last_tracked_output:.3}/{tracked_auto:.3}% step={tracked_step:.3}pp, untracked Auto={untracked_auto:.3}% step={untracked_step:.3}pp, Auto-to-Manual unaligned/aligned={unaligned_step:.3}/{aligned_step:.3}pp, stale tracking step={stale_step:.3}pp"
    );
    assert!(tracked_step <= 1.0e-12);
    assert!((last_tracked_output - 60.0).abs() <= 1.0e-12);
    assert!(untracked_step >= 20.0 - 1.0e-12);
    assert!(unaligned_step >= 30.0 - 1.0e-12);
    assert!(aligned_step <= 1.0e-12);

    run.insert_str(run.find("recovery,").unwrap_or(run.len()), &format!(
        "tracked_to_auto,0.000,6.000000000000,6.000000000000,60.000000000000,60.000000000000,{tracked_auto:.12},{tracked_auto:.12},2,2,{tracked_step:.12},good,none,configured_simulation\n\
untracked_to_auto,0.000,6.000000000000,6.000000000000,60.000000000000,,{untracked_auto:.12},{untracked_auto:.12},2,2,{untracked_step:.12},unavailable,none,configured_simulation\n\
unaligned_to_manual,0.000,6.000000000000,6.000000000000,40.000000000000,,{unaligned_manual_output:.12},{unaligned_manual_output:.12},1,1,{unaligned_step:.12},not_applicable,none,configured_simulation\n\
aligned_to_manual,0.000,6.000000000000,6.000000000000,40.000000000000,,{aligned_manual_output:.12},{aligned_manual_output:.12},1,1,{aligned_step:.12},not_applicable,none,configured_simulation\n\
stale_tracking_infeasible,0.000,6.000000000000,6.000000000000,60.000000000000,40.000000000000,{stale_auto:.12},{stale_auto:.12},2,2,{stale_step:.12},stale,none,configured_simulation\n"
    ));
    let last_applied = [60.0, 60.0, 40.0, 40.0, 60.0];
    let first_request = [
        tracked_auto,
        untracked_auto,
        unaligned_manual_output,
        aligned_manual_output,
        stale_auto,
    ];
    let transition_step = [
        tracked_step,
        untracked_step,
        unaligned_step,
        aligned_step,
        stale_step,
    ];
    let summary = format!(
        "transition,last_applied_percent,first_request_percent,output_step_pp,decision\ntracked external to Auto,60.0,{tracked_auto:.12},{tracked_step:.12},accept aligned controller state\nuntracked external to Auto,60.0,{untracked_auto:.12},{untracked_step:.12},reject stale controller state\nunaligned Auto to Manual,40.0,{unaligned_manual_output:.12},{unaligned_step:.12},reject unrelated manual command\naligned Auto to Manual,40.0,{aligned_manual_output:.12},{aligned_step:.12},accept external station alignment\nstale tracking boundary,60.0,{stale_auto:.12},{stale_step:.12},infeasible because applied-output evidence is stale\npost-transition recovery,,{:.12},,finite bounded pressure recovery\n",
        recovery_pressure,
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/03-production-quality-digital-pid/28-booster-pump-bumpless-transfer/figures");
    write_or_compare_curriculum_artifact(&artifact_root.join("bumpless-transfer-run.csv"), &run);
    evidence::retain_sequence(
        &artifact_root,
        "bumpless-transfer",
        "Tracking aligns authority transfer",
        "Configured Simulation evidence · same pump state; authority alignment changes",
        &[
            evidence::Panel {
                title: "Last applied and first requested valve output",
                unit: "%",
                minimum: 0.0,
                maximum: 70.0,
                series: vec![
                    evidence::Series {
                        name: "last applied",
                        color: "#555555",
                        dashed: true,
                        values: &last_applied,
                    },
                    evidence::Series {
                        name: "first new request",
                        color: "#0072b2",
                        dashed: false,
                        values: &first_request,
                    },
                ],
            },
            evidence::Panel {
                title: "Authority-transition output step",
                unit: "pp",
                minimum: 0.0,
                maximum: 35.0,
                series: vec![evidence::Series {
                    name: "output step",
                    color: "#d55e00",
                    dashed: false,
                    values: &transition_step,
                }],
            },
        ],
        &[],
        &[],
        &summary,
    );
}

fn atomic_update_runtime() -> trust_runtime::Runtime {
    let source = project_file("booster-pump-atomic-update", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "InitializeRequest : BOOL :=",
            "InitializeRequest AT %IX100.0 : BOOL :=",
        )
        .replace(
            "ApplyConfiguration : BOOL :=",
            "ApplyConfiguration AT %IX100.1 : BOOL :=",
        )
        .replace(
            "UnsafeSequentialUpdate : BOOL :=",
            "UnsafeSequentialUpdate AT %IX100.2 : BOOL :=",
        )
        .replace("    Setpoint : LREAL :=", "    Setpoint AT %IL0 : LREAL :=")
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL8 : LREAL :=",
        )
        .replace(
            "ActualValvePosition : LREAL :=",
            "ActualValvePosition AT %IL16 : LREAL :=",
        )
        .replace(
            "CandidateProportionalGain : LREAL :=",
            "CandidateProportionalGain AT %IL24 : LREAL :=",
        )
        .replace(
            "CandidateIntegralGain : LREAL :=",
            "CandidateIntegralGain AT %IL32 : LREAL :=",
        )
        .replace(
            "CandidateTrackingTime : LREAL :=",
            "CandidateTrackingTime AT %IL40 : LREAL :=",
        )
        .replace(
            "    FeedForward : LREAL :=",
            "    FeedForward AT %IL48 : LREAL :=",
        )
        .replace(
            "RateLimitPerSecond : LREAL :=",
            "RateLimitPerSecond AT %IL56 : LREAL :=",
        )
        .replace("ValveCommand : LREAL :=", "ValveCommand AT %QL0 : LREAL :=")
        .replace(
            "ActiveProportionalGain : LREAL :=",
            "ActiveProportionalGain AT %ML0 : LREAL :=",
        )
        .replace(
            "ActiveIntegralGain : LREAL :=",
            "ActiveIntegralGain AT %ML8 : LREAL :=",
        )
        .replace(
            "ActiveTrackingTime : LREAL :=",
            "ActiveTrackingTime AT %ML16 : LREAL :=",
        )
        .replace(
            "RejectedExternalSum : LREAL :=",
            "RejectedExternalSum AT %ML24 : LREAL :=",
        )
        .replace(
            "UnconstrainedTargetDiagnostic : LREAL :=",
            "UnconstrainedTargetDiagnostic AT %ML40 : LREAL :=",
        )
        .replace(
            "RateLowerBound : LREAL :=",
            "RateLowerBound AT %ML48 : LREAL :=",
        )
        .replace(
            "RateUpperBound : LREAL :=",
            "RateUpperBound AT %ML56 : LREAL :=",
        )
        .replace(
            "ActiveRevision : INT :=",
            "ActiveRevision AT %MW32 : INT :=",
        )
        .replace(
            "MixedConfigurationObserved : BOOL :=",
            "MixedConfigurationObserved AT %MX34.0 : BOOL :=",
        )
        .replace(
            "ConfiguredThisScan : BOOL :=",
            "ConfiguredThisScan AT %MX34.1 : BOOL :=",
        )
        .replace("RateLimited : BOOL :=", "RateLimited AT %MX34.2 : BOOL :=")
        .replace(
            "AmplitudeLimited : BOOL :=",
            "AmplitudeLimited AT %MX34.3 : BOOL :=",
        );
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("booster-pump-atomic-update/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped atomic-update lesson")
}

fn initialize_atomic_case(runtime: &mut trust_runtime::Runtime, rate_limit: f64) {
    write_lreal(runtime, "%IL0", 6.0);
    write_lreal(runtime, "%IL8", 6.0);
    write_lreal(runtime, "%IL16", 40.0);
    write_lreal(runtime, "%IL24", 15.0);
    write_lreal(runtime, "%IL32", 3.0);
    write_lreal(runtime, "%IL40", 0.5);
    write_lreal(runtime, "%IL48", 0.0);
    write_lreal(runtime, "%IL56", rate_limit);
    write_bool(runtime, "%IX100.0", true);
    write_bool(runtime, "%IX100.1", false);
    write_bool(runtime, "%IX100.2", false);
    runtime
        .execute_cycle()
        .expect("initialize atomic-update controller");
    write_bool(runtime, "%IX100.0", false);
}

#[test]
fn control_curriculum_booster_pump_applies_atomic_parameters_and_orders_feedforward() {
    let mut run = String::from(
        "phase,scan,sp_bar,pv_bar,candidate_kp_percent_per_bar,candidate_ki_percent_per_bar_s,candidate_tracking_s,active_kp_percent_per_bar,active_ki_percent_per_bar_s,active_tracking_s,active_revision,configured_this_scan,mixed_revision,feedforward_percent,unconstrained_target_diagnostic_percent,previous_applied_percent,rate_lower_percent,rate_upper_percent,mv_applied_percent,rate_limited,amplitude_limited,limit_state,evidence_class\n",
    );
    let mut atomic = atomic_update_runtime();
    initialize_atomic_case(&mut atomic, 1000.0);
    write_lreal(&mut atomic, "%IL0", 7.0);
    write_bool(&mut atomic, "%IX100.1", true);
    atomic
        .execute_cycle()
        .expect("apply coherent parameter record");
    let atomic_output = read_lreal(&atomic, "%QL0");
    let atomic_diagnostic = read_lreal(&atomic, "%ML40");
    assert!((read_lreal(&atomic, "%ML0") - 15.0).abs() <= 1.0e-12);
    assert!((read_lreal(&atomic, "%ML8") - 3.0).abs() <= 1.0e-12);
    assert!((read_lreal(&atomic, "%ML16") - 0.5).abs() <= 1.0e-12);
    assert_eq!(read_int(&atomic, "%MW32"), 1);
    assert!(!read_bool(&atomic, "%MX34.0"));
    assert!(
        (atomic_output - 55.3).abs() <= 1.0e-12,
        "coherent first update was {atomic_output}"
    );
    assert!(
        (atomic_diagnostic - 55.3).abs() <= 1.0e-12,
        "coherent unconstrained target diagnostic was {atomic_diagnostic}"
    );
    run.push_str(&format!(
        "atomic_revision,1,7.000000000000,6.000000000000,15.000000000000,3.000000000000,0.500000000000,15.000000000000,3.000000000000,0.500000000000,1,1,0,0.000000000000,{atomic_diagnostic:.12},40.000000000000,0.000000000000,100.000000000000,{atomic_output:.12},0,0,none,configured_simulation\n"
    ));

    let mut mixed = atomic_update_runtime();
    initialize_atomic_case(&mut mixed, 1000.0);
    write_lreal(&mut mixed, "%IL0", 7.0);
    write_bool(&mut mixed, "%IX100.1", true);
    write_bool(&mut mixed, "%IX100.2", true);
    mixed
        .execute_cycle()
        .expect("execute rejected sequential parameter stage");
    let mixed_output = read_lreal(&mixed, "%QL0");
    assert!((read_lreal(&mixed, "%ML0") - 15.0).abs() <= 1.0e-12);
    assert!((read_lreal(&mixed, "%ML8") - 1.0).abs() <= 1.0e-12);
    assert!((read_lreal(&mixed, "%ML16") - 1.0).abs() <= 1.0e-12);
    assert!(read_bool(&mixed, "%MX34.0"));
    assert!(
        (mixed_output - 55.1).abs() <= 1.0e-12,
        "mixed first update was {mixed_output}"
    );
    run.push_str(&format!(
        "mixed_revision,1,7.000000000000,6.000000000000,15.000000000000,3.000000000000,0.500000000000,15.000000000000,1.000000000000,1.000000000000,0,1,1,0.000000000000,55.100000000000,40.000000000000,0.000000000000,100.000000000000,{mixed_output:.12},0,0,invalid_mixed_record,configured_simulation\n"
    ));

    let mut feedforward = atomic_update_runtime();
    initialize_atomic_case(&mut feedforward, 20.0);
    write_lreal(&mut feedforward, "%IL48", 20.0);
    let mut previous_outputs = Vec::new();
    let mut lower_bounds = Vec::new();
    let mut upper_bounds = Vec::new();
    let mut applied_outputs = Vec::new();
    let mut unconstrained_targets = Vec::new();
    let mut rejected_sums = Vec::new();
    let mut previous: f64 = 40.0;
    for scan in 1..=6 {
        feedforward
            .execute_cycle()
            .expect("continue constraint-aware feedforward scans");
        let applied = read_lreal(&feedforward, "%QL0");
        let lower = read_lreal(&feedforward, "%ML48");
        let upper = read_lreal(&feedforward, "%ML56");
        let diagnostic = read_lreal(&feedforward, "%ML40");
        let rejected = read_lreal(&feedforward, "%ML24");
        assert!(
            (diagnostic - 60.0).abs() <= 1.0e-12,
            "scan {scan} unconstrained target diagnostic was {diagnostic}, expected 60"
        );
        assert!(read_bool(&feedforward, "%MX34.2"),
            "scan {scan} must report the active rate constraint while the 60% target exceeds its bound");
        assert!(
            (lower - (previous - 2.0).max(0.0)).abs() <= 1.0e-12,
            "scan {scan} lower rate bound did not use prior applied output"
        );
        assert!(
            (upper - (previous + 2.0).min(100.0)).abs() <= 1.0e-12,
            "scan {scan} upper rate bound did not use prior applied output"
        );
        assert!(
            applied >= lower - 1.0e-12 && applied <= upper + 1.0e-12,
            "scan {scan} output escaped its rate interval"
        );
        previous_outputs.push(previous);
        lower_bounds.push(lower);
        upper_bounds.push(upper);
        applied_outputs.push(applied);
        unconstrained_targets.push(diagnostic);
        rejected_sums.push(rejected);
        run.push_str(&format!(
            "feedforward_scan_{scan},{scan},6.000000000000,6.000000000000,15.000000000000,3.000000000000,0.500000000000,5.000000000000,1.000000000000,1.000000000000,0,0,0,20.000000000000,{diagnostic:.12},{previous:.12},{lower:.12},{upper:.12},{applied:.12},{},{},rate_checked,configured_simulation\n",
            if read_bool(&feedforward, "%MX34.2") { 1 } else { 0 },
            if read_bool(&feedforward, "%MX34.3") { 1 } else { 0 },
        ));
        previous = applied;
    }
    let constrained_output = applied_outputs[0];
    let rejected_sum = rejected_sums[0];
    assert!((constrained_output - 42.0).abs() <= 1.0e-12);
    assert!((rejected_sum - 60.0).abs() <= 1.0e-12);

    let config_path = temporary_config_path("booster-pump-atomic-update");
    std::fs::write(
        &config_path,
        project_file("booster-pump-atomic-update", "simulation.toml"),
    )
    .expect("write atomic-update simulation config");
    SimulationConfig::load(&config_path).expect("load atomic-update pump simulation");
    let _ = std::fs::remove_file(config_path);

    println!(
        "atomic/mixed first output={atomic_output:.3}/{mixed_output:.3}%, constrained/rejected external sum={constrained_output:.3}/{rejected_sum:.3}%"
    );

    let summary = format!(
        "case,active_kp_percent_per_bar,active_ki_percent_per_bar_s,result_percent,accepted,decision\natomic revision,15.0,3.0,{atomic_output:.12},true,coherent revision 1\nmixed diagnostic,15.0,1.0,{mixed_output:.12},false,reject mixed record\ninternal feedforward,5.0,1.0,{constrained_output:.12},true,rate constrained inside PID\nrejected external sum,,,{rejected_sum:.12},false,reject post-limit sum\n"
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/03-production-quality-digital-pid/29-booster-pump-atomic-update/figures");
    write_or_compare_curriculum_artifact(&artifact_root.join("atomic-update-run.csv"), &run);
    evidence::retain(
        &artifact_root, "atomic-update", "Every rate interval starts at the prior applied output",
        "Configured Simulation evidence · 100 ms scans; internal feedforward stays inside constraints",
        0.1,
        &[
            evidence::Panel { title: "Previous, permitted and applied valve output", unit: "%", minimum: 35.0, maximum: 65.0, series: vec![
                evidence::Series { name: "previous applied", color: "#555555", dashed: true, values: &previous_outputs },
                evidence::Series { name: "rate lower bound", color: "#d55e00", dashed: true, values: &lower_bounds },
                evidence::Series { name: "rate upper bound", color: "#009e73", dashed: true, values: &upper_bounds },
                evidence::Series { name: "applied output", color: "#0072b2", dashed: false, values: &applied_outputs },
            ]},
            evidence::Panel { title: "Independent target diagnostic and rejected external sum", unit: "%", minimum: 35.0, maximum: 65.0, series: vec![
                evidence::Series { name: "unconstrained target diagnostic", color: "#0072b2", dashed: false, values: &unconstrained_targets },
                evidence::Series { name: "rejected post-limit sum", color: "#d55e00", dashed: true, values: &rejected_sums },
            ]},
        ], &[], &[], &summary,
    );
}

fn invalid_data_runtime() -> trust_runtime::Runtime {
    let source = project_file("booster-pump-invalid-data", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "InitializeRequest : BOOL :=",
            "InitializeRequest AT %IX120.0 : BOOL :=",
        )
        .replace(
            "ApplyConfiguration : BOOL :=",
            "ApplyConfiguration AT %IX120.1 : BOOL :=",
        )
        .replace(
            "SensorQualityGood : BOOL :=",
            "SensorQualityGood AT %IX120.2 : BOOL :=",
        )
        .replace(
            "DeadlineMissed : BOOL :=",
            "DeadlineMissed AT %IX120.3 : BOOL :=",
        )
        .replace(
            "AcknowledgeRequest : BOOL :=",
            "AcknowledgeRequest AT %IX120.4 : BOOL :=",
        )
        .replace(
            "ResetRequest : BOOL :=",
            "ResetRequest AT %IX120.5 : BOOL :=",
        )
        .replace(
            "EnableAutoRequest : BOOL :=",
            "EnableAutoRequest AT %IX120.6 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL0 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL8 : LREAL :=")
        .replace(
            "ActualValvePosition : LREAL :=",
            "ActualValvePosition AT %IL16 : LREAL :=",
        )
        .replace("FeedForward : LREAL :=", "FeedForward AT %IL24 : LREAL :=")
        .replace(
            "SensorAgeSeconds : LREAL :=",
            "SensorAgeSeconds AT %IL32 : LREAL :=",
        )
        .replace(
            "MeasuredCycleTime : LREAL :=",
            "MeasuredCycleTime AT %IL40 : LREAL :=",
        )
        .replace(
            "CandidateOutputMinimum : LREAL :=",
            "CandidateOutputMinimum AT %IL48 : LREAL :=",
        )
        .replace(
            "CandidateOutputMaximum : LREAL :=",
            "CandidateOutputMaximum AT %IL56 : LREAL :=",
        )
        .replace(
            "CandidateProportionalGain : LREAL :=",
            "CandidateProportionalGain AT %IL64 : LREAL :=",
        )
        .replace(
            "CandidateIntegralGain : LREAL :=",
            "CandidateIntegralGain AT %IL72 : LREAL :=",
        )
        .replace(
            "CandidateTrackingTime : LREAL :=",
            "CandidateTrackingTime AT %IL80 : LREAL :=",
        )
        .replace("ValveCommand : LREAL :=", "ValveCommand AT %QL0 : LREAL :=")
        .replace("FaultCode : INT :=", "FaultCode AT %MW0 : INT :=")
        .replace("ActiveRevision : INT :=", "ActiveRevision AT %MW2 : INT :=")
        .replace(
            "RecoveryValidCount : INT :=",
            "RecoveryValidCount AT %MW4 : INT :=",
        )
        .replace("FaultLatched : BOOL :=", "FaultLatched AT %MX6.0 : BOOL :=")
        .replace(
            "FaultAcknowledged : BOOL :=",
            "FaultAcknowledged AT %MX6.1 : BOOL :=",
        )
        .replace(
            "RecoveryReady : BOOL :=",
            "RecoveryReady AT %MX6.2 : BOOL :=",
        )
        .replace(
            "RecoveryComplete : BOOL :=",
            "RecoveryComplete AT %MX6.3 : BOOL :=",
        )
        .replace(
            "ControllerCalledThisScan : BOOL :=",
            "ControllerCalledThisScan AT %MX6.4 : BOOL :=",
        )
        .replace(
            "ConfiguredThisScan : BOOL :=",
            "ConfiguredThisScan AT %MX6.5 : BOOL :=",
        )
        .replace(
            "AutoAuthorized : BOOL :=",
            "AutoAuthorized AT %MX6.6 : BOOL :=",
        );
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("booster-pump-invalid-data/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped invalid-data lesson")
}

fn initialize_valid_guard(runtime: &mut trust_runtime::Runtime) {
    write_lreal(runtime, "%IL0", 6.0);
    write_lreal(runtime, "%IL8", 6.0);
    write_lreal(runtime, "%IL16", 40.0);
    write_lreal(runtime, "%IL24", 0.0);
    write_lreal(runtime, "%IL32", 0.0);
    write_lreal(runtime, "%IL40", 0.1);
    write_lreal(runtime, "%IL48", 0.0);
    write_lreal(runtime, "%IL56", 100.0);
    write_lreal(runtime, "%IL64", 5.0);
    write_lreal(runtime, "%IL72", 1.0);
    write_lreal(runtime, "%IL80", 1.0);
    write_bool(runtime, "%IX120.0", true);
    write_bool(runtime, "%IX120.1", false);
    write_bool(runtime, "%IX120.2", true);
    write_bool(runtime, "%IX120.3", false);
    write_bool(runtime, "%IX120.4", false);
    write_bool(runtime, "%IX120.5", false);
    write_bool(runtime, "%IX120.6", true);
    runtime
        .execute_cycle()
        .expect("initialize guarded controller");
    write_bool(runtime, "%IX120.0", false);
    assert!((read_lreal(runtime, "%QL0") - 40.0).abs() <= 1.0e-12);
    assert_eq!(read_int(runtime, "%MW0"), 0);
}

fn assert_guard_fault(
    mut runtime: trust_runtime::Runtime,
    inject: impl FnOnce(&mut trust_runtime::Runtime),
    expected_code: i16,
) -> (f64, f64, f64) {
    initialize_valid_guard(&mut runtime);
    inject(&mut runtime);
    runtime
        .execute_cycle()
        .expect("execute invalid guarded scan");
    assert_eq!(read_int(&runtime, "%MW0"), expected_code);
    assert!(read_bool(&runtime, "%MX6.0"));
    assert!(!read_bool(&runtime, "%MX6.4"));
    assert!(!read_bool(&runtime, "%MX6.5"));
    assert!((read_lreal(&runtime, "%QL0") - 30.0).abs() <= 1.0e-12);
    (
        read_int(&runtime, "%MW0") as f64,
        read_lreal(&runtime, "%QL0"),
        f64::from(read_bool(&runtime, "%MX6.4")) + f64::from(read_bool(&runtime, "%MX6.5")),
    )
}

fn guard_run_row(phase: &str, scan: usize, runtime: &trust_runtime::Runtime) -> String {
    let candidate_valid = read_lreal(runtime, "%IL48") < read_lreal(runtime, "%IL56");
    let bit = |address| if read_bool(runtime, address) { 1 } else { 0 };
    format!(
        "{phase},{scan},{sp:.12},{pv:.12},{quality},{age:.12},{cycle:.12},{deadline},{valid},{revision},{fault},{latched},{acknowledged},{count},{ready},{configured},{called},{auto},{complete},{output:.12},configured_simulation\n",
        sp = read_lreal(runtime, "%IL0"),
        pv = read_lreal(runtime, "%IL8"),
        quality = bit("%IX120.2"),
        age = read_lreal(runtime, "%IL32"),
        cycle = read_lreal(runtime, "%IL40"),
        deadline = bit("%IX120.3"),
        valid = if candidate_valid { 1 } else { 0 },
        revision = read_int(runtime, "%MW2"),
        fault = read_int(runtime, "%MW0"),
        latched = bit("%MX6.0"),
        acknowledged = bit("%MX6.1"),
        count = read_int(runtime, "%MW4"),
        ready = bit("%MX6.2"),
        configured = bit("%MX6.5"),
        called = bit("%MX6.4"),
        auto = bit("%MX6.6"),
        complete = bit("%MX6.3"),
        output = read_lreal(runtime, "%QL0"),
    )
}

#[test]
fn control_curriculum_booster_pump_rejects_invalid_scans_and_requires_qualified_recovery() {
    let mut recovery_run = String::from(
        "phase,scan,sp_bar,pv_bar,sensor_quality,sensor_age_s,cycle_time_s,deadline_missed,candidate_valid,active_revision,fault_code,fault_latched,fault_acknowledged,recovery_valid_count,recovery_ready,configured_this_scan,controller_called_this_scan,auto_authorized,recovery_complete,mv_applied_percent,evidence_class\n",
    );
    let mut nonfinite = invalid_data_runtime();
    initialize_valid_guard(&mut nonfinite);
    write_lreal(&mut nonfinite, "%IL8", f64::NAN);
    let nonfinite_error = nonfinite
        .execute_cycle()
        .expect_err("typed I/O boundary must reject nonfinite pressure before PLC arithmetic");
    assert!(
        nonfinite_error.to_string().contains("must be finite"),
        "unexpected nonfinite rejection: {nonfinite_error}"
    );
    assert!((read_lreal(&nonfinite, "%QL0") - 40.0).abs() <= 1.0e-12);

    let range_fault = assert_guard_fault(
        invalid_data_runtime(),
        |runtime| write_lreal(runtime, "%IL8", 13.0),
        1,
    );
    let stale_fault = assert_guard_fault(
        invalid_data_runtime(),
        |runtime| write_lreal(runtime, "%IL32", 0.6),
        2,
    );
    let timing_fault = assert_guard_fault(
        invalid_data_runtime(),
        |runtime| write_lreal(runtime, "%IL40", 0.0),
        3,
    );
    let deadline_fault = assert_guard_fault(
        invalid_data_runtime(),
        |runtime| write_bool(runtime, "%IX120.3", true),
        4,
    );
    recovery_run.push_str(&format!(
        "physical_range_fault,1,6.000000000000,13.000000000000,1,0.000000000000,0.100000000000,0,1,0,{:.0},1,0,0,0,0,0,0,0,{:.12},configured_simulation\n\
stale_fault,1,6.000000000000,6.000000000000,1,0.600000000000,0.100000000000,0,1,0,{:.0},1,0,0,0,0,0,0,0,{:.12},configured_simulation\n\
timing_fault,1,6.000000000000,6.000000000000,1,0.000000000000,0.000000000000,0,1,0,{:.0},1,0,0,0,0,0,0,0,{:.12},configured_simulation\n\
deadline_fault,1,6.000000000000,6.000000000000,1,0.000000000000,0.100000000000,1,1,0,{:.0},1,0,0,0,0,0,0,0,{:.12},configured_simulation\n",
        range_fault.0, range_fault.1,
        stale_fault.0, stale_fault.1,
        timing_fault.0, timing_fault.1,
        deadline_fault.0, deadline_fault.1,
    ));

    let mut recovery = invalid_data_runtime();
    initialize_valid_guard(&mut recovery);
    write_lreal(&mut recovery, "%IL48", 80.0);
    write_lreal(&mut recovery, "%IL56", 20.0);
    write_bool(&mut recovery, "%IX120.1", true);
    recovery
        .execute_cycle()
        .expect("reject reversed candidate limits");
    assert_eq!(read_int(&recovery, "%MW0"), 5);
    assert_eq!(
        read_int(&recovery, "%MW2"),
        0,
        "invalid record changed revision"
    );
    assert!(!read_bool(&recovery, "%MX6.4"));
    assert!(!read_bool(&recovery, "%MX6.5"));
    assert!((read_lreal(&recovery, "%QL0") - 30.0).abs() <= 1.0e-12);
    recovery_run.push_str(&guard_run_row("configuration_fault", 1, &recovery));

    write_bool(&mut recovery, "%IX120.1", false);
    write_lreal(&mut recovery, "%IL48", 0.0);
    write_lreal(&mut recovery, "%IL56", 100.0);
    write_lreal(&mut recovery, "%IL16", 30.0);
    recovery
        .execute_cycle()
        .expect("valid data cannot clear unacknowledged latch");
    assert!(read_bool(&recovery, "%MX6.0"));
    assert!(!read_bool(&recovery, "%MX6.1"));
    assert_eq!(
        read_int(&recovery, "%MW4"),
        0,
        "recovery qualification must not begin before acknowledgement"
    );
    recovery_run.push_str(&guard_run_row("valid_unacknowledged", 2, &recovery));

    write_bool(&mut recovery, "%IX120.4", true);
    recovery
        .execute_cycle()
        .expect("acknowledge without clearing fault");
    write_bool(&mut recovery, "%IX120.4", false);
    assert!(read_bool(&recovery, "%MX6.0"));
    assert!(read_bool(&recovery, "%MX6.1"));
    assert_eq!(
        read_int(&recovery, "%MW4"),
        0,
        "the acknowledgement scan is not one of the three later qualification scans"
    );
    recovery_run.push_str(&guard_run_row("acknowledged", 3, &recovery));
    for expected_count in 1..=3 {
        recovery.execute_cycle().expect("qualify recovery input");
        assert_eq!(read_int(&recovery, "%MW4"), expected_count);
        recovery_run.push_str(&guard_run_row(
            &format!("valid_{expected_count}"),
            3 + expected_count as usize,
            &recovery,
        ));
    }
    assert!(read_bool(&recovery, "%MX6.2"));

    write_bool(&mut recovery, "%IX120.6", false);
    write_bool(&mut recovery, "%IX120.5", true);
    recovery
        .execute_cycle()
        .expect("reset from present measured state");
    write_bool(&mut recovery, "%IX120.5", false);
    assert!(!read_bool(&recovery, "%MX6.0"));
    assert_eq!(read_int(&recovery, "%MW0"), 0);
    assert!(read_bool(&recovery, "%MX6.3"));
    assert!(!read_bool(&recovery, "%MX6.6"));
    assert!((read_lreal(&recovery, "%QL0") - 30.0).abs() <= 1.0e-12);
    recovery_run.push_str(&guard_run_row("reset", 7, &recovery));

    write_bool(&mut recovery, "%IX120.6", true);
    recovery
        .execute_cycle()
        .expect("grant renewed Auto authority");
    assert!(read_bool(&recovery, "%MX6.6"));
    assert!(read_bool(&recovery, "%MX6.4"));
    let recovered_output = read_lreal(&recovery, "%QL0");
    assert!((recovered_output - 30.0).abs() <= 1.0e-12);
    recovery_run.push_str(&guard_run_row("renewed_auto", 8, &recovery));

    let config_path = temporary_config_path("booster-pump-invalid-data");
    std::fs::write(
        &config_path,
        project_file("booster-pump-invalid-data", "simulation.toml"),
    )
    .expect("write invalid-data simulation config");
    SimulationConfig::load(&config_path).expect("load invalid-data pump simulation");
    let _ = std::fs::remove_file(config_path);

    println!(
        "fault codes numeric/stale/dt/deadline/config=1/2/3/4/5, safe/recovered output=30.000/{recovered_output:.3}%"
    );

    let fault_code = [5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 0.0, 0.0];
    let recovery_count = [0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 0.0];
    let safe_output = [30.0; 8];
    let configured_calls = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let controller_calls = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let summary = format!(
        "phase,fault_code,configure_calls,pid_calls,applied_output_percent,decision\n13 bar pressure,{:.0},0,{:.0},{:.12},safe output\n0.6 s sensor age,{:.0},0,{:.0},{:.12},safe output\nzero cycle time,{:.0},0,{:.0},{:.12},safe output\nmissed deadline,{:.0},0,{:.0},{:.12},safe output\nreversed output limits,5,0,0,30.000000000000,safe output and latch\nacknowledgement,5,0,0,30.000000000000,latch remains and count stays zero\nthree valid scans,5,0,0,30.000000000000,recovery count reaches three\nreset,0,1,0,30.000000000000,configure and initialize without Auto\nrenewed Auto,0,0,1,{recovered_output:.12},controller update occurs on later scan\n",
        range_fault.0, range_fault.2, range_fault.1,
        stale_fault.0, stale_fault.2, stale_fault.1,
        timing_fault.0, timing_fault.2, timing_fault.1,
        deadline_fault.0, deadline_fault.2, deadline_fault.1,
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/03-production-quality-digital-pid/30-booster-pump-invalid-data/figures");
    write_or_compare_curriculum_artifact(
        &artifact_root.join("invalid-data-recovery-run.csv"),
        &recovery_run,
    );
    evidence::retain_sequence(
        &artifact_root, "invalid-data-recovery", "Recovery authority advances in separate qualified scans",
        "Configured Simulation evidence · configuration fault, acknowledgement, three valid scans, reset, later Auto",
        &[
            evidence::Panel { title: "Latched fault and valid-scan count", unit: "code / count", minimum: 0.0, maximum: 5.0, series: vec![
                evidence::Series { name: "fault code", color: "#d55e00", dashed: false, values: &fault_code },
                evidence::Series { name: "valid scan count", color: "#0072b2", dashed: false, values: &recovery_count },
            ]},
            evidence::Panel { title: "Safe output and distinct controller actions", unit: "% / call", minimum: 0.0, maximum: 35.0, series: vec![
                evidence::Series { name: "applied safe output", color: "#0072b2", dashed: false, values: &safe_output },
                evidence::Series { name: "Configure call", color: "#009e73", dashed: false, values: &configured_calls },
                evidence::Series { name: "PID Update call", color: "#555555", dashed: true, values: &controller_calls },
            ]},
        ], &[], &[], &summary,
    );
}

#[test]
fn control_curriculum_pid_production_retains_complete_workflows() {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        return;
    }
    let expectations = [
        (
            "booster-pump-modes",
            "booster-pump-startup-run.csv",
            [
                "zero_state",
                "measured_20",
                "measured_40",
                "measured_70",
                "disabled",
                "manual",
                "track",
                "simulation",
            ]
            .as_slice(),
        ),
        (
            "booster-pump-bumpless-transfer",
            "bumpless-transfer-run.csv",
            [
                "tracked_to_auto",
                "untracked_to_auto",
                "unaligned_to_manual",
                "aligned_to_manual",
                "stale_tracking_infeasible",
                "recovery",
            ]
            .as_slice(),
        ),
        (
            "booster-pump-atomic-update",
            "atomic-update-run.csv",
            [
                "atomic_revision",
                "mixed_revision",
                "feedforward_scan_1",
                "feedforward_scan_2",
                "feedforward_scan_3",
                "feedforward_scan_4",
            ]
            .as_slice(),
        ),
        (
            "booster-pump-invalid-data",
            "invalid-data-recovery-run.csv",
            [
                "configuration_fault",
                "acknowledged",
                "valid_1",
                "valid_2",
                "valid_3",
                "reset",
                "renewed_auto",
            ]
            .as_slice(),
        ),
    ];
    for (project, file, required_rows) in expectations {
        let run = project_file(project, &format!("figures/{file}"));
        let header = run.lines().next().expect("retained workflow header");
        assert!(
            header.contains("evidence_class"),
            "{project} omits evidence class"
        );
        for row in required_rows {
            assert!(
                run.lines().any(|line| line.starts_with(row)),
                "{project} retained workflow omits {row}"
            );
        }
    }
}
