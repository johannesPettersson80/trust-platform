//! The timers `TON`, `TOF` and `TP` (IEC §6.6.3.5.5, Table 46; specification 08 §5), with the
//! runtime's rules: the call that samples the starting edge returns `ET` = 0, as CODESYS and
//! `TwinCAT` time, and each later call adds the task time since the previous call, nothing on the
//! first call; `PT` below zero counts as zero; `ET` stops at `PT`; a running `TP` pulse ignores
//! edges (`docs/IEC_DECISIONS.md`, 2026-10-30).

use core::cell::Cell;

use trust_plc_abi::plain_struct;

plain_struct! {
    /// An on-delay timer: `Q` rises when `IN` has been TRUE for `PT`.
    pub struct Ton {
        /// Input: the timed condition.
        pub IN: Cell<u8>,
        /// Input: the delay, nanoseconds.
        pub PT: Cell<i64>,
        /// Output: `IN` has been TRUE for `PT`.
        pub Q: Cell<u8>,
        /// Output: the time `IN` has been TRUE, at most `PT`, nanoseconds.
        pub ET: Cell<i64>,
        /// The task time of the previous call.
        pub LAST: Cell<i64>,
        /// Not 0 once the timer has been called.
        pub CALLED: Cell<u8>,
        /// `IN` at the previous call.
        pub PREV_IN: Cell<u8>,
    }
}

plain_struct! {
    /// An off-delay timer: `Q` falls when `IN` has been FALSE for `PT`.
    pub struct Tof {
        /// Input: the timed condition.
        pub IN: Cell<u8>,
        /// Input: the delay, nanoseconds.
        pub PT: Cell<i64>,
        /// Output: `IN`, held for `PT` after it falls.
        pub Q: Cell<u8>,
        /// Output: the time since `IN` fell, at most `PT`, nanoseconds.
        pub ET: Cell<i64>,
        /// The task time of the previous call.
        pub LAST: Cell<i64>,
        /// Not 0 once the timer has been called.
        pub CALLED: Cell<u8>,
        /// `IN` at the previous call.
        pub PREV_IN: Cell<u8>,
        /// Not 0 while the delay runs.
        pub TIMING: Cell<u8>,
    }
}

plain_struct! {
    /// A pulse timer: `Q` is TRUE for `PT` from a rising edge of `IN`.
    pub struct Tp {
        /// Input: the trigger.
        pub IN: Cell<u8>,
        /// Input: the pulse's length, nanoseconds.
        pub PT: Cell<i64>,
        /// Output: the pulse.
        pub Q: Cell<u8>,
        /// Output: the pulse's time so far, nanoseconds.
        pub ET: Cell<i64>,
        /// The task time of the previous call.
        pub LAST: Cell<i64>,
        /// Not 0 once the timer has been called.
        pub CALLED: Cell<u8>,
        /// `IN` at the previous call.
        pub PREV_IN: Cell<u8>,
        /// Not 0 while the pulse runs.
        pub ACTIVE: Cell<u8>,
    }
}

/// The task time that passed since the previous call, nothing on the first call and nothing for a
/// time that went back; the call's time is kept for the next one.
fn elapsed(last: &Cell<i64>, called: &Cell<u8>, now: i64) -> i64 {
    let previous = if called.get() == 0 { now } else { last.get() };
    called.set(1);
    last.set(now);
    // A span wider than the nanosecond range saturates, which completes any preset.
    now.saturating_sub(previous).max(0)
}

/// `ET` after `delta` more nanoseconds, at most `pt`.
fn accumulate(et: i64, delta: i64, pt: i64) -> i64 {
    et.saturating_add(delta.max(0)).min(pt)
}

/// Runs `TON` at task time `now`.
pub fn ton(timer: &Ton, now: i64) {
    let delta = elapsed(&timer.LAST, &timer.CALLED, now);
    let pt = timer.PT.get().max(0);
    let input = timer.IN.get() != 0;
    if input {
        // The rising-edge call starts timing at 0; later calls add their task time.
        let et = if timer.PREV_IN.get() == 0 {
            0
        } else {
            accumulate(timer.ET.get(), delta, pt)
        };
        timer.ET.set(et);
        timer.Q.set(u8::from(et >= pt));
    } else {
        timer.ET.set(0);
        timer.Q.set(0);
    }
    timer.PREV_IN.set(u8::from(input));
}

/// Runs `TOF` at task time `now`.
pub fn tof(timer: &Tof, now: i64) {
    let delta = elapsed(&timer.LAST, &timer.CALLED, now);
    let pt = timer.PT.get().max(0);
    let input = timer.IN.get() != 0;
    if input {
        timer.Q.set(1);
        timer.ET.set(0);
        timer.TIMING.set(0);
    } else {
        // The falling-edge call starts timing at 0; later calls add their task time.
        let falling = timer.PREV_IN.get() != 0;
        if falling {
            timer.TIMING.set(1);
            timer.ET.set(0);
        }
        if timer.TIMING.get() == 0 {
            timer.Q.set(0);
        } else {
            let et = if falling {
                0
            } else {
                accumulate(timer.ET.get(), delta, pt)
            };
            if et >= pt {
                timer.Q.set(0);
                timer.ET.set(pt);
                timer.TIMING.set(0);
            } else {
                timer.Q.set(1);
                timer.ET.set(et);
            }
        }
    }
    timer.PREV_IN.set(u8::from(input));
}

/// Runs `TP` at task time `now`.
pub fn tp(timer: &Tp, now: i64) {
    let delta = elapsed(&timer.LAST, &timer.CALLED, now);
    let pt = timer.PT.get().max(0);
    let input = timer.IN.get() != 0;
    // A rising edge starts a pulse at 0 only when none runs: a running pulse ignores edges.
    let started = input && timer.PREV_IN.get() == 0 && timer.ACTIVE.get() == 0;
    if started {
        timer.ACTIVE.set(1);
        timer.ET.set(0);
    }
    let mut expired = false;
    if timer.ACTIVE.get() != 0 {
        let et = if started {
            0
        } else {
            accumulate(timer.ET.get(), delta, pt)
        };
        if et >= pt {
            timer.ACTIVE.set(0);
            timer.ET.set(pt);
            expired = true;
        } else {
            timer.ET.set(et);
        }
    }
    // The call that completes the pulse keeps ET at PT even when it samples IN FALSE; the first
    // later call that samples IN FALSE resets ET (specification 08 §5).
    if !input && timer.ACTIVE.get() == 0 && !expired {
        timer.ET.set(0);
    }
    timer.Q.set(timer.ACTIVE.get());
    timer.PREV_IN.set(u8::from(input));
}
