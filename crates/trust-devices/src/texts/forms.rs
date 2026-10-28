//! The texts of the add dialog and the forms (specification 25 §5.2, §5.4, §5.9 and §5.14).
//! Texts ported from the panel's TypeScript keep its words. The picker's entries and groups, the
//! fields' labels and help, the option labels and the list texts come from the communication
//! catalog, which the runtime serves (specification 11), not from here.

use super::graph::PANEL;

// ---- the add dialog ----

/// The group of the protocols that the catalog's groups do not hold (specification 25 §5.9).
pub const OTHER_CHOICES: &str = "Other choices";
/// The toggle that shows the advanced groups.
pub const SHOW_ADVANCED: &str = "Show advanced integrations";
/// The toggle that hides them.
pub const HIDE_ADVANCED: &str = "Hide advanced integrations";
/// The dialog without a catalog.
pub const CATALOG_UNAVAILABLE: &str =
    "Catalog unavailable (needs a runtime that serves the schema).";
/// The dialog with a catalog that offers nothing.
pub const NO_TYPES: &str = "No device or connection types are available.";

/// The badge of a protocol that the catalog gives no picker entry: its ID without underscores,
/// cut to three letters, in capitals.
#[must_use]
pub fn fallback_badge(protocol: &str) -> String {
    protocol
        .chars()
        .filter(|character| *character != '_')
        .take(3)
        .collect::<String>()
        .to_uppercase()
}

// ---- the Runtimes group (specification 25 §5.2 and §5.14) ----

/// `Connect existing runtime`.
pub const CONNECT_EXISTING_RUNTIME: &str = "Connect existing runtime";
/// What it does.
pub const CONNECT_EXISTING_DETAIL: &str =
    "Use a runtime already running on another computer or controller.";
/// `Run a runtime on this computer`.
pub const RUN_LOCAL_RUNTIME: &str = "Run a runtime on this computer";
/// What it does.
pub const RUN_LOCAL_DETAIL: &str =
    "Create a managed runtime project here, then select it as the Target and click Start.";
/// The installation over SSH, which comes later.
pub const INSTALL_OVER_SSH: &str = "Install over SSH";
/// What it will do.
pub const INSTALL_OVER_SSH_DETAIL: &str = "Install on another computer over SSH.";
/// The runtime in a container, which comes later.
pub const RUN_IN_DOCKER: &str = "Run in Docker";
/// What it will do.
pub const RUN_IN_DOCKER_DETAIL: &str = "Run the runtime as a container.";
/// Why those two are disabled.
pub const LATER_RELEASE: &str = "Available in a later release.";

/// A chip's link kinds as an entry of `Runtimes` names them.
pub const LINK_ETHERNET: &str = "Ethernet";
/// Wi-Fi.
pub const LINK_WIFI: &str = "Wi-Fi";
/// Bluetooth.
pub const LINK_BLUETOOTH: &str = "Bluetooth";
/// USB.
pub const LINK_USB: &str = "USB";

/// What a chip's entry of `Runtimes` offers: the truST firmware, reached over `links`, such as
/// `truST firmware · Ethernet, USB`.
#[must_use]
pub fn chip_purpose(links: &[&str]) -> String {
    if links.is_empty() {
        "truST firmware".to_owned()
    } else {
        format!("truST firmware · {}", links.join(", "))
    }
}

// ---- the quick starts (specification 25 §5.14) ----

/// The quick start of simulated I/O.
pub const QUICK_SIMULATED: &str = "Local simulated counter";
/// What it adds.
pub const QUICK_SIMULATED_DETAIL: &str =
    "No hardware. Adds simulated inputs/outputs with a counter pattern.";
/// The quick start of loopback I/O.
pub const QUICK_LOOPBACK: &str = "Loopback sanity check";
/// What it adds.
pub const QUICK_LOOPBACK_DETAIL: &str =
    "Echoes outputs back into inputs for a fast local wiring check.";
/// The quick start of a Modbus device.
pub const QUICK_MODBUS: &str = "Modbus drive or meter";
/// What it adds.
pub const QUICK_MODBUS_DETAIL: &str = "TCP device at 192.168.1.50:502 with Unit ID 1.";
/// The quick start of an MQTT broker.
pub const QUICK_MQTT: &str = "MQTT broker";
/// What it adds.
pub const QUICK_MQTT_DETAIL: &str = "Broker at 127.0.0.1:1883 with default process topics.";

// ---- the forms of a connection (specification 25 §5.2, §5.3, §5.4 and §5.13) ----

/// The breadcrumb of the form that adds a connection.
pub const ADD_FORM_BREADCRUMB: &str = "Devices & Connections / Add device or connection";

/// The title of the form that adds a connection of the protocol titled `title`.
#[must_use]
pub fn add_title(title: &str) -> String {
    format!("Add {title}")
}

/// The breadcrumb of the form that edits a connection of `protocol_name`.
#[must_use]
pub fn edit_breadcrumb(protocol_name: &str) -> String {
    format!("{PANEL} / Edit {protocol_name}")
}

/// The line that names the runtime a form writes to.
#[must_use]
pub fn owner_line(runtime: &str) -> String {
    format!("on {runtime}")
}

/// The heading of the advanced fields, which stay open (specification 25 §5.13).
pub const ADVANCED: &str = "Advanced";
/// The button that saves.
pub const SAVE: &str = "Save";
/// The save button of a disabled connection.
pub const ENABLE: &str = "Enable";
/// The button that closes the form.
pub const CANCEL: &str = "Cancel";
/// The button that tests.
pub const TEST: &str = "Test";
/// The button that disables a connection.
pub const DISABLE: &str = "Disable";
/// The button that removes a connection.
pub const REMOVE: &str = "Remove";
/// The button that confirms the removal.
pub const CONFIRM_REMOVE: &str = "Confirm remove";
/// What the removal does.
pub const REMOVE_CONFIRMATION: &str =
    "Remove this endpoint from the project? This writes the config file and takes effect after restart.";
/// The button that applies the form to the running runtime.
pub const APPLY_TO_RUNNING_RUNTIME: &str = "Apply to running runtime";
/// The note of a disabled connection.
pub const DISABLED_NOTE: &str =
    "This endpoint is disabled. Use Enable to turn it back on; restart the runtime to apply the change.";
/// Why `Test` is disabled.
pub const TEST_NEEDS_REACHABLE: &str =
    "Select a reachable online runtime before testing Communication setup.";
/// Why `Apply to running runtime` is disabled.
pub const APPLY_NEEDS_REACHABLE: &str =
    "Select a reachable online runtime before applying Communication setup.";
/// Why a test of an OPC UA client failed: its server did not answer (specification 25 §5.10).
pub const OPCUA_UNREACHABLE: &str =
    "OPC UA server is not reachable. Check the endpoint URL, port, and that the server is running.";
/// Why it failed: the server needs valid credentials.
pub const OPCUA_AUTH_REQUIRED: &str =
    "The OPC UA server needs valid credentials. Enter the username and password, then test again.";
/// Why it failed: the server's certificate is not trusted.
pub const OPCUA_CERT_UNTRUSTED: &str =
    "The OPC UA server certificate is not trusted. Use Browse nodes and choose Trust certificate if \
     this is the expected server.";
/// Why it failed: the server offers neither the selected security policy nor its mode.
pub const OPCUA_UNSUPPORTED_SECURITY: &str =
    "The OPC UA server does not offer the selected security policy and mode. Pick a supported \
     endpoint and test again.";
/// Why it failed: the server denied access.
pub const OPCUA_BROWSE_DENIED: &str =
    "The OPC UA server denied access. Check the selected credentials and permissions.";
/// `Diagnose`: the checks of the ADS Doctor, which this panel cannot run yet
/// (specification 25 §5.13).
pub const DIAGNOSE_NOT_AVAILABLE: &str =
    "The connection's diagnosis is not available yet. The ADS Doctor checks arrive in a later \
     release.";
/// Why a form's values were not sent: one of them is a secret, and the runtime is reached over
/// plain TCP from another computer (specification 25 §5.9).
pub const SECRET_NOT_SENT: &str =
    "Secret fields cannot be sent over this runtime control channel. Use a same-host runtime \
     endpoint or apply a generated snippet on the runtime host.";
/// The field error of that secret.
pub const SECRET_FIELD_NOT_SENT: &str =
    "Secret fields cannot be sent over an untrusted remote plain-TCP channel.";
/// Why a form of the canvas was not sent: some of its values do not fit their fields, which are
/// highlighted with the reason (specification 25 §5.9).
pub const FIX_HIGHLIGHTED_FIELDS: &str = "Fix the highlighted fields and try again.";
/// A form of a protocol that the catalog does not hold.
pub const NO_NATIVE_SCHEMA: &str =
    "This runtime did not return a native setup schema for this protocol.";
/// A form without a catalog.
pub const DEVICE_CATALOG_UNAVAILABLE: &str =
    "Device catalog unavailable (needs a newer trust-runtime).";
/// A connection action with no project folder to write to.
pub const NO_PROJECT_FOLDER: &str = "The panel has no project folder to write to.";
/// A connection action that named no runtime.
pub const NO_RUNTIME: &str = "The action did not name the runtime.";
/// A runtime that the panel's session has not shown.
#[must_use]
pub fn runtime_not_shown(runtime: &str) -> String {
    format!("Devices & Connections has not shown the runtime {runtime}. Open the panel again and retry.")
}
/// A runtime that did not answer while it is not the open project's own runtime.
#[must_use]
pub fn runtime_did_not_answer(runtime: &str) -> String {
    format!("{runtime} did not answer, and it is not this project's runtime, so nothing was saved.")
}
/// The draft tile's name while a form adds a connection.
pub const GHOST_NAME: &str = "New device";
/// The draft tile's detail.
pub const GHOST_DETAIL: &str = "Draft: configure and Apply";

// ---- the browse pane's `Create route` login form (specification 25 §5.4, its lines 387-394) ----

/// The breadcrumb of the login form that creates the `TwinCAT` route.
pub const CREATE_ROUTE_BREADCRUMB: &str = "Devices & Connections / Create route";
/// What the form does, as `TwinCAT`'s own Add Route does.
pub const CREATE_ROUTE_PURPOSE: &str =
    "Create the TwinCAT route from this computer, as TwinCAT's own Add Route does.";
/// The `TwinCAT` computer's administrator user name.
pub const TWINCAT_USERNAME: &str = "TwinCAT administrator user name";
/// The `TwinCAT` computer's administrator password.
pub const TWINCAT_PASSWORD: &str = "TwinCAT administrator password";
/// The help under both login fields: the login is used once and never stored.
pub const TWINCAT_LOGIN_HELP: &str =
    "The TwinCAT computer's administrator login. It is sent once to create the route and is never stored.";
/// The note that the login is not kept (specification 25:389-394).
pub const LOGIN_NOT_STORED: &str =
    "The login is used only to create this route. truST never stores or shows it again.";

// ---- list editors ----

/// A list whose value is not a list.
pub const INVALID_LIST: &str =
    "Existing value is not a valid list yet. Fix it here or reset the field.";

/// The button that adds an item called `noun`.
#[must_use]
pub fn add_item(noun: &str) -> String {
    format!("Add {noun}")
}

/// The heading of an item called `noun`, before its number: its first letter in capitals.
#[must_use]
pub fn item_title(noun: &str) -> String {
    let mut characters = noun.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

/// The label of the property `key` of an item: its underscores as spaces and each word's first
/// letter in capitals, such as `Ams Net Id` for `ams_net_id`.
#[must_use]
pub fn property_label(key: &str) -> String {
    let mut label = String::with_capacity(key.len());
    let mut word_starts = true;
    for character in key.chars() {
        let character = if character == '_' { ' ' } else { character };
        let in_word = character.is_ascii_alphanumeric();
        if word_starts && in_word {
            label.push(character.to_ascii_uppercase());
        } else {
            label.push(character);
        }
        word_starts = !in_word;
    }
    label
}

// ---- the checks of submitted values (specification 25 §5.9) ----

/// A number field that holds no whole number.
pub const ENTER_WHOLE_NUMBER: &str = "Enter a whole number.";
/// A choice field, or an on/off field, that holds none of its values.
pub const CHOOSE_LISTED_VALUE: &str = "Choose a listed value.";
/// A list field that holds no list.
pub const ENTER_JSON_ARRAY: &str = "Enter a JSON array.";
/// An object field, or a list item, that holds no object.
pub const ENTER_JSON_OBJECT: &str = "Enter a JSON object.";
/// A text field that holds no text.
pub const ENTER_TEXT: &str = "Enter text.";
/// An entry of a list of unsigned numbers, such as a CPU index, that is no such number.
pub const ENTER_UNSIGNED: &str = "Enter a whole number of 0 or more.";

/// A list item whose required property `label` is empty.
#[must_use]
pub fn item_required(label: &str) -> String {
    format!("{label} is required.")
}

/// A list item whose text property `label` holds no text.
#[must_use]
pub fn item_text(label: &str) -> String {
    format!("Enter text for {label}.")
}

/// A list item whose choice `label` holds none of its options.
#[must_use]
pub fn item_choice(label: &str) -> String {
    format!("Choose a listed value for {label}.")
}

/// A list item that sets none of the properties `labels`, of which it must set one, such as
/// `Enter Source IP or Source CIDR.`.
#[must_use]
pub fn one_of_missing(labels: &[&str]) -> String {
    format!("Enter {}.", listed(labels, "or"))
}

/// A list item that sets more than one of the properties `labels`, such as
/// `Enter only one of Source IP and Source CIDR.`.
#[must_use]
pub fn one_of_both(labels: &[&str]) -> String {
    format!("Enter only one of {}.", listed(labels, "and"))
}

/// `labels` as a phrase: `A`, `A or B`, `A, B or C`.
fn listed(labels: &[&str], conjunction: &str) -> String {
    match labels {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [rest @ .., last] => format!("{} {conjunction} {last}", rest.join(", ")),
    }
}

// ---- the forms of `Runtimes` (specification 25 §5.2) ----

/// The breadcrumb of the runtime setup forms.
pub const RUNTIME_SETUP_BREADCRUMB: &str = "Devices & Connections / Runtime setup";
/// What connecting an existing runtime does.
pub const CONNECT_RUNTIME_PURPOSE: &str = "Add a truST runtime that is already running on another computer or controller. truST checks the address before it is shown as connected.";
/// The runtime's address.
pub const RUNTIME_ADDRESS: &str = "Runtime address";
/// Its example.
pub const RUNTIME_ADDRESS_PLACEHOLDER: &str = "10.0.0.5:5680";
/// Its help.
pub const RUNTIME_ADDRESS_HELP: &str =
    "Host name or IP plus port. Advanced: tcp://host:port or unix:///path/to/socket.";
/// The runtime's token.
pub const RUNTIME_TOKEN: &str = "Runtime auth token (optional)";
/// Its placeholder.
pub const RUNTIME_TOKEN_PLACEHOLDER: &str = "Optional";
/// Its help.
pub const RUNTIME_TOKEN_HELP: &str = "Paste the token configured for that runtime. Leave this empty when the runtime does not require one. VS Code stores it securely and never shows it on the canvas.";
/// The hint to search instead.
pub const USE_DISCOVER_HINT: &str = "If you do not know the address, use Discover instead.";
/// The button of both runtime setup forms.
pub const ADD_RUNTIME: &str = "Add runtime";
/// The title of the form that runs another runtime on this computer.
pub const ADD_A_RUNTIME: &str = "Add a runtime";
/// What it does.
pub const LOCAL_RUNTIME_PURPOSE: &str = "A second runtime (PLC) on this host. It's created as its own project; start it to bring it online on the canvas.";
/// The new runtime's name.
pub const RUNTIME_NAME: &str = "Name";
/// Its example.
pub const RUNTIME_NAME_PLACEHOLDER: &str = "cell1";
/// The new runtime's template.
pub const RUNTIME_TEMPLATE: &str = "Template";
/// The template with the simulator.
pub const TEMPLATE_SIMULATOR: &str = "Simulator (no hardware)";
/// The template with loopback I/O.
pub const TEMPLATE_EMPTY: &str = "Empty (loopback I/O)";

/// The mark that a required field carries after its label (`FormView.tsx`, the old `" *"`).
pub const REQUIRED_MARK: &str = " *";
