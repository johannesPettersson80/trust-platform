//! What an alarm offers on the display (specification 39, §5.7 item 4 and §5.9 item 2;
//! specification 40, §3 and §4): the actions its state allows a role, and how it sounds. The
//! display offers exactly these actions, and the runtime refuses an action outside them.

use serde::Serialize;

use super::status::{AlarmStatus, Priority};
use crate::security::AccessRole;

/// The shelving durations an operator chooses from: 15 min, 1 h, 4 h, 8 h and 24 h.
pub const SHELVE_FOR_SECONDS: [u32; 5] = [900, 3_600, 14_400, 28_800, 86_400];

/// The longest comment: the length of the instance's `Request.Comment` field.
pub const COMMENT_MAX_CHARS: usize = 120;

/// The list an alarm is in (specification 39, §5.7 item 4). An alarm in none of them is normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlarmList {
    /// Annunciating.
    Active,
    /// Shelved by an operator.
    Shelved,
    /// Suppressed by design.
    Suppressed,
    /// Taken out of service by an engineer.
    OutOfService,
}

/// The list the alarm is in: out of service before shelved before suppressed before active.
#[must_use]
pub fn list_of(status: &AlarmStatus) -> Option<AlarmList> {
    if status.out_of_service {
        Some(AlarmList::OutOfService)
    } else if status.shelved {
        Some(AlarmList::Shelved)
    } else if status.suppressed {
        Some(AlarmList::Suppressed)
    } else if status.annunciating {
        Some(AlarmList::Active)
    } else {
        None
    }
}

/// How an alarm sounds on the display (EEMUA 191 practice). Silence is the display's own: it
/// stops the sound without acknowledging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlarmSound {
    /// Until it is acknowledged or silenced: an unacknowledged High alarm.
    Continuous,
    /// Once for each raise: an unacknowledged Medium alarm.
    Once,
    /// Not at all: a Low alarm, and any alarm that is acknowledged or does not annunciate.
    #[default]
    None,
}

/// How the alarm sounds.
#[must_use]
pub fn sound(status: &AlarmStatus) -> AlarmSound {
    if list_of(status) != Some(AlarmList::Active) || !status.unacknowledged {
        return AlarmSound::None;
    }
    match status.priority {
        Priority::High => AlarmSound::Continuous,
        Priority::Medium => AlarmSound::Once,
        Priority::Low => AlarmSound::None,
    }
}

/// The actions the alarm's state allows `role`, in the order the display offers them.
///
/// Without a session the Operator's actions are offered, and choosing one asks to log in, as
/// controls do (specification 39, §5.7 item 4). Taking out of service and returning to service
/// need an Engineer.
#[must_use]
pub fn offered_actions(status: &AlarmStatus, role: Option<AccessRole>) -> Vec<&'static str> {
    let list = list_of(status);
    let engineer = role.is_some_and(|role| role >= AccessRole::Engineer);
    let mut actions = Vec::new();
    if status.unacknowledged && list != Some(AlarmList::Suppressed) {
        actions.push("acknowledge");
    }
    if list == Some(AlarmList::Active) && status.latching && status.active {
        actions.push("reset");
    }
    if list == Some(AlarmList::Shelved) {
        actions.push("unshelve");
    }
    if engineer && list == Some(AlarmList::OutOfService) {
        actions.push("return-to-service");
    }
    if engineer && matches!(list, Some(AlarmList::Active | AlarmList::Shelved)) {
        actions.push("remove-from-service");
    }
    if list == Some(AlarmList::Active) {
        actions.push("shelve");
    }
    actions.push("comment");
    actions
}

/// Whether the alarm's state allows `role` the action: the runtime's own check before it writes.
#[must_use]
pub fn allows(status: &AlarmStatus, role: AccessRole, action: &str) -> bool {
    offered_actions(status, Some(role)).contains(&action)
}
