//! The runtime display's users in `runtime.toml` (specification 39, §12.8 item 1): Screen settings
//! list them, **Add user…** asks a name no other user has whatever its case, a role and a password
//! of at least 8 characters, and appends the user with the hash `trust-runtime hmi-password` makes;
//! **Remove** deletes the user's table. The password itself never reaches this command: the
//! extension sends its length in characters for the check and the hash for the file.
//!
//! - `list { runtimeToml }` → `{ users: [{ name, role }] }`.
//! - `checkName { runtimeToml, name }` → `{ problem }`: why the name cannot be added, or null.
//! - `checkPassword { characters }` → `{ problem }`: why a password that long is refused, or null.
//! - `add { runtimeToml, name, role, hash }` → `{ text }`: `runtime.toml` with the user appended.
//! - `remove { runtimeToml, name }` → `{ text }`: `runtime.toml` without the user's table.

use serde::Deserialize;
use serde_json::{json, Value};
use toml_edit::{value, Table};
use trust_runtime::hmi::access::password::PasswordHash;

use super::decode;
use super::runtime_toml::{self, Entries};

/// The fewest characters a password may have (§12.8 item 1).
const PASSWORD_CHARACTERS: usize = 8;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    List {
        #[serde(rename = "runtimeToml")]
        runtime_toml: String,
    },
    CheckName {
        #[serde(rename = "runtimeToml")]
        runtime_toml: String,
        name: String,
    },
    CheckPassword {
        characters: usize,
    },
    Add {
        #[serde(rename = "runtimeToml")]
        runtime_toml: String,
        name: String,
        role: UserRole,
        hash: String,
    },
    Remove {
        #[serde(rename = "runtimeToml")]
        runtime_toml: String,
        name: String,
    },
}

/// What a user may do (§12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum UserRole {
    Viewer,
    Operator,
    Engineer,
}

impl UserRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Operator => "operator",
            Self::Engineer => "engineer",
        }
    }
}

/// The answer of `trust-lsp.hmiUsers`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::List { runtime_toml } => {
            let users: Vec<Value> = users(&runtime_toml)?
                .into_iter()
                .map(|(name, role)| json!({ "name": name, "role": role }))
                .collect();
            Ok(json!({ "ok": true, "users": users }))
        }
        Args::CheckName { runtime_toml, name } => Ok(json!({
            "ok": true,
            "problem": name_problem(&runtime_toml, &name)?,
        })),
        Args::CheckPassword { characters } => Ok(json!({
            "ok": true,
            "problem": password_problem(characters),
        })),
        Args::Add {
            runtime_toml,
            name,
            role,
            hash,
        } => Ok(json!({ "ok": true, "text": add(&runtime_toml, &name, role, &hash)? })),
        Args::Remove { runtime_toml, name } => Ok(json!({
            "ok": true,
            "text": runtime_toml::without(&runtime_toml, Entries::Users, |table| {
                runtime_toml::text_of(table, "name")
                    .is_some_and(|known| known.to_lowercase() == name.to_lowercase())
            })?,
        })),
    }
}

/// The users `runtime.toml` lists, each its name and role as written (empty when missing), in
/// the file's order.
fn users(text: &str) -> Result<Vec<(String, String)>, String> {
    let document = runtime_toml::parse(text)?;
    Ok(runtime_toml::tables(&document, Entries::Users)?
        .into_iter()
        .map(|table| {
            let field = |key| {
                runtime_toml::text_of(table, key)
                    .unwrap_or_default()
                    .to_owned()
            };
            (field("name"), field("role"))
        })
        .collect())
}

/// Why `name` cannot be added: it is empty, or another user has it whatever its case.
fn name_problem(text: &str, name: &str) -> Result<Option<&'static str>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(Some("Give the user a name"));
    }
    let taken = users(text)?
        .iter()
        .any(|(known, _)| known.to_lowercase() == name.to_lowercase());
    Ok(taken.then_some("A user of this name exists already"))
}

/// Why a password of `characters` characters is refused.
fn password_problem(characters: usize) -> Option<&'static str> {
    (characters < PASSWORD_CHARACTERS).then_some("Use at least 8 characters")
}

/// `runtime.toml` with the user appended: the name trimmed, the role, and the stored hash, which
/// must be one `trust-runtime hmi-password` makes.
fn add(text: &str, name: &str, role: UserRole, hash: &str) -> Result<String, String> {
    if let Some(problem) = name_problem(text, name)? {
        return Err(problem.to_owned());
    }
    PasswordHash::parse(hash).map_err(|_| {
        "The password's hash is not one trust-runtime hmi-password makes.".to_owned()
    })?;
    let mut table = Table::new();
    table.insert("name", value(name.trim()));
    table.insert("role", value(role.as_str()));
    table.insert("password", value(hash));
    runtime_toml::appended(text, Entries::Users, vec![table])
}

#[cfg(test)]
#[path = "users_tests.rs"]
mod tests;
