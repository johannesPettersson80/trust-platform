//! The messages a program gives its alarms (specification 39, §14.5 items 4 and 8): texts of the
//! project, which the editor lists for translation like the texts of its pages.

use serde::Serialize;

use super::status::read_alarms;
use crate::runtime::Runtime;

/// An alarm and the message the program gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlarmMessage {
    /// The instance path, the alarm's identity.
    pub path: String,
    /// What the operator reads.
    pub message: String,
}

/// The messages the program gives its alarms in its first scan, as the running program sets them;
/// alarms without one are left out. The scan runs within the runtime's instruction budget, so a
/// program that runs away stops there and keeps the messages it gave before.
pub fn alarm_messages(runtime: &mut Runtime) -> Vec<AlarmMessage> {
    // A newly compiled configured project is at time zero, before any periodic task is due.
    // Advance only this preview runtime to cover one invocation of every periodic task;
    // executing at zero silently omitted all alarm messages in configured projects.
    if let Some(interval) = runtime
        .tasks()
        .iter()
        .map(|task| task.interval)
        .filter(|interval| interval.as_nanos() > 0)
        .max_by_key(|interval| interval.as_nanos())
    {
        runtime.advance_time(interval);
    }
    // A scan cut short records a fault; the messages already given stay readable.
    let _ = runtime.execute_cycle();
    read_alarms(runtime)
        .into_iter()
        .filter(|alarm| !alarm.message.trim().is_empty())
        .map(|alarm| AlarmMessage {
            path: alarm.path,
            message: alarm.message,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;

    const ALARM: &str = include_str!("../../../../../libraries/alarm/src/alarm.st");

    const PROGRAM: &str = r#"
USING Alarm;

FUNCTION_BLOCK Pump
VAR_INPUT
    Start : BOOL;
    Running : BOOL;
END_VAR
VAR
    FailedToStart : DiscrepancyAlarm;
END_VAR
    FailedToStart(Command := Start, Feedback := Running, Message := 'Pump failed to start', Priority := AlarmPriority#High);
END_FUNCTION_BLOCK

PROGRAM Tank
VAR
    Level : LREAL := 50.0;
    High : LimitAlarm;
    Quiet : DigitalAlarm;
    P1 : Pump;
END_VAR
    High(Value := Level, Limit := 90.0, Message := 'Tank level high', Priority := AlarmPriority#Medium);
    Quiet(Condition := FALSE);
    P1();
END_PROGRAM
"#;

    fn message(path: &str, message: &str) -> AlarmMessage {
        AlarmMessage {
            path: path.to_string(),
            message: message.to_string(),
        }
    }

    #[test]
    fn the_messages_are_those_the_program_gives_its_alarms_in_its_first_scan() {
        let mut harness = TestHarness::from_sources(&[ALARM, PROGRAM]).expect("compiles");
        assert_eq!(
            alarm_messages(harness.runtime_mut()),
            [
                message("Tank.High", "Tank level high"),
                message("Tank.P1.FailedToStart", "Pump failed to start"),
            ],
            "an alarm without a message gives none"
        );
    }

    #[test]
    fn a_first_scan_that_runs_away_stops_at_the_budget_and_keeps_the_messages_given_before() {
        let looping = r#"
USING Alarm;

PROGRAM Runaway
VAR
    Level : LREAL;
    High : LimitAlarm;
    Count : DINT;
END_VAR
    High(Value := Level, Limit := 90.0, Message := 'Level high');
    WHILE TRUE DO
        Count := Count + 1;
    END_WHILE;
END_PROGRAM
"#;
        let mut harness = TestHarness::from_sources(&[ALARM, looping]).expect("compiles");
        assert_eq!(
            alarm_messages(harness.runtime_mut()),
            [message("Runaway.High", "Level high")]
        );
    }
}
