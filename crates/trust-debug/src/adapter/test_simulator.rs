//! The simulator as the session's tests start it (specification 44 NX-16): built from a project
//! folder and started as a launch starts it, with the control state its control server serves. It
//! needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::path::Path;
use std::sync::Arc;

use trust_runtime::control::{ControlEndpoint, ControlState};
use trust_runtime::stdlib::conversions::ConversionProfile;

/// A started simulator.
pub(crate) struct Simulator(super::simulator::Simulator);

impl Simulator {
    /// The control state its control server serves.
    pub(crate) fn state(&self) -> Arc<ControlState> {
        self.0.control_server().state()
    }

    /// Stops its program and its scheduler and closes its control server.
    pub(crate) fn stop(self) {
        self.0.stop();
    }
}

/// The simulator of the project folder `root` whose program is `main`, with its control server at
/// `endpoint`, and the session that holds its sources.
pub(crate) fn start(
    root: &Path,
    main: &Path,
    endpoint: ControlEndpoint,
) -> (crate::DebugSession, Simulator) {
    let (session, simulator) = super::control_bridge::start_simulator(root, main, endpoint, None);
    (session, Simulator(simulator))
}

/// The project's image built from `sources` with the conversion rules `conversions`, as a launch
/// builds it.
pub(crate) fn build(
    root: &Path,
    sources: &[(String, String)],
    conversions: ConversionProfile,
) -> Vec<u8> {
    super::simulator::build_image(root, sources, conversions).expect("the project builds")
}
