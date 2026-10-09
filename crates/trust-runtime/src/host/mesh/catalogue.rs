//! Additive HMI variable metadata on the mesh's existing catalogue query (§7.9).
use std::sync::mpsc;

use serde_json::json;
use smol_str::SmolStr;
use zenoh::{Session, Wait};

use crate::runtime_cloud::contracts::{CatalogEntry, RUNTIME_CLOUD_API_VERSION};
use crate::runtime_cloud::keyspace::meta_catalog_key;
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};

use super::mapping::{mesh_snapshot_timeout, DEFAULT_SITE};

pub(super) fn declare(
    session: &Session,
    runtime: &str,
    publish: &[SmolStr],
    resource: &ResourceControl<StdClock>,
) -> bool {
    let key = meta_catalog_key(DEFAULT_SITE, runtime);
    let names: Vec<String> = publish.iter().map(ToString::to_string).collect();
    let resource = resource.clone();
    session
        .declare_queryable(key.as_str())
        .callback(move |query| match payload(&resource, &names) {
            Ok(bytes) => {
                let _ = query.reply(query.key_expr().clone(), bytes).wait();
            }
            Err(reason) => {
                let _ = query.reply_err(reason).wait();
            }
        })
        .background()
        .wait()
        .is_ok()
}

fn payload(resource: &ResourceControl<StdClock>, names: &[String]) -> Result<Vec<u8>, String> {
    let (tx, rx) = mpsc::channel();
    resource
        .send_command(ResourceCommand::MeshCatalogue {
            names: names.to_vec(),
            respond_to: tx,
        })
        .map_err(|error| format!("mesh catalogue request failed: {error}"))?;
    let variables = rx
        .recv_timeout(mesh_snapshot_timeout())
        .map_err(|error| format!("mesh catalogue unavailable: {error}"))??;
    let entries: Vec<_> = names
        .iter()
        .map(|name| {
            let mut entry = json!(CatalogEntry {
                api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
                schema_id: name.clone(),
                schema_version: 1,
                schema_hash: "sha256:mesh-placeholder".to_string(),
                encoding: "json".to_string(),
                qos: "mesh_t1_fast".to_string(),
            });
            if let Some(variable) = variables.iter().find(|variable| variable.path == *name) {
                entry["variable"] = json!(variable);
            }
            entry
        })
        .collect();
    serde_json::to_vec(&entries).map_err(|error| format!("mesh catalogue encoding failed: {error}"))
}
