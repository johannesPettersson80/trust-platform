use super::*;

mod panels;

pub(super) fn render_ui(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    no_input: bool,
) {
    let mut prompt_height = (state.prompt.output.len() + state.alerts.len() + 1) as u16;
    let is_menu = matches!(
        state.prompt.mode,
        PromptMode::SettingsSelect
            | PromptMode::Menu(_)
            | PromptMode::IoSelect(_)
            | PromptMode::IoValueSelect
    );
    let max_prompt = if is_menu { 14 } else { 8 };
    if prompt_height < 3 {
        prompt_height = 3;
    }
    if prompt_height > max_prompt {
        prompt_height = max_prompt;
    }
    let min_panel_height = 8;
    if prompt_height + min_panel_height >= area.height {
        prompt_height = area.height.saturating_sub(min_panel_height).max(3);
    }
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(area.height.saturating_sub(prompt_height)),
            Constraint::Length(prompt_height),
        ])
        .split(area);
    render_panels(layout[0], frame, state);
    render_prompt(layout[1], frame, state, no_input);
}

pub(super) fn render_panels(area: Rect, frame: &mut ratatui::Frame<'_>, state: &UiState) {
    if let Some(panel) = state.focus {
        panels::render_panel(area, frame, state, panel, true);
        return;
    }
    let width = area.width;
    let panels = state.layout.as_slice();
    if width >= 120 && panels.len() >= 4 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(cols[0]);
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(cols[1]);
        panels::render_panel(left[0], frame, state, panels[0], false);
        panels::render_panel(right[0], frame, state, panels[1], false);
        panels::render_panel(left[1], frame, state, panels[2], false);
        panels::render_panel(right[1], frame, state, panels[3], false);
        return;
    }

    if width >= 80 {
        let pages = panels.len().div_ceil(2);
        let page = state.panel_page % pages.max(1);
        let start = page * 2;
        let stack = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        if let Some(panel) = panels.get(start) {
            panels::render_panel(stack[0], frame, state, *panel, false);
        }
        if let Some(panel) = panels.get(start + 1) {
            panels::render_panel(stack[1], frame, state, *panel, false);
        }
        return;
    }

    let panel = panels
        .get(state.panel_page % panels.len().max(1))
        .copied()
        .unwrap_or(PanelKind::Status);
    panels::render_panel(area, frame, state, panel, false);
}

pub(super) fn render_prompt(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    no_input: bool,
) {
    let mut lines: Vec<Line> = Vec::new();
    for alert in state.alerts.iter().take(3) {
        lines.push(prompt_line_to_line(alert));
    }
    for line in state.prompt.output.iter() {
        lines.push(prompt_line_to_line(line));
    }
    let output_height = area.height.saturating_sub(1);
    let output_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: output_height,
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), output_area);

    let prompt_area = Rect {
        x: area.x,
        y: area.y + output_height,
        width: area.width,
        height: 1,
    };
    if no_input {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Read-only mode",
                Style::default().fg(COLOR_INFO),
            ))),
            prompt_area,
        );
        return;
    }
    if state.prompt.active {
        let prompt = Line::from(vec![
            Span::styled(
                "> ",
                Style::default().fg(COLOR_TEAL).add_modifier(Modifier::BOLD),
            ),
            Span::raw(state.prompt.input.clone()),
        ]);
        frame.render_widget(
            Paragraph::new(prompt).style(Style::default().bg(COLOR_PROMPT_BG)),
            prompt_area,
        );
        frame.set_cursor_position((
            prompt_area.x + 2 + state.prompt.cursor as u16,
            prompt_area.y,
        ));
    } else {
        let hint = Line::from(Span::styled(
            "Press / to type command",
            Style::default()
                .fg(COLOR_INFO)
                .add_modifier(Modifier::DIM)
                .bg(COLOR_PROMPT_BG),
        ));
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().bg(COLOR_PROMPT_BG)),
            prompt_area,
        );
    }
}
