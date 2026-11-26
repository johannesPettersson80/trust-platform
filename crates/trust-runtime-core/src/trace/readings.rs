//! The wall clock's readings that a cycle trace records (specification 48 RU-59): while a recording
//! runs on a thread, the platform's wall clock reports each reading of `CURRENT_DT` here, and the
//! recorder takes them at the cycle's end. A replay gives a cycle's readings back to the build's wall
//! clock (RU-60). It allocates nothing: there is room for [`ROOM`] readings in a cycle, and the
//! readings past them are counted, so that the recording ends `memory_full`.
//!
//! Every runtime keeps them with the same rules (RU-59, "Every runtime"); only where they are kept
//! differs. A computer runs each resource's scans on a thread of its own, so each thread keeps its
//! own readings. A chip runs its scans on its one thread without the standard library, so it keeps
//! them in one place, under a critical section. Only a computer replays (RU-60, in a simulator), so
//! only a computer keeps the room of a replay's readings: a chip's static memory holds the
//! recording's room alone.

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
    #[cfg(feature = "std")]
    playing: bool,
    /// The readings a replay gives the wall clock.
    #[cfg(feature = "std")]
    playback: Playback,
}

/// The readings a replay gives the wall clock ([`play`]).
#[cfg(feature = "std")]
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
            #[cfg(feature = "std")]
            playing: false,
            #[cfg(feature = "std")]
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

    #[cfg(feature = "std")]
    fn play(&mut self, readings: &[i64]) {
        let count = readings.len().min(ROOM);
        self.playback.readings[..count].copy_from_slice(&readings[..count]);
        self.playback.count = count;
        self.playback.next = 0;
        self.playing = true;
    }

    #[cfg(feature = "std")]
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

    #[cfg(feature = "std")]
    fn stop_playing(&mut self) {
        self.playing = false;
        self.playback.count = 0;
        self.playback.next = 0;
        self.playback.last = None;
    }
}

// A chip's static memory holds the recording's room alone (the module's documentation): its
// ROOM readings and a few words, never a replay's room beside them (2 KB of a C6's RAM).
#[cfg(not(feature = "std"))]
const _: () = assert!(core::mem::size_of::<State>() <= ROOM * core::mem::size_of::<i64>() + 16);

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

/// How many readings [`take`] would hand now: those kept since the last take, at most [`ROOM`];
/// a runtime that holds them in memory of its own reserves that much first.
#[must_use]
pub fn kept() -> usize {
    with_state(|state| state.count.min(ROOM))
}

/// Gives this thread's wall clock the readings of a recorded cycle, which a replay gives the build
/// (specification 48 RU-60): [`played`] hands them out in order, then the last of them again when
/// the build reads the clock more often. Readings past the room are not played. A computer's alone:
/// a replay runs in a simulator.
#[cfg(feature = "std")]
pub fn play(readings: &[i64]) {
    with_state(|state| state.play(readings));
}

/// The next reading a replay gives this thread's wall clock ([`play`]): the cycle's readings in
/// order, then the last reading played again, from an earlier cycle when this one has none; `None`
/// while no replay plays, or before it has played any.
#[cfg(feature = "std")]
#[must_use]
pub fn played() -> Option<i64> {
    with_state(State::played)
}

/// Ends a replay's readings: this thread's wall clock is its own again.
#[cfg(feature = "std")]
pub fn stop_playing() {
    with_state(State::stop_playing);
}

#[cfg(test)]
mod tests;
