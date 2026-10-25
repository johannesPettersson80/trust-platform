//! The standard function blocks of a compiled program (IEC §6.6.3.5, Tables 43 to 46; specification
//! 44 §6.8): which names the runtime takes for which block, and how the program library lays out
//! each block's variables and names its body.

use trust_runtime_core::native::image::{SYMBOL_INPUT, SYMBOL_OUTPUT};

use super::types::Elem;

/// A standard function block of the program library (IEC §6.6.3.5; specification 44 §6.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum StdFb {
    Ton,
    Tof,
    Tp,
    RTrig,
    FTrig,
    Sr,
    Rs,
    /// `CTU`, `CTD` or `CTUD` counting in a type (IEC §6.6.3.5.4, Table 45).
    Counter(Counting, Count),
}

/// The direction of a counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Counting {
    Up,
    Down,
    UpDown,
}

/// The type a counter counts in: `CTU` and `CTU_INT` count in `INT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Count {
    Int,
    Dint,
    Lint,
    Udint,
    Ulint,
}

impl Count {
    fn elem(self) -> Elem {
        match self {
            Self::Int => Elem::Int,
            Self::Dint => Elem::DInt,
            Self::Lint => Elem::LInt,
            Self::Udint => Elem::UDInt,
            Self::Ulint => Elem::ULInt,
        }
    }

    /// The suffix of the type's name in the library, `Int` of `CtuInt`.
    fn suffix(self) -> &'static str {
        match self {
            Self::Int => "Int",
            Self::Dint => "Dint",
            Self::Lint => "Lint",
            Self::Udint => "Udint",
            Self::Ulint => "Ulint",
        }
    }
}

/// The members of the library's counter types `$ctu`, `$ctd` and `$ctud` for `$counting`, counting
/// in `$elem`: name, type, offset, flags.
macro_rules! counter_members {
    ($counting:expr, $elem:expr, $ctu:ty, $ctd:ty, $ctud:ty) => {{
        use core::mem::offset_of;
        let (input, output) = (SYMBOL_INPUT, SYMBOL_OUTPUT);
        match $counting {
            Counting::Up => vec![
                ("CU", Elem::Bool, offset_of!($ctu, CU), input),
                ("R", Elem::Bool, offset_of!($ctu, R), input),
                ("PV", $elem, offset_of!($ctu, PV), input),
                ("Q", Elem::Bool, offset_of!($ctu, Q), output),
                ("CV", $elem, offset_of!($ctu, CV), output),
                ("__PREV_CU", Elem::Bool, offset_of!($ctu, PREV_CU), 0),
            ],
            Counting::Down => vec![
                ("CD", Elem::Bool, offset_of!($ctd, CD), input),
                ("LD", Elem::Bool, offset_of!($ctd, LD), input),
                ("PV", $elem, offset_of!($ctd, PV), input),
                ("Q", Elem::Bool, offset_of!($ctd, Q), output),
                ("CV", $elem, offset_of!($ctd, CV), output),
                ("__PREV_CD", Elem::Bool, offset_of!($ctd, PREV_CD), 0),
            ],
            Counting::UpDown => vec![
                ("CU", Elem::Bool, offset_of!($ctud, CU), input),
                ("CD", Elem::Bool, offset_of!($ctud, CD), input),
                ("R", Elem::Bool, offset_of!($ctud, R), input),
                ("LD", Elem::Bool, offset_of!($ctud, LD), input),
                ("PV", $elem, offset_of!($ctud, PV), input),
                ("QU", Elem::Bool, offset_of!($ctud, QU), output),
                ("QD", Elem::Bool, offset_of!($ctud, QD), output),
                ("CV", $elem, offset_of!($ctud, CV), output),
                ("__PREV_CU", Elem::Bool, offset_of!($ctud, PREV_CU), 0),
                ("__PREV_CD", Elem::Bool, offset_of!($ctud, PREV_CD), 0),
            ],
        }
    }};
}

/// The size and alignment of the library's counter type for `$counting`.
macro_rules! counter_layout {
    ($counting:expr, $ctu:ty, $ctd:ty, $ctud:ty) => {
        match $counting {
            Counting::Up => (size_of::<$ctu>(), align_of::<$ctu>()),
            Counting::Down => (size_of::<$ctd>(), align_of::<$ctd>()),
            Counting::UpDown => (size_of::<$ctud>(), align_of::<$ctud>()),
        }
    };
}

impl StdFb {
    /// Whether `name` is a standard function block the program library has.
    pub(super) fn is_standard(name: &str) -> bool {
        Self::named(name).is_some()
    }

    /// The member an input's long name names: `SET1` and `RESET` of `SR`, `SET` and `RESET1` of
    /// `RS` (IEC Table 43); any other name names itself.
    pub(super) fn member_named(self, name: &str) -> &str {
        let aliases: &[(&'static str, &'static str)] = match self {
            Self::Sr => &[("SET1", "S1"), ("RESET", "R")],
            Self::Rs => &[("SET", "S"), ("RESET1", "R1")],
            _ => &[],
        };
        aliases
            .iter()
            .find(|(alias, _)| alias.eq_ignore_ascii_case(name))
            .map_or(name, |(_, member)| member)
    }

    /// The block `name` stands for, as the runtime recognizes it: the typed timers and counters, and
    /// Mitsubishi's `DIFU` and `DIFD` as `R_TRIG` and `F_TRIG`.
    pub(super) fn named(name: &str) -> Option<Self> {
        let counter = |counting, count| Some(Self::Counter(counting, count));
        match name.to_ascii_uppercase().as_str() {
            "TON" | "TON_TIME" | "TON_LTIME" => Some(Self::Ton),
            "TOF" | "TOF_TIME" | "TOF_LTIME" => Some(Self::Tof),
            "TP" | "TP_TIME" | "TP_LTIME" => Some(Self::Tp),
            "R_TRIG" | "DIFU" => Some(Self::RTrig),
            "F_TRIG" | "DIFD" => Some(Self::FTrig),
            "SR" => Some(Self::Sr),
            "RS" => Some(Self::Rs),
            "CTU" | "CTU_INT" => counter(Counting::Up, Count::Int),
            "CTU_DINT" => counter(Counting::Up, Count::Dint),
            "CTU_LINT" => counter(Counting::Up, Count::Lint),
            "CTU_UDINT" => counter(Counting::Up, Count::Udint),
            "CTU_ULINT" => counter(Counting::Up, Count::Ulint),
            "CTD" | "CTD_INT" => counter(Counting::Down, Count::Int),
            "CTD_DINT" => counter(Counting::Down, Count::Dint),
            "CTD_LINT" => counter(Counting::Down, Count::Lint),
            "CTD_UDINT" => counter(Counting::Down, Count::Udint),
            "CTD_ULINT" => counter(Counting::Down, Count::Ulint),
            "CTUD" | "CTUD_INT" => counter(Counting::UpDown, Count::Int),
            "CTUD_DINT" => counter(Counting::UpDown, Count::Dint),
            "CTUD_LINT" => counter(Counting::UpDown, Count::Lint),
            "CTUD_UDINT" => counter(Counting::UpDown, Count::Udint),
            "CTUD_ULINT" => counter(Counting::UpDown, Count::Ulint),
            _ => None,
        }
    }

    /// The IEC name.
    pub(super) fn name(self) -> String {
        match self {
            Self::Ton => "TON".into(),
            Self::Tof => "TOF".into(),
            Self::Tp => "TP".into(),
            Self::RTrig => "R_TRIG".into(),
            Self::FTrig => "F_TRIG".into(),
            Self::Sr => "SR".into(),
            Self::Rs => "RS".into(),
            Self::Counter(counting, count) => {
                let block = match counting {
                    Counting::Up => "CTU",
                    Counting::Down => "CTD",
                    Counting::UpDown => "CTUD",
                };
                match count {
                    Count::Int => block.into(),
                    other => format!("{block}_{}", other.suffix().to_ascii_uppercase()),
                }
            }
        }
    }

    /// The library's type.
    pub(super) fn rust(self) -> String {
        match self {
            Self::Ton => "lib::Ton".into(),
            Self::Tof => "lib::Tof".into(),
            Self::Tp => "lib::Tp".into(),
            Self::RTrig => "lib::RTrig".into(),
            Self::FTrig => "lib::FTrig".into(),
            Self::Sr => "lib::Sr".into(),
            Self::Rs => "lib::Rs".into(),
            Self::Counter(counting, count) => {
                let block = match counting {
                    Counting::Up => "Ctu",
                    Counting::Down => "Ctd",
                    Counting::UpDown => "Ctud",
                };
                format!("lib::{block}{}", count.suffix())
            }
        }
    }

    /// The library's body, and whether it takes the task time.
    pub(super) fn body(self) -> (String, bool) {
        match self {
            Self::Ton => ("lib::ton".into(), true),
            Self::Tof => ("lib::tof".into(), true),
            Self::Tp => ("lib::tp".into(), true),
            Self::RTrig => ("lib::r_trig".into(), false),
            Self::FTrig => ("lib::f_trig".into(), false),
            Self::Sr => ("lib::sr".into(), false),
            Self::Rs => ("lib::rs".into(), false),
            Self::Counter(counting, count) => {
                let block = match counting {
                    Counting::Up => "ctu",
                    Counting::Down => "ctd",
                    Counting::UpDown => "ctud",
                };
                (
                    format!("lib::{block}_{}", count.suffix().to_ascii_lowercase()),
                    false,
                )
            }
        }
    }

    /// The members as the library lays them out: name, type, offset, flags. The offsets are the
    /// library's own, read from its types on the computer that builds (`#[repr(C)]` lays them out
    /// the same on every target of specification 44 §4.4).
    pub(super) fn members(self) -> Vec<(&'static str, Elem, usize, u8)> {
        use core::mem::offset_of;
        use trust_plc_lib::{
            CtdDint, CtdInt, CtdLint, CtdUdint, CtdUlint, CtuDint, CtuInt, CtuLint, CtuUdint,
            CtuUlint, CtudDint, CtudInt, CtudLint, CtudUdint, CtudUlint, FTrig, RTrig, Rs, Sr, Tof,
            Ton, Tp,
        };
        let input = SYMBOL_INPUT;
        let output = SYMBOL_OUTPUT;
        match self {
            Self::Ton => vec![
                ("IN", Elem::Bool, offset_of!(Ton, IN), input),
                ("PT", Elem::Time, offset_of!(Ton, PT), input),
                ("Q", Elem::Bool, offset_of!(Ton, Q), output),
                ("ET", Elem::Time, offset_of!(Ton, ET), output),
                ("__LAST", Elem::LTime, offset_of!(Ton, LAST), 0),
                ("__CALLED", Elem::Bool, offset_of!(Ton, CALLED), 0),
            ],
            Self::Tof => vec![
                ("IN", Elem::Bool, offset_of!(Tof, IN), input),
                ("PT", Elem::Time, offset_of!(Tof, PT), input),
                ("Q", Elem::Bool, offset_of!(Tof, Q), output),
                ("ET", Elem::Time, offset_of!(Tof, ET), output),
                ("__LAST", Elem::LTime, offset_of!(Tof, LAST), 0),
                ("__CALLED", Elem::Bool, offset_of!(Tof, CALLED), 0),
                ("__PREV_IN", Elem::Bool, offset_of!(Tof, PREV_IN), 0),
                ("__TIMING", Elem::Bool, offset_of!(Tof, TIMING), 0),
            ],
            Self::Tp => vec![
                ("IN", Elem::Bool, offset_of!(Tp, IN), input),
                ("PT", Elem::Time, offset_of!(Tp, PT), input),
                ("Q", Elem::Bool, offset_of!(Tp, Q), output),
                ("ET", Elem::Time, offset_of!(Tp, ET), output),
                ("__LAST", Elem::LTime, offset_of!(Tp, LAST), 0),
                ("__CALLED", Elem::Bool, offset_of!(Tp, CALLED), 0),
                ("__PREV_IN", Elem::Bool, offset_of!(Tp, PREV_IN), 0),
                ("__ACTIVE", Elem::Bool, offset_of!(Tp, ACTIVE), 0),
            ],
            Self::RTrig => vec![
                ("CLK", Elem::Bool, offset_of!(RTrig, CLK), input),
                ("Q", Elem::Bool, offset_of!(RTrig, Q), output),
                ("__M", Elem::Bool, offset_of!(RTrig, M), 0),
            ],
            Self::FTrig => vec![
                ("CLK", Elem::Bool, offset_of!(FTrig, CLK), input),
                ("Q", Elem::Bool, offset_of!(FTrig, Q), output),
                ("__M", Elem::Bool, offset_of!(FTrig, M), 0),
            ],
            Self::Sr => vec![
                ("S1", Elem::Bool, offset_of!(Sr, S1), input),
                ("R", Elem::Bool, offset_of!(Sr, R), input),
                ("Q1", Elem::Bool, offset_of!(Sr, Q1), output),
            ],
            Self::Rs => vec![
                ("S", Elem::Bool, offset_of!(Rs, S), input),
                ("R1", Elem::Bool, offset_of!(Rs, R1), input),
                ("Q1", Elem::Bool, offset_of!(Rs, Q1), output),
            ],
            Self::Counter(counting, count) => {
                let elem = count.elem();
                match count {
                    Count::Int => counter_members!(counting, elem, CtuInt, CtdInt, CtudInt),
                    Count::Dint => counter_members!(counting, elem, CtuDint, CtdDint, CtudDint),
                    Count::Lint => counter_members!(counting, elem, CtuLint, CtdLint, CtudLint),
                    Count::Udint => {
                        counter_members!(counting, elem, CtuUdint, CtdUdint, CtudUdint)
                    }
                    Count::Ulint => {
                        counter_members!(counting, elem, CtuUlint, CtdUlint, CtudUlint)
                    }
                }
            }
        }
    }

    pub(super) fn layout(self) -> (usize, usize) {
        use trust_plc_lib::{
            CtdDint, CtdInt, CtdLint, CtdUdint, CtdUlint, CtuDint, CtuInt, CtuLint, CtuUdint,
            CtuUlint, CtudDint, CtudInt, CtudLint, CtudUdint, CtudUlint, FTrig, RTrig, Rs, Sr, Tof,
            Ton, Tp,
        };
        match self {
            Self::Ton => (size_of::<Ton>(), align_of::<Ton>()),
            Self::Tof => (size_of::<Tof>(), align_of::<Tof>()),
            Self::Tp => (size_of::<Tp>(), align_of::<Tp>()),
            Self::RTrig => (size_of::<RTrig>(), align_of::<RTrig>()),
            Self::FTrig => (size_of::<FTrig>(), align_of::<FTrig>()),
            Self::Sr => (size_of::<Sr>(), align_of::<Sr>()),
            Self::Rs => (size_of::<Rs>(), align_of::<Rs>()),
            Self::Counter(counting, count) => match count {
                Count::Int => counter_layout!(counting, CtuInt, CtdInt, CtudInt),
                Count::Dint => counter_layout!(counting, CtuDint, CtdDint, CtudDint),
                Count::Lint => counter_layout!(counting, CtuLint, CtdLint, CtudLint),
                Count::Udint => counter_layout!(counting, CtuUdint, CtdUdint, CtudUdint),
                Count::Ulint => counter_layout!(counting, CtuUlint, CtdUlint, CtudUlint),
            },
        }
    }
}
