//! The read-only connection behind a target's route list (specification 11, O20). The search reads
//! the list over this connection: a connection to the target's system service, on the route the
//! target already has from this computer. Nothing here adds a route.

use crate::ads::diagnostics::TargetIdentity;
use crate::ads::{AdsIndexRead, AdsRsTransport};

use super::browse::derive_local_identity;
use super::wire::route_to_ams_port;

/// The AMS port of the system service, which lists the route table (`adstool route list`).
const SYSTEM_SERVICE_PORT: u16 = 10000;

/// Opens a read-only connection to the system service (AMS port 10000) of `target`.
///
/// # Errors
/// This computer's AMS Net ID cannot be derived, or the target's AMS router does not accept the
/// connection.
pub(crate) fn open_route_list_reads(
    target: &TargetIdentity,
) -> Result<Box<dyn AdsIndexRead>, String> {
    let local = derive_local_identity(target)?;
    let route = route_to_ams_port(target, Some(&local), SYSTEM_SERVICE_PORT);
    let mut transport = AdsRsTransport::new(route);
    transport
        .connect()
        .map_err(|error| format!("connect to the system service: {error}"))?;
    Ok(Box::new(transport))
}
