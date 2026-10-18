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

// ---- the ADS tag browser (design §4.3 B8 and §3.3 J40, J46, J54, J55, J57, J58 and J59;
// specification 25 §5.4 and its lines 878-883) ----

/// The ADS ports the browser suggests, each with its name (specification 25:880-881).
pub const ADS_PORT_SUGGESTIONS: [(&str, &str); 4] = [
    ("301", "I/O"),
    ("501", "Motion"),
    ("851", "PLC runtime 1"),
    ("852", "PLC runtime 2"),
];

/// The help under `Advanced: browse another ADS port` (specification 25:880-883;
/// `W/AdsBrowseTargetControls.tsx:120-123`).
pub const ADS_PORT_HELP: &str = "Each ADS port is a separate server and symbol namespace. The \
server must support Symbol Upload.";

/// Why an ADS port cannot be browsed: it is not a whole number from 1 through 65,535
/// (`parseAdsPortInput`, `W/adsTargetPort.ts:9-22`; design §3.3 J59).
pub const ADS_PORT_RANGE: &str = "ADS port must be between 1 and 65535.";

/// The `TwinCAT` administrator login that `Create route` asks for once (specification 25:387-394).
pub const CREATE_ROUTE: &str = "Create route";

/// Why `Create route` cannot run: no runtime answered to add the route on (design §3.3 J57 and
/// specification 25 §5.4).
pub const CREATE_ROUTE_NEEDS_RUNTIME: &str =
    "Creating a TwinCAT route needs a running runtime. Start the runtime and try again.";

/// The title of an ADS port whose server did not answer (`classifyBrowseError`,
/// `W/browseErrorModel.ts:23-30`).
pub const ADS_PORT_UNAVAILABLE_TITLE: &str = "ADS port unavailable";
/// What to do about an ADS port whose server did not answer.
pub const ADS_PORT_UNAVAILABLE_DETAIL: &str =
    "The selected ADS server did not answer. Check the port and that this server is running, then \
     browse again.";
/// The title of an ADS server without Beckhoff Symbol Upload (`W/browseErrorModel.ts:31-38`).
pub const SYMBOL_UPLOAD_UNSUPPORTED_TITLE: &str = "Symbol Upload unsupported";
/// What to do about an ADS server without Beckhoff Symbol Upload.
pub const SYMBOL_UPLOAD_UNSUPPORTED_DETAIL: &str =
    "This ADS server is reachable but does not expose Beckhoff Symbol Upload. Enable symbol \
     generation for that server or choose another ADS port.";
/// The title of an ADS server with no symbols truST can import (`W/browseErrorModel.ts:39-46`).
pub const NO_COMPATIBLE_SYMBOLS_TITLE: &str = "No compatible symbols";
/// What an empty symbol table means.
pub const NO_COMPATIBLE_SYMBOLS_DETAIL: &str =
    "The ADS server returned an empty symbol table or no symbols that truST can import.";
/// The title of any other ADS browse failure (`W/browseErrorModel.ts:47-53`).
pub const ADS_BROWSE_FAILED_TITLE: &str = "ADS symbol browse failed";
/// What an ADS browse failure means when the origin gave no message.
pub const ADS_BROWSE_FAILED_DETAIL: &str = "ADS symbol browse failed.";

/// The card's title for the ADS port `port`.
#[must_use]
pub fn ads_port_title(port: u16) -> String {
    format!("ADS port {port}")
}

/// The count of a port's tags, singular and plural (design §3.3 J58;
/// `W/AdsMultiPortTagBrowser.tsx:279`).
#[must_use]
pub fn tag_count(count: usize) -> String {
    let noun = if count == 1 { "tag" } else { "tags" };
    format!("{count} {noun}")
}

/// What an untried port says while its browse runs (`W/AdsMultiPortTagBrowser.tsx:279`).
pub const LOADING: &str = "Loading…";

/// The button that browses one more ADS port (`W/AdsMultiPortTagBrowser.tsx:310`).
#[must_use]
pub fn browse_port(port: u16) -> String {
    format!("Browse port {port}")
}

/// Why a port's route is missing, in its card (`W/AdsMultiPortTagBrowser.tsx:534`).
#[must_use]
pub fn route_required(port: u16) -> String {
    format!("An ADS route is required before tags can be read from port {port}.")
}

/// What the route help says when the route plan carries generated scripts (`routeWarningText`,
/// `W/BrowseTagsPanel.tsx:154-158`; specification 25 §5.4).
pub const ROUTE_NEEDS_ADMIN_SCRIPT: &str = "Route needs TwinCAT administrator access. Run the \
generated route script on the TwinCAT computer, then reopen Browse.";

/// What the route help says when the route plan carries no generated script.
pub const ROUTE_NEEDS_ADMIN_CREATE: &str = "Route needs TwinCAT administrator access. Create the \
route on the TwinCAT computer, then reopen Browse.";

/// The label of the button that copies one generated route file
/// (`W/AdsMultiPortTagBrowser.tsx:546`).
#[must_use]
pub fn copy_artifact(label: &str) -> String {
    format!("Copy {label}")
}

// ---- the add action's payload did not decode ----

/// Why `Add` cannot run: its payload did not name the browse request.
pub const ADD_DID_NOT_DECODE: &str = "The action did not name the browse request.";
/// Why `Add` cannot run: the browse request did not decode.
#[must_use]
pub fn add_did_not_decode(error: &str) -> String {
    format!("The browse request did not decode: {error}")
}
