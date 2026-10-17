//! Resource commands (specification 11, §6.6, §6.7.1 and §6.9): the bounded queue's refusals, the
//! scheduler policy set, what each kind of command does to the scan loop, and the reload claim.

use core::fmt;

use trust_runtime_core::value::Duration;
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

/// Commands the resource queue holds; a command beyond this bound is refused, not queued.
pub const RESOURCE_COMMAND_CAPACITY: usize = 1_024;

/// Why the resource queue did not take a command; nothing was queued in either case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSendError {
    /// The queue already holds [`RESOURCE_COMMAND_CAPACITY`] commands.
    Full,
    /// The scan no longer receives commands.
    Closed,
}

impl fmt::Display for CommandSendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => formatter.write_str("resource command queue is full"),
            Self::Closed => formatter.write_str("command channel closed"),
        }
    }
}

impl core::error::Error for CommandSendError {}

/// The scheduler's copies of the watchdog, fault and retain-save policies, which `config.set`
/// changes.
///
/// They travel in one command, so a full queue refuses the whole set and the scan applies the
/// whole set as one step (specification 11, §6.8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerPolicies {
    /// Watchdog policy.
    pub watchdog: WatchdogPolicy,
    /// Fault policy.
    pub fault_policy: FaultPolicy,
    /// Interval of periodic retain saves, if any.
    pub retain_save_interval: Option<Duration>,
}

/// The kind of work a resource command asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
    /// Stop running scan cycles until resumed.
    Pause,
    /// Run scan cycles again.
    Resume,
    /// Replace the program by online change.
    Reload,
    /// Write PLC variables.
    VariableWrite,
    /// Change a configuration or read a report.
    Service,
}

/// What the running scan loop does with a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunningCommandEffect {
    /// Pause: no cycles run and the resource reports `Paused`.
    Pause,
    /// Resume cycles; the resource reports `Running`.
    Resume,
    /// The runtime applies the command.
    Apply,
}

/// What the faulted wait does with a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultedCommandEffect {
    /// Ignore it: the control plane refuses pause and resume while faulted.
    Ignore,
    /// Discard it: fault handling is a clearing boundary for variable writes (specification 11,
    /// §6.9.2).
    Discard,
    /// Apply it as an online change; one that succeeds ends the fault.
    Recover,
    /// Apply it exactly as while running.
    Apply,
}

/// What the running scan loop does with a command of `class`.
#[must_use]
pub fn running_command_effect(class: CommandClass) -> RunningCommandEffect {
    match class {
        CommandClass::Pause => RunningCommandEffect::Pause,
        CommandClass::Resume => RunningCommandEffect::Resume,
        CommandClass::Reload | CommandClass::VariableWrite | CommandClass::Service => {
            RunningCommandEffect::Apply
        }
    }
}

/// What the faulted wait does with a command of `class`.
#[must_use]
pub fn faulted_command_effect(class: CommandClass) -> FaultedCommandEffect {
    match class {
        CommandClass::Pause | CommandClass::Resume => FaultedCommandEffect::Ignore,
        CommandClass::VariableWrite => FaultedCommandEffect::Discard,
        CommandClass::Reload => FaultedCommandEffect::Recover,
        CommandClass::Service => FaultedCommandEffect::Apply,
    }
}

/// Who decided a queued reload (specification 11, §6.7.1).
///
/// The caller withdraws a reload when its wait ends first; the resource takes it before applying
/// it. Exactly one of the two succeeds, so a reload is never applied after its caller was told
/// that it timed out. A platform that shares the claim between threads keeps [`Self::code`] in
/// one byte and moves it by compare-and-exchange to the state that [`Self::take`] or
/// [`Self::withdraw`] gives for the byte's state ([`Self::from_code`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ReloadClaimState {
    /// Neither side has decided.
    #[default]
    Pending = 0,
    /// The resource took the reload and applies it.
    Taken = 1,
    /// The caller withdrew the reload; it is never applied.
    Withdrawn = 2,
}

impl ReloadClaimState {
    /// The state after the resource takes the reload, or `None` when it was decided already.
    #[must_use]
    pub fn take(self) -> Option<Self> {
        (self == Self::Pending).then_some(Self::Taken)
    }

    /// The state after the caller withdraws the reload, or `None` when it was decided already.
    #[must_use]
    pub fn withdraw(self) -> Option<Self> {
        (self == Self::Pending).then_some(Self::Withdrawn)
    }

    /// The state's one-byte code.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// The state with one-byte code `code`, or `None` for a byte that is no state's code.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Pending),
            1 => Some(Self::Taken),
            2 => Some(Self::Withdrawn),
            _ => None,
        }
    }
}

#[cfg(test)]
mod contract_tests;
