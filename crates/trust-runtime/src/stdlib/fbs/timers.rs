use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::value::{Duration, Value};

use super::instance::{get_or_init_bool, read_bool, write_bool};
use super::state::{STATE_ACTIVE, STATE_LAST_TIME, STATE_PREV_IN, STATE_TIMING};
use super::BuiltinExecContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerOutput {
    pub q: bool,
    pub et: Duration,
}

#[derive(Debug, Clone)]
pub struct Ton {
    et: Duration,
    q: bool,
}

impl Ton {
    #[must_use]
    pub fn new() -> Self {
        Self {
            et: Duration::ZERO,
            q: false,
        }
    }

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

#[derive(Debug, Clone)]
pub struct Tof {
    et: Duration,
    q: bool,
    prev_in: bool,
    timing: bool,
}

impl Tof {
    #[must_use]
    pub fn new() -> Self {
        Self {
            et: Duration::ZERO,
            q: false,
            prev_in: false,
            timing: false,
        }
    }

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

#[derive(Debug, Clone)]
pub struct Tp {
    et: Duration,
    q: bool,
    prev_in: bool,
    active: bool,
}

impl Tp {
    #[must_use]
    pub fn new() -> Self {
        Self {
            et: Duration::ZERO,
            q: false,
            prev_in: false,
            active: false,
        }
    }

    pub fn step(&mut self, input: bool, pt: Duration, delta: Duration) -> TimerOutput {
        let pt = normalize_duration(pt);
        let rising = !self.prev_in && input;
        if rising {
            self.active = true;
            self.et = Duration::ZERO;
        }
        if self.active {
            self.et = accumulate_elapsed(self.et, delta, pt);
            if self.et.as_nanos() >= pt.as_nanos() {
                self.active = false;
                self.et = pt;
            }
        }
        if !input && !self.active {
            self.et = Duration::ZERO;
        }
        self.q = self.active;
        self.prev_in = input;
        let et = if self.active || input {
            self.et
        } else {
            Duration::ZERO
        };
        TimerOutput { q: self.q, et }
    }
}

impl Default for Tp {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn exec_ton(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let input = read_bool(ctx, instance_id, "IN")?;
    let (pt, is_ltime) = read_time_input(ctx, instance_id, "PT")?;
    let et = read_time_value(ctx, instance_id, "ET")?;
    let q = read_bool(ctx, instance_id, "Q")?;
    let delta = elapsed_since(ctx, instance_id)?;
    let mut ton = Ton { et, q };
    let out = ton.step(input, pt, delta);
    write_bool(ctx, instance_id, "Q", out.q);
    write_time_value(ctx, instance_id, "ET", out.et, is_ltime);
    Ok(())
}

pub(super) fn exec_tof(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let input = read_bool(ctx, instance_id, "IN")?;
    let (pt, is_ltime) = read_time_input(ctx, instance_id, "PT")?;
    let et = read_time_value(ctx, instance_id, "ET")?;
    let q = read_bool(ctx, instance_id, "Q")?;
    let prev_in = get_or_init_bool(ctx, instance_id, STATE_PREV_IN, false)?;
    let timing = get_or_init_bool(ctx, instance_id, STATE_TIMING, false)?;
    let delta = elapsed_since(ctx, instance_id)?;
    let mut tof = Tof {
        et,
        q,
        prev_in,
        timing,
    };
    let out = tof.step(input, pt, delta);
    write_bool(ctx, instance_id, "Q", out.q);
    write_time_value(ctx, instance_id, "ET", out.et, is_ltime);
    write_bool(ctx, instance_id, STATE_PREV_IN, tof.prev_in);
    write_bool(ctx, instance_id, STATE_TIMING, tof.timing);
    Ok(())
}

pub(super) fn exec_tp(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let input = read_bool(ctx, instance_id, "IN")?;
    let (pt, is_ltime) = read_time_input(ctx, instance_id, "PT")?;
    let et = read_time_value(ctx, instance_id, "ET")?;
    let q = read_bool(ctx, instance_id, "Q")?;
    let prev_in = get_or_init_bool(ctx, instance_id, STATE_PREV_IN, false)?;
    let active = get_or_init_bool(ctx, instance_id, STATE_ACTIVE, false)?;
    let delta = elapsed_since(ctx, instance_id)?;
    let mut tp = Tp {
        et,
        q,
        prev_in,
        active,
    };
    let out = tp.step(input, pt, delta);
    write_bool(ctx, instance_id, "Q", out.q);
    write_time_value(ctx, instance_id, "ET", out.et, is_ltime);
    write_bool(ctx, instance_id, STATE_PREV_IN, tp.prev_in);
    write_bool(ctx, instance_id, STATE_ACTIVE, tp.active);
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

fn read_time_input(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<(Duration, bool), RuntimeError> {
    match ctx.storage.get_instance_var(instance_id, name) {
        Some(Value::Time(value)) => Ok((*value, false)),
        Some(Value::LTime(value)) => Ok((*value, true)),
        Some(Value::Null) | None => Ok((Duration::ZERO, false)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn read_time_value(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<Duration, RuntimeError> {
    match ctx.storage.get_instance_var(instance_id, name) {
        Some(Value::Time(value)) | Some(Value::LTime(value)) => Ok(*value),
        Some(Value::Null) | None => Ok(Duration::ZERO),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn write_time_value(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    value: Duration,
    is_ltime: bool,
) {
    let value = if is_ltime {
        Value::LTime(value)
    } else {
        Value::Time(value)
    };
    ctx.storage.set_instance_var(instance_id, name, value);
}

fn elapsed_since(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
) -> Result<Duration, RuntimeError> {
    let last = get_or_init_duration(ctx, instance_id, STATE_LAST_TIME, ctx.now)?;
    let delta_nanos = ctx.now.as_nanos() - last.as_nanos();
    let delta = if delta_nanos <= 0 {
        Duration::ZERO
    } else {
        Duration::from_nanos(delta_nanos)
    };
    set_internal_duration(ctx, instance_id, STATE_LAST_TIME, ctx.now);
    Ok(delta)
}

fn get_or_init_duration(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    default: Duration,
) -> Result<Duration, RuntimeError> {
    match ctx.storage.get_instance_var(instance_id, name) {
        Some(Value::Time(value)) | Some(Value::LTime(value)) => Ok(*value),
        Some(Value::Null) | None => {
            set_internal_duration(ctx, instance_id, name, default);
            Ok(default)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn set_internal_duration(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    value: Duration,
) {
    ctx.storage
        .set_instance_var(instance_id, name, Value::LTime(value));
}
