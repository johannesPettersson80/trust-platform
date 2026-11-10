//! Hosted hang supervision's tests (specification 11 section 6.6, "Hosted hang supervision").

use super::*;

fn supervision() -> ScanSupervision {
    ScanSupervision::new(
        Arc::new(Mutex::new(ResourceState::Running)),
        Arc::new(Mutex::new(None)),
    )
}

/// The hold view of a resource without a debugger: no hold, none ended.
fn no_hold() -> (bool, StdDuration) {
    (false, StdDuration::ZERO)
}

#[test]
fn overdue_published_cycle_is_declared_hung_and_faulted() {
    let supervision = supervision();
    let start = Instant::now();
    std::thread::sleep(StdDuration::from_millis(20));
    supervision.publish(start, Duration::from_millis(1), StdDuration::ZERO);

    let mut declared = None;
    assert!(supervision.declare_hang_if_overdue(no_hold, &mut declared));
    assert!(supervision.is_hung());
    assert_eq!(
        *recover_mutex_lock(supervision.state.lock()),
        ResourceState::Faulted
    );
    assert_eq!(
        *recover_mutex_lock(supervision.last_error.lock()),
        Some(RuntimeError::WatchdogTimeout)
    );
}

#[test]
fn one_stalled_cycle_is_declared_once() {
    let supervision = supervision();
    let start = Instant::now();
    std::thread::sleep(StdDuration::from_millis(20));
    supervision.publish(start, Duration::from_millis(1), StdDuration::ZERO);

    let mut declared = None;
    assert!(supervision.declare_hang_if_overdue(no_hold, &mut declared));
    assert!(!supervision.declare_hang_if_overdue(no_hold, &mut declared));
}

#[test]
fn cycle_inside_its_budget_is_not_declared() {
    let supervision = supervision();
    supervision.publish(
        Instant::now(),
        Duration::from_millis(10_000),
        StdDuration::ZERO,
    );

    let mut declared = None;
    assert!(!supervision.declare_hang_if_overdue(no_hold, &mut declared));
    assert!(!supervision.is_hung());
}

#[test]
fn withdrawn_cycle_is_never_declared_and_clears_hung() {
    let supervision = supervision();
    let start = Instant::now();
    std::thread::sleep(StdDuration::from_millis(20));
    supervision.publish(start, Duration::from_millis(1), StdDuration::ZERO);
    let mut declared = None;
    assert!(supervision.declare_hang_if_overdue(no_hold, &mut declared));

    supervision.withdraw();

    assert!(!supervision.is_hung(), "a returned call is no longer hung");
    let mut later = None;
    assert!(!supervision.declare_hang_if_overdue(no_hold, &mut later));
}

/// Wrong behaviour caught: the check reads the hold view before the clock, so a hold that
/// begins while the supervisor waits between the two readings counts as active time, and a
/// debugger pause faults the resource. A preempted supervisor waits so; on a loaded Windows
/// runner the supervisor faulted a case of `DEBUG_PAUSE_001`, a 100 ms pause against a 20 ms
/// timeout.
#[test]
fn a_hold_that_begins_during_a_check_never_counts_as_active_time() {
    let supervision = supervision();
    supervision.publish(Instant::now(), Duration::from_millis(10), StdDuration::ZERO);
    // The view as read just before the scan thread's hold began: no hold, none ended. The
    // supervisor then waits 50 ms, five timeouts, while the hold lasts.
    let view_read_before_the_hold = || {
        let view = no_hold();
        std::thread::sleep(StdDuration::from_millis(50));
        view
    };

    let mut declared = None;
    assert!(
        !supervision.declare_hang_if_overdue(view_read_before_the_hold, &mut declared),
        "the hold's time is not active time"
    );
    assert!(!supervision.is_hung());
    assert_eq!(
        *recover_mutex_lock(supervision.state.lock()),
        ResourceState::Running
    );
}

#[test]
fn check_interval_follows_quarter_of_timeout_within_bounds() {
    assert_eq!(
        check_interval_for(100_000_000),
        StdDuration::from_millis(25)
    );
    assert_eq!(check_interval_for(1), StdDuration::from_millis(1));
    assert_eq!(
        check_interval_for(60_000_000_000),
        StdDuration::from_millis(100)
    );
}
