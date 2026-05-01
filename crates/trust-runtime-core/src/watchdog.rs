//! Portable watchdog, retain-mode, and fault-policy model records.

use alloc::format;

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::value::Duration;

/// Action taken when the watchdog times out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogAction {
    /// Fault and halt the resource.
    Halt,
    /// Apply safe outputs, then halt the resource.
    SafeHalt,
    /// Warm-restart the resource.
    Restart,
}

impl WatchdogAction {
    /// Parse a watchdog action from runtime configuration text.
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "halt" => Ok(Self::Halt),
            "safe_halt" => Ok(Self::SafeHalt),
            "restart" => Ok(Self::Restart),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid watchdog action '{text}'").into(),
            )),
        }
    }
}

/// Retain persistence backend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainMode {
    /// Do not persist retained values.
    None,
    /// Persist retained values to a file-backed store.
    File,
}

impl RetainMode {
    /// Parse retain mode from runtime configuration text.
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "file" => Ok(Self::File),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid retain mode '{text}'").into(),
            )),
        }
    }
}

/// Policy applied when runtime execution faults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPolicy {
    /// Fault and halt the resource.
    Halt,
    /// Apply safe outputs, then halt the resource.
    SafeHalt,
    /// Warm-restart the resource.
    Restart,
}

impl FaultPolicy {
    /// Parse fault policy from runtime configuration text.
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "halt" => Ok(Self::Halt),
            "safe_halt" => Ok(Self::SafeHalt),
            "restart" => Ok(Self::Restart),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid fault policy '{text}'").into(),
            )),
        }
    }
}

/// Watchdog configuration used by the scheduler loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchdogPolicy {
    /// Whether watchdog checking is enabled.
    pub enabled: bool,
    /// Maximum allowed cycle execution time.
    pub timeout: Duration,
    /// Action to take on timeout.
    pub action: WatchdogAction,
}

/// Normalized fault action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultAction {
    /// Halt the resource.
    Halt,
    /// Apply safe outputs, then halt the resource.
    SafeHalt,
    /// Warm-restart the resource.
    Restart,
}

/// Fault decision derived from watchdog or fault policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultDecision {
    /// Action to take.
    pub action: FaultAction,
    /// Whether I/O safe state must be applied before stopping.
    pub apply_safe_state: bool,
}

impl FaultDecision {
    /// Convert a watchdog action into the normalized fault decision.
    pub fn from_watchdog(action: WatchdogAction) -> Self {
        match action {
            WatchdogAction::Halt => Self {
                action: FaultAction::Halt,
                apply_safe_state: true,
            },
            WatchdogAction::SafeHalt => Self {
                action: FaultAction::SafeHalt,
                apply_safe_state: true,
            },
            WatchdogAction::Restart => Self {
                action: FaultAction::Restart,
                apply_safe_state: false,
            },
        }
    }

    /// Convert a runtime fault policy into the normalized fault decision.
    pub fn from_fault_policy(policy: FaultPolicy) -> Self {
        match policy {
            FaultPolicy::Halt => Self {
                action: FaultAction::Halt,
                apply_safe_state: false,
            },
            FaultPolicy::SafeHalt => Self {
                action: FaultAction::SafeHalt,
                apply_safe_state: true,
            },
            FaultPolicy::Restart => Self {
                action: FaultAction::Restart,
                apply_safe_state: false,
            },
        }
    }
}

impl Default for WatchdogPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout: Duration::from_millis(0),
            action: WatchdogAction::SafeHalt,
        }
    }
}

/// Last fault information exposed by host runtime surfaces.
#[derive(Debug, Clone)]
pub struct FaultInfo {
    /// Human-readable fault reason.
    pub reason: SmolStr,
}

/// Stateful watchdog policy holder.
pub struct WatchdogSubsystem {
    policy: WatchdogPolicy,
}

impl WatchdogSubsystem {
    /// Create a subsystem with the default disabled watchdog policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            policy: WatchdogPolicy::default(),
        }
    }

    /// Replace the active watchdog policy.
    pub fn set_policy(&mut self, policy: WatchdogPolicy) {
        self.policy = policy;
    }

    /// Return the active watchdog policy.
    #[must_use]
    pub fn policy(&self) -> WatchdogPolicy {
        self.policy
    }

    /// Return the fault decision implied by the active watchdog policy.
    #[must_use]
    pub fn decision(&self) -> FaultDecision {
        FaultDecision::from_watchdog(self.policy.action)
    }
}

impl Default for WatchdogSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful runtime fault holder.
pub struct FaultSubsystem {
    policy: FaultPolicy,
    faulted: bool,
    last_fault: Option<RuntimeError>,
}

impl FaultSubsystem {
    /// Create a subsystem with the default halt-on-fault policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            policy: FaultPolicy::Halt,
            faulted: false,
            last_fault: None,
        }
    }

    /// Return the active fault policy.
    #[must_use]
    pub fn policy(&self) -> FaultPolicy {
        self.policy
    }

    /// Replace the active fault policy.
    pub fn set_policy(&mut self, policy: FaultPolicy) {
        self.policy = policy;
    }

    /// Return the fault decision implied by the active policy.
    #[must_use]
    pub fn decision(&self) -> FaultDecision {
        FaultDecision::from_fault_policy(self.policy)
    }

    /// Record a runtime fault.
    pub fn record(&mut self, err: RuntimeError) {
        self.faulted = true;
        self.last_fault = Some(err);
    }

    /// Clear the faulted state and last fault.
    pub fn clear(&mut self) {
        self.faulted = false;
        self.last_fault = None;
    }

    /// Return whether the runtime is currently faulted.
    #[must_use]
    pub fn is_faulted(&self) -> bool {
        self.faulted
    }

    /// Return the last recorded fault, if any.
    #[must_use]
    pub fn last_fault(&self) -> Option<&RuntimeError> {
        self.last_fault.as_ref()
    }
}

impl Default for FaultSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FaultAction, FaultDecision, FaultPolicy, FaultSubsystem, RetainMode, WatchdogAction,
        WatchdogPolicy, WatchdogSubsystem,
    };
    use crate::error::RuntimeError;
    use crate::value::Duration;

    #[test]
    fn watchdog_and_fault_policy_decisions_are_stable() {
        assert_eq!(
            FaultDecision::from_watchdog(WatchdogAction::Halt),
            FaultDecision {
                action: FaultAction::Halt,
                apply_safe_state: true,
            }
        );
        assert_eq!(
            FaultDecision::from_watchdog(WatchdogAction::SafeHalt),
            FaultDecision {
                action: FaultAction::SafeHalt,
                apply_safe_state: true,
            }
        );
        assert_eq!(
            FaultDecision::from_watchdog(WatchdogAction::Restart),
            FaultDecision {
                action: FaultAction::Restart,
                apply_safe_state: false,
            }
        );

        assert_eq!(
            FaultDecision::from_fault_policy(FaultPolicy::Halt),
            FaultDecision {
                action: FaultAction::Halt,
                apply_safe_state: false,
            }
        );
        assert_eq!(
            FaultDecision::from_fault_policy(FaultPolicy::SafeHalt),
            FaultDecision {
                action: FaultAction::SafeHalt,
                apply_safe_state: true,
            }
        );
        assert_eq!(
            FaultDecision::from_fault_policy(FaultPolicy::Restart),
            FaultDecision {
                action: FaultAction::Restart,
                apply_safe_state: false,
            }
        );
    }

    #[test]
    fn watchdog_retain_and_fault_policy_parsers_match_config_contracts() {
        assert_eq!(
            WatchdogAction::parse(" halt ").unwrap(),
            WatchdogAction::Halt
        );
        assert_eq!(
            WatchdogAction::parse("SAFE_HALT").unwrap(),
            WatchdogAction::SafeHalt
        );
        assert_eq!(
            WatchdogAction::parse("restart").unwrap(),
            WatchdogAction::Restart
        );
        assert_eq!(RetainMode::parse("none").unwrap(), RetainMode::None);
        assert_eq!(RetainMode::parse("FILE").unwrap(), RetainMode::File);
        assert_eq!(FaultPolicy::parse("halt").unwrap(), FaultPolicy::Halt);
        assert_eq!(
            FaultPolicy::parse("safe_halt").unwrap(),
            FaultPolicy::SafeHalt
        );
        assert!(WatchdogAction::parse("warn").is_err());
        assert!(RetainMode::parse("memory").is_err());
        assert!(FaultPolicy::parse("degrade").is_err());
    }

    #[test]
    fn watchdog_policy_default_is_disabled_safe_halt() {
        assert_eq!(
            WatchdogPolicy::default(),
            WatchdogPolicy {
                enabled: false,
                timeout: Duration::from_millis(0),
                action: WatchdogAction::SafeHalt,
            }
        );
    }

    #[test]
    fn watchdog_and_fault_subsystems_preserve_state_contracts() {
        let mut watchdog = WatchdogSubsystem::new();
        assert_eq!(watchdog.policy(), WatchdogPolicy::default());
        watchdog.set_policy(WatchdogPolicy {
            enabled: true,
            timeout: Duration::from_millis(5),
            action: WatchdogAction::Restart,
        });
        assert_eq!(
            watchdog.decision(),
            FaultDecision {
                action: FaultAction::Restart,
                apply_safe_state: false,
            }
        );

        let mut faults = FaultSubsystem::new();
        assert_eq!(faults.policy(), FaultPolicy::Halt);
        assert!(!faults.is_faulted());
        assert!(faults.last_fault().is_none());
        faults.set_policy(FaultPolicy::SafeHalt);
        assert_eq!(
            faults.decision(),
            FaultDecision {
                action: FaultAction::SafeHalt,
                apply_safe_state: true,
            }
        );
        faults.record(RuntimeError::WatchdogTimeout);
        assert!(faults.is_faulted());
        assert_eq!(faults.last_fault(), Some(&RuntimeError::WatchdogTimeout));
        faults.clear();
        assert!(!faults.is_faulted());
        assert!(faults.last_fault().is_none());
    }
}
