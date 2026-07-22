//! Interactive prompt helpers for CLI flows.

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

fn use_dialoguer() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

pub(crate) fn prompt_path(label: &str, default: &Path) -> anyhow::Result<PathBuf> {
    let default_text = default.display().to_string();
    let input = prompt_string(label, &default_text)?;
    Ok(PathBuf::from(input))
}

pub(crate) fn prompt_string(label: &str, default: &str) -> anyhow::Result<String> {
    if use_dialoguer() {
        let theme = ColorfulTheme::default();
        let input = Input::<String>::with_theme(&theme)
            .with_prompt(label)
            .default(default.to_string())
            .interact_text()?;
        return Ok(input);
    }
    let prompt = format!("{label} [{default}]: ");
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn prompt_choice(
    label: &str,
    options: &[&str],
    default: &str,
) -> anyhow::Result<String> {
    if use_dialoguer() {
        let theme = ColorfulTheme::default();
        let default_index = options
            .iter()
            .position(|opt| opt.eq_ignore_ascii_case(default))
            .unwrap_or(0);
        let selection = Select::with_theme(&theme)
            .with_prompt(label)
            .items(options)
            .default(default_index)
            .interact()?;
        return Ok(options[selection].to_string());
    }
    let options_text = options.join("/");
    let prompt = format!("{label} ({options_text}) [{default}]: ");
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let trimmed = line.trim();
    let choice = if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    };
    if options
        .iter()
        .any(|opt| opt.eq_ignore_ascii_case(choice.as_str()))
    {
        Ok(choice)
    } else {
        anyhow::bail!(
            "Invalid choice '{choice}'. Expected: {}. Tip: run trust-runtime wizard to reconfigure.",
            options.join(", ")
        );
    }
}

pub(crate) fn prompt_u64(label: &str, default: u64) -> anyhow::Result<u64> {
    let input = prompt_string(label, &default.to_string())?;
    parse_prompt_u64(label, input.as_str())
}

fn parse_prompt_u64(label: &str, input: &str) -> anyhow::Result<u64> {
    input
        .parse::<u64>()
        .map_err(|err| anyhow::anyhow!("{label} must be a number: {err}"))
}

pub(crate) fn prompt_yes_no(label: &str, default: bool) -> anyhow::Result<bool> {
    if use_dialoguer() {
        let theme = ColorfulTheme::default();
        let confirmed = Confirm::with_theme(&theme)
            .with_prompt(label)
            .default(default)
            .interact()?;
        return Ok(confirmed);
    }
    let default_text = if default { "Y/n" } else { "y/N" };
    let prompt = format!("{label} [{default_text}]: ");
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let trimmed = line.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Ok(default);
    }
    match trimmed.as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => anyhow::bail!("Please answer yes or no."),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_prompt_u64;

    #[test]
    fn numeric_prompt_parser_accepts_the_complete_u64_range() {
        assert_eq!(
            parse_prompt_u64("Cycle time", "18446744073709551615").expect("parse maximum u64"),
            u64::MAX
        );
    }

    #[test]
    fn numeric_prompt_parser_rejects_negative_non_numeric_and_overflow_values() {
        for invalid in ["-1", "1.5", "18446744073709551616"] {
            let error = parse_prompt_u64("Cycle time", invalid)
                .expect_err("invalid numeric prompt input must fail");
            assert!(
                error
                    .to_string()
                    .starts_with("Cycle time must be a number:"),
                "unexpected error: {error:#}"
            );
        }
    }
}
