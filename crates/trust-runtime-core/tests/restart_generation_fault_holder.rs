use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::watchdog::FaultSubsystem;

/// Spec 11 §6.6 fault-holder API: recording a runtime fault sets the faulted observation and stores the exact last fault; a later record replaces the earlier last fault.
#[test]
fn later_recorded_fault_replaces_the_earlier_last_fault() {
    let mut faults = FaultSubsystem::new();
    assert!(!faults.is_faulted());
    assert_eq!(faults.last_fault(), None);

    faults.record(RuntimeError::WatchdogTimeout);
    assert!(faults.is_faulted());
    assert_eq!(faults.last_fault(), Some(&RuntimeError::WatchdogTimeout));

    faults.record(RuntimeError::Overflow);
    assert!(faults.is_faulted());
    assert_eq!(faults.last_fault(), Some(&RuntimeError::Overflow));
}
