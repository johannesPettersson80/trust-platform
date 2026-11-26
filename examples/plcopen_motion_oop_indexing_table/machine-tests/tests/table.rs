//! The indexing table's stations, in `plcopen_motion_oop_indexing_table`'s own program and the PLCopen
//! Motion OOP library's simulated axis (specification 48 RU-58). Every expected value comes from the
//! project's files:
//!
//! - `runtime.toml`: the resource runs `Main` every 10 ms;
//! - `src/Main.st`: the first cycle binds the axis object and the program powers the axis (step 10);
//!   then each index moves the axis to `90.0 * (StationIndex + 1)` with `MoveAbsolute` (step 20),
//!   takes the command's `Execute` back with `Release` (step 21), which ends the move at once, as the
//!   package's virtual axis has no motion profile yet, and on the command's `Done` counts the index
//!   and takes the next station, after the fourth the first again.
//!   So the table stops at 90.0, 180.0, 270.0 and 360.0, and then at 90.0 again.

use trust_sim::{Result, Simulator};

/// The positions of the first five stations the table stops at, by `Main.st`'s formula.
const STATIONS: [f32; 5] = [90.0, 180.0, 270.0, 360.0, 90.0];

/// The most cycles a test runs: 10 minutes of the table's time, far more than five moves take, even
/// at `Main.st`'s velocity of 30.0 a second.
const LIMIT_CYCLES: u32 = 60_000;

/// A simulator of this project, the folder above this crate.
fn table() -> Result<Simulator> {
    trust_sim::Project::open("..")?.simulator()
}

/// The indexes the program has completed.
fn completed(table: &mut Simulator) -> Result<u32> {
    table.read("Main.CompletedIndexes")
}

/// The axis's position as the axis object last read it (`Refresh`).
fn position(table: &mut Simulator) -> Result<f32> {
    table.read("Main.TableAxis.ActualPositionValue")
}

#[test]
fn the_table_stops_at_each_station_in_turn() -> Result<()> {
    // Runs cycle by cycle and, at each cycle that counts an index, takes the axis's position. The
    // command is Done when the axis has arrived, so each count finds the axis at its station: 90.0,
    // 180.0, 270.0, 360.0 and 90.0 again, with no error. Catches a move that never starts (an
    // Execute that stays TRUE, so that the next MoveAbsolute sees no rising edge), a count before the
    // axis arrives, a wrong station, and a table that does not start again from the first station.
    let mut table = table()?;
    let mut stops = Vec::new();
    let mut counted = 0;
    let mut cycles = 0;
    while stops.len() < STATIONS.len() && cycles < LIMIT_CYCLES {
        table.run_cycles(1)?;
        cycles += 1;
        let now = completed(&mut table)?;
        if now != counted {
            assert_eq!(now, counted + 1, "one index a cycle at most");
            counted = now;
            stops.push(position(&mut table)?);
        }
    }
    assert_eq!(
        stops.len(),
        STATIONS.len(),
        "five indexes within {cycles} cycles: {stops:?}"
    );
    for (stop, station) in stops.iter().zip(STATIONS) {
        assert!(
            (stop - station).abs() < 0.001,
            "the stops {stops:?}, the stations {STATIONS:?}"
        );
    }
    assert_eq!(table.read::<u16>("Main.LastError")?, 0, "no error");
    Ok(())
}
