//! The dosing pump's fault (specification 48 RU-59 and RU-60). The simulation's stroke sensor
//! counts 60 strokes a minute from the start, so the program doses 1,200 / 60 = 20 a stroke; at 5 s
//! its wire breaks and it reads 0, and the program divides by zero in that cycle. The project keeps
//! a flight recorder of 300 cycles: its first recording holds the cycles at 0 to 2,990 ms and
//! hands over, the second holds 3,000 ms to the faulted 5,000 ms, so the fault's trace holds those
//! 501 cycles. Replayed in a new simulator of the project, the trace shows the fault again: every
//! cycle's outputs and the fault match. A change that guards the division would make the replay
//! differ at the faulted cycle (the example's README, "Try it yourself").

use std::time::Duration;

#[test]
fn a_broken_stroke_sensor_faults_the_program_and_its_trace_shows_the_fault_again()
-> trust_sim::Result<()> {
    let project = trust_sim::Project::open("..")?;
    let mut sim = project.simulator()?;
    sim.run_for(Duration::from_secs(4))?;
    assert_eq!(sim.read::<i16>("DosePerStroke")?, 20);
    assert_eq!(sim.fault_trace()?, None, "no fault yet");

    let faulted = sim.run_for(Duration::from_secs(2));
    assert!(
        matches!(faulted, Err(trust_sim::Error::Fault { time, .. }) if time == Duration::from_millis(5_000)),
        "{faulted:?}"
    );
    let trace = sim.fault_trace()?.expect("the flight recorder's trace of the fault");
    let replayed = project.simulator()?.replay(&trace)?;
    assert!(replayed.matched, "{replayed}");
    assert_eq!(
        replayed.to_string(),
        "Replayed 501 cycles of the same program: every cycle's outputs match the trace.\n"
    );
    Ok(())
}
