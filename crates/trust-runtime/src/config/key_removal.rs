//! Removing one key from a `runtime.toml` text, keeping every other line and comment. The web IDE
//! settings panel uses it to return an optional setting to its default instead of writing a value:
//! an empty Control Endpoint field removes `[runtime.control] endpoint`, so the platform default
//! applies (spec 11, section 6.8.0a), and an empty control or mesh auth token removes that token,
//! which the runtime refuses as an empty string.

use toml_edit::{DocumentMut, Item, TableLike};

use crate::error::RuntimeError;

/// `text` without `key` in the table `section`, a dotted table name such as `runtime.control`. A
/// missing table or key leaves the text as it is.
///
/// # Errors
///
/// Returns an error when `text` is not valid TOML.
pub fn remove_runtime_toml_key(
    text: &str,
    section: &str,
    key: &str,
) -> Result<String, RuntimeError> {
    let mut document = text.parse::<DocumentMut>().map_err(|error| {
        RuntimeError::InvalidConfig(format!("runtime.toml does not parse: {error}").into())
    })?;
    let mut table: &mut dyn TableLike = document.as_table_mut();
    for part in section.split('.') {
        let Some(next) = table.get_mut(part).and_then(Item::as_table_like_mut) else {
            return Ok(text.to_string());
        };
        table = next;
    }
    if table.remove(key).is_none() {
        return Ok(text.to_string());
    }
    Ok(document.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNTIME_TOML: &str = r#"# Plant line A
[resource]
name = "LineA" # the PLC name

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"

[runtime.log]
level = "info"
"#;

    #[test]
    fn the_key_is_removed_and_every_other_line_and_comment_is_kept() {
        let text =
            remove_runtime_toml_key(RUNTIME_TOML, "runtime.control", "endpoint").expect("remove");

        assert_eq!(
            text,
            RUNTIME_TOML.replace("endpoint = \"unix:///tmp/trust-runtime.sock\"\n", "")
        );
    }

    #[test]
    fn a_missing_key_or_table_leaves_the_text_as_it_is() {
        for (section, key) in [
            ("runtime.control", "auth_token"),
            ("runtime.web", "listen"),
            ("runtime.log.level", "x"),
        ] {
            assert_eq!(
                remove_runtime_toml_key(RUNTIME_TOML, section, key).expect("remove"),
                RUNTIME_TOML,
                "[{section}] {key}"
            );
        }
    }

    /// Reset writes an empty value for each token field; removing the key instead keeps the file
    /// valid, because the runtime refuses an empty `auth_token`.
    #[test]
    fn removing_empty_control_and_mesh_tokens_leaves_a_valid_runtime_toml() {
        let template =
            crate::bundle_template::render_runtime_toml(&smol_str::SmolStr::new("Line_A"), 20);
        let with_empty_tokens = template
            .replacen(
                "[runtime.control]\n",
                "[runtime.control]\nauth_token = \"\"\n",
                1,
            )
            .replacen("[runtime.mesh]\n", "[runtime.mesh]\nauth_token = \"\"\n", 1);
        assert_ne!(with_empty_tokens, template, "the fixture adds both tokens");
        assert!(
            crate::config::validate_runtime_toml_text(&with_empty_tokens).is_err(),
            "the runtime refuses an empty auth_token"
        );

        let without_control =
            remove_runtime_toml_key(&with_empty_tokens, "runtime.control", "auth_token")
                .expect("remove the control token");
        let without_tokens =
            remove_runtime_toml_key(&without_control, "runtime.mesh", "auth_token")
                .expect("remove the mesh token");

        assert_eq!(without_tokens, template);
        crate::config::validate_runtime_toml_text(&without_tokens)
            .expect("the runtime.toml without empty tokens is valid");
    }

    #[test]
    fn a_text_that_is_not_toml_is_an_error() {
        assert!(remove_runtime_toml_key("not = [valid", "runtime.control", "endpoint").is_err());
    }
}
