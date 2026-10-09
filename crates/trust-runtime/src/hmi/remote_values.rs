//! Remote HMI samples are received off the PLC thread and copied at Hmi.Display boundaries.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

use super::commands::write::path_text;
use super::display_values::{HmiRead, Shown, WireValue};
use super::member_paths::MemberPath;

/// Remote samples become stale after three mesh periods and unknown after ten.
pub const STALE_AFTER: Duration = Duration::from_secs(3);
/// Retaining an old remote value beyond this age would disguise a disconnected owner.
pub const UNKNOWN_AFTER: Duration = Duration::from_secs(10);

struct Sample {
    value: Option<WireValue>,
    present: bool,
    instance: Option<String>,
    seen: Instant,
}

/// A bounded cache; it never writes program memory or performs network I/O.
#[derive(Default)]
pub struct RemoteValues {
    catalogue_requested: AtomicBool,
    samples: Mutex<BTreeMap<String, Sample>>,
    catalogues: Mutex<BTreeMap<String, (Instant, Vec<serde_json::Value>)>>,
}
impl std::fmt::Debug for RemoteValues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteValues").finish_non_exhaustive()
    }
}

/// Split an explicitly qualified path without reinterpreting a local member path.
pub fn remote_path(path: &MemberPath) -> Option<(String, MemberPath)> {
    let (peer, root) = path.root.split_once(':')?;
    let mut local = path.clone();
    local.root = root.into();
    Some((peer.to_string(), local))
}

/// Runtime identities are case-sensitive; IEC member names are not.
pub fn qualified_key(path: &str) -> String {
    let folded = |local: &str| {
        MemberPath::parse(local)
            .ok()
            .map(|path| path_text(&path).to_ascii_lowercase())
            .unwrap_or_else(|| local.to_ascii_lowercase())
    };
    path.split_once(':').map_or_else(
        || folded(path),
        |(peer, local)| format!("{peer}:{}", folded(local)),
    )
}

impl RemoteValues {
    /// Whether an editor or deployment check requested fresh discovery.
    pub fn take_catalogue_request(&self) -> bool {
        self.catalogue_requested.swap(false, Ordering::Relaxed)
    }
    /// Whether at least one fresh catalogue response has arrived, including an empty publish list.
    pub fn catalogue_ready(&self) -> bool {
        self.catalogues
            .lock()
            .map(|entries| {
                entries
                    .values()
                    .any(|(seen, _)| seen.elapsed() < STALE_AFTER)
            })
            .unwrap_or(false)
    }
    /// Refresh typed metadata from the peer's existing catalogue endpoint.
    pub fn update_catalogue(&self, peer: &str, entries: Vec<serde_json::Value>) {
        if entries.len() > 1000 {
            return;
        }
        if let Ok(mut catalogues) = self.catalogues.lock() {
            catalogues.retain(|_, (seen, _)| seen.elapsed() < UNKNOWN_AFTER);
            if catalogues.len() < 32 || catalogues.contains_key(peer) {
                catalogues.insert(peer.to_string(), (Instant::now(), entries));
            }
        }
    }
    /// Only fresh owner declarations may establish remote binding validity.
    pub fn catalogue(&self) -> BTreeMap<String, serde_json::Value> {
        self.catalogue_requested.store(true, Ordering::Relaxed);
        let Ok(catalogues) = self.catalogues.lock() else {
            return BTreeMap::new();
        };
        catalogues
            .iter()
            .filter(|(_, (seen, _))| seen.elapsed() < STALE_AFTER)
            .flat_map(|(peer, (_, entries))| {
                entries.iter().filter_map(move |entry| {
                    let variable = entry.get("variable")?;
                    let path = variable.get("path")?.as_str()?;
                    let qualified = format!("{peer}:{path}");
                    let mut variable = variable.clone();
                    variable["path"] = serde_json::json!(qualified);
                    variable["runtime"] = serde_json::json!(peer);
                    Some((qualified_key(&qualified), variable))
                })
            })
            .collect()
    }

    /// Keep only paths that current screens or trends need.
    pub fn retain(&self, paths: &[MemberPath]) {
        let wanted: BTreeSet<_> = paths
            .iter()
            .map(|path| qualified_key(&path_text(path)))
            .collect();
        if let Ok(mut samples) = self.samples.lock() {
            samples.retain(|key, _| wanted.contains(key));
        }
    }

    /// Accept one bounded owner response. Caller validates peer and requested path identities.
    pub fn update(&self, paths: &[MemberPath], answer: &serde_json::Value) {
        let Some(values) = answer.get("values").and_then(serde_json::Value::as_array) else {
            return;
        };
        if values.len() != paths.len() || paths.len() > 500 {
            return;
        }
        let Ok(mut samples) = self.samples.lock() else {
            return;
        };
        for (index, path) in paths.iter().enumerate() {
            let value = values[index].get("value").map(|value| WireValue {
                value: Shown::Single(value.clone()),
                lower_bounds: values[index]
                    .get("lowerBounds")
                    .and_then(|value| serde_json::from_value(value.clone()).ok()),
                stale_ms: values[index]
                    .get("staleMs")
                    .and_then(serde_json::Value::as_u64),
                presentation: values[index]
                    .get("presentation")
                    .filter(|value| value.is_object())
                    .map(|value| crate::hmi::presentation::EquipmentPresentation {
                        unit: value["unit"].as_str().map(str::to_string),
                        range: serde_json::from_value(value["range"].clone()).ok(),
                        limits: serde_json::from_value(value["limits"].clone()).ok(),
                        priorities: ["low-low", "low", "high", "high-high"]
                            .into_iter()
                            .filter_map(|key| {
                                value["priorities"][key]
                                    .as_str()
                                    .map(|priority| (key, priority.to_string()))
                            })
                            .collect(),
                    }),
            });
            samples.insert(
                qualified_key(&path_text(path)),
                Sample {
                    value,
                    present: answer["present"][index].as_bool().unwrap_or(false),
                    instance: answer["instance_types"][index].as_str().map(str::to_string),
                    seen: Instant::now(),
                },
            );
        }
    }

    /// Overlay a local read without blocking on peers. Unknown remains distinct from missing.
    pub fn overlay(&self, paths: &[MemberPath], read: &mut HmiRead) {
        let Ok(samples) = self.samples.lock() else {
            return;
        };
        for (index, path) in paths.iter().enumerate() {
            if remote_path(path).is_none() {
                continue;
            }
            let Some(sample) = samples.get(&qualified_key(&path_text(path))) else {
                continue;
            };
            let age = sample.seen.elapsed();
            read.present[index] = sample.present;
            read.instance_types[index] = sample.instance.clone();
            read.values[index] = if age >= UNKNOWN_AFTER {
                None
            } else {
                sample.value.clone().map(|mut value| {
                    if age >= STALE_AFTER {
                        value.stale_ms = Some(age.as_millis() as u64);
                    }
                    value
                })
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn read() -> HmiRead {
        HmiRead {
            cycle: 1,
            values: vec![None],
            present: vec![false],
            instance_types: vec![None],
        }
    }
    #[test]
    fn remote_samples_are_stale_then_unknown_without_touching_program_storage() {
        let values = RemoteValues::default();
        let paths = vec![MemberPath::parse("Owner:Main.Level").unwrap()];
        values.update(
            &paths,
            &json!({"values":[{"value":42}],"present":[true],"instance_types":[null]}),
        );
        let mut fresh = read();
        values.overlay(&paths, &mut fresh);
        assert_eq!(serde_json::to_value(&fresh.values[0]).unwrap()["value"], 42);
        {
            let mut samples = values.samples.lock().unwrap();
            samples.get_mut("Owner:main.level").unwrap().seen =
                Instant::now() - Duration::from_secs(4);
        }
        let mut stale = read();
        values.overlay(&paths, &mut stale);
        assert!(stale.values[0].as_ref().unwrap().stale_ms.unwrap() >= 4000);
        values
            .samples
            .lock()
            .unwrap()
            .get_mut("Owner:main.level")
            .unwrap()
            .seen = Instant::now() - Duration::from_secs(11);
        let mut unknown = read();
        values.overlay(&paths, &mut unknown);
        assert!(unknown.values[0].is_none());
        assert!(unknown.present[0]);
        values.retain(&[]);
        assert!(values.samples.lock().unwrap().is_empty());
    }
    #[test]
    fn peer_identity_is_not_case_folded_and_unpublished_children_are_not_invented() {
        let values = RemoteValues::default();
        values.update_catalogue(
            "Owner",
            vec![json!({"variable":{"path":"Main.Level","kinds":["number"],"writable":false}})],
        );
        let metadata = values.catalogue();
        assert!(metadata.contains_key("Owner:main.level"));
        assert!(!metadata.contains_key("owner:main.level"));
        assert_eq!(metadata.len(), 1);
        assert!(MemberPath::parse("Owner:Other:Main.Level").is_err());
    }
}
