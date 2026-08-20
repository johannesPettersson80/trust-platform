use crate::security::{constant_time_eq, AccessRole};

use super::{ControlRequest, ControlState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthFailure {
    MissingToken,
    InvalidToken,
}

impl AuthFailure {
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::MissingToken => "missing auth token",
            Self::InvalidToken => "invalid auth token",
        }
    }

    pub(super) fn code(self) -> &'static str {
        match self {
            Self::MissingToken => "missing_auth_token",
            Self::InvalidToken => "invalid_auth_token",
        }
    }
}

pub(super) fn resolve_request_role(
    request: &ControlRequest,
    state: &ControlState,
    client: Option<&str>,
) -> Result<AccessRole, AuthFailure> {
    let provided = request.auth.as_deref();
    let expected = state
        .auth_token
        .lock()
        .map_err(|_| AuthFailure::InvalidToken)?
        .clone();
    if let Some(expected) = expected {
        if let Some(provided) = provided {
            if constant_time_eq(expected.as_str(), provided) {
                return Ok(AccessRole::Admin);
            }
        }
        if let Some(token) = provided {
            if let Some(store) = state.pairing.as_ref() {
                if let Some(role) = store.validate_with_role(token) {
                    return Ok(role);
                }
            }
        }
        return Err(if provided.is_some() {
            AuthFailure::InvalidToken
        } else {
            AuthFailure::MissingToken
        });
    }
    if let Some(token) = provided {
        if let Some(store) = state.pairing.as_ref() {
            if let Some(role) = store.validate_with_role(token) {
                return Ok(role);
            }
        }
    }
    if state.control_requires_auth {
        return Err(if provided.is_some() {
            AuthFailure::InvalidToken
        } else {
            AuthFailure::MissingToken
        });
    }
    if control_client_is_untrusted_transport(client) {
        return Ok(AccessRole::Viewer);
    }
    Ok(AccessRole::Admin)
}

fn control_client_is_untrusted_transport(client: Option<&str>) -> bool {
    let Some(client) = client else {
        return false;
    };
    if client == "unix" {
        return false;
    }
    client.contains(':')
}

#[cfg(test)]
#[path = "auth/contract_tests.rs"]
mod contract_tests;
