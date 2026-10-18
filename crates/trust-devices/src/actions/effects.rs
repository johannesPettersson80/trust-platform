//! The host-effect lists of the runtime actions (design §1.4.1 and §4.3, inventories I9, I11, I73
//! and J36): the VS Code calls that the extension runs after `act` answers. Each function returns
//! only effects; the rules that choose them are in [`crate::actions::runtimes`].
//!
//! The order of a list is the order the extension runs it. Connect attaches and then selects the
//! run target; a managed Start imports the token before it attaches, then selects the target.

use serde_json::json;

use crate::view::types::{HostEffect, ItemId, Secret, SettingTarget};

/// The setting that holds the runtimes added with `Connect existing runtime`.
pub(crate) const FLEET_ENDPOINTS: &str = "trust.runtime.fleetEndpoints";

/// The name a managed runtime is shown with in an attach: `<name> (this computer)`
/// (`managedRuntimeLabel`, `localRuntimeModel.ts:71-73`), which tells it apart from the Simulator
/// and from remotes.
#[must_use]
pub(crate) fn managed_label(name: &str) -> String {
    format!("{name} (this computer)")
}

/// Connects to `endpoint` and selects it as the run target (design §3.3 I9, specification 25:304).
#[must_use]
pub(crate) fn connect(endpoint: &str, label: &str) -> Vec<HostEffect> {
    vec![
        HostEffect::Attach {
            endpoint: endpoint.to_owned(),
            label: label.to_owned(),
        },
        HostEffect::SetRunTarget {
            id: endpoint.to_owned(),
        },
    ]
}

/// A successful managed Start (design §4.3 I11, specification 25:858-860): the token first, then
/// the attach with the reached endpoint and the friendly label, then the run target. The token is
/// omitted while the runtime needs none.
#[must_use]
pub(crate) fn managed_started(name: &str, endpoint: &str, token: Option<&str>) -> Vec<HostEffect> {
    let mut effects = Vec::new();
    if let Some(token) = token {
        effects.push(store_secret(endpoint, token));
    }
    effects.push(HostEffect::Attach {
        endpoint: endpoint.to_owned(),
        label: managed_label(name),
    });
    effects.push(HostEffect::SetRunTarget {
        id: name.to_owned(),
    });
    effects
}

/// Stores `token` for `endpoint` in VS Code's secret store.
#[must_use]
pub(crate) fn store_secret(endpoint: &str, token: &str) -> HostEffect {
    HostEffect::StoreSecret {
        endpoint: endpoint.to_owned(),
        token: Secret(token.to_owned()),
    }
}

/// Stores the friendly name `label` of `endpoint`.
#[must_use]
pub(crate) fn store_label(endpoint: &str, label: &str) -> HostEffect {
    HostEffect::StoreLabel {
        endpoint: endpoint.to_owned(),
        label: label.to_owned(),
    }
}

/// Writes the whole `trust.runtime.fleetEndpoints` list, which the extension keeps in the
/// workspace (specification 25:333-334).
#[must_use]
pub(crate) fn write_fleet_endpoints(endpoints: &[String]) -> HostEffect {
    HostEffect::WriteSetting {
        key: FLEET_ENDPOINTS.to_owned(),
        value: json!(endpoints),
        target: SettingTarget::Workspace,
    }
}

/// Selects the item, which the webview draws as selected and focused (specification 25:89-90).
#[must_use]
pub(crate) fn select(item: ItemId) -> HostEffect {
    HostEffect::Select { item }
}
