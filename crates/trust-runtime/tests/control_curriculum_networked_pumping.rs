use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const DT: f64 = 0.1;
const MAX_AGE: f64 = 0.50;
const MAX_CLOCK_UNCERTAINTY: f64 = 0.05;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/13-networked-fault-aware-and-diagnostic-control/73-networked-pumping")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 73 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 73 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 73 program before configuration")
        .replace(
            "RemoteEnabled : BOOL :=",
            "RemoteEnabled AT %IX64.0 : BOOL :=",
        )
        .replace(
            "RemoteSampleReceived : BOOL :=",
            "RemoteSampleReceived AT %IX64.1 : BOOL :=",
        )
        .replace(
            "RemoteQualityGood : BOOL :=",
            "RemoteQualityGood AT %IX64.2 : BOOL :=",
        )
        .replace(
            "ClockSynchronized : BOOL :=",
            "ClockSynchronized AT %IX64.3 : BOOL :=",
        )
        .replace(
            "RecoveryApproved : BOOL :=",
            "RecoveryApproved AT %IX64.4 : BOOL :=",
        )
        .replace(
            "LocalTimeSeconds : LREAL :=",
            "LocalTimeSeconds AT %IL16 : LREAL :=",
        )
        .replace(
            "RemoteSourceTimeSeconds : LREAL :=",
            "RemoteSourceTimeSeconds AT %IL24 : LREAL :=",
        )
        .replace(
            "ClockUncertaintySeconds : LREAL :=",
            "ClockUncertaintySeconds AT %IL32 : LREAL :=",
        )
        .replace(
            "RemoteDemand : LREAL :=",
            "RemoteDemand AT %IL40 : LREAL :=",
        )
        .replace(
            "RemoteSequence : DINT :=",
            "RemoteSequence AT %ID48 : DINT :=",
        )
        .replace(
            "NaivePressure : LREAL :=",
            "NaivePressure AT %IL0 : LREAL :=",
        )
        .replace(
            "ProtectedPressure : LREAL :=",
            "ProtectedPressure AT %IL8 : LREAL :=",
        )
        .replace("NaiveApplied : LREAL :=", "NaiveApplied AT %QL0 : LREAL :=")
        .replace(
            "ProtectedApplied : LREAL :=",
            "ProtectedApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "NaiveRequested : LREAL :=",
            "NaiveRequested AT %ML64 : LREAL :=",
        )
        .replace(
            "ProtectedRequested : LREAL :=",
            "ProtectedRequested AT %ML72 : LREAL :=",
        )
        .replace(
            "NominalAgeSeconds : LREAL :=",
            "NominalAgeSeconds AT %ML80 : LREAL :=",
        )
        .replace(
            "ConservativeAgeSeconds : LREAL :=",
            "ConservativeAgeSeconds AT %ML88 : LREAL :=",
        )
        .replace(
            "LastAcceptedSequence : DINT :=",
            "LastAcceptedSequence AT %MD96 : DINT :=",
        )
        .replace(
            "RejectionReason : INT :=",
            "RejectionReason AT %MW50 : INT :=",
        )
        .replace(
            "CommunicationFaultLatched : BOOL :=",
            "CommunicationFaultLatched AT %MX52.0 : BOOL :=",
        )
        .replace(
            "ProtectedRemoteValid : BOOL :=",
            "ProtectedRemoteValid AT %MX52.1 : BOOL :=",
        )
        .replace(
            "ProtectedRemoteOwned : BOOL :=",
            "ProtectedRemoteOwned AT %MX52.2 : BOOL :=",
        )
        .replace(
            "NewSampleEligible : BOOL :=",
            "NewSampleEligible AT %MX52.3 : BOOL :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "networked-pumping/src/NetworkedPumpSupervisor.st",
            project_file("src/NetworkedPumpSupervisor.st"),
        ),
        SourceFile::with_path("networked-pumping/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 73 Structured Text and Control library")
}

fn simulation() -> SimulationController {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-control-networked-pumping-{}-{stamp}.toml",
        std::process::id()
    ));
    std::fs::write(&path, project_file("simulation.toml"))
        .expect("write Lesson 73 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 73 simulation config");
    let _ = std::fs::remove_file(path);
    SimulationController::new(config)
}

fn write_lreal(runtime: &mut Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid LREAL address"),
            Value::LWord(value.to_bits()),
        )
        .expect("write Lesson 73 LREAL input");
}

fn write_dint(runtime: &mut Runtime, address: &str, value: i32) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid DINT address"),
            Value::DWord(value as u32),
        )
        .expect("write Lesson 73 DINT input");
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid BOOL address"),
            Value::Bool(value),
        )
        .expect("write Lesson 73 BOOL input");
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid LREAL address"))
        .expect("read Lesson 73 LREAL signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &str) -> i32 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid DINT address"))
        .expect("read Lesson 73 DINT signal")
    {
        Value::DWord(value) => value as i32,
        value => panic!("expected DINT, got {value:?}"),
    }
}

fn read_int(runtime: &Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid INT address"))
        .expect("read Lesson 73 INT signal")
    {
        Value::Word(value) => value as i16,
        value => panic!("expected INT, got {value:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid BOOL address"))
        .expect("read Lesson 73 BOOL signal")
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

#[derive(Clone, Copy, Debug)]
struct Packet {
    received: bool,
    quality: bool,
    synchronized: bool,
    recovery: bool,
    source_time: f64,
    uncertainty: f64,
    sequence: i32,
    demand: f64,
}

impl Packet {
    fn no_receive(time: f64, sequence: i32) -> Self {
        Self {
            received: false,
            quality: true,
            synchronized: true,
            recovery: false,
            source_time: time,
            uncertainty: 0.05,
            sequence,
            demand: 50.0,
        }
    }
}

#[derive(Debug, Default)]
struct PolicyOracle {
    naive_demand: f64,
    has_naive: bool,
    accepted_demand: f64,
    accepted_source_time: f64,
    last_sequence: i32,
    has_protected: bool,
    fault: bool,
}

#[derive(Clone, Copy, Debug)]
struct PolicyResult {
    naive: f64,
    remote_valid: bool,
    remote_owned: bool,
    eligible: bool,
    rejection: i16,
}

impl PolicyOracle {
    fn update(&mut self, time: f64, packet: Packet, previous_applied: f64) -> PolicyResult {
        if packet.received {
            self.naive_demand = packet.demand.clamp(0.0, 100.0);
            self.has_naive = true;
        }

        let nominal_age = time - packet.source_time;
        let conservative_age = nominal_age + packet.uncertainty;
        let mut rejection = 0_i16;
        if packet.received {
            if !packet.quality {
                rejection = 1;
            } else if !packet.synchronized {
                rejection = 2;
            } else if !(0.0..=MAX_CLOCK_UNCERTAINTY).contains(&packet.uncertainty) {
                rejection = 3;
            } else if nominal_age < -packet.uncertainty {
                rejection = 4;
            } else if conservative_age > MAX_AGE {
                rejection = 5;
            } else if packet.sequence <= self.last_sequence {
                rejection = 6;
            } else if !packet.demand.is_finite() || !(0.0..=100.0).contains(&packet.demand) {
                rejection = 7;
            } else if self.fault
                && (!packet.recovery || (packet.demand - previous_applied).abs() > 2.0)
            {
                rejection = 8;
            }
        }
        let eligible = packet.received && rejection == 0;
        if eligible {
            self.accepted_demand = packet.demand;
            self.accepted_source_time = packet.source_time;
            self.last_sequence = packet.sequence;
            self.has_protected = true;
            self.fault = false;
        } else if packet.received {
            self.fault = true;
        }

        let retained_age = time - self.accepted_source_time + packet.uncertainty;
        let retained_valid = self.has_protected
            && packet.quality
            && packet.synchronized
            && (0.0..=MAX_CLOCK_UNCERTAINTY).contains(&packet.uncertainty)
            && time - self.accepted_source_time >= -packet.uncertainty
            && retained_age <= MAX_AGE;
        if self.has_protected && !retained_valid {
            self.fault = true;
            if rejection == 0 {
                rejection = if !packet.quality {
                    1
                } else if !packet.synchronized {
                    2
                } else if !(0.0..=MAX_CLOCK_UNCERTAINTY).contains(&packet.uncertainty) {
                    3
                } else if time - self.accepted_source_time < -packet.uncertainty {
                    4
                } else {
                    5
                };
            }
        }
        let remote_valid = retained_valid && !self.fault;
        PolicyResult {
            naive: if self.has_naive {
                self.naive_demand
            } else {
                50.0
            },
            remote_valid,
            remote_owned: remote_valid,
            eligible,
            rejection,
        }
    }
}

fn packet_for_scan(scan: usize, time: f64, last_sequence: i32, protected_applied: f64) -> Packet {
    let mut packet = Packet::no_receive(time, last_sequence);
    match scan {
        0 | 4 | 8 | 12 | 16 => {
            packet.received = true;
            packet.sequence = scan as i32 / 4 + 1;
            packet.source_time = time - 0.05;
        }
        30 => {
            packet.received = true;
            packet.sequence = last_sequence + 1;
            packet.source_time = time - 0.55;
            packet.demand = 90.0;
        }
        40 | 70 | 90 | 110 | 130 | 150 => {
            packet.received = true;
            packet.recovery = true;
            packet.sequence = last_sequence + 1;
            packet.source_time = time - 0.05;
            packet.demand = protected_applied;
        }
        50 => {
            packet.received = true;
            packet.sequence = last_sequence + 2;
            packet.source_time = time - 0.05;
            packet.demand = 50.0;
        }
        60 => {
            packet.received = true;
            packet.sequence = last_sequence - 1;
            packet.source_time = time - 0.05;
            packet.demand = 10.0;
        }
        80 => {
            packet.received = true;
            packet.quality = false;
            packet.sequence = last_sequence + 1;
            packet.source_time = time - 0.05;
            packet.demand = 95.0;
        }
        100 => {
            packet.received = true;
            packet.synchronized = false;
            packet.sequence = last_sequence + 1;
            packet.source_time = time - 0.05;
            packet.demand = 0.0;
        }
        120 => {
            packet.received = true;
            packet.uncertainty = 0.08;
            packet.sequence = last_sequence + 1;
            packet.source_time = time - 0.05;
            packet.demand = 100.0;
        }
        140 => {
            packet.received = true;
            packet.sequence = last_sequence + 1;
            packet.source_time = time + 0.10;
            packet.demand = 0.0;
        }
        _ if scan % 4 == 0
            && !(20..40).contains(&scan)
            && !(60..70).contains(&scan)
            && !(80..90).contains(&scan)
            && !(100..110).contains(&scan)
            && !(120..130).contains(&scan)
            && !(140..150).contains(&scan) =>
        {
            packet.received = true;
            packet.sequence = last_sequence + 1;
            packet.source_time = time - 0.05;
            packet.demand = protected_applied;
        }
        _ => {}
    }
    packet
}

#[test]
fn control_curriculum_networked_pumping_bounds_age_order_and_transfer() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 73 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 73 must use schema 4");
    assert_eq!(lesson["sequence"], 73);
    assert_eq!(
        lesson["prerequisites"][0],
        "control-flexible-airframe-robust"
    );
    assert_eq!(lesson["nextLesson"], "control-pumping-fault-aware");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_networked_pumping_bounds_age_order_and_transfer"
    );

    let static_loop_gain: f64 = 200.0 * 0.04;
    let crossover = (static_loop_gain.powi(2) - 1.0).sqrt() / 4.0;
    let delay_margin = (std::f64::consts::PI - (4.0 * crossover).atan()) / crossover;
    let delay_budget = 0.10 + 0.02 + 0.15 + 0.05 + 0.05 + 0.10;
    let delay_reserve = delay_margin - delay_budget;
    assert!((crossover - 1.984_313_483).abs() < 1e-9);
    assert!((delay_margin - 0.854_766_231).abs() < 1e-9);
    assert!((delay_budget - 0.47_f64).abs() < 1e-12);
    assert!((delay_reserve - 0.384_766_231).abs() < 1e-9);

    let mut runtime = build_runtime();
    let mut plant = simulation();
    plant.apply_pre_cycle(Duration::ZERO, &mut runtime).unwrap();
    let mut oracle = PolicyOracle::default();
    let mut previous_naive_applied = 50.0;
    let mut previous_protected_applied = 50.0;
    let mut naive_pressure_oracle = 6.0;
    let mut protected_pressure_oracle = 6.0;
    let decay = (-DT / 4.0).exp();
    let mut naive_invalid_acceptances = 0_usize;
    let mut protected_invalid_acceptances = 0_usize;
    let mut maximum_recovery_step = 0.0_f64;
    let mut naive_iae = 0.0_f64;
    let mut protected_iae = 0.0_f64;
    let mut naive_peak = 0.0_f64;
    let mut protected_peak = 0.0_f64;
    let mut naive_travel = 0.0_f64;
    let mut protected_travel = 0.0_f64;
    let mut protected_remote_samples = 0_usize;
    let mut last_sequence = 0_i32;
    let mut time_trace = Vec::with_capacity(250);
    let mut naive_pressure_trace = Vec::with_capacity(250);
    let mut protected_pressure_trace = Vec::with_capacity(250);
    let mut naive_command_trace = Vec::with_capacity(250);
    let mut protected_command_trace = Vec::with_capacity(250);

    for scan in 0..250 {
        let time = scan as f64 * DT;
        let packet = packet_for_scan(scan, time, last_sequence, previous_protected_applied);
        let expected = oracle.update(time, packet, previous_protected_applied);
        last_sequence = oracle.last_sequence;

        write_bool(&mut runtime, "%IX64.0", true);
        write_bool(&mut runtime, "%IX64.1", packet.received);
        write_bool(&mut runtime, "%IX64.2", packet.quality);
        write_bool(&mut runtime, "%IX64.3", packet.synchronized);
        write_bool(&mut runtime, "%IX64.4", packet.recovery);
        write_lreal(&mut runtime, "%IL16", time);
        write_lreal(&mut runtime, "%IL24", packet.source_time);
        write_lreal(&mut runtime, "%IL32", packet.uncertainty);
        write_lreal(&mut runtime, "%IL40", packet.demand);
        write_dint(&mut runtime, "%ID48", packet.sequence);

        runtime
            .execute_cycle()
            .expect("execute Lesson 73 station scan");
        let naive_applied = read_lreal(&runtime, "%QL0");
        let protected_applied = read_lreal(&runtime, "%QL8");
        assert!(
            (naive_applied - expected.naive).abs() <= 1e-9,
            "scan {scan}"
        );
        assert_eq!(
            read_bool(&runtime, "%MX52.1"),
            expected.remote_valid,
            "scan {scan}"
        );
        assert_eq!(
            read_bool(&runtime, "%MX52.2"),
            expected.remote_owned,
            "scan {scan}"
        );
        assert_eq!(
            read_bool(&runtime, "%MX52.3"),
            expected.eligible,
            "scan {scan}"
        );
        assert_eq!(
            read_int(&runtime, "%MW50"),
            expected.rejection,
            "scan {scan}"
        );
        assert_eq!(
            read_dint(&runtime, "%MD96"),
            oracle.last_sequence,
            "scan {scan}"
        );
        assert_eq!(read_bool(&runtime, "%MX52.0"), oracle.fault, "scan {scan}");
        assert!(naive_applied.is_finite() && (0.0..=100.0).contains(&naive_applied));
        assert!(protected_applied.is_finite() && (0.0..=100.0).contains(&protected_applied));
        time_trace.push(time);
        naive_pressure_trace.push(read_lreal(&runtime, "%IL0"));
        protected_pressure_trace.push(read_lreal(&runtime, "%IL8"));
        naive_command_trace.push(naive_applied);
        protected_command_trace.push(protected_applied);

        if packet.received && !expected.eligible {
            naive_invalid_acceptances += 1;
            if read_bool(&runtime, "%MX52.2") {
                protected_invalid_acceptances += 1;
            }
        }
        if packet.received && packet.recovery && expected.eligible {
            maximum_recovery_step =
                maximum_recovery_step.max((protected_applied - previous_protected_applied).abs());
        }
        if expected.remote_owned {
            protected_remote_samples += 1;
        }
        naive_iae += (6.0 - read_lreal(&runtime, "%IL0")).abs() * DT;
        protected_iae += (6.0 - read_lreal(&runtime, "%IL8")).abs() * DT;
        naive_peak = naive_peak.max((6.0 - read_lreal(&runtime, "%IL0")).abs());
        protected_peak = protected_peak.max((6.0 - read_lreal(&runtime, "%IL8")).abs());
        naive_travel += (naive_applied - previous_naive_applied).abs();
        protected_travel += (protected_applied - previous_protected_applied).abs();

        naive_pressure_oracle = 4.0
            + 0.04 * naive_applied
            + (naive_pressure_oracle - (4.0 + 0.04 * naive_applied)) * decay;
        protected_pressure_oracle = 4.0
            + 0.04 * protected_applied
            + (protected_pressure_oracle - (4.0 + 0.04 * protected_applied)) * decay;
        previous_naive_applied = naive_applied;
        previous_protected_applied = protected_applied;

        let now = Duration::from_millis((scan as i64 + 1) * 100);
        plant.apply_post_cycle(now, &runtime).unwrap();
        plant.apply_pre_cycle(now, &mut runtime).unwrap();
        assert!((read_lreal(&runtime, "%IL0") - naive_pressure_oracle).abs() <= 2e-8);
        assert!((read_lreal(&runtime, "%IL8") - protected_pressure_oracle).abs() <= 2e-8);
    }

    println!(
        "network delay margin={delay_margin:.6}s, budget={delay_budget:.2}s, reserve={delay_reserve:.6}s; invalid accepted naive/protected={naive_invalid_acceptances}/{protected_invalid_acceptances}; IAE={naive_iae:.4}/{protected_iae:.4} bar*s; peak={naive_peak:.4}/{protected_peak:.4} bar; travel={naive_travel:.1}/{protected_travel:.1} pp; recovery step={maximum_recovery_step:.2} pp; protected availability={:.1}%",
        100.0 * protected_remote_samples as f64 / 250.0
    );
    assert!(naive_invalid_acceptances >= 6);
    assert_eq!(protected_invalid_acceptances, 0);
    assert!(maximum_recovery_step <= 2.0 + 1e-9);
    assert!(
        protected_iae < naive_iae,
        "protected IAE {protected_iae} must improve naive {naive_iae}"
    );
    assert!(
        protected_peak < naive_peak,
        "protected peak {protected_peak} must improve naive {naive_peak}"
    );
    assert!(protected_remote_samples > 0 && protected_remote_samples < 250);

    let mut summary = String::from(
        "policy,invalid_packets_accepted,pressure_iae_bar_s,peak_pressure_error_bar,command_travel_pp\n",
    );
    writeln!(
        summary,
        "naive,{naive_invalid_acceptances},{naive_iae:.9},{naive_peak:.9},{naive_travel:.9}"
    )
    .unwrap();
    writeln!(summary, "protected,{protected_invalid_acceptances},{protected_iae:.9},{protected_peak:.9},{protected_travel:.9}").unwrap();
    writeln!(summary, "delay_budget_s,{delay_budget:.9},delay_margin_s,{delay_margin:.9},delay_reserve_s,{delay_reserve:.9}").unwrap();
    let figure_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/13-networked-fault-aware-and-diagnostic-control/73-networked-pumping/figures");
    evidence::retain(
        &figure_root,
        "networked-pumping-evidence",
        "Network validity policy prevents unsafe remote-command acceptance",
        "The naive and protected stations receive the same stale, reordered, and invalid packets.",
        &[
            evidence::Panel {
                title: "Station discharge pressure",
                x_unit: "time (s)",
                y_unit: "bar",
                x_minimum: 0.0,
                x_maximum: 24.9,
                y_minimum: 3.5,
                y_maximum: 8.5,
                series: vec![
                    evidence::Series {
                        name: "Naive",
                        color: "#dc2626",
                        dashed: false,
                        x: &time_trace,
                        y: &naive_pressure_trace,
                    },
                    evidence::Series {
                        name: "Protected",
                        color: "#2563eb",
                        dashed: false,
                        x: &time_trace,
                        y: &protected_pressure_trace,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied pump-speed command",
                x_unit: "time (s)",
                y_unit: "%",
                x_minimum: 0.0,
                x_maximum: 24.9,
                y_minimum: 0.0,
                y_maximum: 100.0,
                series: vec![
                    evidence::Series {
                        name: "Naive",
                        color: "#dc2626",
                        dashed: false,
                        x: &time_trace,
                        y: &naive_command_trace,
                    },
                    evidence::Series {
                        name: "Protected",
                        color: "#2563eb",
                        dashed: false,
                        x: &time_trace,
                        y: &protected_command_trace,
                    },
                ],
            },
        ],
        &[],
        &summary,
    );
}
