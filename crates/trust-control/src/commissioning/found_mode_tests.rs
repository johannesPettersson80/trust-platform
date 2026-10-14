// The noise allowance and the mode values are exact numbers of the fixture.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::entry_allowance::auto_entry_tolerance;
use crate::commissioning::fixture_loops::{parallel_pid_loop, proportional_loop, requirements};
use crate::commissioning::transfer::{pid_transfer_binding, proportional_transfer_binding};
use crate::commissioning::transfer_tracking::{
    parallel_pid_tracking, proportional_tracking, ParallelPidKind,
};
use std::collections::BTreeMap;

fn gains(kp: f64, ki: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Parameters.Kp".to_owned(), kp),
        ("Main.Parameters.Ki".to_owned(), ki),
        ("Main.Parameters.Kd".to_owned(), kd),
    ])
}

/// The current reading of a loop whose mode the restore has left at `mode`.
fn reading(symbol: &str, mode: bool) -> BTreeMap<String, TransferValue> {
    BTreeMap::from([(symbol.to_owned(), TransferValue::Boolean(mode))])
}

#[test]
fn restoring_a_control_pid_found_in_auto_returns_it_to_auto_with_the_noise_allowance() {
    // V4.6: the mode reads Auto (true, the Manual value is false) and the restore has put the loop back in Manual. The re-entry
    // carries the noise-aware tolerance of A6.4 (2·Kp·n with Kp 1 and n 0.05) and the Control.PID tracking.
    let loop_definition = parallel_pid_loop();
    let binding = pid_transfer_binding(&loop_definition).expect("the fixture binds");
    let task = requirements();
    let found = found_mode_of(Some(true), binding.mode.manual_value);
    assert_eq!(found, Some(FoundMode::Auto));

    let noise = 0.05;
    let current = reading(&binding.mode.symbol, false);
    let reentry = reenter_found_mode(found, &binding.mode, &current, |_| {
        let tolerance = auto_entry_tolerance(
            &loop_definition,
            &task,
            &gains(1.0, 0.0, 0.0),
            Some(noise),
            Some(0.0),
        )
        .ok()?;
        let tracking = parallel_pid_tracking(&loop_definition, &binding, true).ok()?;
        Some(AutoReentry {
            values: BTreeMap::from([
                ("Main.Integral".to_owned(), 0.0),
                ("Main.PreviousError".to_owned(), 0.0),
            ]),
            tolerance,
            tracking: TransferTracking::ParallelPid(tracking),
        })
    })
    .expect("a loop found in Auto re-enters Auto");

    assert!(
        (reentry.tolerance - 2.0 * 1.0 * noise).abs() < 1e-12,
        "A6.4: 2·Kp·n = {}",
        reentry.tolerance
    );
    match &reentry.tracking {
        TransferTracking::ParallelPid(tracking) => {
            assert_eq!(
                tracking.kind,
                ParallelPidKind::ParallelPidV1,
                "the Control.PID tracking"
            );
            assert!(
                tracking.align_output,
                "the entry starts from the current output"
            );
        }
        other => panic!("the Control.PID tracking, not {other:?}"),
    }
    assert_eq!(reentry.values.len(), 2);
}

#[test]
fn a_proportional_loop_found_in_auto_returns_to_auto_with_its_proportional_tracking() {
    let loop_definition = proportional_loop();
    let binding = proportional_transfer_binding(&loop_definition).expect("the fixture binds");
    let found = found_mode_of(Some(true), binding.mode.manual_value);
    let current = reading(&binding.mode.symbol, false);
    let reentry = reenter_found_mode(found, &binding.mode, &current, |_| {
        Some(AutoReentry {
            values: BTreeMap::new(),
            tolerance: 0.04,
            tracking: TransferTracking::Proportional(
                proportional_tracking(&loop_definition, &binding, false).ok()?,
            ),
        })
    })
    .expect("re-enters Auto");
    assert!(matches!(
        reentry.tracking,
        TransferTracking::Proportional(_)
    ));
}

#[test]
fn a_loop_found_in_manual_or_of_unknown_mode_stays_in_manual_and_builds_no_entry() {
    let binding = pid_transfer_binding(&parallel_pid_loop()).expect("the fixture binds");
    let manual_now = reading(&binding.mode.symbol, false);
    let no_entry = |_: &BTreeMap<String, TransferValue>| -> Option<AutoReentry> {
        panic!("a loop found in Manual builds no Auto entry")
    };
    assert_eq!(
        reenter_found_mode(
            Some(FoundMode::Manual),
            &binding.mode,
            &manual_now,
            no_entry
        ),
        None
    );
    assert_eq!(
        reenter_found_mode(None, &binding.mode, &manual_now, no_entry),
        None,
        "an unknown mode stays in Manual"
    );
}

#[test]
fn a_loop_that_already_reads_auto_or_whose_entry_cannot_be_built_stays_where_it_is() {
    let binding = pid_transfer_binding(&parallel_pid_loop()).expect("the fixture binds");
    let in_auto = reading(&binding.mode.symbol, true);
    let never = |_: &BTreeMap<String, TransferValue>| -> Option<AutoReentry> {
        panic!("a loop not in Manual builds no entry")
    };
    assert_eq!(
        reenter_found_mode(Some(FoundMode::Auto), &binding.mode, &in_auto, never),
        None
    );

    let in_manual = reading(&binding.mode.symbol, false);
    assert_eq!(
        reenter_found_mode(Some(FoundMode::Auto), &binding.mode, &in_manual, |_| None),
        None,
        "an entry that cannot be built leaves the loop in Manual"
    );
    let silent = BTreeMap::new();
    assert_eq!(
        reenter_found_mode(Some(FoundMode::Auto), &binding.mode, &silent, never),
        None,
        "an unread mode is not Manual"
    );
}

#[test]
fn a_found_mode_is_read_from_the_review_and_maps_back_to_the_mode_value() {
    assert_eq!(
        found_mode_of(Some(false), false),
        Some(FoundMode::Manual),
        "the mode reads its Manual value"
    );
    assert_eq!(found_mode_of(Some(true), false), Some(FoundMode::Auto));
    assert_eq!(
        found_mode_of(None, false),
        None,
        "an unread mode claims no return"
    );

    assert_eq!(
        reviewed_mode_of(Some(FoundMode::Manual), false),
        Some(false)
    );
    assert_eq!(reviewed_mode_of(Some(FoundMode::Auto), false), Some(true));
    assert_eq!(reviewed_mode_of(None, false), None);
    assert_eq!(
        reviewed_mode_of(Some(FoundMode::Auto), true),
        Some(false),
        "the Manual value is the other way round"
    );
}
