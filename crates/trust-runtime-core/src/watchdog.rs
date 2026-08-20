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

impl WatchdogPolicy {
    /// Normalize a policy before runtime use.
    ///
    /// A disabled watchdog may carry a zero timeout. An enabled watchdog may not:
    /// zero creates an always-expired deadline and can brick the scan loop.
    #[must_use]
    pub fn normalized_for_runtime(mut self) -> Self {
        if self.enabled && self.timeout.as_nanos() <= 0 {
            self.timeout = Duration::from_millis(1);
        }
        self
    }
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
        self.policy = policy.normalized_for_runtime();
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
        FaultAction, FaultDecision, FaultInfo, FaultPolicy, FaultSubsystem, RetainMode,
        WatchdogAction, WatchdogPolicy, WatchdogSubsystem,
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

        for (policy, expected) in [
            (
                FaultPolicy::Halt,
                FaultDecision {
                    action: FaultAction::Halt,
                    apply_safe_state: false,
                },
            ),
            (
                FaultPolicy::SafeHalt,
                FaultDecision {
                    action: FaultAction::SafeHalt,
                    apply_safe_state: true,
                },
            ),
            (
                FaultPolicy::Restart,
                FaultDecision {
                    action: FaultAction::Restart,
                    apply_safe_state: false,
                },
            ),
        ] {
            let mut faults = FaultSubsystem::new();
            faults.set_policy(policy);
            assert_eq!(faults.decision(), expected);
            assert_eq!(faults.policy(), policy);
            assert!(!faults.is_faulted());
            assert_eq!(faults.last_fault(), None);

            faults.record(RuntimeError::WatchdogTimeout);
            assert_eq!(faults.decision(), expected);
            assert_eq!(faults.policy(), policy);
            assert!(faults.is_faulted());
            assert_eq!(faults.last_fault(), Some(&RuntimeError::WatchdogTimeout));
        }
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
    fn configuration_parsers_cover_complete_tokens_and_exact_invalid_text() {
        for (text, expected) in [
            ("halt", WatchdogAction::Halt),
            (" SAFE_HALT ", WatchdogAction::SafeHalt),
            ("\trestart\n", WatchdogAction::Restart),
        ] {
            assert_eq!(WatchdogAction::parse(text), Ok(expected));
        }
        for (text, expected) in [(" none ", RetainMode::None), ("FILE", RetainMode::File)] {
            assert_eq!(RetainMode::parse(text), Ok(expected));
        }
        for (text, expected) in [
            ("HALT", FaultPolicy::Halt),
            (" safe_halt ", FaultPolicy::SafeHalt),
            ("Restart", FaultPolicy::Restart),
        ] {
            assert_eq!(FaultPolicy::parse(text), Ok(expected));
        }

        assert_eq!(
            WatchdogAction::parse(" warn "),
            Err(RuntimeError::InvalidConfig(
                "invalid watchdog action ' warn '".into()
            ))
        );
        assert_eq!(
            RetainMode::parse("memory"),
            Err(RuntimeError::InvalidConfig(
                "invalid retain mode 'memory'".into()
            ))
        );
        assert_eq!(
            FaultPolicy::parse("degrade"),
            Err(RuntimeError::InvalidConfig(
                "invalid fault policy 'degrade'".into()
            ))
        );
    }

    #[test]
    fn subsystem_defaults_and_fault_info_preserve_inert_identity() {
        for watchdog in [WatchdogSubsystem::new(), WatchdogSubsystem::default()] {
            assert_eq!(watchdog.policy(), WatchdogPolicy::default());
            assert_eq!(
                watchdog.decision(),
                FaultDecision {
                    action: FaultAction::SafeHalt,
                    apply_safe_state: true,
                }
            );
        }

        for faults in [FaultSubsystem::new(), FaultSubsystem::default()] {
            assert_eq!(faults.policy(), FaultPolicy::Halt);
            assert_eq!(
                faults.decision(),
                FaultDecision {
                    action: FaultAction::Halt,
                    apply_safe_state: false,
                }
            );
            assert!(!faults.is_faulted());
            assert_eq!(faults.last_fault(), None);
        }

        let info = FaultInfo {
            reason: "cycle deadline expired".into(),
        };
        let cloned = info.clone();
        assert_eq!(info.reason.as_str(), "cycle deadline expired");
        assert_eq!(cloned.reason, info.reason);
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
    fn enabled_watchdog_policy_normalizes_zero_timeout() {
        let policy = WatchdogPolicy {
            enabled: true,
            timeout: Duration::ZERO,
            action: WatchdogAction::Halt,
        }
        .normalized_for_runtime();

        assert_eq!(policy.timeout, Duration::from_millis(1));

        let disabled = WatchdogPolicy::default().normalized_for_runtime();
        assert_eq!(disabled.timeout, Duration::ZERO);
    }

    #[test]
    fn watchdog_policy_normalization_and_installation_cover_boundaries() {
        for (input, expected) in [
            (
                WatchdogPolicy {
                    enabled: false,
                    timeout: Duration::from_nanos(-1),
                    action: WatchdogAction::Halt,
                },
                WatchdogPolicy {
                    enabled: false,
                    timeout: Duration::from_nanos(-1),
                    action: WatchdogAction::Halt,
                },
            ),
            (
                WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::from_nanos(-1),
                    action: WatchdogAction::Restart,
                },
                WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::from_millis(1),
                    action: WatchdogAction::Restart,
                },
            ),
            (
                WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::ZERO,
                    action: WatchdogAction::Halt,
                },
                WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::from_millis(1),
                    action: WatchdogAction::Halt,
                },
            ),
            (
                WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::from_millis(7),
                    action: WatchdogAction::SafeHalt,
                },
                WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::from_millis(7),
                    action: WatchdogAction::SafeHalt,
                },
            ),
        ] {
            assert_eq!(input.normalized_for_runtime(), expected);

            let mut watchdog = WatchdogSubsystem::new();
            watchdog.set_policy(input);
            assert_eq!(watchdog.policy(), expected);
            assert_eq!(
                watchdog.decision(),
                FaultDecision::from_watchdog(expected.action)
            );
        }
    }

    #[test]
    fn fault_record_and_policy_updates_preserve_independent_state() {
        let mut faults = FaultSubsystem::new();
        faults.set_policy(FaultPolicy::SafeHalt);
        faults.record(RuntimeError::WatchdogTimeout);
        assert_eq!(faults.policy(), FaultPolicy::SafeHalt);
        assert!(faults.is_faulted());
        assert_eq!(faults.last_fault(), Some(&RuntimeError::WatchdogTimeout));

        faults.set_policy(FaultPolicy::Restart);
        assert_eq!(faults.policy(), FaultPolicy::Restart);
        assert!(faults.is_faulted());
        assert_eq!(faults.last_fault(), Some(&RuntimeError::WatchdogTimeout));

        faults.record(RuntimeError::DivisionByZero);
        assert_eq!(faults.policy(), FaultPolicy::Restart);
        assert!(faults.is_faulted());
        assert_eq!(faults.last_fault(), Some(&RuntimeError::DivisionByZero));
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
        assert_eq!(faults.policy(), FaultPolicy::SafeHalt);
        faults.clear();
        assert!(!faults.is_faulted());
        assert!(faults.last_fault().is_none());
        assert_eq!(faults.policy(), FaultPolicy::SafeHalt);
    }
}
