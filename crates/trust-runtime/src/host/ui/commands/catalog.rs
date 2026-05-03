use super::*;
pub(super) fn help_lines(state: &UiState) -> Vec<PromptLine> {
    suggestion_lines(&command_suggestions(state, None), None)
}

pub(super) fn update_command_suggestions(state: &mut UiState) {
    if state.prompt.mode != PromptMode::Normal || !state.prompt.active {
        return;
    }
    let input = state.prompt.input.trim();
    if !input.starts_with('/') {
        state.prompt.clear_suggestions();
        return;
    }
    let query = input.trim_start_matches('/').trim();
    if query.contains(' ') {
        state.prompt.clear_suggestions();
        return;
    }
    let filter = if query.is_empty() { None } else { Some(query) };
    let suggestions = command_suggestions(state, filter);
    if suggestions.is_empty() {
        state.prompt.clear_suggestions();
        return;
    }
    state.prompt.set_suggestions_list(suggestions);
}

pub(super) fn command_suggestions(state: &UiState, filter: Option<&str>) -> Vec<CommandHelp> {
    let catalog = command_catalog(state.beginner_mode);
    catalog
        .into_iter()
        .filter(|entry| {
            if let Some(filter) = filter {
                entry.cmd.starts_with(filter)
            } else {
                true
            }
        })
        .collect()
}

pub(super) fn suggestion_lines(
    suggestions: &[CommandHelp],
    selected: Option<usize>,
) -> Vec<PromptLine> {
    let mut lines = Vec::new();
    lines.push(PromptLine::plain("Commands:", header_style()));
    if suggestions.is_empty() {
        lines.push(PromptLine::plain(
            "No matches.",
            Style::default().fg(COLOR_INFO),
        ));
        return lines;
    }
    for (idx, entry) in suggestions.iter().enumerate() {
        let is_selected = selected == Some(idx);
        if is_selected {
            let style = Style::default()
                .bg(COLOR_TEAL)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD);
            lines.push(PromptLine::from_segments(vec![
                seg(format!("/{:<8}", entry.cmd), style),
                seg(entry.desc, style),
            ]));
        } else {
            lines.push(PromptLine::from_segments(vec![
                seg(
                    format!("/{:<8}", entry.cmd),
                    Style::default().fg(COLOR_CYAN),
                ),
                seg(entry.desc, value_style()),
            ]));
        }
    }
    lines
}

pub(super) fn command_catalog(beginner: bool) -> Vec<CommandHelp> {
    let mut entries = vec![
        CommandHelp {
            cmd: "help",
            desc: "Show all commands",
            beginner: true,
        },
        CommandHelp {
            cmd: "status",
            desc: "Show runtime status",
            beginner: true,
        },
        CommandHelp {
            cmd: "settings",
            desc: "Open settings menu",
            beginner: true,
        },
        CommandHelp {
            cmd: "io",
            desc: "I/O menu (read/write/force)",
            beginner: true,
        },
        CommandHelp {
            cmd: "control",
            desc: "Pause, resume, restart",
            beginner: true,
        },
        CommandHelp {
            cmd: "info",
            desc: "Show version, uptime",
            beginner: true,
        },
        CommandHelp {
            cmd: "exit",
            desc: "Leave console",
            beginner: true,
        },
        CommandHelp {
            cmd: "access",
            desc: "Access PLC tokens",
            beginner: false,
        },
        CommandHelp {
            cmd: "linking",
            desc: "PLC Linking (mesh)",
            beginner: false,
        },
        CommandHelp {
            cmd: "watch",
            desc: "Watch variable",
            beginner: false,
        },
        CommandHelp {
            cmd: "log",
            desc: "Show/set log level",
            beginner: false,
        },
        CommandHelp {
            cmd: "build",
            desc: "Recompile sources",
            beginner: false,
        },
        CommandHelp {
            cmd: "reload",
            desc: "Reload program bytecode",
            beginner: false,
        },
        CommandHelp {
            cmd: "layout",
            desc: "Set panel layout",
            beginner: false,
        },
        CommandHelp {
            cmd: "focus",
            desc: "Focus a panel",
            beginner: false,
        },
        CommandHelp {
            cmd: "unfocus",
            desc: "Return to grid view",
            beginner: false,
        },
        CommandHelp {
            cmd: "clear",
            desc: "Clear prompt output",
            beginner: false,
        },
    ];
    if beginner {
        entries.retain(|entry| entry.beginner);
    }
    entries
}

pub(super) fn is_beginner_command(head: &str) -> bool {
    matches!(
        head,
        "help" | "status" | "settings" | "io" | "control" | "info" | "exit"
    )
}

pub(super) fn status_lines(state: &UiState) -> Vec<PromptLine> {
    let status = state.data.status.clone().unwrap_or_default();
    let uptime = format_uptime(status.uptime_ms);
    let chip = status_chip(status.state.as_str());
    let line = PromptLine::from_segments(vec![
        seg(chip.0, chip.1),
        seg(format!(" {}  ", status.resource), Style::default()),
        seg("Cycle: ", label_style()),
        seg(format!("{:.1}ms  ", status.cycle_last), value_style()),
        seg("Uptime: ", label_style()),
        seg(uptime, value_style()),
    ]);
    let web = state
        .data
        .settings
        .as_ref()
        .map(|s| format!("http://{}", s.web_listen))
        .unwrap_or_else(|| "--".to_string());
    let mode = if status.simulation_mode.is_empty() {
        "production".to_string()
    } else {
        format!(
            "{} x{}",
            status.simulation_mode, status.simulation_time_scale
        )
    };
    let line2 = PromptLine::from_segments(vec![
        seg("I/O: ", label_style()),
        seg(
            status
                .drivers
                .first()
                .map(|d| d.name.as_str())
                .unwrap_or("unknown"),
            value_style(),
        ),
        seg("  Web: ", label_style()),
        seg(web, value_style()),
        seg("  Mode: ", label_style()),
        seg(mode, value_style()),
    ]);
    if status.simulation_mode.eq_ignore_ascii_case("simulation")
        && !status.simulation_warning.is_empty()
    {
        let warning = PromptLine::from_segments(vec![
            seg("Warning: ", Style::default().fg(COLOR_AMBER)),
            seg(status.simulation_warning, Style::default().fg(COLOR_AMBER)),
        ]);
        vec![line, line2, warning]
    } else {
        vec![line, line2]
    }
}

pub(super) fn info_lines(state: &UiState) -> Vec<PromptLine> {
    let uptime = state
        .data
        .status
        .as_ref()
        .map(|s| format_uptime(s.uptime_ms))
        .unwrap_or_else(|| "--:--:--".to_string());
    vec![
        PromptLine::from_segments(vec![
            seg("Version: ", label_style()),
            seg(env!("CARGO_PKG_VERSION"), value_style()),
        ]),
        PromptLine::from_segments(vec![
            seg("Uptime: ", label_style()),
            seg(uptime, value_style()),
        ]),
    ]
}
