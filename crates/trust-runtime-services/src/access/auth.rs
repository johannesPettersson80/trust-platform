//! Which role a control request acts with (specification 11, §6.9.5 and §6.9.6). The platform
//! supplies the configured token, the pairing lookup and the transport label of the client.

use super::role::{constant_time_eq, AccessRole};

/// Why a control request was refused before its role was known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailure {
    /// Authentication is required and the request carries no token.
    MissingToken,
    /// The request's token matches neither the control token nor a pairing.
    InvalidToken,
}

impl AuthFailure {
    /// The stable error message.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::MissingToken => "missing auth token",
            Self::InvalidToken => "invalid auth token",
        }
    }

    /// The stable error code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::MissingToken => "missing_auth_token",
            Self::InvalidToken => "invalid_auth_token",
        }
    }
}

/// Resolves the role of a control request.
///
/// `provided` is the request's `auth` token, `expected` the configured control token,
/// `requires_auth` whether the control connection requires authentication, `client` the
/// transport label of the client, and `pairing_role` looks a token up in the pairing store. The
/// control token grants `Admin`; a pairing token grants its stored role. Without a configured
/// token and without required authentication, a client on a network transport gets `Viewer` and
/// a local client `Admin`. `pairing_role` is called at most once, and only for a provided token
/// that is not the control token.
///
/// # Errors
///
/// [`AuthFailure::MissingToken`] or [`AuthFailure::InvalidToken`] when the request must
/// authenticate and does not.
pub fn resolve_role(
    provided: Option<&str>,
    expected: Option<&str>,
    requires_auth: bool,
    client: Option<&str>,
    mut pairing_role: impl FnMut(&str) -> Option<AccessRole>,
) -> Result<AccessRole, AuthFailure> {
    if let Some(expected) = expected {
        if let Some(provided) = provided
            && constant_time_eq(expected, provided)
        {
            return Ok(AccessRole::Admin);
        }
        if let Some(token) = provided
            && let Some(role) = pairing_role(token)
        {
            return Ok(role);
        }
        return Err(if provided.is_some() {
            AuthFailure::InvalidToken
        } else {
            AuthFailure::MissingToken
        });
    }
    if let Some(token) = provided
        && let Some(role) = pairing_role(token)
    {
        return Ok(role);
    }
    if requires_auth {
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
mod tests {
    use super::*;

    #[test]
    fn auth_failures_have_stable_distinct_messages_and_codes() {
        assert_eq!(AuthFailure::MissingToken.message(), "missing auth token");
        assert_eq!(AuthFailure::MissingToken.code(), "missing_auth_token");
        assert_eq!(AuthFailure::InvalidToken.message(), "invalid auth token");
        assert_eq!(AuthFailure::InvalidToken.code(), "invalid_auth_token");
        assert_ne!(AuthFailure::MissingToken, AuthFailure::InvalidToken);
    }

    #[test]
    fn untrusted_transport_classifier_uses_exact_local_and_network_rules() {
        assert!(!control_client_is_untrusted_transport(None));
        assert!(!control_client_is_untrusted_transport(Some("unix")));
        assert!(!control_client_is_untrusted_transport(Some("local")));
        assert!(!control_client_is_untrusted_transport(Some("")));
        assert!(control_client_is_untrusted_transport(Some("tcp:9000")));
        assert!(control_client_is_untrusted_transport(Some(
            "127.0.0.1:9000"
        )));
        assert!(control_client_is_untrusted_transport(Some("[::1]:9000")));
        assert!(control_client_is_untrusted_transport(Some("unix:other")));
    }

    /// The decision and how often it consulted the pairing store, which holds `paired` as
    /// `Operator`.
    fn decide(
        provided: Option<&str>,
        expected: Option<&str>,
        requires_auth: bool,
        client: Option<&str>,
    ) -> (Result<AccessRole, AuthFailure>, usize) {
        let mut lookups = 0;
        let role = resolve_role(provided, expected, requires_auth, client, |token| {
            lookups += 1;
            (token == "paired").then_some(AccessRole::Operator)
        });
        (role, lookups)
    }

    #[test]
    fn role_follows_control_token_then_pairing_then_transport() {
        let admin = Ok(AccessRole::Admin);
        let operator = Ok(AccessRole::Operator);
        let viewer = Ok(AccessRole::Viewer);
        let missing = Err(AuthFailure::MissingToken);
        let invalid = Err(AuthFailure::InvalidToken);

        // A configured control token decides alone; the transport does not matter.
        for (provided, client, role, lookups) in [
            (Some("secret"), Some("tcp:1"), admin, 0),
            (Some("paired"), Some("unix"), operator, 1),
            (Some("wrong"), Some("unix"), invalid, 1),
            (None, Some("unix"), missing, 0),
        ] {
            assert_eq!(
                decide(provided, Some("secret"), false, client),
                (role, lookups),
                "provided={provided:?}, client={client:?}"
            );
        }

        // Without a control token, required authentication still accepts a pairing.
        for (provided, client, role, lookups) in [
            (Some("paired"), Some("tcp:1"), operator, 1),
            (Some("wrong"), Some("unix"), invalid, 1),
            (None, None, missing, 0),
        ] {
            assert_eq!(
                decide(provided, None, true, client),
                (role, lookups),
                "provided={provided:?}, client={client:?}"
            );
        }

        // Without required authentication a pairing comes first, then the transport decides.
        for (provided, client, role, lookups) in [
            (Some("paired"), Some("tcp:1"), operator, 1),
            (None, Some("127.0.0.1:9000"), viewer, 0),
            (Some("wrong"), Some("tcp:1"), viewer, 1),
            (Some("wrong"), Some("unix"), admin, 1),
            (None, None, admin, 0),
        ] {
            assert_eq!(
                decide(provided, None, false, client),
                (role, lookups),
                "provided={provided:?}, client={client:?}"
            );
        }
    }
}
