//! The edge detectors `R_TRIG` and `F_TRIG` (IEC §6.6.3.5.3, Table 44), as the standard defines
//! their bodies: `Q := CLK AND NOT M; M := CLK;` and `Q := NOT CLK AND NOT M; M := NOT CLK;`, with `M`
//! FALSE at the start, so a first call of `F_TRIG` with `CLK` FALSE gives `Q` TRUE.

use core::cell::Cell;

use trust_plc_abi::plain_struct;

plain_struct! {
    /// A rising-edge detector.
    pub struct RTrig {
        /// Input: the signal.
        pub CLK: Cell<u8>,
        /// Output: TRUE for the call at which `CLK` rose.
        pub Q: Cell<u8>,
        /// `CLK` at the previous call.
        pub M: Cell<u8>,
    }
}

plain_struct! {
    /// A falling-edge detector.
    pub struct FTrig {
        /// Input: the signal.
        pub CLK: Cell<u8>,
        /// Output: TRUE for the call at which `CLK` fell.
        pub Q: Cell<u8>,
        /// `NOT CLK` at the previous call.
        pub M: Cell<u8>,
    }
}

/// Runs `R_TRIG`.
pub fn r_trig(trigger: &RTrig) {
    let clk = trigger.CLK.get() != 0;
    trigger.Q.set(u8::from(clk && trigger.M.get() == 0));
    trigger.M.set(u8::from(clk));
}

/// Runs `F_TRIG`.
pub fn f_trig(trigger: &FTrig) {
    let not_clk = trigger.CLK.get() == 0;
    trigger.Q.set(u8::from(not_clk && trigger.M.get() == 0));
    trigger.M.set(u8::from(not_clk));
}
