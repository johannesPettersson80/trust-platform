//! The room heating's start, stop and low-water protection, in `room_heating`'s own program and its
//! simulated plant (specification 48 RU-58). Every expected value comes from the project's files:
//!
//! - `runtime.toml`: the resource runs a cycle every 10 ms; `src/config.st`: `MainTask` runs `Main`
//!   and `Simulation` every 100 ms, so the program runs at every tenth cycle, at each whole 100 ms;
//! - `src/HeatingSequence.st`: Start runs the pump (step 10), the heater once the sequence sees
//!   flow (step 20), and the room loop once the tank is ready; Stop stops the heater at once and runs
//!   the pump on for `PumpOverrun`, 30 s; low water fails the sequence, and Reset returns it to Idle;
//! - `src/Main.st`: each run hands the sequence's outputs to the pump and the heater, and FS-101's
//!   value to the sequence, before it runs them, so each follows one run later;
//! - `simulation.toml`: FS-101 sees flow 4,000 ms after the pump's output, and the Simulation
//!   program's `LowWater` drives the low-water switch.

use std::time::Duration;

use trust_sim::{Inputs, Result, Simulator};

/// The interval of `MainTask`, which runs the program (`src/config.st`).
const TASK: Duration = Duration::from_millis(100);

/// A simulator of this project, the folder above this crate, after its first cycle, at 0 ms, where
/// the program ran once.
fn heating_plant() -> Result<Simulator> {
    let mut plant = trust_sim::Project::open("..")?.simulator()?;
    plant.run_cycles(1)?;
    Ok(plant)
}

/// Presses a command of the heating's sequence, as its HMI button does, and answers whether the
/// sequence took it.
fn press(plant: &mut Simulator, command: &str) -> Result<bool> {
    plant.call("Main.Heating", "Plant.ISequence", command, Inputs::new())
}

/// The sequence's state, as its interface answers it (`GetState`).
fn sequence_state(plant: &mut Simulator) -> Result<String> {
    plant.call("Main.Heating", "Plant.ISequence", "GetState", Inputs::new())
}

/// Runs the program's runs one at a time until `holds` reads true, at most `limit` of logical time,
/// and answers the logical time of the run where it first held.
fn first_run_where(
    plant: &mut Simulator,
    limit: Duration,
    mut holds: impl FnMut(&mut Simulator) -> Result<bool>,
) -> Result<Option<Duration>> {
    let end = plant.now() + limit;
    while plant.now() < end {
        plant.run_for(TASK)?;
        if holds(plant)? {
            return Ok(Some(plant.now()));
        }
    }
    Ok(None)
}

#[test]
fn a_start_runs_the_pump_first_and_the_heater_only_once_flow_is_confirmed() -> Result<()> {
    // Start puts the sequence in step 10 at the next run, and the pump's output follows one run
    // later: 200 ms after the press. FS-101 sees flow 4,000 ms after the pump's output, at the run
    // of 4,200 ms or the next; the sequence sees it one run later and goes to step 20, and the
    // heater's output follows one run after that. At no run is the heater on without flow. Catches
    // a heater that starts with the pump, a pump that does not start, and a sequence that does not
    // wait for flow.
    let mut plant = heating_plant()?;
    let pressed = plant.now();
    assert!(press(&mut plant, "Start")?);

    let pump = first_run_where(&mut plant, Duration::from_secs(1), |plant| {
        plant.read::<bool>("Main.P101_Run")
    })?
    .expect("the pump starts");
    assert_eq!(pump, pressed + 2 * TASK);

    let flow = first_run_where(&mut plant, Duration::from_secs(5), |plant| {
        let flowing = plant.read::<bool>("Main.FS101_Flow")?;
        if plant.read::<bool>("Main.EH101_Contactor")? {
            assert!(
                flowing,
                "the heater runs only with flow, at {:?}",
                plant.now()
            );
        }
        Ok(flowing)
    })?
    .expect("FS-101 confirms flow");
    assert!(
        flow == pump + Duration::from_millis(4_000) || flow == pump + Duration::from_millis(4_100),
        "flow at {flow:?}, the pump at {pump:?}"
    );
    assert_eq!(plant.read::<i16>("Main.Heating.CurrentStep")?, 10);

    plant.run_for(TASK)?;
    assert_eq!(plant.read::<i16>("Main.Heating.CurrentStep")?, 20);
    assert!(!plant.read::<bool>("Main.EH101_Contactor")?);
    plant.run_for(TASK)?;
    assert!(
        plant.read::<bool>("Main.EH101_Contactor")?,
        "the heater starts"
    );
    Ok(())
}

#[test]
fn a_stop_ends_the_heating_at_once_and_runs_the_pump_on_for_30_s() -> Result<()> {
    // Once the heater runs, Stop puts the sequence in Stopping at the next run, and the heater's
    // output follows one run later. The pump runs on while the step has lasted less than 30 s: it
    // still runs 29 s after the press, and 31 s after it the pump has stopped and the sequence is
    // Stopped. Catches a heater left on after a stop, and a pump that stops before the hot water has
    // left the heater, or never.
    let mut plant = heating_plant()?;
    assert!(press(&mut plant, "Start")?);
    first_run_where(&mut plant, Duration::from_secs(6), |plant| {
        plant.read::<bool>("Main.EH101_Contactor")
    })?
    .expect("the heater starts");

    let pressed = plant.now();
    assert!(press(&mut plant, "Stop")?);
    plant.run_for(2 * TASK)?;
    assert!(!plant.read::<bool>("Main.EH101_Contactor")?);
    assert_eq!(plant.read::<String>("Main.Heating.State")?, "Stopping");

    plant.run_for(pressed + Duration::from_secs(29) - plant.now())?;
    assert!(plant.read::<bool>("Main.P101_Run")?, "the pump runs on");
    plant.run_for(Duration::from_secs(2))?;
    assert!(
        !plant.read::<bool>("Main.P101_Run")?,
        "the pump has stopped"
    );
    assert_eq!(plant.read::<String>("Main.Heating.State")?, "Stopped");
    Ok(())
}

#[test]
fn low_water_fails_the_heating_and_reset_returns_it_to_idle() -> Result<()> {
    // The Simulation program's LowWater drives T-101's low-water switch. Raised while the heating
    // runs, it fails the sequence with 'T-101 water low', and the pump stops. Reset is refused while
    // the sequence runs and taken once it has failed, and returns it to Idle at once. Catches a pump
    // left running on a dry tank, a failure without its reason, and a Reset that does not return the
    // sequence to Idle.
    let mut plant = heating_plant()?;
    assert!(press(&mut plant, "Start")?);
    plant.run_for(3 * TASK)?;
    assert!(plant.read::<bool>("Main.P101_Run")?);
    assert!(
        !press(&mut plant, "Reset")?,
        "a running sequence takes no Reset"
    );

    plant.write("Simulation.LowWater", true)?;
    first_run_where(&mut plant, Duration::from_secs(1), |plant| {
        Ok(plant.read::<String>("Main.Heating.State")? == "Failed")
    })?
    .expect("the sequence fails");
    assert_eq!(
        plant.read::<String>("Main.Heating.Message")?,
        "T-101 water low"
    );
    plant.run_for(2 * TASK)?;
    assert!(
        !plant.read::<bool>("Main.P101_Run")?,
        "the pump stops on low water"
    );

    plant.write("Simulation.LowWater", false)?;
    plant.run_for(2 * TASK)?;
    assert!(press(&mut plant, "Reset")?);
    assert_eq!(sequence_state(&mut plant)?, "Idle");
    Ok(())
}
