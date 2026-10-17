//! Client roles and the constant-time comparison of secrets.

use serde::{Deserialize, Serialize};

/// Role of a control or web client, ordered from fewest to most rights; a role allows every
/// operation that a lower role allows (specification 11, §6.9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessRole {
    /// Reads status, values, events, settings and debugger state.
    Viewer,
    /// Also pauses, resumes and restarts the resource, acknowledges alarms and claims a pairing.
    Operator,
    /// Also writes and forces values, steps, sets breakpoints and changes settings, HMI and
    /// communication.
    Engineer,
    /// Every operation, including shutdown, program reload, pairing administration and security
    /// settings.
    Admin,
}

impl AccessRole {
    /// Parses a role name, ignoring surrounding whitespace and ASCII case.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "viewer" => Some(Self::Viewer),
            "operator" => Some(Self::Operator),
            "engineer" => Some(Self::Engineer),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }

    /// The lowercase wire name of the role.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Operator => "operator",
            Self::Engineer => "engineer",
            Self::Admin => "admin",
        }
    }

    /// Whether this role may run an operation that requires `required`.
    #[must_use]
    pub fn allows(self, required: Self) -> bool {
        self >= required
    }
}

/// Compare two secrets in constant time with respect to `expected` length.
#[must_use]
pub fn constant_time_eq(expected: &str, provided: &str) -> bool {
    let expected_bytes = expected.as_bytes();
    let provided_bytes = provided.as_bytes();
    let mut diff = expected_bytes.len() ^ provided_bytes.len();
    for (idx, expected_byte) in expected_bytes.iter().enumerate() {
        let provided_byte = provided_bytes.get(idx).copied().unwrap_or_default();
        diff |= usize::from(*expected_byte ^ provided_byte);
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_role_parse_is_trimmed_ascii_case_insensitive_and_closed() {
        for (text, expected) in [
            ("viewer", AccessRole::Viewer),
            (" VIEWER ", AccessRole::Viewer),
            ("\tOperator\n", AccessRole::Operator),
            ("EnGiNeEr", AccessRole::Engineer),
            ("admin", AccessRole::Admin),
        ] {
            assert_eq!(AccessRole::parse(text), Some(expected), "text={text:?}");
        }
        for text in ["", "view", "administrator", "engineer role", "viewer\0"] {
            assert_eq!(AccessRole::parse(text), None, "text={text:?}");
        }
    }

    #[test]
    fn access_role_display_and_serialization_use_lowercase_wire_tokens() {
        for (role, expected) in [
            (AccessRole::Viewer, "viewer"),
            (AccessRole::Operator, "operator"),
            (AccessRole::Engineer, "engineer"),
            (AccessRole::Admin, "admin"),
        ] {
            assert_eq!(role.as_str(), expected);
            assert_eq!(
                serde_json::to_value(role).expect("serialize role"),
                serde_json::json!(expected)
            );
            assert_eq!(
                serde_json::from_value::<AccessRole>(serde_json::json!(expected))
                    .expect("deserialize role"),
                role
            );
        }
    }

    #[test]
    fn access_role_allows_exact_total_order() {
        let roles = [
            AccessRole::Viewer,
            AccessRole::Operator,
            AccessRole::Engineer,
            AccessRole::Admin,
        ];
        for (actual_index, actual) in roles.into_iter().enumerate() {
            for (required_index, required) in roles.into_iter().enumerate() {
                assert_eq!(
                    actual.allows(required),
                    actual_index >= required_index,
                    "actual={actual:?}, required={required:?}"
                );
            }
        }
    }

    #[test]
    fn constant_time_secret_equality_requires_exact_bytes_and_length() {
        for (expected, provided, equal) in [
            ("", "", true),
            ("a", "a", true),
            ("secret", "secret", true),
            ("secret", "Secret", false),
            ("secret", "secret ", false),
            ("secret", "secre", false),
            ("secret", "secret-extra", false),
            ("åäö", "åäö", true),
            ("åäö", "åäo", false),
            ("a\0b", "a\0b", true),
            ("a\0b", "a\0c", false),
        ] {
            assert_eq!(
                constant_time_eq(expected, provided),
                equal,
                "expected={expected:?}, provided={provided:?}"
            );
        }
    }
}
