//! The wall clock's readings that a cycle trace records (specification 48 RU-59): while a recording
//! runs on a thread, the platform's wall clock reports each reading of `CURRENT_DT` here, and the
//! recorder takes them at the cycle's end. A replay gives a cycle's readings back to the build's wall
//! clock (RU-60). It allocates nothing: there is room for [`ROOM`] readings in a cycle, and the
//! readings past them are counted, so that the recording ends `memory_full`.
//!
//! Every runtime keeps them with the same rules (RU-59, "Every runtime"); only where they are kept
//! differs. A computer runs each resource's scans on a thread of its own, so each thread keeps its
//! own readings. A chip runs its scans on its one thread without the standard library, so it keeps
//! them in one place, under a critical section.

/// The readings kept between two takes.
pub const ROOM: usize = 256;

/// The readings of the thread's scans: those a recording keeps, and those a replay plays.
struct State {
    /// A recording keeps the readings.
    armed: bool,
    /// The readings kept since the last take, the first [`ROOM`] of them.
    readings: [i64; ROOM],
    /// How many readings there were since the last take, past the room too.
    count: usize,
    /// A replay plays its readings ([`play`]).
    playing: bool,
    /// The readings a replay gives the wall clock.
    playback: Playback,
}

/// The readings a replay gives the wall clock ([`play`]).
struct Playback {
    readings: [i64; ROOM],
    count: usize,
    next: usize,
    last: Option<i64>,
}

impl State {
    const fn new() -> Self {
        Self {
            armed: false,
            readings: [0; ROOM],
            count: 0,
            playing: false,
            playback: Playback {
                readings: [0; ROOM],
                count: 0,
                next: 0,
                last: None,
            },
        }
    }

    fn arm(&mut self) {
        self.count = 0;
        self.armed = true;
    }

    fn record(&mut self, millis: i64) {
        if !self.armed {
            return;
        }
        if let Some(slot) = self.readings.get_mut(self.count) {
            *slot = millis;
        }
        self.count = self.count.saturating_add(1);
    }

    fn take(&mut self, each: &mut dyn FnMut(i64)) -> usize {
        let count = self.count;
        for reading in &self.readings[..count.min(ROOM)] {
            each(*reading);
        }
        self.count = 0;
        count
    }

    fn play(&mut self, readings: &[i64]) {
        let count = readings.len().min(ROOM);
        self.playback.readings[..count].copy_from_slice(&readings[..count]);
        self.playback.count = count;
        self.playback.next = 0;
        self.playing = true;
    }

    fn played(&mut self) -> Option<i64> {
        if !self.playing {
            return None;
        }
        let playback = &mut self.playback;
        if playback.next < playback.count {
            let reading = playback.readings[playback.next];
            playback.next += 1;
            playback.last = Some(reading);
        }
        playback.last
    }

    fn stop_playing(&mut self) {
        self.playing = false;
        self.playback.count = 0;
        self.playback.next = 0;
        self.playback.last = None;
    }
}

/// A computer: each thread's own readings.
#[cfg(feature = "std")]
fn with_state<R>(change: impl FnOnce(&mut State) -> R) -> R {
    std::thread_local! {
        static STATE: core::cell::RefCell<State> = const { core::cell::RefCell::new(State::new()) };
    }
    STATE.with(|state| change(&mut state.borrow_mut()))
}

/// A chip: the readings of its one scan thread, under a critical section, which the chip's
/// platform provides.
#[cfg(not(feature = "std"))]
fn with_state<R>(change: impl FnOnce(&mut State) -> R) -> R {
    static STATE: critical_section::Mutex<core::cell::RefCell<State>> =
        critical_section::Mutex::new(core::cell::RefCell::new(State::new()));
    critical_section::with(|section| change(&mut STATE.borrow_ref_mut(section)))
}

/// Starts keeping this thread's readings, from none.
pub fn arm() {
    with_state(State::arm);
}

/// Stops keeping this thread's readings.
pub fn disarm() {
    with_state(|state| state.armed = false);
}

/// A reading of the wall clock in UTC milliseconds, kept while this thread is armed.
pub fn record(millis: i64) {
    with_state(|state| state.record(millis));
}

/// Hands each kept reading to `each`, in the order they were read, and keeps none. It gives how many
/// readings there were: more than it handed when they outgrew the room.
pub fn take(each: &mut dyn FnMut(i64)) -> usize {
    with_state(|state| state.take(each))
}

/// Gives this thread's wall clock the readings of a recorded cycle, which a replay gives the build
/// (specification 48 RU-60): [`played`] hands them out in order, then the last of them again when
/// the build reads the clock more often. Readings past the room are not played.
pub fn play(readings: &[i64]) {
    with_state(|state| state.play(readings));
}

/// The next reading a replay gives this thread's wall clock ([`play`]): the cycle's readings in
/// order, then the last reading played again, from an earlier cycle when this one has none; `None`
/// while no replay plays, or before it has played any.
#[must_use]
pub fn played() -> Option<i64> {
    with_state(State::played)
}

/// Ends a replay's readings: this thread's wall clock is its own again.
pub fn stop_playing() {
    with_state(State::stop_playing);
}

#[cfg(test)]
mod tests {
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
}
