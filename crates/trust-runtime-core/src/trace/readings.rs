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
}
