//! Runtime mesh helpers (snapshot/apply).

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::value::Value;

use super::core::Runtime;

impl Runtime {
    /// Snapshot global values for mesh publishing.
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
            }
        }
        out
    }

    /// Apply mesh updates to globals (skips unknown names).
    pub fn apply_mesh_updates(&mut self, updates: &IndexMap<SmolStr, Value>) {
        for (name, value) in updates {
            let canonical = self
                .storage()
                .globals()
                .keys()
                .find(|canonical| canonical.eq_ignore_ascii_case(name.as_str()))
                .cloned();
            if let Some(canonical) = canonical {
                self.storage_mut().set_global(canonical, value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_snapshot_resolves_case_insensitively_and_preserves_canonical_order() {
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("Pressure", Value::DInt(7));
        runtime
            .storage_mut()
            .set_global("Enabled", Value::Bool(true));

        let snapshot =
            runtime.snapshot_globals(&["enabled".into(), "missing".into(), "PRESSURE".into()]);
        let entries = snapshot.iter().collect::<Vec<_>>();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], (&SmolStr::new("Enabled"), &Value::Bool(true)));
        assert_eq!(entries[1], (&SmolStr::new("Pressure"), &Value::DInt(7)));
    }

    #[test]
    fn mesh_updates_existing_globals_case_insensitively_without_creating_unknowns() {
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("Pressure", Value::DInt(7));
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
}
