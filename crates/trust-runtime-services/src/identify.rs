//! `identify` (specification 11, "Identify"): a runtime shows where it is by blinking one output.
//!
//! The request, its timer, the blink and the `identifying` member of `status` are this module
//! (specification 34, ARCH-11 and SERVICE-07). A platform supplies only the way to drive the
//! output ([`IdentifyOutput`]) and the monotonic clock the blink is measured on; it holds one
//! [`Identify`] and calls [`Identify::tick`] at every point where it can change an output: where
//! it hands its outputs to the drivers at the end of a scan, and in every pass of its loop in
//! which no scan runs (a resource that is paused, stopped or faulted; on a chip the pass that
//! calls [`RunnerPlatform::idle`](crate::runner::RunnerPlatform::idle)). `identify` has no
//! restriction on the state of the resource (specification 11, "Identify"): it is accepted in
//! every state, and a stop, a fault or a pause does not end it. Nothing here starts a thread or
//! sleeps: time is a reading of the clock, so the blink follows the points at which the platform
//! ticks. [`Identify::cancel`] ends a run at once, as a platform does when its process ends.
//!
//! [`Identify`] is reached by the request and by `status` through
//! [`RecordsPort::with_identify`] as an [`IdentifyState`]. A platform that returns none has no
//! identify output and answers `identify_not_configured`.

use alloc::format;
use core::time::Duration;

use serde_json::{json, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant, TimeSpan};

use crate::ports::RecordsPort;
use crate::protocol::ControlResponse;
use crate::status::invalid_params;

/// Stable code of an `identify` request on a runtime that has no identify output.
pub const IDENTIFY_NOT_CONFIGURED: &str = "identify_not_configured";

/// Stable code of an `identify` request whose output could not be made ready.
pub const IDENTIFY_OUTPUT_UNAVAILABLE: &str = "identify_output_unavailable";

/// The refusal text of every problem with the parameters of `identify`.
const DURATION_REFUSAL: &str = "duration_s must be a whole number of seconds from 1 to 60";

/// Half a period of the blink: 250 ms on and 250 ms off make 2 Hz with 50 % on.
pub const BLINK_HALF_PERIOD: TimeSpan = TimeSpan::new(Duration::from_millis(250));

/// How long `identify` runs: a whole number of seconds from [`Self::MIN_SECONDS`] through
/// [`Self::MAX_SECONDS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentifyDuration(u8);

impl IdentifyDuration {
    /// The shortest duration, in seconds.
    pub const MIN_SECONDS: u8 = 1;
    /// The longest duration, in seconds.
    pub const MAX_SECONDS: u8 = 60;

    /// The duration of `seconds`, or `None` outside 1 through 60.
    #[must_use]
    pub fn new(seconds: u64) -> Option<Self> {
        u8::try_from(seconds)
            .ok()
            .filter(|seconds| (Self::MIN_SECONDS..=Self::MAX_SECONDS).contains(seconds))
            .map(Self)
    }

    /// The duration in seconds.
    #[must_use]
    pub const fn seconds(self) -> u8 {
        self.0
    }

    fn span(self) -> TimeSpan {
        TimeSpan::new(Duration::from_secs(u64::from(self.0)))
    }
}

/// The platform's way to drive the identify output: a bit of the output image that the platform
/// hands to its drivers, or a pin it drives itself.
pub trait IdentifyOutput {
    /// Gets the output ready before `identify` starts, for example requests a pin as an output.
    /// Every start calls it, so it must be cheap once the output is ready. A failure refuses the
    /// request with `identify_output_unavailable` and starts nothing.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an output that cannot be made ready.
    fn prepare(&mut self) -> Result<(), RuntimeError>;

    /// Drives the output on (`true`) or off (`false`).
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an output that cannot be driven.
    fn drive(&mut self, on: bool) -> Result<(), RuntimeError>;
}

/// What the `identify` request and `status` use of a platform's identify state.
pub trait IdentifyState {
    /// Starts `identify` for `duration` from now, or starts it again if it runs: the end is
    /// `duration` after this call and the blink begins again with the output on.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an output that cannot be made ready; nothing starts.
    fn start(&mut self, duration: IdentifyDuration) -> Result<(), RuntimeError>;

    /// Whether `identify` runs now: it was started and its duration has not passed.
    fn is_identifying(&self) -> bool;
}

/// One run of `identify`.
#[derive(Debug, Clone, Copy)]
struct Run {
    /// The end of the duration.
    until: MonotonicInstant,
    /// The next 250 ms edge of the blink; none until the first tick, which starts the blink.
    next_edge: Option<MonotonicInstant>,
    /// The blink's level now.
    on: bool,
}

/// The identify state of one runtime: its clock, its output, the run in progress and the level
/// the output was last driven to.
///
/// The blink is on for 250 ms and off for 250 ms ([`BLINK_HALF_PERIOD`]). The first
/// [`tick`](Self::tick) after a start turns the output on and counts the 250 ms from there; each
/// later tick changes the output when an edge is due, at most once per tick, so a platform that
/// ticks less often than every 250 ms toggles the output at every tick instead of showing a
/// constant level.
#[derive(Debug)]
pub struct Identify<C, O> {
    clock: C,
    output: O,
    run: Option<Run>,
    driven: bool,
}

impl<C: MonotonicClock, O: IdentifyOutput> Identify<C, O> {
    /// An identify state with nothing running and the output off, measured on `clock`.
    #[must_use]
    pub const fn new(clock: C, output: O) -> Self {
        Self {
            clock,
            output,
            run: None,
            driven: false,
        }
    }

    /// The platform's output, for a platform that reads what it drives (an output image bit).
    #[must_use]
    pub const fn output(&self) -> &O {
        &self.output
    }

    /// Applies the blink at the clock's current reading: drives the output to the level of now
    /// when that differs from the level it was last driven to, and ends the run once its duration
    /// has passed, with the output off.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an output that cannot be driven. A run in progress ends; an output
    /// that failed to go off is driven off again at the next call.
    pub fn tick(&mut self) -> Result<(), RuntimeError> {
        let now = self.clock.now();
        let level = match self.run {
            None => false,
            Some(run) if now >= run.until => {
                self.run = None;
                false
            }
            Some(mut run) => {
                match run.next_edge {
                    // The first tick starts the blink with the output on, however late it comes.
                    None => run.next_edge = Some(now.saturating_add(BLINK_HALF_PERIOD)),
                    Some(edge) if now >= edge => {
                        run.on = !run.on;
                        // A late edge never schedules the next one in the past, so a platform
                        // that ticks every cycle of more than 250 ms changes the output at
                        // every tick.
                        run.next_edge = Some(edge.saturating_add(BLINK_HALF_PERIOD).max(now));
                    }
                    Some(_) => {}
                }
                self.run = Some(run);
                run.on
            }
        };
        self.set_output(level)
    }

    /// Ends the run now and drives the output off, as the platform does when its process ends.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an output that cannot be driven.
    pub fn cancel(&mut self) -> Result<(), RuntimeError> {
        self.run = None;
        self.set_output(false)
    }

    /// Drives the output to `level` unless it is there already. A failure ends the run.
    fn set_output(&mut self, level: bool) -> Result<(), RuntimeError> {
        if level == self.driven {
            return Ok(());
        }
        match self.output.drive(level) {
            Ok(()) => {
                self.driven = level;
                Ok(())
            }
            Err(error) => {
                self.run = None;
                Err(error)
            }
        }
    }
}

impl<C: MonotonicClock, O: IdentifyOutput> IdentifyState for Identify<C, O> {
    fn start(&mut self, duration: IdentifyDuration) -> Result<(), RuntimeError> {
        self.output.prepare()?;
        let now = self.clock.now();
        self.run = Some(Run {
            until: now.saturating_add(duration.span()),
            next_edge: None,
            on: true,
        });
        Ok(())
    }

    fn is_identifying(&self) -> bool {
        self.run.is_some_and(|run| self.clock.now() < run.until)
    }
}

/// The duration in the parameters of `identify`: exactly `{"duration_s": n}` with `n` a whole
/// number of seconds from 1 through 60.
fn requested_duration(params: Option<&Value>) -> Option<IdentifyDuration> {
    let members = params?.as_object()?;
    if members.len() != 1 {
        return None;
    }
    IdentifyDuration::new(members.get("duration_s")?.as_u64()?)
}

/// `identify` with `{"duration_s": n}`: starts the blink of the platform's identify output
/// (specification 11, "Identify").
///
/// The parameters are checked first, then whether the platform has an identify output, then
/// whether the output can be made ready. The state of the resource is not checked: `identify` is
/// accepted while the resource runs, is paused, is stopped or is faulted. A refusal starts nothing.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn identify<H: RecordsPort>(id: u64, params: Option<&Value>, host: &H) -> ControlResponse {
    let Some(duration) = requested_duration(params) else {
        return invalid_params(id, DURATION_REFUSAL);
    };
    if host.with_identify(|_| ()).is_none() {
        return not_configured(id);
    }
    match host.with_identify(|identify| identify.start(duration)) {
        Some(Ok(())) => ControlResponse::ok(
            id,
            json!({"identifying": true, "duration_s": duration.seconds()}),
        ),
        Some(Err(error)) => ControlResponse::error_with_code(
            id,
            format!("identify output unavailable: {error}"),
            IDENTIFY_OUTPUT_UNAVAILABLE,
        ),
        None => not_configured(id),
    }
}

/// The refusal of a runtime without an identify output.
fn not_configured(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(
        id,
        "identify is not configured: set [runtime.identify] output in runtime.toml".into(),
        IDENTIFY_NOT_CONFIGURED,
    )
}

/// Whether `identify` runs on `host`; `false` on a runtime without an identify output.
#[must_use]
pub fn is_identifying<H: RecordsPort>(host: &H) -> bool {
    host.with_identify(|identify| identify.is_identifying())
        .unwrap_or(false)
}

#[cfg(test)]
mod contract_tests;
