//! Tests of the test signals (N2.8): the closed-form properties of a maximal-length PRBS, the masking of its seed, the
//! exact sum of a multisine, its common scaling to a peak bound, and the refusals. The TypeScript case is
//! `control-studio-numerics.test.ts` ("generates bounded excitation ...": the PRBS and the bounded multisine).

#![allow(clippy::float_cmp)] // The sequence values are exact copies of the amplitude.

use std::f64::consts::PI;

use super::{generate_multisine, generate_prbs, ExcitationRejection, Tone};

/// The period of a seven-bit maximal-length register.
const PERIOD: usize = 127;

#[test]
fn a_seven_bit_prbs_is_a_maximal_length_sequence() {
    let amplitude = 0.8;
    let sequence = generate_prbs(7, 2 * PERIOD, amplitude, 0x5d).expect("a valid PRBS");
    assert_eq!(sequence.scale, 1.0);
    let (first, second) = sequence.values.split_at(PERIOD);
    // The sequence repeats every 2⁷ − 1 values.
    assert_eq!(first, second);
    // Over one period the register visits every nonzero state once; 64 of them have their least significant bit set.
    let high = first.iter().filter(|&&value| value == amplitude).count();
    assert_eq!(high, 64);
    assert!(first
        .iter()
        .all(|&value| value == amplitude || value == -amplitude));
    // The periodic autocorrelation of a maximal-length sequence is −a² at every lag that is not a multiple of the period.
    for lag in 1..PERIOD {
        let correlation: f64 = (0..PERIOD)
            .map(|index| first[index] * first[(index + lag) % PERIOD])
            .sum();
        assert!(
            (correlation + amplitude * amplitude).abs() < 1e-9,
            "lag {lag}: {correlation}"
        );
    }
}

#[test]
fn the_seed_is_masked_to_the_register_order() {
    // 221 = 93 + 128: the bit above a seven-bit register is dropped, so both seeds give the same sequence.
    let direct = generate_prbs(7, 64, 1.0, 93).expect("valid");
    let masked = generate_prbs(7, 64, 1.0, 221).expect("valid");
    assert_eq!(direct, masked);
    // A seed whose low seven bits are zero is refused.
    assert_eq!(
        generate_prbs(7, 64, 1.0, 128),
        Err(ExcitationRejection::InvalidConfiguration)
    );
}

#[test]
fn an_invalid_prbs_declaration_is_refused() {
    let invalid = ExcitationRejection::InvalidConfiguration;
    assert_eq!(generate_prbs(1, 64, 1.0, 1), Err(invalid));
    assert_eq!(generate_prbs(17, 64, 1.0, 1), Err(invalid));
    assert_eq!(generate_prbs(7, 1, 1.0, 1), Err(invalid));
    assert_eq!(generate_prbs(7, 64, 0.0, 1), Err(invalid));
    assert_eq!(generate_prbs(7, 64, f64::NAN, 1), Err(invalid));
    assert!(generate_prbs(2, 2, 1.0, 1).is_ok());
    assert!(generate_prbs(16, 2, 1.0, 1).is_ok());
}

#[test]
fn a_single_tone_multisine_is_the_declared_sine() {
    // Closed form: values[n] = A·sin(ω·n·Ts + φ), and nothing exceeds the bound, so the scale is one.
    let tone = Tone {
        frequency_rad_per_second: 1.0,
        amplitude: 0.5,
        phase_radians: 0.3,
    };
    let excitation = generate_multisine(&[tone], 0.1, 20, 1.0).expect("a valid multisine");
    assert_eq!(excitation.scale, 1.0);
    assert_eq!(excitation.values.len(), 20);
    for (index, value) in (0..20_u32).zip(excitation.values.iter()) {
        let expected = 0.5 * (0.1 * f64::from(index) + 0.3).sin();
        assert!((value - expected).abs() < 1e-12, "sample {index}: {value}");
    }
}

#[test]
fn a_multisine_above_its_bound_is_scaled_by_one_common_factor() {
    let tones = [
        Tone {
            frequency_rad_per_second: 1.0,
            amplitude: 2.0,
            phase_radians: 0.0,
        },
        Tone {
            frequency_rad_per_second: 2.0,
            amplitude: 1.0,
            phase_radians: 0.5,
        },
    ];
    let raw: Vec<f64> = (0..50)
        .map(|index| {
            let time = f64::from(index) * 0.1;
            tones
                .iter()
                .map(|tone| {
                    tone.amplitude
                        * (tone.frequency_rad_per_second * time + tone.phase_radians).sin()
                })
                .sum::<f64>()
        })
        .collect();
    let peak = raw
        .iter()
        .fold(0.0_f64, |largest, value| largest.max(value.abs()));
    assert!(peak > 1.0, "the declared multisine must exceed its bound");
    let excitation = generate_multisine(&tones, 0.1, 50, 1.0).expect("a valid multisine");
    assert!((excitation.scale - 1.0 / peak).abs() < 1e-12);
    for (value, raw_value) in excitation.values.iter().zip(&raw) {
        assert!((value - raw_value / peak).abs() < 1e-12);
    }
    let bounded = excitation
        .values
        .iter()
        .fold(0.0_f64, |largest, value| largest.max(value.abs()));
    assert!((bounded - 1.0).abs() < 1e-12);
}

#[test]
fn an_invalid_or_aliasing_multisine_is_refused() {
    let tone = Tone {
        frequency_rad_per_second: 1.0,
        amplitude: 1.0,
        phase_radians: 0.0,
    };
    let invalid = ExcitationRejection::InvalidConfiguration;
    assert_eq!(generate_multisine(&[], 0.1, 20, 1.0), Err(invalid));
    assert_eq!(generate_multisine(&[tone], 0.1, 7, 1.0), Err(invalid));
    assert_eq!(generate_multisine(&[tone], 0.1, 20, 0.0), Err(invalid));
    assert_eq!(generate_multisine(&[tone], 0.0, 20, 1.0), Err(invalid));
    let zero_amplitude = Tone {
        amplitude: 0.0,
        ..tone
    };
    assert_eq!(
        generate_multisine(&[zero_amplitude], 0.1, 20, 1.0),
        Err(invalid)
    );
    let at_nyquist = Tone {
        frequency_rad_per_second: PI / 0.1,
        ..tone
    };
    assert_eq!(
        generate_multisine(&[at_nyquist], 0.1, 20, 1.0),
        Err(ExcitationRejection::AliasRisk)
    );
}
