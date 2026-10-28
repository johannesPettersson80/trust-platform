//! Settings at one call between scans (specification 39, §21.4 item 4): the members' values, and
//! changes written all together, or none when a member no longer holds the value it held when the
//! changes were made, or when one change cannot be written.

use std::sync::mpsc::Sender;

use crate::hmi::commands::write::{write_points, WriteAction};
use crate::hmi::member_paths::{member_value, MemberPath};
use crate::runtime::program_access::ProgramWrite;
use crate::value::Value;

/// One change: the member, the value it must still hold, and what is written.
pub(crate) struct SettingChange {
    pub path: MemberPath,
    pub unchanged: Value,
    pub write: WriteAction,
}

/// What a settings job asks: changes, all or none, then the members' values.
pub(crate) struct SettingsJob {
    /// The members read after the changes, in order.
    pub read: Vec<MemberPath>,
    /// The changes; none for a read.
    pub changes: Vec<SettingChange>,
}

/// What a settings job gives.
#[derive(Debug)]
pub(crate) enum SettingsOutcome {
    /// The members' values after the changes, in the order read; `None` where one is unknown.
    Values(Vec<Option<Value>>),
    /// This member changed meanwhile; nothing was written.
    Changed(MemberPath),
    /// A change could not be written; nothing was.
    Refused(String),
}

/// A job waiting for the next call.
pub(super) struct PendingSettings {
    pub(super) id: u64,
    pub(super) job: SettingsJob,
    pub(super) respond_to: Sender<SettingsOutcome>,
}

/// Runs a settings job on the runtime thread, at a call of `Hmi.Display`.
pub(super) fn run(program: &mut dyn ProgramWrite, job: &SettingsJob) -> SettingsOutcome {
    for change in &job.changes {
        if member_value(&change.path, &*program).as_ref() != Some(&change.unchanged) {
            return SettingsOutcome::Changed(change.path.clone());
        }
    }
    if !job.changes.is_empty() {
        let writes: Vec<(MemberPath, WriteAction)> = job
            .changes
            .iter()
            .map(|change| (change.path.clone(), change.write.clone()))
            .collect();
        if let Err(refused) = write_points(program, &writes) {
            return SettingsOutcome::Refused(refused.reason);
        }
    }
    SettingsOutcome::Values(
        job.read
            .iter()
            .map(|path| member_value(path, &*program))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PROGRAM: &str = r#"
FUNCTION_BLOCK Pump
VAR PUBLIC RETAIN
    StartTimeout : TIME := T#5s;
    ServiceInterval : LREAL := 4000.0;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    P101 : Pump;
END_VAR
    P101();
END_PROGRAM
"#;

    fn path(text: &str) -> MemberPath {
        MemberPath::parse(text).expect("a path")
    }

    fn change(member: &str, unchanged: Value, requested: serde_json::Value) -> SettingChange {
        SettingChange {
            path: path(member),
            unchanged,
            write: WriteAction::Set(requested),
        }
    }

    #[test]
    fn changes_are_written_together_and_read_back() {
        let mut harness = crate::harness::TestHarness::from_source(PROGRAM).expect("compiles");
        harness.cycle();
        let job = SettingsJob {
            read: vec![
                path("Main.P101.StartTimeout"),
                path("Main.P101.ServiceInterval"),
            ],
            changes: vec![
                change(
                    "Main.P101.StartTimeout",
                    Value::Time(crate::value::Duration::from_secs(5)),
                    json!(10.0),
                ),
                change(
                    "Main.P101.ServiceInterval",
                    Value::LReal(4000.0),
                    json!(2000.0),
                ),
            ],
        };
        match harness
            .runtime_mut()
            .with_program(|program| run(program, &job))
        {
            SettingsOutcome::Values(values) => assert_eq!(
                values,
                vec![
                    Some(Value::Time(crate::value::Duration::from_secs(10))),
                    Some(Value::LReal(2000.0))
                ]
            ),
            other => panic!("written: {other:?}"),
        }
    }

    #[test]
    fn a_member_changed_meanwhile_or_a_value_that_does_not_fit_writes_nothing() {
        let mut harness = crate::harness::TestHarness::from_source(PROGRAM).expect("compiles");
        harness.cycle();
        let stale = SettingsJob {
            read: Vec::new(),
            changes: vec![
                change(
                    "Main.P101.ServiceInterval",
                    Value::LReal(4000.0),
                    json!(2000.0),
                ),
                change(
                    "Main.P101.StartTimeout",
                    Value::Time(crate::value::Duration::from_secs(7)),
                    json!(10.0),
                ),
            ],
        };
        assert!(matches!(
            harness.runtime_mut().with_program(|program| run(program, &stale)),
            SettingsOutcome::Changed(member) if member == path("Main.P101.StartTimeout")
        ));
        let unfit = SettingsJob {
            read: Vec::new(),
            changes: vec![
                change(
                    "Main.P101.ServiceInterval",
                    Value::LReal(4000.0),
                    json!(2000.0),
                ),
                change(
                    "Main.P101.StartTimeout",
                    Value::Time(crate::value::Duration::from_secs(5)),
                    json!("soon"),
                ),
            ],
        };
        assert!(matches!(
            harness
                .runtime_mut()
                .with_program(|program| run(program, &unfit)),
            SettingsOutcome::Refused(_)
        ));
        assert_eq!(
            harness.runtime().with_program_view(|program| member_value(
                &path("Main.P101.ServiceInterval"),
                program
            )),
            Some(Value::LReal(4000.0)),
            "nothing was written"
        );
    }
}
