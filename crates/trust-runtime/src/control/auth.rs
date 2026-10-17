use crate::security::AccessRole;
use trust_runtime_services::access::{resolve_role, AuthFailure};

use super::{ControlRequest, ControlState};

/// Reads the control credentials from `state` and lets the shared access rules decide. A
/// poisoned token lock fails closed.
pub(super) fn resolve_request_role(
    request: &ControlRequest,
    state: &ControlState,
    client: Option<&str>,
) -> Result<AccessRole, AuthFailure> {
    let expected = state
        .auth_token
        .lock()
        .map_err(|_| AuthFailure::InvalidToken)?
        .clone();
    resolve_role(
        request.auth.as_deref(),
        expected.as_deref(),
        state.control_requires_auth,
        client,
        |token| {
            state
                .pairing
                .as_ref()
                .and_then(|store| store.validate_with_role(token))
        },
    )
}

#[cfg(test)]
#[path = "auth/contract_tests.rs"]
mod contract_tests;
