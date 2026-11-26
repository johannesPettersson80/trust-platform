//! Shared styling helpers for CLI output.

use std::io::IsTerminal;

use owo_colors::OwoColorize;

fn should_color() -> bool {
    std::io::stdout().is_terminal()
}

pub fn success(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    if should_color() {
        format!("{}", text.green())
    } else {
        text.to_string()
    }
}

pub fn warning(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    if should_color() {
        format!("{}", text.yellow())
    } else {
        text.to_string()
    }
}

pub fn error(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    if should_color() {
        format!("{}", text.red())
    } else {
        text.to_string()
    }
}

pub fn accent(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    if should_color() {
        format!("{}", text.cyan())
    } else {
        text.to_string()
    }
}

pub fn print_logo() {
    outln!(" _            ___ _____");
    outln!("| |_ _ _ _  _/ __|_   _|");
    outln!("|  _| '_| || \\__ \\ | |");
    outln!(" \\__|_|  \\_,_|___/ |_|");
    outln!();
}
