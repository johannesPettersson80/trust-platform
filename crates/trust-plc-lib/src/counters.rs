//! The counters `CTU`, `CTD` and `CTUD` of IEC §6.6.3.5.4 (Table 45) in their five count types,
//! with the runtime's rules (specification 08 §4): a count input counts on its rising edge, and its
//! edge memory is updated at every call, also one that a reset or a load decides (decision of 15
//! October 2026); `CV` stays at the limits of its type instead of wrapping; `CTU`'s `Q` is
//! `CV >= PV`, `CTD`'s `CV <= 0`, and `CTUD`'s `QU` and `QD` are both; `CTUD` counts neither way
//! when both inputs rise at one call.

use core::cell::Cell;

use trust_plc_abi::plain_struct;

macro_rules! counters {
    ($($ctu:ident $ctd:ident $ctud:ident $up:ident $down:ident $updown:ident $t:ty;)*) => {
        $(
            plain_struct! {
                /// An up counter.
                pub struct $ctu {
                    /// Input: counts up on its rising edge.
                    pub CU: Cell<u8>,
                    /// Input: sets `CV` to 0.
                    pub R: Cell<u8>,
                    /// Input: the preset.
                    pub PV: Cell<$t>,
                    /// Output: `CV >= PV`.
                    pub Q: Cell<u8>,
                    /// Output: the count.
                    pub CV: Cell<$t>,
                    /// `CU` at the previous call.
                    pub PREV_CU: Cell<u8>,
                }
            }

            plain_struct! {
                /// A down counter.
                pub struct $ctd {
                    /// Input: counts down on its rising edge.
                    pub CD: Cell<u8>,
                    /// Input: loads `PV` into `CV`.
                    pub LD: Cell<u8>,
                    /// Input: the preset.
                    pub PV: Cell<$t>,
                    /// Output: `CV <= 0`.
                    pub Q: Cell<u8>,
                    /// Output: the count.
                    pub CV: Cell<$t>,
                    /// `CD` at the previous call.
                    pub PREV_CD: Cell<u8>,
                }
            }

            plain_struct! {
                /// An up-down counter.
                pub struct $ctud {
                    /// Input: counts up on its rising edge.
                    pub CU: Cell<u8>,
                    /// Input: counts down on its rising edge.
                    pub CD: Cell<u8>,
                    /// Input: sets `CV` to 0.
                    pub R: Cell<u8>,
                    /// Input: loads `PV` into `CV`.
                    pub LD: Cell<u8>,
                    /// Input: the preset.
                    pub PV: Cell<$t>,
                    /// Output: `CV >= PV`.
                    pub QU: Cell<u8>,
                    /// Output: `CV <= 0`.
                    pub QD: Cell<u8>,
                    /// Output: the count.
                    pub CV: Cell<$t>,
                    /// `CU` at the previous call.
                    pub PREV_CU: Cell<u8>,
                    /// `CD` at the previous call.
                    pub PREV_CD: Cell<u8>,
                }
            }

            #[doc = concat!("Runs `", stringify!($ctu), "`.")]
            pub fn $up(counter: &$ctu) {
                let cu = counter.CU.get() != 0;
                let rising = cu && counter.PREV_CU.get() == 0;
                let mut cv = counter.CV.get();
                if counter.R.get() != 0 {
                    cv = 0;
                } else if rising && cv < <$t>::MAX {
                    cv += 1;
                }
                counter.CV.set(cv);
                counter.Q.set(u8::from(cv >= counter.PV.get()));
                counter.PREV_CU.set(u8::from(cu));
            }

            #[doc = concat!("Runs `", stringify!($ctd), "`.")]
            pub fn $down(counter: &$ctd) {
                let cd = counter.CD.get() != 0;
                let rising = cd && counter.PREV_CD.get() == 0;
                let mut cv = counter.CV.get();
                if counter.LD.get() != 0 {
                    cv = counter.PV.get();
                } else if rising && cv > <$t>::MIN {
                    cv -= 1;
                }
                let zero: $t = 0;
                counter.CV.set(cv);
                counter.Q.set(u8::from(cv <= zero));
                counter.PREV_CD.set(u8::from(cd));
            }

            #[doc = concat!("Runs `", stringify!($ctud), "`.")]
            pub fn $updown(counter: &$ctud) {
                let cu = counter.CU.get() != 0;
                let cd = counter.CD.get() != 0;
                let rising_up = cu && counter.PREV_CU.get() == 0;
                let rising_down = cd && counter.PREV_CD.get() == 0;
                let mut cv = counter.CV.get();
                if counter.R.get() != 0 {
                    cv = 0;
                } else if counter.LD.get() != 0 {
                    cv = counter.PV.get();
                } else if !(rising_up && rising_down) {
                    if rising_up && cv < <$t>::MAX {
                        cv += 1;
                    } else if rising_down && cv > <$t>::MIN {
                        cv -= 1;
                    }
                }
                let zero: $t = 0;
                counter.CV.set(cv);
                counter.QU.set(u8::from(cv >= counter.PV.get()));
                counter.QD.set(u8::from(cv <= zero));
                counter.PREV_CU.set(u8::from(cu));
                counter.PREV_CD.set(u8::from(cd));
            }
        )*
    };
}

counters! {
    CtuInt CtdInt CtudInt ctu_int ctd_int ctud_int i16;
    CtuDint CtdDint CtudDint ctu_dint ctd_dint ctud_dint i32;
    CtuLint CtdLint CtudLint ctu_lint ctd_lint ctud_lint i64;
    CtuUdint CtdUdint CtudUdint ctu_udint ctd_udint ctud_udint u32;
    CtuUlint CtdUlint CtudUlint ctu_ulint ctd_ulint ctud_ulint u64;
}
