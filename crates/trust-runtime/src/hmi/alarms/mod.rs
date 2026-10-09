//! The runtime's alarm service (specification 40, §5): every instance of a block that extends
//! `Alarm.Alarm`, its state, the transitions between reads as timestamped events, and the alarm
//! list the display shows.

mod actions;
mod board;
mod discover;
mod messages;
mod runtime_service;
mod status;

pub use actions::{alarm_action, AlarmAction};
pub use board::{AlarmBoard, AlarmEvent, AlarmRow, AlarmSummary, EventKind};
pub use discover::alarm_paths;
pub use messages::{alarm_messages, AlarmMessage};
pub use runtime_service::{AlarmSnapshot, OperatorAction, RuntimeAlarms};
pub use status::{read_alarms, AlarmStatus, Priority, Raised};

#[cfg(test)]
mod actions_tests;
#[cfg(test)]
mod tests;
