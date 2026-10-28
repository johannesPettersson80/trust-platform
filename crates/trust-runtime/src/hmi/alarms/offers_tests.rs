//! Specification 39, §5.7 item 4 and §5.9 item 2; specification 40, §3 and §4: the actions an
//! alarm's state allows each role, and how it sounds.

use super::board::{AlarmBoard, AlarmSummary};
use super::offers::{
    allows, list_of, offered_actions, sound, AlarmList, AlarmSound, SHELVE_FOR_SECONDS,
};
use super::status::{AlarmStatus, Priority};
use crate::security::AccessRole;

/// An alarm in a list: annunciating unless it is shelved, suppressed or out of service.
fn alarm(list: AlarmList, unacknowledged: bool) -> AlarmStatus {
    AlarmStatus {
        path: "Tank.High".into(),
        message: "Tank level high".into(),
        priority: Priority::High,
        active: true,
        unacknowledged,
        shelved: list == AlarmList::Shelved,
        suppressed: list == AlarmList::Suppressed,
        out_of_service: list == AlarmList::OutOfService,
        annunciating: list == AlarmList::Active,
        ..AlarmStatus::default()
    }
}

#[test]
fn an_alarm_is_in_one_list_out_of_service_before_shelved_before_suppressed_before_active() {
    assert_eq!(
        list_of(&alarm(AlarmList::Active, true)),
        Some(AlarmList::Active)
    );
    let everything = AlarmStatus {
        shelved: true,
        suppressed: true,
        out_of_service: true,
        annunciating: true,
        ..AlarmStatus::default()
    };
    assert_eq!(list_of(&everything), Some(AlarmList::OutOfService));
    assert_eq!(
        list_of(&AlarmStatus {
            shelved: true,
            suppressed: true,
            ..AlarmStatus::default()
        }),
        Some(AlarmList::Shelved)
    );
    assert_eq!(
        list_of(&AlarmStatus {
            suppressed: true,
            annunciating: true,
            ..AlarmStatus::default()
        }),
        Some(AlarmList::Suppressed)
    );
    assert_eq!(
        list_of(&AlarmStatus::default()),
        None,
        "a normal alarm is in no list"
    );
}

#[test]
fn an_active_unacknowledged_alarm_offers_acknowledge_shelve_and_comment_and_an_engineer_also_out_of_service(
) {
    let status = alarm(AlarmList::Active, true);
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Operator)),
        ["acknowledge", "shelve", "comment"]
    );
    assert_eq!(
        offered_actions(&status, None),
        ["acknowledge", "shelve", "comment"],
        "without a session the Operator's actions are offered, and choosing one asks to log in"
    );
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Viewer)),
        ["acknowledge", "shelve", "comment"]
    );
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Engineer)),
        ["acknowledge", "remove-from-service", "shelve", "comment"]
    );
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Admin)),
        ["acknowledge", "remove-from-service", "shelve", "comment"]
    );
}

#[test]
fn an_acknowledged_alarm_offers_no_acknowledge() {
    let status = alarm(AlarmList::Active, false);
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Operator)),
        ["shelve", "comment"]
    );
    assert!(!allows(&status, AccessRole::Operator, "acknowledge"));
}

#[test]
fn a_latching_alarm_that_is_active_offers_reset_on_the_active_list_only() {
    let latched = AlarmStatus {
        latching: true,
        ..alarm(AlarmList::Active, false)
    };
    assert_eq!(
        offered_actions(&latched, Some(AccessRole::Operator)),
        ["reset", "shelve", "comment"]
    );
    let inactive = AlarmStatus {
        active: false,
        ..latched.clone()
    };
    assert!(!offered_actions(&inactive, Some(AccessRole::Operator)).contains(&"reset"));
    let shelved = AlarmStatus {
        latching: true,
        ..alarm(AlarmList::Shelved, false)
    };
    assert!(!offered_actions(&shelved, Some(AccessRole::Operator)).contains(&"reset"));
}

#[test]
fn a_shelved_alarm_offers_unshelve_and_an_engineer_also_out_of_service() {
    let status = alarm(AlarmList::Shelved, false);
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Operator)),
        ["unshelve", "comment"]
    );
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Engineer)),
        ["unshelve", "remove-from-service", "comment"]
    );
    assert!(
        !allows(&status, AccessRole::Operator, "shelve"),
        "only an active alarm is shelved"
    );
}

#[test]
fn a_suppressed_alarm_offers_only_a_comment_even_when_unacknowledged() {
    let status = alarm(AlarmList::Suppressed, true);
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Engineer)),
        ["comment"]
    );
}

#[test]
fn an_alarm_out_of_service_offers_return_to_service_to_an_engineer_only() {
    let status = alarm(AlarmList::OutOfService, false);
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Operator)),
        ["comment"]
    );
    assert_eq!(
        offered_actions(&status, Some(AccessRole::Engineer)),
        ["return-to-service", "comment"]
    );
    let unacknowledged = alarm(AlarmList::OutOfService, true);
    assert_eq!(
        offered_actions(&unacknowledged, Some(AccessRole::Operator)),
        ["acknowledge", "comment"]
    );
}

#[test]
fn the_runtime_allows_exactly_what_the_display_offers() {
    let lists = [
        AlarmList::Active,
        AlarmList::Shelved,
        AlarmList::Suppressed,
        AlarmList::OutOfService,
    ];
    let roles = [
        AccessRole::Viewer,
        AccessRole::Operator,
        AccessRole::Engineer,
        AccessRole::Admin,
    ];
    let every = [
        "acknowledge",
        "shelve",
        "unshelve",
        "reset",
        "comment",
        "remove-from-service",
        "return-to-service",
    ];
    for list in lists {
        for unacknowledged in [false, true] {
            for latching in [false, true] {
                let status = AlarmStatus {
                    latching,
                    ..alarm(list, unacknowledged)
                };
                for role in roles {
                    let offered = offered_actions(&status, Some(role));
                    for action in every {
                        assert_eq!(allows(&status, role, action), offered.contains(&action),
                            "{list:?} unacknowledged {unacknowledged} latching {latching} {role:?} {action}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_high_alarm_sounds_until_acknowledged_a_medium_alarm_once_and_a_low_alarm_never() {
    let with = |priority, unacknowledged| AlarmStatus {
        priority,
        ..alarm(AlarmList::Active, unacknowledged)
    };
    assert_eq!(sound(&with(Priority::High, true)), AlarmSound::Continuous);
    assert_eq!(sound(&with(Priority::Medium, true)), AlarmSound::Once);
    assert_eq!(sound(&with(Priority::Low, true)), AlarmSound::None);
    for priority in [Priority::High, Priority::Medium, Priority::Low] {
        assert_eq!(
            sound(&with(priority, false)),
            AlarmSound::None,
            "an acknowledged alarm is quiet"
        );
    }
}

#[test]
fn an_alarm_that_does_not_annunciate_is_quiet() {
    for list in [
        AlarmList::Shelved,
        AlarmList::Suppressed,
        AlarmList::OutOfService,
    ] {
        assert_eq!(sound(&alarm(list, true)), AlarmSound::None, "{list:?}");
    }
}

#[test]
fn the_list_carries_the_shelving_durations_and_each_session_gets_its_own_actions() {
    let mut board = AlarmBoard::default();
    board.update(vec![alarm(AlarmList::Active, true)], 1_000);
    let summary: AlarmSummary = board.summary();
    assert_eq!(summary.shelve_for, SHELVE_FOR_SECONDS);
    assert_eq!(summary.comment_max_chars, 120);
    assert_eq!(summary.active[0].sound, AlarmSound::Continuous);
    assert!(
        summary.active[0].actions.is_empty(),
        "actions are filled for a session"
    );
    let operator = summary.clone().for_role(Some(AccessRole::Operator));
    assert_eq!(
        operator.active[0].actions,
        ["acknowledge", "shelve", "comment"]
    );
    assert_eq!(
        operator.newest.as_ref().map(|row| row.actions.clone()),
        Some(operator.active[0].actions.clone())
    );
    let engineer = summary.for_role(Some(AccessRole::Engineer));
    assert_eq!(
        engineer.active[0].actions,
        ["acknowledge", "remove-from-service", "shelve", "comment"]
    );
}
