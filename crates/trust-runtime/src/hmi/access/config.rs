//! `[runtime.web.hmi]` (specification 39, §12.8): users, session times and the write policy.

use std::time::Duration;

use serde::Deserialize;
use smol_str::SmolStr;

use super::password::PasswordHash;
use crate::hmi::member_paths::{MemberPath, PathSegment};
use crate::security::AccessRole;

/// The section as written.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiSection {
    /// Minutes without a command or interaction before a session ends.
    pub idle_minutes: Option<u64>,
    /// Hours before a session ends in any case.
    pub session_hours: Option<u64>,
    /// `[[runtime.web.hmi.user]]` entries.
    #[serde(default)]
    pub user: Vec<UserSection>,
    /// `[[runtime.web.hmi.write]]` entries.
    #[serde(default)]
    pub write: Vec<WriteSection>,
}

/// A user as written.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSection {
    /// The name the user logs in with.
    pub name: String,
    /// `viewer`, `operator` or `engineer`.
    pub role: String,
    /// The stored hash from `trust-runtime hmi-password`.
    pub password: String,
}

/// A write-policy entry as written.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteSection {
    /// A runtime instance path, or `path.*` for every member below it.
    pub path: String,
    /// The lowest role that may write: `operator` or `engineer`.
    pub role: String,
    /// The lowest number that may be written.
    pub min: Option<f64>,
    /// The highest number that may be written.
    pub max: Option<f64>,
}

/// Users, session times and the write policy, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct HmiAccessConfig {
    /// A session ends after this long without a command or interaction.
    pub idle: Duration,
    /// A session ends after this long in any case.
    pub session: Duration,
    /// Who may log in.
    pub users: Vec<HmiUser>,
    /// What may be written; nothing else may (deny by default).
    pub write: Vec<WriteRule>,
}

impl Default for HmiAccessConfig {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(15 * 60),
            session: Duration::from_secs(12 * 60 * 60),
            users: Vec::new(),
            write: Vec::new(),
        }
    }
}

/// A user who may log in to the runtime HMI.
#[derive(Debug, Clone, PartialEq)]
pub struct HmiUser {
    /// The name the user logs in with.
    pub name: SmolStr,
    /// What the user may do.
    pub role: AccessRole,
    /// The stored password hash.
    pub password: PasswordHash,
}

/// One entry of the write policy.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteRule {
    /// The point, or the point whose members the rule covers.
    pub path: MemberPath,
    /// `path.*`: every member below the path, not the path itself.
    pub below: bool,
    /// The lowest role that may write.
    pub role: AccessRole,
    /// The lowest number that may be written.
    pub min: Option<f64>,
    /// The highest number that may be written.
    pub max: Option<f64>,
}

impl WriteRule {
    /// Whether the rule covers a point; identifiers compare without regard to case.
    pub fn covers(&self, path: &MemberPath) -> bool {
        let rule = &self.path;
        let longer = if self.below {
            path.segments.len() > rule.segments.len()
        } else {
            path.segments.len() == rule.segments.len()
        };
        longer
            && crate::hmi::remote_values::qualified_key(&path.root)
                == crate::hmi::remote_values::qualified_key(&rule.root)
            && rule
                .segments
                .iter()
                .zip(&path.segments)
                .all(|(wanted, step)| same_step(wanted, step))
    }
}

fn same_step(left: &PathSegment, right: &PathSegment) -> bool {
    match (left, right) {
        (PathSegment::Field(left), PathSegment::Field(right)) => left.eq_ignore_ascii_case(right),
        (PathSegment::Index(left), PathSegment::Index(right)) => left == right,
        _ => false,
    }
}

const KEY: &str = "runtime.web.hmi";

impl HmiAccessConfig {
    /// Checks the section; the error names the offending key.
    pub fn from_section(section: HmiSection) -> Result<Self, String> {
        let defaults = Self::default();
        let idle = match section.idle_minutes {
            Some(0) => return Err(format!("{KEY}.idle_minutes must be at least 1")),
            Some(minutes) => Duration::from_secs(minutes.saturating_mul(60)),
            None => defaults.idle,
        };
        let session = match section.session_hours {
            Some(0) => return Err(format!("{KEY}.session_hours must be at least 1")),
            Some(hours) => Duration::from_secs(hours.saturating_mul(60 * 60)),
            None => defaults.session,
        };
        let mut users: Vec<HmiUser> = Vec::with_capacity(section.user.len());
        for user in section.user {
            let name = user.name.trim();
            if name.is_empty() {
                return Err(format!("{KEY}.user.name must not be empty"));
            }
            if users
                .iter()
                .any(|known| known.name.eq_ignore_ascii_case(name))
            {
                return Err(format!("{KEY}.user \"{name}\" is listed twice"));
            }
            let role = match AccessRole::parse(&user.role) {
                Some(role @ (AccessRole::Viewer | AccessRole::Operator | AccessRole::Engineer)) => {
                    role
                }
                _ => {
                    return Err(format!(
                    "{KEY}.user \"{name}\": role must be viewer, operator or engineer, not \"{}\"",
                    user.role
                ))
                }
            };
            let password = PasswordHash::parse(&user.password)
                .map_err(|error| format!("{KEY}.user \"{name}\": password: {error}"))?;
            users.push(HmiUser {
                name: SmolStr::new(name),
                role,
                password,
            });
        }
        let mut write = Vec::with_capacity(section.write.len());
        for rule in section.write {
            let text = rule.path.trim();
            let (prefix, below) = match text.strip_suffix(".*") {
                Some(prefix) => (prefix, true),
                None => (text, false),
            };
            let path = MemberPath::parse(prefix)
                .map_err(|error| format!("{KEY}.write.path \"{text}\": {error}"))?;
            let role = match AccessRole::parse(&rule.role) {
                Some(role @ (AccessRole::Operator | AccessRole::Engineer)) => role,
                _ => {
                    return Err(format!(
                        "{KEY}.write \"{text}\": role must be operator or engineer, not \"{}\"",
                        rule.role
                    ))
                }
            };
            for (name, limit) in [("min", rule.min), ("max", rule.max)] {
                if limit.is_some_and(|limit| !limit.is_finite()) {
                    return Err(format!(
                        "{KEY}.write \"{text}\": {name} must be a finite number"
                    ));
                }
            }
            if let (Some(min), Some(max)) = (rule.min, rule.max) {
                if min > max {
                    return Err(format!(
                        "{KEY}.write \"{text}\": min {min} is above max {max}"
                    ));
                }
            }
            write.push(WriteRule {
                path,
                below,
                role,
                min: rule.min,
                max: rule.max,
            });
        }
        Ok(Self {
            idle,
            session,
            users,
            write,
        })
    }
}

/// The `[runtime.web.hmi]` part of a project's `runtime.toml`, for the editor's checks; a file
/// without the section has no users and no write policy.
pub fn hmi_access_from_runtime_toml(text: &str) -> Result<HmiAccessConfig, String> {
    let document: toml::Value =
        toml::from_str(text).map_err(|error| format!("runtime.toml: {error}"))?;
    let section = document
        .get("runtime")
        .and_then(|runtime| runtime.get("web"))
        .and_then(|web| web.get("hmi"));
    let section: HmiSection = match section {
        Some(section) => section
            .clone()
            .try_into()
            .map_err(|error| format!("{KEY}: {error}"))?,
        None => HmiSection::default(),
    };
    HmiAccessConfig::from_section(section)
}
