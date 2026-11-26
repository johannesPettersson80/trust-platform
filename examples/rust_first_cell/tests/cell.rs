//! The press cell's machine tests (specification 48 RU-58): the cell's own program and its
//! simulated clamp and press run under `cargo test` with trust-sim, on a clock that only the test
//! moves. Every expected value is worked out by hand from the project's files:
//!
//! - `runtime.toml` and `trust.toml`: the resource runs a cycle every 10 ms from 0 ms, and
//!   `CellTask` runs the program `Cell` in every one;
//! - `simulation.toml`: the clamp's sensor follows its valve 300 ms later and the press's 800 ms
//!   later, and a sensor's change is in the inputs from the first cycle at which it is due
//!   (specification 46 §2);
//! - `src/lib.rs`: the sequence and its times. A wait completes at the first later cycle at whose
//!   start its condition holds, `HOLD` at the first cycle 500 ms or more after it began, and a wait
//!   that lasts longer than its `.within` faults at the first cycle more than that after it began
//!   (specification 48 RU-72). The fault names the wait, its place among the sequence's waits from
//!   1, then the limit and the time waited in milliseconds.
//!
//! A test writes the part sensor and the Start button as Live Values writes an input: the simulated
//! input keeps the value until the next write.

use std::time::Duration;

use trust_sim::{Error, Result, Simulator};

/// The cell after its first cycle, at 0 ms, where the sequence entered `READY`.
fn cell() -> Result<Simulator> {
    let mut cell = trust_sim::Project::open(".")?.simulator()?;
    cell.run_cycles(1)?;
    Ok(cell)
}

/// Puts a part in the fixture and presses Start, as the operator does: the next cycle sees both.
fn load_and_press_start(cell: &mut Simulator) -> Result<()> {
    cell.write("Cell.part_present", true)?;
    cell.write("Cell.start", true)
}

/// A logical time in whole milliseconds.
fn millis(time: Duration) -> u64 {
    u64::try_from(time.as_millis()).expect("a test's time fits in milliseconds")
}

/// Runs the cell one cycle at a time until `until` and gives each step the sequence entered, with
/// the time of the cycle that entered it in milliseconds. The operator lets go of Start after the
/// first cycle. At every cycle it checks the press's interlock: the press's valve is open only
/// while the clamp's sensor says that the clamp is closed.
fn steps_until(cell: &mut Simulator, until: Duration) -> Result<Vec<(u64, String)>> {
    let mut steps = Vec::new();
    let mut last: String = cell.read("Cell.active_step")?;
    let mut released = false;
    while cell.now() < until {
        cell.run_cycles(1)?;
        if !released {
            cell.write("Cell.start", false)?;
            released = true;
        }
        let press: bool = cell.read("Cell.press")?;
        let clamped: bool = cell.read("Cell.clamp_closed")?;
        assert!(
            !press || clamped,
            "the press's valve is open at {} ms, while the clamp is not closed",
            millis(cell.now())
        );
        let step: String = cell.read("Cell.active_step")?;
        if step != last {
            steps.push((millis(cell.now()), step.clone()));
            last = step;
        }
    }
    Ok(steps)
}

#[test]
fn a_part_is_clamped_pressed_held_lifted_and_unclamped_in_turn() -> Result<()> {
    // The part and Start come in before the cycle at 10 ms, which leaves READY for CLAMP and opens
    // the clamp's valve. The clamp's sensor follows 300 ms later, at 310 ms: PRESS. The press's
    // sensor follows 800 ms later, at 1,110 ms: HOLD, which ends at the first cycle 500 ms later,
    // at 1,610 ms: LIFT. The press's sensor clears 800 ms later, at 2,410 ms: UNCLAMP, and the
    // clamp's 300 ms after that, at 2,710 ms: UNLOAD, the part counted. Taking the part out makes
    // the cell READY at the next cycle. Catches a step out of order, a step that does not wait for
    // its sensor, a hold of another length, a press that moves before the clamp is closed, and a
    // part counted before the clamp is open.
    let mut cell = cell()?;
    assert_eq!(cell.read::<String>("Cell.active_step")?, "READY");
    assert!(cell.read::<bool>("Cell.ready")?, "the Ready lamp is on");

    load_and_press_start(&mut cell)?;
    let steps = steps_until(&mut cell, Duration::from_millis(3_000))?;
    let expected = [
        (10, "CLAMP"),
        (310, "PRESS"),
        (1_110, "HOLD"),
        (1_610, "LIFT"),
        (2_410, "UNCLAMP"),
        (2_710, "UNLOAD"),
    ]
    .map(|(time, step)| (time, step.to_string()));
    assert_eq!(steps, expected);
    assert_eq!(cell.read::<u32>("Cell.parts")?, 1);
    assert!(
        !cell.read::<bool>("Cell.ready")?,
        "the Ready lamp stays off until the part is out"
    );

    cell.write("Cell.part_present", false)?;
    cell.run_cycles(1)?;
    assert_eq!(cell.read::<String>("Cell.active_step")?, "READY");
    assert!(
        cell.read::<bool>("Cell.ready")?,
        "the Ready lamp is on again"
    );
    Ok(())
}

#[test]
fn start_without_a_part_leaves_the_cell_ready() -> Result<()> {
    // READY waits for a part and Start at the start of one cycle. With Start alone the sequence
    // stays in READY, its lamp on, and the clamp's valve stays closed, however long Start is held.
    // Catches a cell that clamps an empty fixture.
    let mut cell = cell()?;
    cell.write("Cell.start", true)?;
    cell.run_for(Duration::from_secs(2))?;
    assert_eq!(cell.read::<String>("Cell.active_step")?, "READY");
    assert!(cell.read::<bool>("Cell.ready")?);
    assert!(!cell.read::<bool>("Cell.clamp")?);
    Ok(())
}

#[test]
fn a_press_that_does_not_come_down_faults_the_cell_at_its_limit() -> Result<()> {
    // The press's sensor is broken: forced FALSE, it never sees the press down. PRESS's wait, the
    // sequence's third, begins at 310 ms with a limit of 2,000 ms. A wait of exactly 2,000 ms is
    // still in time, so the first cycle more than that after it began, at 2,320 ms, faults after a
    // wait of 2,010 ms: `Rust fault 3 (2000, 2010)`. Catches a limit that never runs out, one that
    // runs out a cycle early or late, and a fault that names another wait or other times.
    let mut cell = cell()?;
    cell.force("Cell.press_down", false)?;
    load_and_press_start(&mut cell)?;
    cell.run_cycles(1)?;
    cell.write("Cell.start", false)?;
    match cell.run_for(Duration::from_secs(3)) {
        Err(Error::Fault { time, fault, .. }) => {
            assert_eq!(millis(time), 2_320);
            assert_eq!(fault, "Rust fault 3 (2000, 2010)");
        }
        other => panic!("the press's wait should fault at 2,320 ms: {other:?}"),
    }
    assert_eq!(cell.read::<String>("Cell.active_step")?, "PRESS");
    Ok(())
}

#[test]
fn a_clamp_that_does_not_close_faults_the_cell_before_the_press_moves() -> Result<()> {
    // The clamp's sensor is broken: forced FALSE. CLAMP's wait, the sequence's second, begins at
    // 10 ms with a limit of 1,000 ms, and the cycle at 1,020 ms faults after a wait of 1,010 ms:
    // `Rust fault 2 (1000, 1010)`. The program never opened the press's valve. Catches a press that
    // moves without the clamp, and a clamp's limit of another length.
    let mut cell = cell()?;
    cell.force("Cell.clamp_closed", false)?;
    load_and_press_start(&mut cell)?;
    cell.run_cycles(1)?;
    cell.write("Cell.start", false)?;
    match cell.run_for(Duration::from_secs(2)) {
        Err(Error::Fault { time, fault, .. }) => {
            assert_eq!(millis(time), 1_020);
            assert_eq!(fault, "Rust fault 2 (1000, 1010)");
        }
        other => panic!("the clamp's wait should fault at 1,020 ms: {other:?}"),
    }
    assert_eq!(cell.read::<String>("Cell.active_step")?, "CLAMP");
    assert!(!cell.read::<bool>("Cell.press")?, "the press never moved");
    Ok(())
}
