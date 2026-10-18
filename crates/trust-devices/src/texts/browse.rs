//! The texts of the browse panes (design §4.3 B8; specification 25 §5.4 and §11.2): the pane
//! titles, the `Add` button and the reasons it cannot be chosen, the write toggle, and the
//! human-facing text of every browse failure. Texts ported from the panel's TypeScript keep its
//! words (`W/BrowseTagsPanel.tsx` and `W/opcuaClientModel.ts`).

use crate::texts::connections::protocol_name;

// ---- the pane and its add button ----

/// The title of the OPC UA node browser.
pub const NODES_TITLE: &str = "Browse OPC UA nodes";
/// The title of the `EtherCAT` channel browser.
pub const CHANNELS_TITLE: &str = "Browse PDO channels";
/// The title of the picker of the globals a server exposes.
pub const EXPOSE_TITLE: &str = "Choose globals to expose";
/// The title of the ADS tag browser.
pub const ADS_TITLE: &str = "Browse tags";
/// The `Add` label of the OPC UA node browser.
pub const NODES_ADD: &str = "Add nodes";
/// The `Add` label of the `EtherCAT` channel browser.
pub const CHANNELS_ADD: &str = "Add channels";
/// The `Add` label of the picker of the globals a server exposes.
pub const EXPOSE_ADD: &str = "Expose selected";
/// The `Add` label of the ADS tag browser.
pub const ADS_ADD: &str = "Add tags";

/// The write toggle's label (`W/BrowseTagsPanel.tsx:331`).
pub const ALLOW_WRITES: &str = "Allow writes (default: read-only)";

/// Why `Add` cannot be chosen while the browse still runs.
pub const WAIT_FOR_RESULTS: &str = "Wait for browse results before adding tags.";
/// Why `Add` cannot be chosen while the browse failed.
pub const RESOLVE_ERROR: &str = "Resolve the browse error before adding tags.";
/// Why `Add` cannot be chosen when nothing was browsed yet: a non-ADS connection.
pub const NOT_BROWSED: &str = "Start or connect the runtime, then Browse again to load symbols.";
/// Why `Add` cannot be chosen when nothing was browsed yet: an ADS port.
pub const ADS_NOT_BROWSED: &str = "Choose an ADS port and browse its symbols first.";
/// Why `Add` cannot be chosen when the browse answered no symbols.
pub const NO_SYMBOLS: &str = "No symbols are available to add.";
/// Why `Add` cannot be chosen while nothing is ticked.
pub const NO_SELECTION: &str = "Select at least one symbol to add.";

// ---- what failed ----

/// The title of a browse failure with no protocol to name.
pub const BROWSE_FAILED: &str = "Browse failed";
/// The title of a browse failure whose protocol did not classify it (`W/opcuaClientModel.ts:200`).
#[must_use]
pub fn browse_failed(protocol: &str) -> String {
    if protocol == "opcua_client" {
        "OPC UA browse failed".to_owned()
    } else {
        format!("{} browse failed", protocol_name(protocol))
    }
}

/// Why a browse request named no connection or found device.
pub const NO_SOURCE: &str = "The browse request named no connection.";
/// Why a connection cannot be browsed: the panel's session has not shown it.
#[must_use]
pub fn connection_not_shown(connection: &str) -> String {
    format!(
        "Devices & Connections has not shown the connection {connection}. \
         Open the panel again and retry."
    )
}
/// Why a connection cannot be browsed: its runtime did not answer and holds no project files.
#[must_use]
pub fn runtime_did_not_answer(runtime: &str) -> String {
    format!("{runtime} did not answer, so its connections cannot be browsed.")
}
/// Why a connection cannot be browsed: the panel has no project folder to browse offline.
pub const NO_PROJECT: &str = "The panel has no project folder to browse.";
/// Why a connection cannot be browsed: its protocol has no browse pane here yet.
#[must_use]
pub fn not_browsable(protocol: &str) -> String {
    format!(
        "A {} connection cannot be browsed here yet.",
        protocol_name(protocol)
    )
}

// ---- the OPC UA failures and their recovery (`W/opcuaClientModel.ts:157-203`) ----

/// The title of a server certificate that is not trusted.
pub const CERT_UNTRUSTED_TITLE: &str = "Server certificate not trusted";
/// What to do about an untrusted server certificate.
pub const CERT_UNTRUSTED_DETAIL: &str =
    "Confirm this is the expected server, then choose Trust certificate to browse it.";
/// The recovery that browses a server whose certificate is not trusted.
pub const TRUST_CERTIFICATE: &str = "Trust certificate";
/// The title of an OPC UA server that needs authentication.
pub const AUTH_REQUIRED_TITLE: &str = "Authentication required";
/// What to do about an OPC UA server that needs authentication.
pub const AUTH_REQUIRED_DETAIL: &str =
    "Choose username authentication or update the saved OPC UA credentials, then browse again.";
/// The recovery that reopens the connection's form to edit its credentials.
pub const EDIT_CREDENTIALS: &str = "Edit credentials";
/// The title of a security profile the server does not offer.
pub const UNSUPPORTED_SECURITY_TITLE: &str = "Unsupported security profile";
/// What to do about an unsupported security profile.
pub const UNSUPPORTED_SECURITY_DETAIL: &str =
    "Choose a security policy and mode that this OPC UA server offers.";
/// The title of a server that could not be reached.
pub const UNREACHABLE_TITLE: &str = "Server unreachable";
/// What to do about a server that could not be reached.
pub const UNREACHABLE_DETAIL: &str =
    "Check the endpoint URL, port, and that the OPC UA server is running.";
/// The title of a server that refused browsing.
pub const BROWSE_DENIED_TITLE: &str = "Browse denied";
/// Why the server refused browsing.
pub const BROWSE_DENIED_DETAIL: &str =
    "The server refused node browsing for the selected identity.";

// ---- the add action's payload did not decode ----

/// Why `Add` cannot run: its payload did not name the browse request.
pub const ADD_DID_NOT_DECODE: &str = "The action did not name the browse request.";
/// Why `Add` cannot run: the browse request did not decode.
#[must_use]
pub fn add_did_not_decode(error: &str) -> String {
    format!("The browse request did not decode: {error}")
}
