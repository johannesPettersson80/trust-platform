//! The HMI web routes' port into the running resource. The web layer is a transport adapter: it
//! reaches the resource, its HMI exchange and the historian only through this control port, never
//! through `ControlState`'s fields (the architecture policy's host-surface rule, FULLMAP-CHECK-07).

use std::sync::Arc;

use super::ControlState;
use crate::historian::HistorianService;
use crate::hmi::exchange::HmiExchange;
use crate::scheduler::{ResourceControl, StdClock};

/// The exchange between the screens and the program's `Hmi.Display` calls.
#[must_use]
pub(crate) fn hmi_exchange(state: &ControlState) -> Arc<HmiExchange> {
    state.resource.hmi_exchange()
}

/// The resource that the HMI's alarm list and value stream read.
#[must_use]
pub(crate) fn resource(state: &ControlState) -> ResourceControl<StdClock> {
    state.resource.clone()
}

/// The runtime's historian, when it keeps one.
#[must_use]
pub(crate) fn historian(state: &ControlState) -> Option<&Arc<HistorianService>> {
    state.historian.as_ref()
}
