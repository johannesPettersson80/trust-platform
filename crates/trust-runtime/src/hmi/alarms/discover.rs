//! Finding the alarms of the running program (specification 40, §5, items 1 and 5): every
//! instance of a block that is `Alarm.Alarm` or extends it, at any depth, so customer alarms
//! appear without registration.

use crate::hmi::instance_holdings::Holdings;
use crate::hmi::instance_walk::{base_part, instances_of};
use crate::memory::{InstanceData, InstanceId, VariableStorage};

pub(super) use crate::hmi::instance_walk::Found;

/// The alarm base's type name.
pub(super) const ALARM_BASE: &str = "Alarm.Alarm";
/// Most alarms listed.
const MAX_ALARMS: usize = 10_000;

/// The instance path of every alarm of the running program, in declaration order, each once:
/// where it is declared, not again where an interface or reference refers to it.
pub fn alarm_paths(runtime: &crate::Runtime) -> Vec<String> {
    let holdings = holdings_of(runtime);
    alarm_instances(runtime.storage(), &holdings, &|_| false)
        .into_iter()
        .map(|found| found.path)
        .collect()
}

/// The running program's declarations, for the walk.
pub(super) fn holdings_of(runtime: &crate::Runtime) -> Holdings<'_> {
    Holdings::new(
        runtime.function_blocks(),
        runtime.classes(),
        runtime.programs(),
        runtime.registry(),
    )
}

/// Every alarm in declaration order, with its equipment; `is_program` tells program instances,
/// which are not equipment, from function block instances. An alarm's own members are not
/// searched: an alarm does not contain alarms.
pub(super) fn alarm_instances(
    storage: &VariableStorage,
    holdings: &Holdings<'_>,
    is_program: &dyn Fn(&str) -> bool,
) -> Vec<Found> {
    instances_of(storage, holdings, ALARM_BASE, MAX_ALARMS, is_program)
}

/// The alarm base part of an instance whose type is, or extends, the alarm base.
pub(super) fn alarm_base(storage: &VariableStorage, id: InstanceId) -> Option<&InstanceData> {
    base_part(storage, id, ALARM_BASE)
}
