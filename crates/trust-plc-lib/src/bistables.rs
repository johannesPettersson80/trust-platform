//! The bistables `SR` and `RS` of IEC §6.6.3.5.2 (Table 43): `SR` sets dominantly,
//! `Q1 := S1 OR (NOT R AND Q1)`, and `RS` resets dominantly, `Q1 := NOT R1 AND (S OR Q1)`.

use core::cell::Cell;

use trust_plc_abi::plain_struct;

plain_struct! {
    /// A set-dominant bistable.
    pub struct Sr {
        /// Input: sets `Q1`, before `R`.
        pub S1: Cell<u8>,
        /// Input: resets `Q1`.
        pub R: Cell<u8>,
        /// Output: the state.
        pub Q1: Cell<u8>,
    }
}

plain_struct! {
    /// A reset-dominant bistable.
    pub struct Rs {
        /// Input: sets `Q1`.
        pub S: Cell<u8>,
        /// Input: resets `Q1`, before `S`.
        pub R1: Cell<u8>,
        /// Output: the state.
        pub Q1: Cell<u8>,
    }
}

/// Runs `SR`.
pub fn sr(bistable: &Sr) {
    if bistable.S1.get() != 0 {
        bistable.Q1.set(1);
    } else if bistable.R.get() != 0 {
        bistable.Q1.set(0);
    }
}

/// Runs `RS`.
pub fn rs(bistable: &Rs) {
    if bistable.R1.get() != 0 {
        bistable.Q1.set(0);
    } else if bistable.S.get() != 0 {
        bistable.Q1.set(1);
    }
}
