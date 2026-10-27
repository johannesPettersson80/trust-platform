//! Finding the alarms of the running program (specification 40, §5, items 1 and 5): every
//! instance of a block that is `Alarm.Alarm` or extends it, at any depth, so customer alarms
//! appear without registration.

use crate::hmi::program_instances::{declares_block, instances_of};
use crate::runtime::program_access::ProgramRead;

pub(super) use crate::hmi::program_instances::Found;

/// The alarm base's type name.
pub(super) const ALARM_BASE: &str = "Alarm.Alarm";
/// Most alarms listed.
const MAX_ALARMS: usize = 10_000;

/// The instance path of every alarm of the running program, in declaration order, each once:
/// where it is declared, not again where an interface or reference refers to it.
pub fn alarm_paths(program: &dyn ProgramRead) -> Vec<String> {
    alarm_instances(program)
        .into_iter()
        .map(|found| found.path)
        .collect()
}

/// Every alarm in declaration order, with its equipment: the function block instance nearest
/// above it, never a program. A program without the alarm library is not searched. An alarm's
/// own members are not searched: an alarm does not contain alarms.
pub(super) fn alarm_instances(program: &dyn ProgramRead) -> Vec<Found> {
    if !declares_block(program, ALARM_BASE) {
        return Vec::new();
    }
    instances_of(program, ALARM_BASE, MAX_ALARMS)
}
