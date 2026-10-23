//! Runtime mesh helpers (snapshot/apply).

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::hmi::member_paths::{resolve_member, MemberPath};
use crate::value::Value;

use super::Runtime;

impl Runtime {
    /// Snapshot explicitly published globals and nested program/equipment paths.
    /// Globals retain their canonical spelling; nested keys retain the configured path.
    pub fn snapshot_globals(&self, names: &[SmolStr]) -> IndexMap<SmolStr, Value> {
        let mut out = IndexMap::new();
        for name in names {
            if let Some((canonical, value)) = self
                .storage()
                .globals()
                .iter()
                .find(|(canonical, _)| canonical.eq_ignore_ascii_case(name.as_str()))
            {
                out.insert(canonical.clone(), value.clone());
            } else if let Some(value) = MemberPath::parse(name)
                .ok()
                .and_then(|path| resolve_member(&path, self.storage()))
            {
                out.insert(name.clone(), value.clone());
            }
        }
        out
    }

    /// Apply declared mesh globals through common checked value policy (unknown
    /// names remain skipped). Invalid requests do not become PLC scan faults.
    pub fn apply_mesh_updates(&mut self, updates: &IndexMap<SmolStr, Value>) {
        for (name, value) in updates {
            let canonical = self
                .storage()
                .globals()
                .keys()
                .find(|canonical| canonical.eq_ignore_ascii_case(name.as_str()))
                .cloned();
            if let Some(canonical) = canonical
                && let Err(error) = self.write_global(&canonical, value.clone())
            {
                tracing::warn!(target = "trust_runtime", variable = %canonical, %error, "mesh write rejected");
                if let Some(debug) = self.debug_control() {
                    debug.push_runtime_event(crate::debug::RuntimeEvent::WriteRejected {
                        target: canonical,
                        error: error.to_string(),
                        time: self.current_time(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_snapshot_resolves_case_insensitively_and_preserves_canonical_order() {
        let runtime = crate::harness::CompileSession::from_source(
            "VAR_GLOBAL Pressure : DINT := DINT#7; Enabled : BOOL := TRUE; END_VAR PROGRAM Main END_PROGRAM"
        ).build_runtime().unwrap();

        let snapshot =
            runtime.snapshot_globals(&["enabled".into(), "missing".into(), "PRESSURE".into()]);
        let entries = snapshot.iter().collect::<Vec<_>>();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], (&SmolStr::new("Enabled"), &Value::Bool(true)));
        assert_eq!(entries[1], (&SmolStr::new("Pressure"), &Value::DInt(7)));
    }

    #[test]
    fn mesh_publish_snapshot_reads_only_requested_nested_equipment_members() {
        let mut harness = crate::harness::TestHarness::from_source(
            r#"
FUNCTION_BLOCK Pump
VAR_OUTPUT
    Running : BOOL;
    Speed : REAL;
END_VAR
    Running := TRUE;
    Speed := 1460.0;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    P101 : Pump;
    Levels : ARRAY[1..2] OF INT := [12, 24];
END_VAR
    P101();
END_PROGRAM
"#,
        )
        .expect("the publish fixture compiles");
        harness.cycle();
        let snapshot = harness.runtime().snapshot_globals(&[
            "Main.P101.Running".into(),
            "main.p101.speed".into(),
            "Main.Levels[2]".into(),
            "Main.Missing".into(),
            "Main.Levels[9]".into(),
        ]);
        assert_eq!(snapshot.get("Main.P101.Running"), Some(&Value::Bool(true)));
        assert_eq!(snapshot.get("main.p101.speed"), Some(&Value::Real(1460.0)));
        assert_eq!(snapshot.get("Main.Levels[2]"), Some(&Value::Int(24)));
        assert_eq!(
            snapshot.len(),
            3,
            "no missing paths or unrequested instance members are published"
        );
    }

    #[test]
    fn mesh_updates_existing_globals_case_insensitively_without_creating_unknowns() {
        let mut runtime = crate::harness::CompileSession::from_source(
            "VAR_GLOBAL Pressure : DINT := DINT#7; END_VAR PROGRAM Main END_PROGRAM",
        )
        .build_runtime()
        .unwrap();
        let updates = IndexMap::from([
            (SmolStr::new("pressure"), Value::DInt(9)),
            (SmolStr::new("Unknown"), Value::DInt(1)),
        ]);

        runtime.apply_mesh_updates(&updates);

        assert_eq!(
            runtime.storage().get_global("Pressure"),
            Some(&Value::DInt(9))
        );
        assert_eq!(runtime.storage().get_global("pressure"), None);
        assert_eq!(runtime.storage().get_global("Unknown"), None);
    }

    #[test]
    fn mesh_rejects_wrong_types_and_constants_without_faulting_or_mutating_state() {
        let mut runtime = crate::harness::CompileSession::from_source(
            "VAR_GLOBAL Pressure : DINT := DINT#7; END_VAR VAR_GLOBAL CONSTANT Fixed : DINT := DINT#3; END_VAR PROGRAM Main END_PROGRAM"
        ).build_runtime().unwrap();
        let debug = runtime.enable_debug();
        runtime.apply_mesh_updates(&IndexMap::from([
            (SmolStr::new("Pressure"), Value::Bool(true)),
            (SmolStr::new("Fixed"), Value::DInt(5)),
        ]));
        assert_eq!(
            runtime.storage().get_global("Pressure"),
            Some(&Value::DInt(7))
        );
        assert_eq!(runtime.storage().get_global("Fixed"), Some(&Value::DInt(3)));
        assert!(!runtime.faulted());
        assert_eq!(
            debug
                .drain_runtime_events()
                .iter()
                .filter(|event| matches!(event, crate::debug::RuntimeEvent::WriteRejected { .. }))
                .count(),
            2
        );
    }
}
