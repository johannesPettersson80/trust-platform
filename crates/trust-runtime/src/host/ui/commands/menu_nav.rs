use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct MenuEntry {
    pub(super) label: &'static str,
    pub(super) command: &'static str,
    pub(super) needs_input: bool,
}
fn menu_title(kind: MenuKind) -> &'static str {
    match kind {
        MenuKind::Io => "I/O Menu",
        MenuKind::Control => "Control Menu",
        MenuKind::Access => "Access Menu",
        MenuKind::Linking => "PLC Linking Menu",
        MenuKind::Log => "Log Menu",
        MenuKind::Restart => "Restart Required",
    }
}

pub(super) fn menu_entries(kind: MenuKind) -> Vec<MenuEntry> {
    match kind {
        MenuKind::Io => vec![
            MenuEntry {
                label: "Read value",
                command: "/io read",
                needs_input: true,
            },
            MenuEntry {
                label: "Set value",
                command: "/io set",
                needs_input: true,
            },
            MenuEntry {
                label: "Force value",
                command: "/io force",
                needs_input: true,
            },
            MenuEntry {
                label: "Release force",
                command: "/io unforce",
                needs_input: true,
            },
            MenuEntry {
                label: "List all I/O",
                command: "/io list",
                needs_input: false,
            },
            MenuEntry {
                label: "List forced",
                command: "/io forced",
                needs_input: false,
            },
            MenuEntry {
                label: "Back",
                command: "",
                needs_input: false,
            },
        ],
        MenuKind::Control => vec![
            MenuEntry {
                label: "Pause",
                command: "/control pause",
                needs_input: false,
            },
            MenuEntry {
                label: "Resume",
                command: "/control resume",
                needs_input: false,
            },
            MenuEntry {
                label: "Step into",
                command: "/control step",
                needs_input: false,
            },
            MenuEntry {
                label: "Step over",
                command: "/control step-over",
                needs_input: false,
            },
            MenuEntry {
                label: "Step out",
                command: "/control step-out",
                needs_input: false,
            },
            MenuEntry {
                label: "Restart (warm/cold)",
                command: "/control restart",
                needs_input: true,
            },
            MenuEntry {
                label: "Shutdown",
                command: "/control shutdown",
                needs_input: false,
            },
            MenuEntry {
                label: "Set breakpoint",
                command: "/control break",
                needs_input: true,
            },
            MenuEntry {
                label: "List breakpoints",
                command: "/control breaks",
                needs_input: false,
            },
            MenuEntry {
                label: "Delete breakpoint",
                command: "/control delete",
                needs_input: true,
            },
            MenuEntry {
                label: "Back",
                command: "",
                needs_input: false,
            },
        ],
        MenuKind::Access => vec![
            MenuEntry {
                label: "Generate access code",
                command: "/access start",
                needs_input: false,
            },
            MenuEntry {
                label: "Claim access code",
                command: "/access claim",
                needs_input: true,
            },
            MenuEntry {
                label: "List tokens",
                command: "/access list",
                needs_input: false,
            },
            MenuEntry {
                label: "Revoke token",
                command: "/access revoke",
                needs_input: true,
            },
            MenuEntry {
                label: "Back",
                command: "",
                needs_input: false,
            },
        ],
        MenuKind::Linking => vec![
            MenuEntry {
                label: "Enable linking",
                command: "/linking enable",
                needs_input: false,
            },
            MenuEntry {
                label: "Disable linking",
                command: "/linking disable",
                needs_input: false,
            },
            MenuEntry {
                label: "Publish variable",
                command: "/linking publish",
                needs_input: true,
            },
            MenuEntry {
                label: "Subscribe variable",
                command: "/linking subscribe",
                needs_input: true,
            },
            MenuEntry {
                label: "Back",
                command: "",
                needs_input: false,
            },
        ],
        MenuKind::Log => vec![
            MenuEntry {
                label: "Show level",
                command: "/log",
                needs_input: false,
            },
            MenuEntry {
                label: "Set info",
                command: "/log info",
                needs_input: false,
            },
            MenuEntry {
                label: "Set warn",
                command: "/log warn",
                needs_input: false,
            },
            MenuEntry {
                label: "Set debug",
                command: "/log debug",
                needs_input: false,
            },
            MenuEntry {
                label: "Tail logs",
                command: "/log tail",
                needs_input: true,
            },
            MenuEntry {
                label: "Back",
                command: "",
                needs_input: false,
            },
        ],
        MenuKind::Restart => vec![
            MenuEntry {
                label: "Restart now (warm)",
                command: "/control restart warm",
                needs_input: false,
            },
            MenuEntry {
                label: "Restart now (cold) — resets variables",
                command: "/control restart cold",
                needs_input: false,
            },
            MenuEntry {
                label: "Restart later",
                command: "",
                needs_input: false,
            },
        ],
    }
}

pub(super) fn menu_lines(kind: MenuKind, selected: usize) -> Vec<PromptLine> {
    let entries = menu_entries(kind);
    let mut lines = Vec::new();
    lines.push(PromptLine::plain(menu_title(kind), header_style()));
    if kind == MenuKind::Restart {
        lines.push(PromptLine::plain(
            "Saved. Restart required.",
            Style::default().fg(COLOR_AMBER),
        ));
    }
    if entries.is_empty() {
        lines.push(PromptLine::plain(
            "No options.",
            Style::default().fg(COLOR_INFO),
        ));
        return lines;
    }
    for (idx, entry) in entries.iter().enumerate() {
        if selected == idx {
            let style = Style::default()
                .bg(COLOR_TEAL)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD);
            let mut text = entry.label.to_string();
            if !entry.command.is_empty() {
                text.push(' ');
                text.push_str(entry.command);
            }
            lines.push(PromptLine::plain(text, style));
        } else {
            let mut segs = vec![seg(entry.label, value_style())];
            if !entry.command.is_empty() {
                segs.push(seg(" ", value_style()));
                segs.push(seg(entry.command, Style::default().fg(COLOR_CYAN)));
            }
            lines.push(PromptLine::from_segments(segs));
        }
    }
    lines.push(PromptLine::plain(
        "Use ↑/↓ and Enter. Esc to go back.",
        Style::default().fg(COLOR_INFO),
    ));
    lines
}

pub(super) fn move_menu_selection(state: &mut UiState, kind: MenuKind, delta: i32) {
    let entries = menu_entries(kind);
    let len = entries.len();
    if len == 0 {
        return;
    }
    let mut next = state.menu_index as i32 + delta;
    if next < 0 {
        next = len as i32 - 1;
    } else if next >= len as i32 {
        next = 0;
    }
    state.menu_index = next as usize;
    state.prompt.set_output(menu_lines(kind, state.menu_index));
}

pub(super) fn open_menu(kind: MenuKind, state: &mut UiState) {
    state.prompt.mode = PromptMode::Menu(kind);
    state.menu_index = 0;
    state.prompt.set_output(menu_lines(kind, state.menu_index));
    state.prompt.activate_with("");
}
pub(super) fn advance_panel_page(state: &mut UiState) {
    let len = state.layout.len().max(1);
    state.panel_page = (state.panel_page + 1) % len;
}
