//! The rules of today's Devices & Connections canvas (`editors/vscode/src/networkCanvas`;
//! specification 25 §5.15). The canvas gathers what the runtimes answer, sends it here, and draws
//! the answer; every rule of the canvas is in this module, which reuses the panel engine's topology
//! merge, health rules, computer headings, form conversion and connection planners, so that the
//! canvas and the redesigned panel keep one copy of each rule.
//!
//! - `topology`: `trust-lsp.devicesTopology`: the runtimes' answers merged into one topology, with
//!   the fleet view the canvas draws (identities, health roll-ups, headings, names, faults, the
//!   summary, the dimming of the search, and each connection's browse target), and what the merged
//!   topology says about the ADS devices that the last search found.
//! - `act`: `trust-lsp.devicesCanvasAct`: one action of the canvas, performed here: a form applied,
//!   tested or saved, an ADS device, ADS tags, exposed globals, `EtherCAT` channels or an OPC UA
//!   client connection written into the project, a typed ADS port checked, and a runtime address
//!   added to the fleet.

mod act;
mod ads;
mod ads_tags;
mod comm;
mod fleet;
mod hosts;
mod identify;
mod local;
mod plans;
mod topology;
mod views;
mod wire;

pub use act::{act, CanvasActRequest, CanvasNotice, CanvasNoticeAction, CanvasReply, LiveRuntime};
pub use ads_tags::PortSelection;
pub use fleet::{
    CanvasActionState, CanvasFleet, ExternalView, FleetContainerView, FleetEndpointView,
    FleetFault, FleetHostView, FleetLinkView, FleetRuntimeView,
};
pub use local::{
    CanvasBanner, CanvasBannerAction, CanvasFailure, CanvasManaged, CanvasSession, CanvasSetup,
    CanvasSimulator,
};
pub use topology::{
    topology, Answer, CanvasReach, CanvasRuntime, CanvasTopology, CanvasTopologyRequest,
    DiscoveredAds,
};
pub use views::{AdsClientSummaryView, AdsDeviceView, AdsPortView, BrowseErrorView};

/// The schema version of the canvas's answers.
pub const CANVAS_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod local_tests;
#[cfg(test)]
mod tests;
