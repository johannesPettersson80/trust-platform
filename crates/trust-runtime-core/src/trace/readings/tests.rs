//! The readings' rules (specification 48 RU-59, RU-60): kept while armed and taken in order,
//! played back in order, and the same wherever the readings are kept.

use alloc::vec::Vec;

use super::{arm, disarm, record, take, State, ROOM};

#[test]
fn a_threads_readings_are_kept_while_armed_and_taken_in_order() {
    // Catches a reading kept while no recording runs, readings taken out of order or twice, and
    // readings past the room that a recording would miss without knowing.
    record(1);
    arm();
    record(10);
    record(20);
    let mut taken = Vec::new();
    assert_eq!(take(&mut |reading| taken.push(reading)), 2);
    assert_eq!(taken, [10, 20]);
    assert_eq!(take(&mut |_| panic!("taken twice")), 0);
    for reading in 0..=ROOM {
        record(i64::try_from(reading).unwrap());
    }
    let mut handed = 0;
    assert_eq!(take(&mut |_| handed += 1), ROOM + 1);
    assert_eq!(handed, ROOM);
    disarm();
    record(30);
    assert_eq!(take(&mut |_| panic!("kept while disarmed")), 0);
}

#[test]
fn a_replay_plays_a_cycles_readings_in_order_and_the_last_again() {
    // Specification 48 RU-60, "Each cycle": a replay gives the build the trace's wall-clock
    // readings in order, the last one again when the build reads the clock more often. Worked
    // by hand: a cycle with 10 and 20 reads 10, 20, 20; a cycle without readings reads 20 again;
    // a stopped replay plays nothing. Catches readings played out of order, a clock that runs
    // dry, and a replay that leaves the wall clock replaced.
    use super::{play, played, stop_playing};
    assert_eq!(played(), None);
    play(&[10, 20]);
    assert_eq!(
        [played(), played(), played()],
        [Some(10), Some(20), Some(20)]
    );
    play(&[]);
    assert_eq!(played(), Some(20));
    stop_playing();
    assert_eq!(played(), None);
}

#[test]
fn the_rules_are_the_same_wherever_the_readings_are_kept() {
    // Specification 48 RU-59, "Every runtime": a chip keeps its readings with the computer's
    // rules, in one place under a critical section instead of each thread's own. The rules live
    // in `State`, which both keep, so this checks them on a State of its own, as a chip holds
    // it. Worked by hand: unarmed, 1 is not kept; armed, 10 and 20 are taken in order, then
    // nothing; ROOM + 1 readings give ROOM and count ROOM + 1; disarmed, 30 is not kept; a replay
    // of 7 and 8 reads 7, 8, 8, and nothing once stopped. Catches a rule that only the computer's
    // store keeps.
    let mut state = State::new();
    state.record(1);
    state.arm();
    state.record(10);
    state.record(20);
    let mut taken = Vec::new();
    assert_eq!(state.take(&mut |reading| taken.push(reading)), 2);
    assert_eq!(taken, [10, 20]);
    assert_eq!(state.take(&mut |_| panic!("taken twice")), 0);
    for reading in 0..=ROOM {
        state.record(i64::try_from(reading).unwrap());
    }
    let mut handed = 0;
    assert_eq!(state.take(&mut |_| handed += 1), ROOM + 1);
    assert_eq!(handed, ROOM);
    state.armed = false;
    state.record(30);
    assert_eq!(state.take(&mut |_| panic!("kept while disarmed")), 0);
    state.play(&[7, 8]);
    assert_eq!(
        [state.played(), state.played(), state.played()],
        [Some(7), Some(8), Some(8)]
    );
    state.stop_playing();
    assert_eq!(state.played(), None);
}

#[test]
fn a_computer_keeps_a_replays_room_beside_the_recordings() {
    // Specification 48 RU-60: a replay runs in a simulator on a computer, so a computer's store keeps
    // the room a replay plays from beside the recording's room, while a chip's keeps the recording's
    // alone (the compile-time guard of a build without `std`). Catches a computer's store that drops
    // the replay's room with the chip's, which would leave a replay no readings to play.
    let room = ROOM * core::mem::size_of::<i64>();
    assert!(core::mem::size_of::<State>() >= 2 * room);
}
