use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::InstanceId;
use crate::value::{Duration, Value};

use super::instance::{
    get_or_init_bool, leaf_bytes, read_bool, read_scalar_or_null, set_instance_value, write_bool,
    write_leaf_bytes,
};
use super::members::Member;
use super::BuiltinExecContext;

/// The primitives of a `TIME` and an `LTIME` leaf, eight little-endian bytes of nanoseconds, and of
/// a native duration slot, the same eight bytes and a family byte, 0 for a `TIME` and 1 for an
/// `LTIME` (`memory::flat`): a timer's `PT` and `ET`, which take either, are such slots, as is the
/// time of its last call.
const TIME: u16 = 16;
const LTIME: u16 = 17;
const DURATION_SLOT: u16 = 0x0101;
const TIME_FAMILY: u8 = 0;
const LTIME_FAMILY: u8 = 1;

/// The duration of a leaf's eight little-endian bytes of nanoseconds, as its view reads it; `None`
/// for a leaf of fewer bytes. The bytes are taken one by one: a slice copy calls `memcpy` when the
/// compiler builds for size (checklist P1-7).
fn leaf_duration(bytes: &[u8]) -> Option<Duration> {
    let &[b0, b1, b2, b3, b4, b5, b6, b7, ..] = bytes else {
        return None;
    };
    Some(Duration::from_nanos(i64::from_le_bytes([
        b0, b1, b2, b3, b4, b5, b6, b7,
    ])))
}

/// The duration `member` holds where it lies, a `TIME`, an `LTIME` or a duration slot's, with
/// whether it is an `LTIME`, which a slot's family byte says; `(ZERO, false)` when it holds no value,
/// as a member that holds `Null` reads (checklist P1-4). `None` when the member is read through its
/// view.
fn duration_leaf<S: StoreAccess>(
    ctx: &BuiltinExecContext<'_, S>,
    member: Member,
) -> Option<(Duration, bool)> {
    for (prim, is_ltime) in [
        (TIME, Some(false)),
        (LTIME, Some(true)),
        (DURATION_SLOT, None),
    ] {
        match leaf_bytes(ctx, member, prim) {
            Some(Some(bytes)) => {
                let value = leaf_duration(bytes)?;
                let is_ltime = is_ltime.unwrap_or(bytes.get(8) == Some(&LTIME_FAMILY));
                return Some((value, is_ltime));
            }
            Some(None) => return Some((Duration::ZERO, false)),
            None => {}
        }
    }
    None
}

/// Timer output and elapsed duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerOutput {
    /// Current Boolean output.
    pub q: bool,
    /// Elapsed timer duration.
    pub et: Duration,
}

/// On-delay timer state.
#[derive(Debug, Clone)]
pub struct Ton {
    et: Duration,
    q: bool,
}

impl Ton {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            et: Duration::ZERO,
            q: false,
        }
    }

    /// Advance timing from the supplied input, preset and elapsed duration.
    pub fn step(&mut self, input: bool, pt: Duration, delta: Duration) -> TimerOutput {
        let pt = normalize_duration(pt);
        if !input {
            self.et = Duration::ZERO;
            self.q = false;
        } else {
            self.et = accumulate_elapsed(self.et, delta, pt);
            self.q = self.et.as_nanos() >= pt.as_nanos();
        }
        let et = if self.et.as_nanos() >= pt.as_nanos() {
            pt
        } else {
            self.et
        };
        TimerOutput { q: self.q, et }
    }
}

impl Default for Ton {
    fn default() -> Self {
        Self::new()
    }
}

/// Off-delay timer state.
#[derive(Debug, Clone)]
pub struct Tof {
    et: Duration,
    q: bool,
    prev_in: bool,
    timing: bool,
}

impl Tof {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            et: Duration::ZERO,
            q: false,
            prev_in: false,
            timing: false,
        }
    }

    /// Advance timing from the supplied input, preset and elapsed duration.
    pub fn step(&mut self, input: bool, pt: Duration, delta: Duration) -> TimerOutput {
        let pt = normalize_duration(pt);
        if input {
            self.q = true;
            self.et = Duration::ZERO;
            self.timing = false;
        } else {
            if self.prev_in {
                self.timing = true;
                self.et = Duration::ZERO;
            }
            if self.timing {
                self.et = accumulate_elapsed(self.et, delta, pt);
                if self.et.as_nanos() >= pt.as_nanos() {
                    self.q = false;
                    self.et = pt;
                    self.timing = false;
                } else {
                    self.q = true;
                }
            } else {
                self.q = false;
            }
        }
        self.prev_in = input;
        TimerOutput {
            q: self.q,
            et: self.et,
        }
    }
}

impl Default for Tof {
    fn default() -> Self {
        Self::new()
    }
}

/// Pulse timer state.
#[derive(Debug, Clone)]
pub struct Tp {
    et: Duration,
    q: bool,
    prev_in: bool,
    active: bool,
}

impl Tp {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            et: Duration::ZERO,
            q: false,
            prev_in: false,
            active: false,
        }
    }

    /// Advance timing from the supplied input, preset and elapsed duration.
    pub fn step(&mut self, input: bool, pt: Duration, delta: Duration) -> TimerOutput {
        let pt = normalize_duration(pt);
        let rising = !self.prev_in && input;
        if rising {
            self.active = true;
            self.et = Duration::ZERO;
        }
        let mut expired = false;
        if self.active {
            self.et = accumulate_elapsed(self.et, delta, pt);
            if self.et.as_nanos() >= pt.as_nanos() {
                self.active = false;
                self.et = pt;
                expired = true;
            }
        }
        // The call that completes the pulse keeps ET at PT even when it samples IN FALSE. The first
        // later executed call sampling IN FALSE resets ET (spec 08 section 5).
        if !input && !self.active && !expired {
            self.et = Duration::ZERO;
        }
        self.q = self.active;
        self.prev_in = input;
        TimerOutput {
            q: self.q,
            et: self.et,
        }
    }
}

impl Default for Tp {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn exec_ton<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let input = read_bool(ctx, instance_id, Member::In)?;
    let (pt, is_ltime) = read_time_input(ctx, instance_id, Member::Pt)?;
    let et = read_time_value(ctx, instance_id, Member::Et)?;
    let q = read_bool(ctx, instance_id, Member::Q)?;
    let delta = elapsed_since(ctx, instance_id)?;
    let mut ton = Ton { et, q };
    let out = ton.step(input, pt, delta);
    write_bool(ctx, instance_id, Member::Q, out.q);
    write_time_value(ctx, instance_id, Member::Et, out.et, is_ltime);
    Ok(())
}

pub(super) fn exec_tof<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let input = read_bool(ctx, instance_id, Member::In)?;
    let (pt, is_ltime) = read_time_input(ctx, instance_id, Member::Pt)?;
    let et = read_time_value(ctx, instance_id, Member::Et)?;
    let q = read_bool(ctx, instance_id, Member::Q)?;
    let prev_in = get_or_init_bool(ctx, instance_id, Member::PrevIn, false)?;
    let timing = get_or_init_bool(ctx, instance_id, Member::Timing, false)?;
    let delta = elapsed_since(ctx, instance_id)?;
    let mut tof = Tof {
        et,
        q,
        prev_in,
        timing,
    };
    let out = tof.step(input, pt, delta);
    write_bool(ctx, instance_id, Member::Q, out.q);
    write_time_value(ctx, instance_id, Member::Et, out.et, is_ltime);
    write_bool(ctx, instance_id, Member::PrevIn, tof.prev_in);
    write_bool(ctx, instance_id, Member::Timing, tof.timing);
    Ok(())
}

pub(super) fn exec_tp<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let input = read_bool(ctx, instance_id, Member::In)?;
    let (pt, is_ltime) = read_time_input(ctx, instance_id, Member::Pt)?;
    let et = read_time_value(ctx, instance_id, Member::Et)?;
    let q = read_bool(ctx, instance_id, Member::Q)?;
    let prev_in = get_or_init_bool(ctx, instance_id, Member::PrevIn, false)?;
    let active = get_or_init_bool(ctx, instance_id, Member::Active, false)?;
    let delta = elapsed_since(ctx, instance_id)?;
    let mut tp = Tp {
        et,
        q,
        prev_in,
        active,
    };
    let out = tp.step(input, pt, delta);
    write_bool(ctx, instance_id, Member::Q, out.q);
    write_time_value(ctx, instance_id, Member::Et, out.et, is_ltime);
    write_bool(ctx, instance_id, Member::PrevIn, tp.prev_in);
    write_bool(ctx, instance_id, Member::Active, tp.active);
    Ok(())
}

fn normalize_duration(value: Duration) -> Duration {
    if value.as_nanos() < 0 {
        Duration::ZERO
    } else {
        value
    }
}

fn accumulate_elapsed(elapsed: Duration, delta: Duration, pt: Duration) -> Duration {
    let delta_nanos = delta.as_nanos().max(0);
    let next = elapsed.as_nanos().saturating_add(delta_nanos);
    Duration::from_nanos(next.min(pt.as_nanos()))
}

fn read_time_input<S: StoreAccess>(
    ctx: &BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
) -> Result<(Duration, bool), RuntimeError> {
    if let Some(input) = duration_leaf(ctx, member) {
        return Ok(input);
    }
    match read_scalar_or_null(ctx, instance_id, member)? {
        Value::Time(value) => Ok((value, false)),
        Value::LTime(value) => Ok((value, true)),
        Value::Null => Ok((Duration::ZERO, false)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn read_time_value<S: StoreAccess>(
    ctx: &BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
) -> Result<Duration, RuntimeError> {
    if let Some((value, _)) = duration_leaf(ctx, member) {
        return Ok(value);
    }
    match read_scalar_or_null(ctx, instance_id, member)? {
        Value::Time(value) | Value::LTime(value) => Ok(value),
        Value::Null => Ok(Duration::ZERO),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn write_time_value<S: StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
    value: Duration,
    is_ltime: bool,
) {
    // A duration of the member's own primitive is its leaf's bytes, and a duration slot takes them
    // and the duration's family byte, as the store writes a `TIME` or an `LTIME`.
    let prim = if is_ltime { LTIME } else { TIME };
    let bytes = value.as_nanos().to_le_bytes();
    if write_leaf_bytes(ctx, member, prim, &bytes) {
        return;
    }
    let [b0, b1, b2, b3, b4, b5, b6, b7] = bytes;
    let family = if is_ltime { LTIME_FAMILY } else { TIME_FAMILY };
    if write_leaf_bytes(
        ctx,
        member,
        DURATION_SLOT,
        &[b0, b1, b2, b3, b4, b5, b6, b7, family],
    ) {
        return;
    }
    let value = if is_ltime {
        Value::LTime(value)
    } else {
        Value::Time(value)
    };
    set_instance_value(ctx, instance_id, member, value);
}

fn elapsed_since<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
) -> Result<Duration, RuntimeError> {
    let last = get_or_init_duration(ctx, instance_id, Member::LastTime, ctx.now)?;
    // A span wider than the signed nanosecond range saturates at i64::MAX, which is never less
    // than a preset, so the accumulated ET completes the timer (spec 08 section 5). A negative
    // span still contributes nothing.
    let delta_nanos = ctx.now.as_nanos().saturating_sub(last.as_nanos());
    let delta = if delta_nanos <= 0 {
        Duration::ZERO
    } else {
        Duration::from_nanos(delta_nanos)
    };
    set_internal_duration(ctx, instance_id, Member::LastTime, ctx.now);
    Ok(delta)
}

fn get_or_init_duration<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
    default: Duration,
) -> Result<Duration, RuntimeError> {
    match leaf_bytes(ctx, member, DURATION_SLOT) {
        Some(Some(bytes)) => {
            if let Some(value) = leaf_duration(bytes) {
                return Ok(value);
            }
        }
        Some(None) => {
            set_internal_duration(ctx, instance_id, member, default);
            return Ok(default);
        }
        None => {}
    }
    match read_scalar_or_null(ctx, instance_id, member)? {
        Value::Time(value) | Value::LTime(value) => Ok(value),
        Value::Null => {
            set_internal_duration(ctx, instance_id, member, default);
            Ok(default)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn set_internal_duration<S: StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
    value: Duration,
) {
    // The slot holds the duration's eight bytes and the `LTIME` family byte, as the store writes an
    // `LTIME` into it.
    let [b0, b1, b2, b3, b4, b5, b6, b7] = value.as_nanos().to_le_bytes();
    let slot = [b0, b1, b2, b3, b4, b5, b6, b7, LTIME_FAMILY];
    if !write_leaf_bytes(ctx, member, DURATION_SLOT, &slot) {
        set_instance_value(ctx, instance_id, member, Value::LTime(value));
    }
}
