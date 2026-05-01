//! Prompt helpers for developer/workbench commands.

use dialoguer::{theme::ColorfulTheme, Confirm, Input};

pub(crate) fn prompt_string(label: &str, default: &str) -> anyhow::Result<String> {
    Ok(Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt(label)
        .default(default.to_string())
        .interact_text()?)
}

pub(crate) fn prompt_yes_no(label: &str, default: bool) -> anyhow::Result<bool> {
    Ok(Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(label)
        .default(default)
        .interact()?)
}
