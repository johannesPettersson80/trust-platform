use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;
use evidence::{Panel, RetainedSeries, Series};

const MOVES: [f64; 3] = [-20.0, 0.0, 20.0];

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/83-building-hvac")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 83 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 83 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    let mut source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 83 program before configuration")
        .to_owned();
    let workflow_start = source
        .find("// The simulated capstone starts from its reviewed automatic operating state.")
        .expect("Lesson 83 project-owned workflow start");
    let workflow_end_marker = "AutoMode := Enabled AND (OperatingMode = DINT#1);";
    let workflow_end = source[workflow_start..]
        .find(workflow_end_marker)
        .map(|offset| workflow_start + offset + workflow_end_marker.len())
        .expect("Lesson 83 project-owned workflow end");
    source.replace_range(workflow_start..workflow_end, "");

    // These tests qualify the controller and plant independently of the visible
    // lesson sequencer. The course-wide workflow contract separately proves
    // that the shipped project owns the complete Control Studio experiment.
    source
        .replace(
            "FeasibleCandidateCount : DINT :=",
            "FeasibleCandidateCount AT %MD4 : DINT :=",
        )
        .replace(
            "CandidateCount : DINT :=",
            "CandidateCount AT %MD0 : DINT :=",
        )
        .replace(
            "BestObjective : LREAL :=",
            "BestObjective AT %ML8 : LREAL :=",
        )
        .replace(
            "Zone1Setpoint : LREAL :=",
            "Zone1Setpoint AT %ML16 : LREAL :=",
        )
        .replace(
            "Zone2Setpoint : LREAL :=",
            "Zone2Setpoint AT %ML24 : LREAL :=",
        )
        .replace(
            "ComfortMinimum : LREAL :=",
            "ComfortMinimum AT %ML32 : LREAL :=",
        )
        .replace(
            "ComfortMaximum : LREAL :=",
            "ComfortMaximum AT %ML40 : LREAL :=",
        )
        .replace(
            "TemperatureWeight : LREAL :=",
            "TemperatureWeight AT %ML48 : LREAL :=",
        )
        .replace(
            "EnergyWeight : LREAL :=",
            "EnergyWeight AT %ML56 : LREAL :=",
        )
        .replace("MoveWeight : LREAL :=", "MoveWeight AT %ML64 : LREAL :=")
        .replace(
            "Zone1HeatRequested : LREAL :=",
            "Zone1HeatRequested AT %ML72 : LREAL :=",
        )
        .replace(
            "Zone2HeatRequested : LREAL :=",
            "Zone2HeatRequested AT %ML80 : LREAL :=",
        )
        .replace(
            "PreviousZone1Heat : LREAL :=",
            "PreviousZone1Heat AT %ML88 : LREAL :=",
        )
        .replace(
            "PreviousZone2Heat : LREAL :=",
            "PreviousZone2Heat AT %ML96 : LREAL :=",
        )
        .replace(
            "OutsideCurrent : LREAL :=",
            "OutsideCurrent AT %ML104 : LREAL :=",
        )
        .replace(
            "Occupancy1Current : LREAL :=",
            "Occupancy1Current AT %ML112 : LREAL :=",
        )
        .replace(
            "Occupancy2Current : LREAL :=",
            "Occupancy2Current AT %ML120 : LREAL :=",
        )
        .replace(
            "OutsideForecast1 : LREAL :=",
            "OutsideForecast1 AT %ML128 : LREAL :=",
        )
        .replace(
            "OutsideForecast2 : LREAL :=",
            "OutsideForecast2 AT %ML136 : LREAL :=",
        )
        .replace(
            "OutsideForecast3 : LREAL :=",
            "OutsideForecast3 AT %ML144 : LREAL :=",
        )
        .replace(
            "Occupancy1Forecast1 : LREAL :=",
            "Occupancy1Forecast1 AT %ML152 : LREAL :=",
        )
        .replace(
            "Occupancy1Forecast2 : LREAL :=",
            "Occupancy1Forecast2 AT %ML160 : LREAL :=",
        )
        .replace(
            "Occupancy1Forecast3 : LREAL :=",
            "Occupancy1Forecast3 AT %ML168 : LREAL :=",
        )
        .replace(
            "Occupancy2Forecast1 : LREAL :=",
            "Occupancy2Forecast1 AT %ML176 : LREAL :=",
        )
        .replace(
            "Occupancy2Forecast2 : LREAL :=",
            "Occupancy2Forecast2 AT %ML184 : LREAL :=",
        )
        .replace(
            "Occupancy2Forecast3 : LREAL :=",
            "Occupancy2Forecast3 AT %ML192 : LREAL :=",
        )
        .replace(
            "TariffForecast1 : LREAL :=",
            "TariffForecast1 AT %ML200 : LREAL :=",
        )
        .replace(
            "TariffForecast2 : LREAL :=",
            "TariffForecast2 AT %ML208 : LREAL :=",
        )
        .replace(
            "TariffForecast3 : LREAL :=",
            "TariffForecast3 AT %ML216 : LREAL :=",
        )
        .replace(
            "OperatingMode : DINT :=",
            "OperatingMode AT %MD224 : DINT :=",
        )
        .replace("Enabled : BOOL :=", "Enabled AT %MX240.0 : BOOL :=")
        .replace("UseMpc : BOOL :=", "UseMpc AT %MX240.1 : BOOL :=")
        .replace(
            "ConfigurationValid : BOOL :=",
            "ConfigurationValid AT %MX240.2 : BOOL :=",
        )
        .replace(
            "SolverComplete : BOOL :=",
            "SolverComplete AT %MX240.3 : BOOL :=",
        )
        .replace(
            "SolverFeasible : BOOL :=",
            "SolverFeasible AT %MX240.4 : BOOL :=",
        )
        .replace("Infeasible : BOOL :=", "Infeasible AT %MX240.5 : BOOL :=")
        .replace(
            "FallbackActive : BOOL :=",
            "FallbackActive AT %MX240.6 : BOOL :=",
        )
        .replace(
            "ConstraintActive : BOOL :=",
            "ConstraintActive AT %MX240.7 : BOOL :=",
        )
        .replace("QualityGood : BOOL :=", "QualityGood AT %MX241.0 : BOOL :=")
        .replace(
            "Heater1Available : BOOL :=",
            "Heater1Available AT %MX241.1 : BOOL :=",
        )
        .replace(
            "Heater2Available : BOOL :=",
            "Heater2Available AT %MX241.2 : BOOL :=",
        )
        .replace(
            "EstimatorInitialized : BOOL :=",
            "EstimatorInitialized AT %MX241.3 : BOOL :=",
        )
        .replace(
            "EstimatorValid : BOOL :=",
            "EstimatorValid AT %MX241.4 : BOOL :=",
        )
        .replace(
            "TrackingActive : BOOL :=",
            "TrackingActive AT %MX241.5 : BOOL :=",
        )
        .replace("FaultActive : BOOL :=", "FaultActive AT %MX241.6 : BOOL :=")
        .replace(
            "ManualZone1Heat : LREAL :=",
            "ManualZone1Heat AT %ML248 : LREAL :=",
        )
        .replace(
            "ManualZone2Heat : LREAL :=",
            "ManualZone2Heat AT %ML256 : LREAL :=",
        )
        .replace(
            "PreviousPrediction1 : LREAL :=",
            "PreviousPrediction1 AT %ML264 : LREAL :=",
        )
        .replace(
            "PreviousPrediction2 : LREAL :=",
            "PreviousPrediction2 AT %ML272 : LREAL :=",
        )
        .replace("Innovation1 : LREAL :=", "Innovation1 AT %ML280 : LREAL :=")
        .replace("Innovation2 : LREAL :=", "Innovation2 AT %ML288 : LREAL :=")
        .replace(
            "EstimatedBias1 : LREAL :=",
            "EstimatedBias1 AT %ML296 : LREAL :=",
        )
        .replace(
            "EstimatedBias2 : LREAL :=",
            "EstimatedBias2 AT %ML304 : LREAL :=",
        )
        .replace(
            "EstimatorGain : LREAL :=",
            "EstimatorGain AT %ML312 : LREAL :=",
        )
        .replace(
            "ControllerRevision : DINT :=",
            "ControllerRevision AT %MD320 : DINT :=",
        )
        .replace(
            "Zone1HeatApplied : LREAL :=",
            "Zone1HeatApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "Zone2HeatApplied : LREAL :=",
            "Zone2HeatApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "OutsideApplied : LREAL :=",
            "OutsideApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "Occupancy1Applied : LREAL :=",
            "Occupancy1Applied AT %QL24 : LREAL :=",
        )
        .replace(
            "Occupancy2Applied : LREAL :=",
            "Occupancy2Applied AT %QL32 : LREAL :=",
        )
        .replace(
            "Zone1Temperature : LREAL :=",
            "Zone1Temperature AT %IL0 : LREAL :=",
        )
        .replace(
            "Zone2Temperature : LREAL :=",
            "Zone2Temperature AT %IL8 : LREAL :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "building-hvac-capstone/src/MpcTypes.st",
            project_file("src/MpcTypes.st"),
        ),
        SourceFile::with_path(
            "building-hvac-capstone/src/FiniteGridMpcController.st",
            project_file("src/FiniteGridMpcController.st"),
        ),
        SourceFile::with_path("building-hvac-capstone/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 83 Structured Text")
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-building-hvac-capstone-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn simulation(config_name: &str) -> SimulationController {
    let path = temporary_config_path();
    std::fs::write(&path, project_file(config_name)).expect("write Lesson 83 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 83 simulation config");
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
        .expect("write LREAL lesson input");
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid BOOL address"),
            Value::Bool(value),
        )
        .expect("write BOOL lesson selector");
}

fn write_dint(runtime: &mut Runtime, address: &str, value: i32) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid DINT address"),
            Value::DWord(value as u32),
        )
        .expect("write DINT lesson input");
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid LREAL address"))
        .expect("read LREAL lesson signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &str) -> i32 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid DINT address"))
        .expect("read DINT lesson signal")
    {
        Value::DWord(value) => value as i32,
        value => panic!("expected DINT, got {value:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid BOOL address"))
        .expect("read BOOL lesson signal")
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

#[derive(Clone, Copy)]
struct Forecast {
    outside: [f64; 3],
    occupancy1: [f64; 3],
    occupancy2: [f64; 3],
    tariff: [f64; 3],
}

#[derive(Clone, Copy)]
struct Problem {
    temperature: [f64; 2],
    setpoint: [f64; 2],
    previous_heat: [f64; 2],
    comfort: [f64; 2],
    weights: [f64; 3],
    bias: [f64; 2],
    forecast: Forecast,
}

#[derive(Debug)]
struct Solution {
    candidate_count: usize,
    feasible_count: usize,
    first_heat: [f64; 2],
    objective: f64,
}

fn predict(
    temperature: [f64; 2],
    heat: [f64; 2],
    outside: f64,
    occupancy: [f64; 2],
    bias: [f64; 2],
) -> [f64; 2] {
    [
        0.875_659_571_463 * temperature[0]
            + 0.029_177_846_573 * temperature[1]
            + 0.018_727_354_703 * heat[0]
            + 0.000_305_161_690 * heat[1]
            + 0.095_162_581_964 * outside
            + 0.093_636_773_515 * occupancy[0]
            + 0.001_525_808_449 * occupancy[1]
            + bias[0],
        0.029_177_846_573 * temperature[0]
            + 0.875_659_571_463 * temperature[1]
            + 0.000_305_161_690 * heat[0]
            + 0.018_727_354_703 * heat[1]
            + 0.095_162_581_964 * outside
            + 0.001_525_808_449 * occupancy[0]
            + 0.093_636_773_515 * occupancy[1]
            + bias[1],
    ]
}

fn solve(problem: Problem) -> Solution {
    let mut solution = Solution {
        candidate_count: 0,
        feasible_count: 0,
        first_heat: [0.0; 2],
        objective: f64::INFINITY,
    };
    for m10 in MOVES {
        for m20 in MOVES {
            for m11 in MOVES {
                for m21 in MOVES {
                    for m12 in MOVES {
                        for m22 in MOVES {
                            solution.candidate_count += 1;
                            let moves = [[m10, m20], [m11, m21], [m12, m22]];
                            let mut previous = problem.previous_heat;
                            let mut state = problem.temperature;
                            let mut objective = 0.0;
                            let mut feasible = true;
                            for horizon in 0..3 {
                                let heat = [
                                    previous[0] + moves[horizon][0],
                                    previous[1] + moves[horizon][1],
                                ];
                                if heat[0] < 0.0
                                    || heat[0] > 100.0
                                    || heat[1] < 0.0
                                    || heat[1] > 100.0
                                    || heat[0] + heat[1] > 170.0
                                {
                                    feasible = false;
                                    break;
                                }
                                state = predict(
                                    state,
                                    heat,
                                    problem.forecast.outside[horizon],
                                    [
                                        problem.forecast.occupancy1[horizon],
                                        problem.forecast.occupancy2[horizon],
                                    ],
                                    problem.bias,
                                );
                                if state[0] < problem.comfort[0]
                                    || state[0] > problem.comfort[1]
                                    || state[1] < problem.comfort[0]
                                    || state[1] > problem.comfort[1]
                                {
                                    feasible = false;
                                    break;
                                }
                                objective += problem.weights[0]
                                    * ((state[0] - problem.setpoint[0]).powi(2)
                                        + (state[1] - problem.setpoint[1]).powi(2))
                                    + problem.weights[1]
                                        * problem.forecast.tariff[horizon]
                                        * (heat[0] + heat[1])
                                    + problem.weights[2]
                                        * (moves[horizon][0].powi(2) + moves[horizon][1].powi(2));
                                previous = heat;
                            }
                            if feasible {
                                solution.feasible_count += 1;
                                if objective < solution.objective {
                                    solution.objective = objective;
                                    solution.first_heat = [
                                        problem.previous_heat[0] + m10,
                                        problem.previous_heat[1] + m20,
                                    ];
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    solution
}

fn configure(runtime: &mut Runtime, problem: Problem, use_mpc: bool) {
    write_bool(runtime, "%MX240.0", true);
    write_bool(runtime, "%MX240.1", use_mpc);
    write_dint(runtime, "%MD224", 1);
    write_bool(runtime, "%MX241.0", true);
    write_bool(runtime, "%MX241.1", true);
    write_bool(runtime, "%MX241.2", true);
    for (address, value) in [
        ("%ML16", problem.setpoint[0]),
        ("%ML24", problem.setpoint[1]),
        ("%ML32", problem.comfort[0]),
        ("%ML40", problem.comfort[1]),
        ("%ML48", problem.weights[0]),
        ("%ML56", problem.weights[1]),
        ("%ML64", problem.weights[2]),
        ("%ML128", problem.forecast.outside[0]),
        ("%ML136", problem.forecast.outside[1]),
        ("%ML144", problem.forecast.outside[2]),
        ("%ML152", problem.forecast.occupancy1[0]),
        ("%ML160", problem.forecast.occupancy1[1]),
        ("%ML168", problem.forecast.occupancy1[2]),
        ("%ML176", problem.forecast.occupancy2[0]),
        ("%ML184", problem.forecast.occupancy2[1]),
        ("%ML192", problem.forecast.occupancy2[2]),
        ("%ML200", problem.forecast.tariff[0]),
        ("%ML208", problem.forecast.tariff[1]),
        ("%ML216", problem.forecast.tariff[2]),
    ] {
        write_lreal(runtime, address, value);
    }
}

fn schedule(minute: usize) -> (f64, f64, f64, f64) {
    let outside = if minute < 6 { 5.0 } else { 4.0 };
    let occupancy1 = if minute < 8 { 0.0 } else { 2.0 };
    let occupancy2 = if minute < 10 { 0.0 } else { 1.0 };
    let tariff = if minute < 8 {
        0.10
    } else if minute < 13 {
        0.60
    } else {
        0.15
    };
    (outside, occupancy1, occupancy2, tariff)
}

fn forecast(minute: usize) -> Forecast {
    let a = schedule(minute);
    let b = schedule(minute + 1);
    let c = schedule(minute + 2);
    Forecast {
        outside: [a.0, b.0, c.0],
        occupancy1: [a.1, b.1, c.1],
        occupancy2: [a.2, b.2, c.2],
        tariff: [a.3, b.3, c.3],
    }
}

#[derive(Default)]
struct RunMetrics {
    absolute_error_integral: f64,
    comfort_violation: f64,
    comfort_violation_seconds: f64,
    tariff_cost: f64,
    movement: f64,
    peak_shared: f64,
    fallback_seconds: f64,
}

fn retained_metric(value: f64) -> f64 {
    // Decisions use unrounded metrics. Retained lesson evidence uses the
    // documented two-decimal engineering precision across supported hosts.
    (value * 100.0).round() / 100.0
}

fn run(use_mpc: bool, move_weight: f64) -> RunMetrics {
    let mut runtime = build_runtime();
    let mut plant = simulation("simulation.toml");
    write_lreal(&mut runtime, "%ML88", 80.0);
    write_lreal(&mut runtime, "%ML96", 80.0);
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial zone temperatures");
    let mut metrics = RunMetrics::default();
    let mut previous = [80.0, 80.0];
    for minute in 0..20 {
        let current = schedule(minute);
        let problem = Problem {
            temperature: [read_lreal(&runtime, "%IL0"), read_lreal(&runtime, "%IL8")],
            setpoint: [21.0, 21.0],
            previous_heat: previous,
            comfort: [20.0, 22.5],
            weights: [100.0, 0.01, move_weight],
            bias: [0.0, 0.0],
            forecast: forecast(minute),
        };
        configure(&mut runtime, problem, use_mpc);
        write_lreal(&mut runtime, "%ML104", current.0);
        write_lreal(&mut runtime, "%ML112", current.1);
        write_lreal(&mut runtime, "%ML120", current.2);
        runtime
            .execute_cycle()
            .expect("execute HVAC control interval");
        let applied = [read_lreal(&runtime, "%QL0"), read_lreal(&runtime, "%QL8")];
        assert!(applied.iter().all(|value| (0.0..=100.0).contains(value)));
        assert!(applied[0] + applied[1] <= 170.0 + 1e-9);
        assert!((applied[0] - previous[0]).abs() <= 20.0 + 1e-9);
        assert!((applied[1] - previous[1]).abs() <= 20.0 + 1e-9);
        if use_mpc {
            let oracle = solve(Problem {
                bias: [
                    read_lreal(&runtime, "%ML296"),
                    read_lreal(&runtime, "%ML304"),
                ],
                ..problem
            });
            assert_eq!(read_dint(&runtime, "%MD0"), oracle.candidate_count as i32);
            assert_eq!(read_dint(&runtime, "%MD4"), oracle.feasible_count as i32);
            assert!(read_bool(&runtime, "%MX240.3"));
            assert!(
                read_bool(&runtime, "%MX240.4"),
                "normal MPC became infeasible at minute {minute}: temperature={:?}, previous_heat={previous:?}, oracle_feasible={}",
                problem.temperature,
                oracle.feasible_count
            );
            assert!(!read_bool(&runtime, "%MX240.6"));
            assert!((applied[0] - oracle.first_heat[0]).abs() <= 1e-9);
            assert!((applied[1] - oracle.first_heat[1]).abs() <= 1e-9);
            assert!((read_lreal(&runtime, "%ML8") - oracle.objective).abs() <= 1e-6);
        }
        let temperatures = problem.temperature;
        for temperature in temperatures {
            metrics.absolute_error_integral += (21.0 - temperature).abs() * 60.0;
            let violation = (20.0 - temperature).max(0.0) + (temperature - 22.5).max(0.0);
            metrics.comfort_violation += violation * 60.0;
            if violation > 0.0 {
                metrics.comfort_violation_seconds += 60.0;
            }
        }
        metrics.tariff_cost += current.3 * (applied[0] + applied[1]) * 60.0;
        metrics.movement += (applied[0] - previous[0]).abs() + (applied[1] - previous[1]).abs();
        metrics.peak_shared = metrics.peak_shared.max(applied[0] + applied[1]);
        if read_bool(&runtime, "%MX240.6") {
            metrics.fallback_seconds += 60.0;
        }
        previous = applied;
        let now = Duration::from_millis((minute as i64 + 1) * 60_000);
        plant
            .apply_post_cycle(now, &runtime)
            .expect("advance two-zone office");
        plant
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish zone temperatures");
    }
    write_bool(&mut runtime, "%MX240.0", false);
    runtime.execute_cycle().expect("disable building control");
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);
    metrics
}

#[derive(Debug)]
struct EstimatorEvidence {
    uncorrected_rms: f64,
    corrected_rms: f64,
    final_bias: [f64; 2],
}

fn mismatch_estimator_evidence() -> EstimatorEvidence {
    let mut runtime = build_runtime();
    let mut plant = simulation("simulation-outdoor-loss-mismatch.toml");
    write_lreal(&mut runtime, "%ML88", 80.0);
    write_lreal(&mut runtime, "%ML96", 80.0);
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial mismatch temperatures");

    let mut uncorrected_squared = 0.0;
    let mut corrected_squared = 0.0;
    let mut samples = 0usize;
    for minute in 0..14 {
        let measured = [read_lreal(&runtime, "%IL0"), read_lreal(&runtime, "%IL8")];
        let prior_prediction = [
            read_lreal(&runtime, "%ML264"),
            read_lreal(&runtime, "%ML272"),
        ];
        let prior_bias = [
            read_lreal(&runtime, "%ML296"),
            read_lreal(&runtime, "%ML304"),
        ];
        if minute >= 4 {
            for zone in 0..2 {
                let uncorrected = measured[zone] - (prior_prediction[zone] - prior_bias[zone]);
                let corrected = measured[zone] - prior_prediction[zone];
                uncorrected_squared += uncorrected * uncorrected;
                corrected_squared += corrected * corrected;
                samples += 1;
            }
        }

        let problem = Problem {
            temperature: measured,
            setpoint: [21.0, 21.0],
            previous_heat: [read_lreal(&runtime, "%ML88"), read_lreal(&runtime, "%ML96")],
            comfort: [20.0, 22.5],
            weights: [100.0, 0.01, 0.0025],
            bias: prior_bias,
            forecast: Forecast {
                outside: [5.0; 3],
                occupancy1: [0.0; 3],
                occupancy2: [0.0; 3],
                tariff: [0.1; 3],
            },
        };
        configure(&mut runtime, problem, false);
        write_lreal(&mut runtime, "%ML104", 5.0);
        write_lreal(&mut runtime, "%ML112", 0.0);
        write_lreal(&mut runtime, "%ML120", 0.0);
        runtime
            .execute_cycle()
            .expect("execute mismatch estimator interval");
        assert!(read_bool(&runtime, "%MX241.4"));
        let now = Duration::from_millis((minute as i64 + 1) * 60_000);
        plant
            .apply_post_cycle(now, &runtime)
            .expect("advance mismatch office");
        plant
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish mismatch temperatures");
    }

    EstimatorEvidence {
        uncorrected_rms: (uncorrected_squared / samples as f64).sqrt(),
        corrected_rms: (corrected_squared / samples as f64).sqrt(),
        final_bias: [
            read_lreal(&runtime, "%ML296"),
            read_lreal(&runtime, "%ML304"),
        ],
    }
}

#[test]
fn control_curriculum_building_hvac_capstone_qualifies_load_bias_estimator() {
    let evidence = mismatch_estimator_evidence();
    eprintln!("building HVAC mismatch estimator: {evidence:?}");
    assert!(
        evidence.final_bias[0] < 0.0 && evidence.final_bias[1] < 0.0,
        "increased outdoor loss must produce a negative additive-load estimate: {evidence:?}"
    );
    assert!(evidence.final_bias.iter().all(|value| value.abs() <= 1.0));
    assert!(
        evidence.corrected_rms < evidence.uncorrected_rms * 0.75,
        "qualified bias correction must materially reduce mismatch residual: {evidence:?}"
    );
}

#[test]
fn control_curriculum_building_hvac_capstone_makes_evidence_based_decision() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 83 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 83 must use schema 4");
    assert_eq!(lesson["sequence"], 83);
    assert_eq!(lesson["prerequisites"][0], "control-capstone-distillation");
    assert_eq!(lesson["nextLesson"], "control-capstone-independent");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_building_hvac_capstone_makes_evidence_based_decision"
    );
    let source = format!(
        "{}\n{}",
        project_file("src/Main.st"),
        project_file("src/FiniteGridMpcController.st")
    );
    for forbidden in [
        "UnconstrainedMove",
        "AcceptedCandidate",
        "TemperatureIae",
        "PrecomputedPolicy",
    ] {
        assert!(
            !source.contains(forbidden),
            "ST embeds forbidden shortcut '{forbidden}'"
        );
    }

    let worked = Problem {
        temperature: [21.0, 21.0],
        setpoint: [21.0, 21.0],
        previous_heat: [80.0, 80.0],
        comfort: [20.0, 22.5],
        weights: [100.0, 0.01, 0.0025],
        bias: [0.0, 0.0],
        forecast: Forecast {
            outside: [5.0; 3],
            occupancy1: [0.0; 3],
            occupancy2: [0.0; 3],
            tariff: [0.1; 3],
        },
    };
    let predicted = predict([21.0, 21.0], [80.0, 80.0], 5.0, [0.0, 0.0], [0.0, 0.0]);
    assert!((predicted[0] - 21.0).abs() <= 1e-9);
    assert!((predicted[1] - 21.0).abs() <= 1e-9);
    let oracle = solve(worked);
    assert_eq!(oracle.candidate_count, 729);
    assert!(oracle.feasible_count > 0);
    assert_eq!(oracle.first_heat, [80.0, 80.0]);

    let mut runtime = build_runtime();
    write_lreal(&mut runtime, "%ML88", 80.0);
    write_lreal(&mut runtime, "%ML96", 80.0);
    write_lreal(&mut runtime, "%IL0", 21.0);
    write_lreal(&mut runtime, "%IL8", 21.0);
    configure(&mut runtime, worked, true);
    write_lreal(&mut runtime, "%ML104", 5.0);
    runtime
        .execute_cycle()
        .expect("execute worked MPC interval");
    assert!(read_bool(&runtime, "%MX240.2"));
    assert!(read_bool(&runtime, "%MX240.3"));
    assert!(read_bool(&runtime, "%MX240.4"));
    assert_eq!(read_dint(&runtime, "%MD0"), 729);
    assert_eq!(read_dint(&runtime, "%MD4"), oracle.feasible_count as i32);
    assert_eq!(read_lreal(&runtime, "%QL0"), oracle.first_heat[0]);
    assert_eq!(read_lreal(&runtime, "%QL8"), oracle.first_heat[1]);
    assert!((read_lreal(&runtime, "%ML8") - oracle.objective).abs() <= 1e-6);

    let pi = run(false, 0.0025);
    let mpc = run(true, 0.0025);
    let conservative = run(true, 0.025);
    assert_eq!(mpc.fallback_seconds, 0.0);
    assert_eq!(mpc.comfort_violation, 0.0);
    assert!(mpc.tariff_cost <= pi.tariff_cost);
    assert!(mpc.comfort_violation <= pi.comfort_violation);
    assert!(
        mpc.movement > pi.movement * 5.0,
        "Balanced MPC must expose, not hide, its actuator-movement tradeoff"
    );
    assert!(conservative.movement < mpc.movement);
    assert!(
        conservative.absolute_error_integral > pi.absolute_error_integral,
        "the actuator-conservative candidate must not be presented as an unqualified win"
    );

    let estimator = mismatch_estimator_evidence();
    assert!(estimator.final_bias[0] < 0.0 && estimator.final_bias[1] < 0.0);
    assert!(estimator.final_bias.iter().all(|value| value.abs() <= 1.0));
    assert!(
        estimator.corrected_rms < estimator.uncorrected_rms * 0.75,
        "qualified bias correction must materially reduce mismatch residual: {estimator:?}"
    );

    let infeasible_problem = Problem {
        comfort: [22.0, 22.5],
        ..worked
    };
    let infeasible_oracle = solve(infeasible_problem);
    assert_eq!(infeasible_oracle.candidate_count, 729);
    assert_eq!(infeasible_oracle.feasible_count, 0);
    configure(&mut runtime, infeasible_problem, true);
    runtime
        .execute_cycle()
        .expect("execute completed infeasible search");
    assert!(read_bool(&runtime, "%MX240.2"));
    assert!(read_bool(&runtime, "%MX240.3"));
    assert!(!read_bool(&runtime, "%MX240.4"));
    assert!(read_bool(&runtime, "%MX240.5"));
    assert!(read_bool(&runtime, "%MX240.6"));
    assert_eq!(read_dint(&runtime, "%MD0"), 729);
    assert_eq!(read_dint(&runtime, "%MD4"), 0);

    let invalid_problem = Problem {
        weights: [-1.0, 0.01, 0.0025],
        ..worked
    };
    configure(&mut runtime, invalid_problem, true);
    runtime
        .execute_cycle()
        .expect("execute invalid solver configuration");
    assert!(!read_bool(&runtime, "%MX240.2"));
    assert!(!read_bool(&runtime, "%MX240.3"));
    assert!(!read_bool(&runtime, "%MX240.5"));
    assert!(read_bool(&runtime, "%MX240.6"));
    assert_eq!(read_dint(&runtime, "%MD0"), 0);

    configure(&mut runtime, worked, true);
    runtime.execute_cycle().expect("recover valid MPC solve");
    assert!(read_bool(&runtime, "%MX240.2"));
    assert!(read_bool(&runtime, "%MX240.3"));
    assert!(read_bool(&runtime, "%MX240.4"));
    assert!(!read_bool(&runtime, "%MX240.6"));
    assert_eq!(read_dint(&runtime, "%MD0"), 729);

    assert_eq!(read_dint(&runtime, "%MD320"), 83);
    write_bool(&mut runtime, "%MX241.0", false);
    runtime
        .execute_cycle()
        .expect("remove authority for invalid temperature quality");
    assert!(read_bool(&runtime, "%MX241.6"));
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);

    write_bool(&mut runtime, "%MX241.0", true);
    write_dint(&mut runtime, "%MD224", 2);
    write_lreal(&mut runtime, "%ML248", 60.0);
    write_lreal(&mut runtime, "%ML256", 40.0);
    runtime
        .execute_cycle()
        .expect("enter bounded Manual authority");
    assert!(read_bool(&runtime, "%MX241.5"));
    assert_eq!(read_lreal(&runtime, "%QL0"), 20.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 20.0);

    write_dint(&mut runtime, "%MD224", 3);
    runtime
        .execute_cycle()
        .expect("execute bounded ControlledStop");
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);

    write_dint(&mut runtime, "%MD224", 1);
    write_bool(&mut runtime, "%MX241.1", false);
    runtime
        .execute_cycle()
        .expect("remove authority for unavailable heater");
    assert!(read_bool(&runtime, "%MX241.6"));
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);

    configure(&mut runtime, worked, true);
    write_lreal(&mut runtime, "%IL0", 24.0);
    write_lreal(&mut runtime, "%IL8", 24.0);
    runtime
        .execute_cycle()
        .expect("reject unbounded estimator innovation");
    assert!(!read_bool(&runtime, "%MX241.4"));
    assert!(read_bool(&runtime, "%MX240.6"));
    assert_eq!(read_dint(&runtime, "%MD320"), 83);

    let architecture = [0.0, 1.0, 2.0];
    let error = [
        retained_metric(pi.absolute_error_integral),
        retained_metric(mpc.absolute_error_integral),
        retained_metric(conservative.absolute_error_integral),
    ];
    let movement = [
        retained_metric(pi.movement),
        retained_metric(mpc.movement),
        retained_metric(conservative.movement),
    ];
    let tariff = [
        retained_metric(pi.tariff_cost),
        retained_metric(mpc.tariff_cost),
        retained_metric(conservative.tariff_cost),
    ];
    let mut summary = String::from("architecture,absolute_error_degC_s,tariff_cost_percent_s,movement_pp,comfort_violation_degC_s,verdict\n");
    for (index, (name, metrics)) in [
        ("PI", &pi),
        ("balanced-MPC", &mpc),
        ("conservative-MPC", &conservative),
    ]
    .iter()
    .enumerate()
    {
        let verdict = if index == 0 {
            "selected"
        } else {
            "reject-movement-limit"
        };
        writeln!(
            summary,
            "{name},{:.9},{:.9},{:.9},{:.9},{verdict}",
            retained_metric(metrics.absolute_error_integral),
            retained_metric(metrics.tariff_cost),
            retained_metric(metrics.movement),
            retained_metric(metrics.comfort_violation)
        )
        .unwrap();
    }
    writeln!(
        summary,
        "estimator-residual-rms,{:.9},{:.9},raw-and-corrected,,qualified",
        retained_metric(estimator.uncorrected_rms),
        retained_metric(estimator.corrected_rms)
    )
    .unwrap();
    evidence::retain(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/control/14-integrated-capstones/83-building-hvac/figures"),
        "building-hvac-capstone-evidence",
        "Predictive control does not earn application when actuator movement fails the requirement",
        "Index 0 is strong PI, index 1 balanced MPC, and index 2 actuator-conservative MPC; every candidate meets comfort, but both MPC designs exceed the movement limit.",
        &[
            Panel {
                title: "Temperature integrated absolute error",
                x_unit: "architecture index",
                y_unit: "degC*s",
                x_minimum: -0.1,
                x_maximum: 2.1,
                y_minimum: 0.0,
                y_maximum: 240.0,
                series: vec![Series { name: "Absolute error", color: "#2563eb", dashed: false, x: &architecture, y: &error }],
            },
            Panel {
                title: "Applied-heater movement",
                x_unit: "architecture index",
                y_unit: "percentage points",
                x_minimum: -0.1,
                x_maximum: 2.1,
                y_minimum: 0.0,
                y_maximum: 180.0,
                series: vec![Series { name: "Movement", color: "#dc2626", dashed: false, x: &architecture, y: &movement }],
            },
        ],
        &[RetainedSeries { panel: "Operating cost", name: "Tariff-weighted heating", x: &architecture, y: &tariff, x_unit: "architecture index", y_unit: "cost-percent-s" }],
        &summary,
    );

    eprintln!(
        "building HVAC PI/Balanced/Conservative: IAE={:.2}/{:.2}/{:.2}degC*s, comfort magnitude={:.2}/{:.2}/{:.2}degC*s, tariff cost={:.2}/{:.2}/{:.2}, movement={:.1}/{:.1}/{:.1}pp, peak shared={:.1}/{:.1}/{:.1}%, fallback={:.0}/{:.0}/{:.0}s; mismatch RMS raw/corrected={:.5}/{:.5}degC, bias={:?}",
        pi.absolute_error_integral,
        mpc.absolute_error_integral,
        conservative.absolute_error_integral,
        pi.comfort_violation,
        mpc.comfort_violation,
        conservative.comfort_violation,
        pi.tariff_cost,
        mpc.tariff_cost,
        conservative.tariff_cost,
        pi.movement,
        mpc.movement,
        conservative.movement,
        pi.peak_shared,
        mpc.peak_shared,
        conservative.peak_shared,
        pi.fallback_seconds,
        mpc.fallback_seconds,
        conservative.fallback_seconds,
        estimator.uncorrected_rms,
        estimator.corrected_rms,
        estimator.final_bias,
    );
}
