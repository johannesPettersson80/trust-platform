use super::*;

pub(super) fn render_panel(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    panel: PanelKind,
    focused: bool,
) {
    match panel {
        PanelKind::Cycle => render_cycle_panel(area, frame, state, focused),
        PanelKind::Io => render_io_panel(area, frame, state, focused),
        PanelKind::Status => render_status_panel(area, frame, state, focused),
        PanelKind::Events => render_events_panel(area, frame, state, focused),
        PanelKind::Tasks => render_tasks_panel(area, frame, state, focused),
        PanelKind::Watch => render_watch_panel(area, frame, state, focused),
    }
}

pub(super) fn render_cycle_panel(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    focused: bool,
) {
    let status = state.data.status.clone().unwrap_or_default();
    let block = panel_block(PanelKind::Cycle, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut data: Vec<u64> = state.cycle_history.iter().copied().collect();
    if data.is_empty() {
        data.push(0);
    }
    let spark_height = inner.height.saturating_sub(1);
    let spark_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: spark_height,
    };
    frame.render_widget(
        Sparkline::default()
            .data(&data)
            .style(Style::default().fg(COLOR_TEAL)),
        spark_area,
    );
    let stats = Line::from(vec![
        Span::styled("min ", label_style()),
        Span::styled(format!("{:.1}ms  ", status.cycle_min), value_style()),
        Span::styled("avg ", label_style()),
        Span::styled(format!("{:.1}ms  ", status.cycle_avg), value_style()),
        Span::styled("max ", label_style()),
        Span::styled(format!("{:.1}ms  ", status.cycle_max), value_style()),
        Span::styled("last ", label_style()),
        Span::styled(format!("{:.1}ms", status.cycle_last), value_style()),
    ]);
    let stats_area = Rect {
        x: inner.x,
        y: inner.y + spark_height,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(stats), stats_area);
}

pub(super) fn render_io_panel(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    focused: bool,
) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(format!("{:<4}", "DIR"), header_style()),
        Span::raw(" "),
        Span::styled(format!("{:<12}", "NAME"), header_style()),
        Span::raw(" "),
        Span::styled(format!("{:<8}", "ADDR"), header_style()),
        Span::raw(" "),
        Span::styled(format!("{:<10}", "VALUE"), header_style()),
        Span::raw(" "),
        Span::styled("F", header_style()),
    ]));
    for entry in state
        .data
        .io
        .iter()
        .take(area.height.saturating_sub(3) as usize)
    {
        let name = if entry.name.is_empty() {
            "-".to_string()
        } else {
            entry.name.clone()
        };
        let forced = if state.forced_io.contains(&entry.address) {
            "F"
        } else {
            ""
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:<4}", entry.direction), label_style()),
            Span::raw(" "),
            Span::styled(format!("{:<12}", name), value_style()),
            Span::raw(" "),
            Span::styled(format!("{:<8}", entry.address), value_style()),
            Span::raw(" "),
            Span::styled(format!("{:<10}", entry.value), value_style()),
            Span::raw(" "),
            Span::styled(forced, Style::default().fg(COLOR_MAGENTA)),
        ]));
    }
    let block = panel_block(PanelKind::Io, focused);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub(super) fn render_status_panel(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    focused: bool,
) {
    let status = state.data.status.clone().unwrap_or_default();
    let settings = state.data.settings.clone().unwrap_or_default();
    let uptime = format_uptime(status.uptime_ms);
    let chip = status_chip(status.state.as_str());
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(chip.0, chip.1),
        Span::raw(" "),
        Span::styled(status.resource, value_style()),
    ]));
    lines.push(label_value_line("Uptime", &uptime, 12, value_style()));
    if let Some(driver) = status.drivers.first() {
        if driver.status.is_empty() {
            lines.push(label_value_line("I/O", &driver.name, 12, value_style()));
        } else {
            lines.push(label_value_line(
                "I/O",
                &format!("{} ({})", driver.name, driver.status),
                12,
                value_style(),
            ));
        }
        if let Some(err) = driver.error.as_ref() {
            lines.push(label_value_line(
                "I/O error",
                err,
                12,
                Style::default().fg(COLOR_RED),
            ));
        }
    } else {
        lines.push(label_value_line(
            "I/O",
            "unknown",
            12,
            Style::default().fg(COLOR_INFO),
        ));
    }
    lines.push(label_value_line(
        "Control",
        &format!(
            "{} (debug {})",
            status.control_mode,
            if status.debug_enabled { "on" } else { "off" }
        ),
        12,
        value_style(),
    ));
    let simulation_mode = if status.simulation_mode.is_empty() {
        if settings.simulation_mode.is_empty() {
            if settings.simulation_enabled {
                "simulation".to_string()
            } else {
                "production".to_string()
            }
        } else {
            settings.simulation_mode.clone()
        }
    } else {
        status.simulation_mode.clone()
    };
    let simulation_time_scale = if status.simulation_time_scale > 0 {
        status.simulation_time_scale
    } else {
        settings.simulation_time_scale
    };
    lines.push(label_value_line(
        "Mode",
        &format!("{simulation_mode} (x{simulation_time_scale})"),
        12,
        value_style(),
    ));
    if simulation_mode.eq_ignore_ascii_case("simulation") {
        let warning = if !status.simulation_warning.is_empty() {
            status.simulation_warning.clone()
        } else {
            settings.simulation_warning.clone()
        };
        if !warning.is_empty() {
            lines.push(label_value_line(
                "Warning",
                &warning,
                12,
                Style::default().fg(COLOR_AMBER),
            ));
        }
    }
    let web = if settings.web_listen.is_empty() {
        "disabled".to_string()
    } else {
        format!("http://{}", settings.web_listen)
    };
    lines.push(label_value_line("Web", &web, 12, value_style()));
    if !status.fault.is_empty() && status.fault != "none" {
        lines.push(label_value_line(
            "Fault",
            &status.fault,
            12,
            Style::default().fg(COLOR_RED),
        ));
    }
    if status.overruns > 0 {
        lines.push(label_value_line(
            "Overruns",
            &status.overruns.to_string(),
            12,
            value_style(),
        ));
    }
    if status.faults > 0 {
        lines.push(label_value_line(
            "Faults",
            &status.faults.to_string(),
            12,
            value_style(),
        ));
    }
    let watchdog = if settings.watchdog_enabled {
        format!(
            "Watchdog: {} ms ({})",
            settings.watchdog_timeout_ms, settings.watchdog_action
        )
    } else {
        "Watchdog: disabled".to_string()
    };
    lines.push(label_value_line("Watchdog", &watchdog, 12, value_style()));
    let fault_policy = if settings.fault_policy.is_empty() {
        "unknown".to_string()
    } else {
        settings.fault_policy.clone()
    };
    lines.push(label_value_line(
        "Fault policy",
        &fault_policy,
        12,
        value_style(),
    ));
    let retain = if settings.retain_mode.is_empty() {
        "none".to_string()
    } else {
        settings.retain_mode.clone()
    };
    let retain_line = match settings.retain_save_interval_ms {
        Some(ms) => format!("Retain: {retain} ({ms} ms)"),
        None => format!("Retain: {retain}"),
    };
    lines.push(label_value_line("Retain", &retain_line, 12, value_style()));
    let block = panel_block(PanelKind::Status, focused);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub(super) fn render_events_panel(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    focused: bool,
) {
    let mut lines = Vec::new();
    for event in state
        .data
        .events
        .iter()
        .take(area.height.saturating_sub(2) as usize)
    {
        let (tag, tag_style, msg_style) = match event.kind {
            EventKind::Fault => (
                "[FAULT]",
                Style::default().fg(COLOR_RED).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            ),
            EventKind::Warn => (
                "[WARN]",
                Style::default()
                    .fg(COLOR_AMBER)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            ),
            EventKind::Info => (
                "[INFO]",
                Style::default().fg(COLOR_CYAN),
                Style::default().fg(Color::White),
            ),
        };
        let mut spans = Vec::new();
        if let Some(ts) = event.timestamp.as_ref() {
            spans.push(Span::styled(
                format!("{ts} "),
                Style::default().fg(COLOR_INFO).add_modifier(Modifier::DIM),
            ));
        }
        spans.push(Span::styled(format!("{tag} "), tag_style));
        spans.push(Span::styled(event.message.clone(), msg_style));
        lines.push(Line::from(spans));
    }
    let block = panel_block(PanelKind::Events, focused);
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_tasks_panel(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    focused: bool,
) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(format!("{:<12}", "TASK"), header_style()),
        Span::raw(" "),
        Span::styled(format!("{:>6}", "LAST"), header_style()),
        Span::raw(" "),
        Span::styled(format!("{:>6}", "AVG"), header_style()),
        Span::raw(" "),
        Span::styled(format!("{:>6}", "MAX"), header_style()),
        Span::raw(" "),
        Span::styled(format!("{:>4}", "OVR"), header_style()),
    ]));
    for task in state
        .data
        .tasks
        .iter()
        .take(area.height.saturating_sub(3) as usize)
    {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<12}", task.name), value_style()),
            Span::raw(" "),
            Span::styled(format!("{:>6.2}", task.last_ms), value_style()),
            Span::raw(" "),
            Span::styled(format!("{:>6.2}", task.avg_ms), value_style()),
            Span::raw(" "),
            Span::styled(format!("{:>6.2}", task.max_ms), value_style()),
            Span::raw(" "),
            Span::styled(format!("{:>4}", task.overruns), value_style()),
        ]));
    }
    let block = panel_block(PanelKind::Tasks, focused);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub(super) fn render_watch_panel(
    area: Rect,
    frame: &mut ratatui::Frame<'_>,
    state: &UiState,
    focused: bool,
) {
    let mut lines = Vec::new();
    if state.watch_values.is_empty() {
        lines.push(Line::from(Span::styled(
            "No watches configured.",
            Style::default().fg(COLOR_INFO),
        )));
    } else {
        for (name, value) in state
            .watch_values
            .iter()
            .take(area.height.saturating_sub(2) as usize)
        {
            lines.push(label_value_line(name, value, 14, value_style()));
        }
    }
    let block = panel_block(PanelKind::Watch, focused);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
