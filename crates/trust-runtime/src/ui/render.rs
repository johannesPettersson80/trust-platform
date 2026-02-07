use super::*;

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
        render_panel(area, frame, state, panel, true);
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
        render_panel(left[0], frame, state, panels[0], false);
        render_panel(right[0], frame, state, panels[1], false);
        render_panel(left[1], frame, state, panels[2], false);
        render_panel(right[1], frame, state, panels[3], false);
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
            render_panel(stack[0], frame, state, *panel, false);
        }
        if let Some(panel) = panels.get(start + 1) {
            render_panel(stack[1], frame, state, *panel, false);
        }
        return;
    }

    let panel = panels
        .get(state.panel_page % panels.len().max(1))
        .copied()
        .unwrap_or(PanelKind::Status);
    render_panel(area, frame, state, panel, false);
}
