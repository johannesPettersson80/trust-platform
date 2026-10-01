use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON49: &str = "05-experiments-identification-and-validation/22-excitation-for-dynamics";
/// The three heaters' angular frequencies in rad/s, in the order [slow, middle, fast].
const OMEGAS: [f64; 3] = [0.1, 0.2, 0.4];

/// One 0.1 s update of the three heaters, each as [slow, middle, fast].
#[derive(Clone, Copy, Debug)]
struct Sines {
    command: [f64; 3],
    temperature: [f64; 3],
}

/// `rest` updates at 35 °C with 75 %, then Control Studio's project experiment (activation at %MX24.0) for `updates`.
fn lesson49_run(rest: u64, updates: u64) -> Vec<Sines> {
    let (mut runtime, mut simulation) = setup(LESSON49);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        let read = |address: &str| number(&runtime, address);
        record.push(Sines {
            command: [read("%QL8"), read("%QL0"), read("%QL16")],
            temperature: [read("%IL8"), read("%IL0"), read("%IL16")],
        });
    }
    record
}

#[test]
fn lesson49_each_heater_gets_five_cycles_of_its_bounded_sine() {
    // u = 75 + 5·sin(ω·t) for five periods P = 2π/ω from the start, then 75 % again; never outside 70–80 %.
    let record = lesson49_run(20, 3200);
    assert!(
        record[..20]
            .iter()
            .all(|update| update.command == [75.0; 3]),
        "at rest before the start"
    );
    for (index, omega) in OMEGAS.iter().enumerate() {
        let five_periods = 5.0 * 2.0 * std::f64::consts::PI / omega;
        for (k, update) in record[20..].iter().enumerate() {
            let t = k as f64 * 0.1;
            let expected = if t < five_periods - 1e-9 {
                75.0 + 5.0 * (omega * t).sin()
            } else {
                75.0
            };
            assert!(
                (update.command[index] - expected).abs() < 1e-9,
                "ω = {omega} at {t:.1} s: {update:?}"
            );
            assert!((70.0..=80.0).contains(&update.command[index]));
        }
    }
}

#[test]
fn lesson49_the_settled_cycles_swing_about_the_operating_point() {
    // Cycles 3–5 have left the start behind: they swing about 35 °C by 0.2·5/√(25ω² + 1) = 0.894, 0.707 and 0.447 °C.
    let record = lesson49_run(20, 3200);
    for (index, omega) in OMEGAS.iter().enumerate() {
        let period = 2.0 * std::f64::consts::PI / omega;
        let settled: Vec<f64> = record[20..]
            .iter()
            .enumerate()
            .filter(|(k, _)| {
                let t = *k as f64 * 0.1;
                t >= 2.0 * period + 1.0 && t < 5.0 * period + 1.0
            })
            .map(|(_, update)| update.temperature[index])
            .collect();
        let mean = settled.iter().sum::<f64>() / settled.len() as f64;
        let swing = (settled.iter().cloned().fold(f64::MIN, f64::max)
            - settled.iter().cloned().fold(f64::MAX, f64::min))
            / 2.0;
        let expected = 0.2 * 5.0 / (25.0 * omega * omega + 1.0).sqrt();
        assert!(
            (mean - 35.0).abs() <= 0.01,
            "ω = {omega}: settled cycles centred on 35 °C, mean {mean}"
        );
        assert!(
            (swing - expected).abs() <= 0.03,
            "ω = {omega}: half peak-to-peak {swing} vs {expected}"
        );
    }
}

#[test]
fn lesson49_hmi_only_binds_signals_of_the_executed_project() {
    let (runtime, _) = setup(LESSON49);
    let diagnostics = hmi_diagnostics(LESSON49, &runtime);
    assert!(
        diagnostics.is_empty(),
        "The Lesson 53 HMI must bind the executed project: {diagnostics:?}"
    );
}

const LESSON50: &str = "07-classical-stability-and-frequency-design/46-bode-frequency-response";

/// Least-squares fit of k + a·cos(ωt) + b·sin(ωt): the amplitude √(a² + b²) and the phase of a·cos + b·sin.
fn sine_fit(values: &[f64], omega: f64, dt: f64) -> (f64, f64) {
    let (mut s, mut c) = ([0.0; 3], [[0.0; 3]; 3]);
    for (k, value) in values.iter().enumerate() {
        let t = k as f64 * dt;
        let row = [1.0, (omega * t).cos(), (omega * t).sin()];
        for i in 0..3 {
            s[i] += row[i] * value;
            for j in 0..3 {
                c[i][j] += row[i] * row[j];
            }
        }
    }
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(c);
    let replace = |col: usize| {
        let mut m = c;
        for i in 0..3 {
            m[i][col] = s[i];
        }
        m
    };
    let (a, b) = (det(replace(1)) / d, det(replace(2)) / d);
    ((a * a + b * b).sqrt(), (-b).atan2(a))
}

#[test]
fn lesson50_the_settled_cycles_follow_the_frequency_response() {
    // G(jω) = 0.2·e^(−jω)/(1 + 5jω) at ω = 0.2: M = 0.141421 °C per point and φ = −0.985398 rad. The 0.1 s hold of the
    // command adds half an update of lag, 0.01 rad.
    let (mut runtime, mut simulation) = setup(LESSON50);
    let debug = runtime.enable_debug();
    let (mut commands, mut temperatures) = (Vec::new(), Vec::new());
    for step in 0..20 + 1575 {
        if step == 20 {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        if step >= 20 {
            commands.push(number(&runtime, "%QL0"));
            temperatures.push(number(&runtime, "%IL0"));
        }
    }
    assert!(
        commands.iter().all(|u| (70.0..=80.0).contains(u)),
        "unclipped 75 ± 5 %"
    );
    let period = 2.0 * std::f64::consts::PI / 0.2;
    let settled = (2.0 * period / 0.1).round() as usize..(5.0 * period / 0.1).round() as usize;
    let (input_amplitude, input_phase) = sine_fit(&commands[settled.clone()], 0.2, 0.1);
    let (output_amplitude, output_phase) = sine_fit(&temperatures[settled], 0.2, 0.1);
    let magnitude = output_amplitude / input_amplitude;
    let phase = output_phase - input_phase;
    assert!(
        (input_amplitude - 5.0).abs() < 0.01,
        "command amplitude {input_amplitude}"
    );
    assert!((magnitude - 0.141421).abs() <= 0.0015, "M = {magnitude}");
    assert!((phase + 0.985398 + 0.01).abs() <= 0.01, "φ = {phase}");
}

const LESSON51: &str = "07-classical-stability-and-frequency-design/48-sensitivity-functions";

/// Both PI-controlled heaters, `rest` updates at 35 °C and then the project experiment (activation at %MX24.0): the
/// first heater receives D = 10·sin(0.2t) kW, the second's controller reads its temperature plus 0.05·sin(0.2t) °C.
/// Returns the settled cycles 3–5 of [load temperature, load command, noise-heater temperature, noise-heater command,
/// measured temperature].
fn lesson51_settled() -> [Vec<f64>; 5] {
    let (mut runtime, mut simulation) = setup(LESSON51);
    let debug = runtime.enable_debug();
    let mut record: [Vec<f64>; 5] = Default::default();
    let period = 2.0 * std::f64::consts::PI / 0.2;
    for step in 0..20 + 1575 {
        if step == 20 {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        let t = (step as f64 - 20.0) * 0.1;
        if t >= 2.0 * period && t < 5.0 * period - 0.05 {
            for (series, address) in record
                .iter_mut()
                .zip(["%IL0", "%QL0", "%IL8", "%QL16", "%ML8"])
            {
                series.push(number(&runtime, address));
            }
        }
    }
    record
}

#[test]
fn lesson51_the_heat_load_reaches_the_water_through_s_gd() {
    // L = C·G = 0.2·e^(−s)/s with C = 5 + 1/s: at 0.2 rad/s |S| = 0.78991 and |G_D| = 0.1/√2, so 10 kW moves the water
    // by 10·0.070711·0.78991 = 0.5586 °C and the command by |C·S·G_D|·10 = 3.95 %.
    let [temperature, command, ..] = lesson51_settled();
    let (water, _) = sine_fit(&temperature, 0.2, 0.1);
    let (heater, _) = sine_fit(&command, 0.2, 0.1);
    assert!(
        (water - 0.5586).abs() <= 0.02,
        "load: water amplitude {water}"
    );
    assert!(
        (heater - 3.950).abs() <= 0.15,
        "load: command amplitude {heater}"
    );
}

#[test]
fn lesson51_measurement_noise_moves_the_command_more_than_the_water() {
    // n = 0.05 °C: the water moves by |T|·0.05 = 0.0395 °C, the reading by |S|·0.05 = 0.0395 °C and the command by
    // |C·S|·0.05 = 0.2793 %.
    let [.., temperature, command, measured] = lesson51_settled();
    let (water, _) = sine_fit(&temperature, 0.2, 0.1);
    let (heater, _) = sine_fit(&command, 0.2, 0.1);
    let (reading, _) = sine_fit(&measured, 0.2, 0.1);
    assert!(
        (water - 0.0395).abs() <= 0.002,
        "noise: water amplitude {water}"
    );
    assert!(
        (reading - 0.0395).abs() <= 0.002,
        "noise: reading amplitude {reading}"
    );
    assert!(
        (heater - 0.2793).abs() <= 0.012,
        "noise: command amplitude {heater}"
    );
}
