//! Terminal UI for runtime monitoring and control.

#![allow(missing_docs)]

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration as StdDuration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Sparkline, Wrap},
    Terminal,
};

use crate::bundle::detect_bundle_path;
use crate::bundle_builder::build_program_stbc;
use crate::config::RuntimeBundle;
use crate::control::ControlEndpoint;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde_json::json;

mod client;
mod commands;
mod input;
mod parsing;
mod render;
mod state;

#[cfg(test)]
mod tests;

use client::ControlClient;
use state::*;

const COLOR_TEAL: Color = Color::Rgb(0, 168, 150);
const COLOR_GREEN: Color = Color::Rgb(46, 204, 113);
const COLOR_AMBER: Color = Color::Rgb(243, 156, 18);
const COLOR_RED: Color = Color::Rgb(231, 76, 60);
const COLOR_INFO: Color = Color::Rgb(142, 142, 147);
const COLOR_YELLOW: Color = Color::Rgb(245, 196, 66);
const COLOR_CYAN: Color = Color::Rgb(64, 212, 255);
const COLOR_MAGENTA: Color = Color::Rgb(191, 90, 242);
const COLOR_PROMPT_BG: Color = Color::Rgb(24, 24, 24);

pub fn run_ui(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
    refresh_ms: u64,
    no_input: bool,
    beginner: bool,
) -> anyhow::Result<()> {
    let (endpoint, auth_token, bundle_root) = resolve_endpoint(bundle, endpoint, token)?;
    let console_config = bundle_root
        .as_ref()
        .map(|root| load_console_config(root))
        .unwrap_or_default();
    let layout = console_config.layout.unwrap_or_else(|| {
        vec![
            PanelKind::Cycle,
            PanelKind::Io,
            PanelKind::Status,
            PanelKind::Events,
        ]
    });
    let refresh_ms = if refresh_ms == 250 {
        console_config.refresh_ms.unwrap_or(refresh_ms)
    } else {
        refresh_ms
    };
    let mut state = UiState::new(layout, beginner, bundle_root);
    let mut client = ControlClient::connect(endpoint.clone(), auth_token.clone())?;
    let mut last_refresh = Instant::now();
    let refresh = StdDuration::from_millis(refresh_ms);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        loop {
            if last_refresh.elapsed() >= refresh {
                match fetch_data(&mut client) {
                    Ok(data) => {
                        if !state.connected {
                            push_alert(
                                &mut state,
                                "CONNECTED Control restored.",
                                Style::default().fg(COLOR_GREEN),
                            );
                        }
                        state.connected = true;
                        state.data = data;
                        if let Some(status) = state.data.status.as_ref() {
                            state.debug_controls = !state.beginner_mode && status.debug_enabled;
                        }
                        update_cycle_history(&mut state);
                        update_watch_values(&mut client, &mut state);
                        update_event_alerts(&mut state);
                    }
                    Err(_) => {
                        if state.connected {
                            push_alert(
                                &mut state,
                                "DISCONNECTED Reconnecting...",
                                Style::default().fg(COLOR_AMBER),
                            );
                        }
                        state.connected = false;
                        if let Ok(new_client) =
                            ControlClient::connect(endpoint.clone(), auth_token.clone())
                        {
                            client = new_client;
                        }
                    }
                }
                last_refresh = Instant::now();
            }

            terminal.draw(|frame| render_ui(frame.area(), frame, &state, no_input))?;

            if event::poll(StdDuration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if handle_key(key, &mut client, &mut state, no_input)? {
                        break;
                    }
                }
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn resolve_endpoint(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
) -> anyhow::Result<(ControlEndpoint, Option<String>, Option<PathBuf>)> {
    client::resolve_endpoint(bundle, endpoint, token)
}

fn load_console_config(root: &Path) -> ConsoleConfig {
    client::load_console_config(root)
}

fn fetch_data(client: &mut ControlClient) -> anyhow::Result<UiData> {
    client::fetch_data(client)
}

fn parse_status(response: &serde_json::Value) -> Option<StatusSnapshot> {
    parsing::parse_status(response)
}

fn parse_tasks(response: &serde_json::Value) -> Vec<TaskSnapshot> {
    parsing::parse_tasks(response)
}

fn parse_io(response: &serde_json::Value) -> Vec<IoEntry> {
    parsing::parse_io(response)
}

fn parse_events(response: &serde_json::Value) -> Vec<EventSnapshot> {
    parsing::parse_events(response)
}

fn parse_settings(response: &serde_json::Value) -> Option<SettingsSnapshot> {
    parsing::parse_settings(response)
}

fn handle_key(
    key: KeyEvent,
    client: &mut ControlClient,
    state: &mut UiState,
    no_input: bool,
) -> anyhow::Result<bool> {
    input::handle_key(key, client, state, no_input)
}

fn render_ui(area: Rect, frame: &mut ratatui::Frame<'_>, state: &UiState, no_input: bool) {
    render::render_ui(area, frame, state, no_input)
}

fn push_alert(state: &mut UiState, text: &str, style: Style) {
    state::push_alert(state, text, style);
}

fn update_cycle_history(state: &mut UiState) {
    state::update_cycle_history(state);
}

fn update_watch_values(client: &mut ControlClient, state: &mut UiState) {
    state::update_watch_values(client, state);
}

fn update_event_alerts(state: &mut UiState) {
    state::update_event_alerts(state);
}

fn handle_settings_select(
    input: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    commands::handle_settings_select(input, client, state)
}

fn handle_settings_value(
    input: &str,
    key: SettingKey,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    commands::handle_settings_value(input, key, client, state)
}

fn execute_command(
    input: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    commands::execute_command(input, client, state)
}

fn update_command_suggestions(state: &mut UiState) {
    commands::update_command_suggestions(state);
}

fn command_suggestions(state: &UiState, filter: Option<&str>) -> Vec<CommandHelp> {
    commands::command_suggestions(state, filter)
}

fn suggestion_lines(suggestions: &[CommandHelp], selected: Option<usize>) -> Vec<PromptLine> {
    commands::suggestion_lines(suggestions, selected)
}

fn move_settings_selection(state: &mut UiState, delta: i32) {
    commands::move_settings_selection(state, delta);
}

fn move_menu_selection(state: &mut UiState, kind: MenuKind, delta: i32) {
    commands::move_menu_selection(state, kind, delta);
}

fn move_io_selection(state: &mut UiState, action: IoActionKind, delta: i32) {
    commands::move_io_selection(state, action, delta);
}

fn move_io_value_selection(state: &mut UiState, delta: i32) {
    commands::move_io_value_selection(state, delta);
}

fn handle_menu_select(
    input: &str,
    kind: MenuKind,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    commands::handle_menu_select(input, kind, client, state)
}

fn handle_io_select(
    input: &str,
    action: IoActionKind,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    commands::handle_io_select(input, action, client, state)
}

fn handle_io_value_select(
    input: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    commands::handle_io_value_select(input, client, state)
}

fn open_menu(kind: MenuKind, state: &mut UiState) {
    commands::open_menu(kind, state);
}

fn open_io_select(action: IoActionKind, state: &mut UiState) {
    commands::open_io_select(action, state);
}

fn advance_panel_page(state: &mut UiState) {
    commands::advance_panel_page(state);
}

fn prompt_line_to_line(line: &PromptLine) -> Line<'_> {
    let spans = line
        .segments
        .iter()
        .map(|(text, style)| Span::styled(text.clone(), *style))
        .collect::<Vec<_>>();
    Line::from(spans)
}

fn panel_block(kind: PanelKind, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(COLOR_TEAL)
    } else {
        Style::default().fg(COLOR_INFO)
    };
    Block::default()
        .title(Span::styled(
            format!(" {} ", kind.title()),
            Style::default()
                .fg(COLOR_YELLOW)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(border_style)
}

fn label_style() -> Style {
    Style::default().fg(COLOR_CYAN)
}

fn header_style() -> Style {
    Style::default()
        .fg(COLOR_YELLOW)
        .add_modifier(Modifier::BOLD)
}

fn value_style() -> Style {
    Style::default().fg(Color::White)
}

fn label_value_line(label: &str, value: &str, width: usize, value_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<width$}"), label_style()),
        Span::raw(" "),
        Span::styled(value.to_string(), value_style),
    ])
}

fn seg(text: impl Into<String>, style: Style) -> (String, Style) {
    (text.into(), style)
}

fn status_chip(state: &str) -> (String, Style) {
    let upper = state.trim().to_ascii_uppercase();
    let (bg, fg) = match upper.as_str() {
        "RUNNING" => (COLOR_TEAL, Color::White),
        "PAUSED" => (COLOR_AMBER, Color::Black),
        "FAULTED" => (COLOR_RED, Color::White),
        "STOPPED" => (Color::DarkGray, Color::White),
        _ => (Color::DarkGray, Color::White),
    };
    (
        format!("[{}]", upper),
        Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD),
    )
}

fn format_uptime(uptime_ms: u64) -> String {
    let secs = uptime_ms / 1000;
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}
