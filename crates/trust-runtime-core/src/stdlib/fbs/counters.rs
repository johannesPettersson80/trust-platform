use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::value::Value;

use super::instance::{
    get_or_init_bool, read_bool, read_value, read_value_or_null, set_instance_value, write_bool,
};
use super::state::{STATE_PREV_CD, STATE_PREV_CU};
use super::BuiltinExecContext;

/// Counter output and current count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterOutput {
    /// Current Boolean output.
    pub q: bool,
    /// Current counter value.
    pub cv: i16,
}

/// Up/down-counter thresholds and current count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterUpDownOutput {
    /// Upper preset threshold output.
    pub qu: bool,
    /// Zero threshold output.
    pub qd: bool,
    /// Current counter value.
    pub cv: i16,
}

/// Up-counter state.
#[derive(Debug, Clone)]
pub struct Ctu {
    cv: i16,
    prev_cu: bool,
}

impl Ctu {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cv: 0,
            prev_cu: false,
        }
    }

    /// Sample count edges and apply the reset/load controls for this counter.
    pub fn step(&mut self, cu: bool, reset: bool, pv: i16) -> CounterOutput {
        let rising = !self.prev_cu && cu;
        if reset {
            self.cv = 0;
        } else if rising && self.cv < i16::MAX {
            self.cv += 1;
        }
        self.prev_cu = cu;
        CounterOutput {
            q: self.cv >= pv,
            cv: self.cv,
        }
    }
}

impl Default for Ctu {
    fn default() -> Self {
        Self::new()
    }
}

/// Down-counter state.
#[derive(Debug, Clone)]
pub struct Ctd {
    cv: i16,
    prev_cd: bool,
}

impl Ctd {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cv: 0,
            prev_cd: false,
        }
    }

    /// Sample count edges and apply the reset/load controls for this counter.
    pub fn step(&mut self, cd: bool, load: bool, pv: i16) -> CounterOutput {
        let rising = !self.prev_cd && cd;
        if load {
            self.cv = pv;
        } else if rising && self.cv > i16::MIN {
            self.cv -= 1;
        }
        self.prev_cd = cd;
        CounterOutput {
            q: self.cv <= 0,
            cv: self.cv,
        }
    }
}

impl Default for Ctd {
    fn default() -> Self {
        Self::new()
    }
}

/// Up/down-counter state.
#[derive(Debug, Clone)]
pub struct Ctud {
    cv: i16,
    prev_cu: bool,
    prev_cd: bool,
}

impl Ctud {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cv: 0,
            prev_cu: false,
            prev_cd: false,
        }
    }

    /// Sample count edges and apply the reset/load controls for this counter.
    pub fn step(
        &mut self,
        cu: bool,
        cd: bool,
        reset: bool,
        load: bool,
        pv: i16,
    ) -> CounterUpDownOutput {
        let rising_cu = !self.prev_cu && cu;
        let rising_cd = !self.prev_cd && cd;

        if reset {
            self.cv = 0;
        } else if load {
            self.cv = pv;
        } else if !(rising_cu && rising_cd) {
            if rising_cu && self.cv < i16::MAX {
                self.cv += 1;
            } else if rising_cd && self.cv > i16::MIN {
                self.cv -= 1;
            }
        }

        self.prev_cu = cu;
        self.prev_cd = cd;

        CounterUpDownOutput {
            qu: self.cv >= pv,
            qd: self.cv <= 0,
            cv: self.cv,
        }
    }
}

impl Default for Ctud {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! counter_up_signed {
    ($pv:expr, $cv:expr, $ty:ty, $variant:ident, $reset:expr, $rising:expr) => {{
        let mut cv: $ty = $cv;
        if $reset {
            cv = 0;
        } else if $rising && cv < <$ty>::MAX {
            cv += 1;
        }
        let q = cv >= $pv;
        (Value::$variant(cv), q)
    }};
}

macro_rules! counter_up_unsigned {
    ($pv:expr, $cv:expr, $ty:ty, $variant:ident, $reset:expr, $rising:expr) => {{
        let mut cv: $ty = $cv;
        if $reset {
            cv = 0;
        } else if $rising && cv < <$ty>::MAX {
            cv += 1;
        }
        let q = cv >= $pv;
        (Value::$variant(cv), q)
    }};
}

macro_rules! counter_down_signed {
    ($pv:expr, $cv:expr, $ty:ty, $variant:ident, $load:expr, $rising:expr) => {{
        let mut cv: $ty = $cv;
        if $load {
            cv = $pv;
        } else if $rising && cv > <$ty>::MIN {
            cv -= 1;
        }
        let q = cv <= 0;
        (Value::$variant(cv), q)
    }};
}

macro_rules! counter_down_unsigned {
    ($pv:expr, $cv:expr, $ty:ty, $variant:ident, $load:expr, $rising:expr) => {{
        let mut cv: $ty = $cv;
        if $load {
            cv = $pv;
        } else if $rising && cv > 0 {
            cv -= 1;
        }
        let q = cv == 0;
        (Value::$variant(cv), q)
    }};
}

macro_rules! counter_up_down_signed {
    (
        $pv:expr,
        $cv:expr,
        $ty:ty,
        $variant:ident,
        $reset:expr,
        $load:expr,
        $rising_cu:expr,
        $rising_cd:expr
    ) => {{
        let mut cv: $ty = $cv;
        if $reset {
            cv = 0;
        } else if $load {
            cv = $pv;
        } else if !($rising_cu && $rising_cd) {
            if $rising_cu && cv < <$ty>::MAX {
                cv += 1;
            } else if $rising_cd && cv > <$ty>::MIN {
                cv -= 1;
            }
        }
        let qu = cv >= $pv;
        let qd = cv <= 0;
        (Value::$variant(cv), qu, qd)
    }};
}

macro_rules! counter_up_down_unsigned {
    (
        $pv:expr,
        $cv:expr,
        $ty:ty,
        $variant:ident,
        $reset:expr,
        $load:expr,
        $rising_cu:expr,
        $rising_cd:expr
    ) => {{
        let mut cv: $ty = $cv;
        if $reset {
            cv = 0;
        } else if $load {
            cv = $pv;
        } else if !($rising_cu && $rising_cd) {
            if $rising_cu && cv < <$ty>::MAX {
                cv += 1;
            } else if $rising_cd && cv > 0 {
                cv -= 1;
            }
        }
        let qu = cv >= $pv;
        let qd = cv == 0;
        (Value::$variant(cv), qu, qd)
    }};
}

pub(super) fn exec_ctu(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let cu = read_bool(ctx, instance_id, "CU")?;
    let reset = read_bool(ctx, instance_id, "R")?;
    let pv = read_value(ctx, instance_id, "PV")?;
    let cv = read_value_or_null(ctx, instance_id, "CV");
    let prev_cu = get_or_init_bool(ctx, instance_id, STATE_PREV_CU, false)?;
    let rising = cu && !prev_cu;

    let (new_cv, q) = match (&pv, &cv) {
        (Value::SInt(pv), Value::SInt(cv)) => {
            counter_up_signed!(*pv, *cv, i8, SInt, reset, rising)
        }
        (Value::SInt(pv), Value::Null) => counter_up_signed!(*pv, 0, i8, SInt, reset, rising),
        (Value::Int(pv), Value::Int(cv)) => {
            counter_up_signed!(*pv, *cv, i16, Int, reset, rising)
        }
        (Value::Int(pv), Value::Null) => counter_up_signed!(*pv, 0, i16, Int, reset, rising),
        (Value::DInt(pv), Value::DInt(cv)) => {
            counter_up_signed!(*pv, *cv, i32, DInt, reset, rising)
        }
        (Value::DInt(pv), Value::Null) => counter_up_signed!(*pv, 0, i32, DInt, reset, rising),
        (Value::LInt(pv), Value::LInt(cv)) => {
            counter_up_signed!(*pv, *cv, i64, LInt, reset, rising)
        }
        (Value::LInt(pv), Value::Null) => counter_up_signed!(*pv, 0, i64, LInt, reset, rising),
        (Value::USInt(pv), Value::USInt(cv)) => {
            counter_up_unsigned!(*pv, *cv, u8, USInt, reset, rising)
        }
        (Value::USInt(pv), Value::Null) => {
            counter_up_unsigned!(*pv, 0, u8, USInt, reset, rising)
        }
        (Value::UInt(pv), Value::UInt(cv)) => {
            counter_up_unsigned!(*pv, *cv, u16, UInt, reset, rising)
        }
        (Value::UInt(pv), Value::Null) => counter_up_unsigned!(*pv, 0, u16, UInt, reset, rising),
        (Value::UDInt(pv), Value::UDInt(cv)) => {
            counter_up_unsigned!(*pv, *cv, u32, UDInt, reset, rising)
        }
        (Value::UDInt(pv), Value::Null) => {
            counter_up_unsigned!(*pv, 0, u32, UDInt, reset, rising)
        }
        (Value::ULInt(pv), Value::ULInt(cv)) => {
            counter_up_unsigned!(*pv, *cv, u64, ULInt, reset, rising)
        }
        (Value::ULInt(pv), Value::Null) => {
            counter_up_unsigned!(*pv, 0, u64, ULInt, reset, rising)
        }
        _ => return Err(RuntimeError::TypeMismatch),
    };

    set_instance_value(ctx, instance_id, "CV", new_cv);
    write_bool(ctx, instance_id, "Q", q);
    write_bool(ctx, instance_id, STATE_PREV_CU, cu);
    Ok(())
}

pub(super) fn exec_ctd(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let cd = read_bool(ctx, instance_id, "CD")?;
    let load = read_bool(ctx, instance_id, "LD")?;
    let pv = read_value(ctx, instance_id, "PV")?;
    let cv = read_value_or_null(ctx, instance_id, "CV");
    let prev_cd = get_or_init_bool(ctx, instance_id, STATE_PREV_CD, false)?;
    let rising = cd && !prev_cd;

    let (new_cv, q) = match (&pv, &cv) {
        (Value::SInt(pv), Value::SInt(cv)) => {
            counter_down_signed!(*pv, *cv, i8, SInt, load, rising)
        }
        (Value::SInt(pv), Value::Null) => counter_down_signed!(*pv, 0, i8, SInt, load, rising),
        (Value::Int(pv), Value::Int(cv)) => {
            counter_down_signed!(*pv, *cv, i16, Int, load, rising)
        }
        (Value::Int(pv), Value::Null) => counter_down_signed!(*pv, 0, i16, Int, load, rising),
        (Value::DInt(pv), Value::DInt(cv)) => {
            counter_down_signed!(*pv, *cv, i32, DInt, load, rising)
        }
        (Value::DInt(pv), Value::Null) => counter_down_signed!(*pv, 0, i32, DInt, load, rising),
        (Value::LInt(pv), Value::LInt(cv)) => {
            counter_down_signed!(*pv, *cv, i64, LInt, load, rising)
        }
        (Value::LInt(pv), Value::Null) => counter_down_signed!(*pv, 0, i64, LInt, load, rising),
        (Value::USInt(pv), Value::USInt(cv)) => {
            counter_down_unsigned!(*pv, *cv, u8, USInt, load, rising)
        }
        (Value::USInt(pv), Value::Null) => {
            counter_down_unsigned!(*pv, 0, u8, USInt, load, rising)
        }
        (Value::UInt(pv), Value::UInt(cv)) => {
            counter_down_unsigned!(*pv, *cv, u16, UInt, load, rising)
        }
        (Value::UInt(pv), Value::Null) => counter_down_unsigned!(*pv, 0, u16, UInt, load, rising),
        (Value::UDInt(pv), Value::UDInt(cv)) => {
            counter_down_unsigned!(*pv, *cv, u32, UDInt, load, rising)
        }
        (Value::UDInt(pv), Value::Null) => {
            counter_down_unsigned!(*pv, 0, u32, UDInt, load, rising)
        }
        (Value::ULInt(pv), Value::ULInt(cv)) => {
            counter_down_unsigned!(*pv, *cv, u64, ULInt, load, rising)
        }
        (Value::ULInt(pv), Value::Null) => {
            counter_down_unsigned!(*pv, 0, u64, ULInt, load, rising)
        }
        _ => return Err(RuntimeError::TypeMismatch),
    };

    set_instance_value(ctx, instance_id, "CV", new_cv);
    write_bool(ctx, instance_id, "Q", q);
    write_bool(ctx, instance_id, STATE_PREV_CD, cd);
    Ok(())
}

pub(super) fn exec_ctud(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let cu = read_bool(ctx, instance_id, "CU")?;
    let cd = read_bool(ctx, instance_id, "CD")?;
    let reset = read_bool(ctx, instance_id, "R")?;
    let load = read_bool(ctx, instance_id, "LD")?;
    let pv = read_value(ctx, instance_id, "PV")?;
    let cv = read_value_or_null(ctx, instance_id, "CV");
    let prev_cu = get_or_init_bool(ctx, instance_id, STATE_PREV_CU, false)?;
    let prev_cd = get_or_init_bool(ctx, instance_id, STATE_PREV_CD, false)?;
    let rising_cu = cu && !prev_cu;
    let rising_cd = cd && !prev_cd;

    let (new_cv, qu, qd) = match (&pv, &cv) {
        (Value::SInt(pv), Value::SInt(cv)) => {
            counter_up_down_signed!(*pv, *cv, i8, SInt, reset, load, rising_cu, rising_cd)
        }
        (Value::SInt(pv), Value::Null) => {
            counter_up_down_signed!(*pv, 0, i8, SInt, reset, load, rising_cu, rising_cd)
        }
        (Value::Int(pv), Value::Int(cv)) => {
            counter_up_down_signed!(*pv, *cv, i16, Int, reset, load, rising_cu, rising_cd)
        }
        (Value::Int(pv), Value::Null) => {
            counter_up_down_signed!(*pv, 0, i16, Int, reset, load, rising_cu, rising_cd)
        }
        (Value::DInt(pv), Value::DInt(cv)) => {
            counter_up_down_signed!(*pv, *cv, i32, DInt, reset, load, rising_cu, rising_cd)
        }
        (Value::DInt(pv), Value::Null) => {
            counter_up_down_signed!(*pv, 0, i32, DInt, reset, load, rising_cu, rising_cd)
        }
        (Value::LInt(pv), Value::LInt(cv)) => {
            counter_up_down_signed!(*pv, *cv, i64, LInt, reset, load, rising_cu, rising_cd)
        }
        (Value::LInt(pv), Value::Null) => {
            counter_up_down_signed!(*pv, 0, i64, LInt, reset, load, rising_cu, rising_cd)
        }
        (Value::USInt(pv), Value::USInt(cv)) => {
            counter_up_down_unsigned!(*pv, *cv, u8, USInt, reset, load, rising_cu, rising_cd)
        }
        (Value::USInt(pv), Value::Null) => {
            counter_up_down_unsigned!(*pv, 0, u8, USInt, reset, load, rising_cu, rising_cd)
        }
        (Value::UInt(pv), Value::UInt(cv)) => {
            counter_up_down_unsigned!(*pv, *cv, u16, UInt, reset, load, rising_cu, rising_cd)
        }
        (Value::UInt(pv), Value::Null) => {
            counter_up_down_unsigned!(*pv, 0, u16, UInt, reset, load, rising_cu, rising_cd)
        }
        (Value::UDInt(pv), Value::UDInt(cv)) => {
            counter_up_down_unsigned!(*pv, *cv, u32, UDInt, reset, load, rising_cu, rising_cd)
        }
        (Value::UDInt(pv), Value::Null) => {
            counter_up_down_unsigned!(*pv, 0, u32, UDInt, reset, load, rising_cu, rising_cd)
        }
        (Value::ULInt(pv), Value::ULInt(cv)) => {
            counter_up_down_unsigned!(*pv, *cv, u64, ULInt, reset, load, rising_cu, rising_cd)
        }
        (Value::ULInt(pv), Value::Null) => {
            counter_up_down_unsigned!(*pv, 0, u64, ULInt, reset, load, rising_cu, rising_cd)
        }
        _ => return Err(RuntimeError::TypeMismatch),
    };

    set_instance_value(ctx, instance_id, "CV", new_cv);
    write_bool(ctx, instance_id, "QU", qu);
    write_bool(ctx, instance_id, "QD", qd);
    write_bool(ctx, instance_id, STATE_PREV_CU, cu);
    write_bool(ctx, instance_id, STATE_PREV_CD, cd);
    Ok(())
}
