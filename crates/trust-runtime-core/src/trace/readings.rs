//! The wall clock's readings that a cycle trace records (specification 48 RU-59): while a recording
//! runs on a thread, the platform's wall clock reports each reading of `CURRENT_DT` here, and the
//! recorder takes them at the cycle's end. A computer runs each resource's scans on one thread, so
//! each thread keeps its own readings. It allocates nothing: there is room for [`ROOM`] readings in a
//! cycle, and the readings past them are counted, so that the recording ends `memory_full`.

use std::cell::{Cell, RefCell};

/// The readings kept between two takes.
pub const ROOM: usize = 256;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static READINGS: RefCell<([i64; ROOM], usize)> = const { RefCell::new(([0; ROOM], 0)) };
    static PLAYING: Cell<bool> = const { Cell::new(false) };
    static PLAYBACK: RefCell<Playback> = const {
        RefCell::new(Playback {
            readings: [0; ROOM],
            count: 0,
            next: 0,
            last: None,
        })
    };
}

/// The readings a replay gives this thread's wall clock ([`play`]).
struct Playback {
    readings: [i64; ROOM],
    count: usize,
    next: usize,
    last: Option<i64>,
}

/// Starts keeping this thread's readings, from none.
pub fn arm() {
    READINGS.with(|readings| readings.borrow_mut().1 = 0);
    ARMED.with(|armed| armed.set(true));
}

/// Stops keeping this thread's readings.
pub fn disarm() {
    ARMED.with(|armed| armed.set(false));
}

/// A reading of the wall clock in UTC milliseconds, kept while this thread is armed.
pub fn record(millis: i64) {
    if !ARMED.with(Cell::get) {
        return;
    }
    READINGS.with(|readings| {
        let mut readings = readings.borrow_mut();
        let count = readings.1;
        if let Some(slot) = readings.0.get_mut(count) {
            *slot = millis;
        }
        readings.1 = count.saturating_add(1);
    });
}

/// Hands each kept reading to `each`, in the order they were read, and keeps none. It gives how many
/// readings there were: more than it handed when they outgrew the room.
pub fn take(each: &mut dyn FnMut(i64)) -> usize {
    READINGS.with(|readings| {
        let mut readings = readings.borrow_mut();
        let count = readings.1;
        for reading in &readings.0[..count.min(ROOM)] {
            each(*reading);
        }
        readings.1 = 0;
        count
    })
}

/// Gives this thread's wall clock the readings of a recorded cycle, which a replay gives the build
/// (specification 48 RU-60): [`played`] hands them out in order, then the last of them again when
/// the build reads the clock more often. Readings past the room are not played.
pub fn play(readings: &[i64]) {
    PLAYBACK.with(|playback| {
        let mut playback = playback.borrow_mut();
        let count = readings.len().min(ROOM);
        playback.readings[..count].copy_from_slice(&readings[..count]);
        playback.count = count;
        playback.next = 0;
    });
    PLAYING.with(|playing| playing.set(true));
}

/// The next reading a replay gives this thread's wall clock ([`play`]): the cycle's readings in
/// order, then the last reading played again, from an earlier cycle when this one has none; `None`
/// while no replay plays, or before it has played any.
#[must_use]
pub fn played() -> Option<i64> {
    if !PLAYING.with(Cell::get) {
        return None;
    }
    PLAYBACK.with(|playback| {
        let mut playback = playback.borrow_mut();
        if playback.next < playback.count {
            let reading = playback.readings[playback.next];
            playback.next += 1;
            playback.last = Some(reading);
        }
        playback.last
    })
}

/// Ends a replay's readings: this thread's wall clock is its own again.
pub fn stop_playing() {
    PLAYING.with(|playing| playing.set(false));
    PLAYBACK.with(|playback| {
        let mut playback = playback.borrow_mut();
        playback.count = 0;
        playback.next = 0;
        playback.last = None;
    });
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{arm, disarm, record, take, ROOM};

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
}
