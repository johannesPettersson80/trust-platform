use super::*;

#[derive(Default, Clone)]
pub(super) struct UiData {
    pub(super) status: Option<StatusSnapshot>,
    pub(super) tasks: Vec<TaskSnapshot>,
    pub(super) io: Vec<IoEntry>,
    pub(super) events: Vec<EventSnapshot>,
    pub(super) settings: Option<SettingsSnapshot>,
}

#[derive(Default, Clone)]
pub(super) struct StatusSnapshot {
    pub(super) state: String,
    pub(super) fault: String,
    pub(super) resource: String,
    pub(super) uptime_ms: u64,
    pub(super) cycle_min: f64,
    pub(super) cycle_avg: f64,
    pub(super) cycle_max: f64,
    pub(super) cycle_last: f64,
    pub(super) overruns: u64,
    pub(super) faults: u64,
    pub(super) drivers: Vec<DriverSnapshot>,
    pub(super) debug_enabled: bool,
    pub(super) control_mode: String,
    pub(super) simulation_mode: String,
    pub(super) simulation_time_scale: u32,
    pub(super) simulation_warning: String,
}

#[derive(Default, Clone)]
pub(super) struct TaskSnapshot {
    pub(super) name: String,
    pub(super) last_ms: f64,
    pub(super) avg_ms: f64,
    pub(super) max_ms: f64,
    pub(super) overruns: u64,
}

#[derive(Default, Clone)]
pub(super) struct DriverSnapshot {
    pub(super) name: String,
    pub(super) status: String,
    pub(super) error: Option<String>,
}

#[derive(Default, Clone)]
pub(super) struct IoEntry {
    pub(super) name: String,
    pub(super) address: String,
    pub(super) value: String,
    pub(super) direction: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EventKind {
    Info,
    Warn,
    Fault,
}

#[derive(Clone)]
pub(super) struct EventSnapshot {
    pub(super) label: String,
    pub(super) kind: EventKind,
    pub(super) timestamp: Option<String>,
    pub(super) message: String,
}

impl Default for EventSnapshot {
    fn default() -> Self {
        Self {
            label: String::new(),
            kind: EventKind::Info,
            timestamp: None,
            message: String::new(),
        }
    }
}

#[derive(Default, Clone)]
pub(super) struct SettingsSnapshot {
    pub(super) cycle_interval_ms: Option<u64>,
    pub(super) log_level: String,
    pub(super) watchdog_enabled: bool,
    pub(super) watchdog_timeout_ms: i64,
    pub(super) watchdog_action: String,
    pub(super) fault_policy: String,
    pub(super) retain_mode: String,
    pub(super) retain_save_interval_ms: Option<i64>,
    pub(super) web_listen: String,
    pub(super) web_auth: String,
    pub(super) discovery_enabled: bool,
    pub(super) mesh_enabled: bool,
    pub(super) mesh_publish: Vec<String>,
    pub(super) mesh_subscribe: Vec<(String, String)>,
    pub(super) control_mode: String,
    pub(super) simulation_enabled: bool,
    pub(super) simulation_time_scale: u32,
    pub(super) simulation_mode: String,
    pub(super) simulation_warning: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfirmAction {
    RestartWarm,
    RestartCold,
    Shutdown,
    ExitConsole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelKind {
    Cycle,
    Io,
    Status,
    Events,
    Tasks,
    Watch,
}

impl PanelKind {
    pub(super) fn title(self) -> &'static str {
        match self {
            PanelKind::Cycle => "Cycle Time",
            PanelKind::Io => "I/O",
            PanelKind::Status => "Status",
            PanelKind::Events => "Events",
            PanelKind::Tasks => "Tasks",
            PanelKind::Watch => "Watch",
        }
    }

    pub(super) fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "cycle" => Some(Self::Cycle),
            "io" => Some(Self::Io),
            "status" => Some(Self::Status),
            "events" => Some(Self::Events),
            "tasks" => Some(Self::Tasks),
            "watch" => Some(Self::Watch),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PromptMode {
    Normal,
    SettingsSelect,
    SettingsValue(SettingKey),
    IoSelect(IoActionKind),
    IoValueSelect,
    ConfirmAction(ConfirmAction),
    Menu(MenuKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuKind {
    Io,
    Control,
    Access,
    Linking,
    Log,
    Restart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IoActionKind {
    Read,
    Set,
    Force,
    Unforce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingKey {
    PlcName,
    CycleInterval,
    LogLevel,
    ControlMode,
    WebListen,
    WebAuth,
    DiscoveryEnabled,
    MeshEnabled,
}

#[derive(Debug, Clone)]
pub(super) struct PromptLine {
    pub(super) segments: Vec<(String, Style)>,
}

impl PromptLine {
    pub(super) fn plain(text: impl Into<String>, style: Style) -> Self {
        Self {
            segments: vec![(text.into(), style)],
        }
    }

    pub(super) fn from_segments<T: Into<String>>(segments: Vec<(T, Style)>) -> Self {
        Self {
            segments: segments
                .into_iter()
                .map(|(text, style)| (text.into(), style))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CommandHelp {
    pub(super) cmd: &'static str,
    pub(super) desc: &'static str,
    pub(super) beginner: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PromptState {
    pub(super) active: bool,
    pub(super) input: String,
    pub(super) cursor: usize,
    pub(super) history: Vec<String>,
    pub(super) history_index: Option<usize>,
    pub(super) output: Vec<PromptLine>,
    pub(super) mode: PromptMode,
    pub(super) showing_suggestions: bool,
    pub(super) suggestions: Vec<CommandHelp>,
    pub(super) suggestion_index: usize,
}

impl PromptState {
    pub(super) fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            output: Vec::new(),
            mode: PromptMode::Normal,
            showing_suggestions: false,
            suggestions: Vec::new(),
            suggestion_index: 0,
        }
    }

    pub(super) fn activate_with(&mut self, text: &str) {
        self.active = true;
        self.input.clear();
        self.input.push_str(text);
        self.cursor = self.input.len();
        self.history_index = None;
    }

    pub(super) fn deactivate(&mut self) {
        self.active = false;
        self.cursor = 0;
        self.history_index = None;
    }

    pub(super) fn set_output(&mut self, lines: Vec<PromptLine>) {
        self.output = lines;
        self.showing_suggestions = false;
    }

    pub(super) fn clear_output(&mut self) {
        self.output.clear();
        self.showing_suggestions = false;
    }

    pub(super) fn set_suggestions_list(&mut self, suggestions: Vec<CommandHelp>) {
        self.suggestions = suggestions;
        self.suggestion_index = 0;
        self.showing_suggestions = true;
        self.output = suggestion_lines(&self.suggestions, Some(self.suggestion_index));
    }

    pub(super) fn clear_suggestions(&mut self) {
        if self.showing_suggestions {
            self.output.clear();
        }
        self.showing_suggestions = false;
        self.suggestions.clear();
        self.suggestion_index = 0;
    }

    pub(super) fn move_suggestion(&mut self, delta: i32) {
        if self.suggestions.is_empty() {
            return;
        }
        let len = self.suggestions.len() as i32;
        let mut next = self.suggestion_index as i32 + delta;
        if next < 0 {
            next = len - 1;
        } else if next >= len {
            next = 0;
        }
        self.suggestion_index = next as usize;
        self.output = suggestion_lines(&self.suggestions, Some(self.suggestion_index));
    }

    pub(super) fn selected_suggestion(&self) -> Option<CommandHelp> {
        self.suggestions.get(self.suggestion_index).copied()
    }

    pub(super) fn push_history(&mut self, entry: String) {
        if !entry.trim().is_empty() {
            self.history.push(entry);
        }
        self.history_index = None;
    }

    pub(super) fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next = match self.history_index {
            None => Some(self.history.len().saturating_sub(1)),
            Some(idx) if idx > 0 => Some(idx - 1),
            Some(idx) => Some(idx),
        };
        if let Some(idx) = next {
            self.history_index = Some(idx);
            self.input = self.history[idx].clone();
            self.cursor = self.input.len();
        }
    }

    pub(super) fn history_next(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next = match self.history_index {
            None => None,
            Some(idx) if idx + 1 < self.history.len() => Some(idx + 1),
            Some(_) => None,
        };
        self.history_index = next;
        match next {
            Some(idx) => {
                self.input = self.history[idx].clone();
                self.cursor = self.input.len();
            }
            None => {
                self.input.clear();
                self.cursor = 0;
            }
        }
    }
}

pub(super) struct UiState {
    pub(super) data: UiData,
    pub(super) pending_confirm: Option<ConfirmAction>,
    pub(super) beginner_mode: bool,
    pub(super) debug_controls: bool,
    pub(super) prompt: PromptState,
    pub(super) layout: Vec<PanelKind>,
    pub(super) focus: Option<PanelKind>,
    pub(super) panel_page: usize,
    pub(super) settings_index: usize,
    pub(super) menu_index: usize,
    pub(super) io_index: usize,
    pub(super) io_value_index: usize,
    pub(super) io_pending_address: Option<String>,
    pub(super) io_pending_action: Option<IoActionKind>,
    pub(super) cycle_history: VecDeque<u64>,
    pub(super) watch_list: Vec<String>,
    pub(super) watch_values: Vec<(String, String)>,
    pub(super) forced_io: HashSet<String>,
    pub(super) alerts: VecDeque<PromptLine>,
    pub(super) seen_events: HashSet<String>,
    pub(super) connected: bool,
    pub(super) bundle_root: Option<PathBuf>,
}

impl UiState {
    pub(super) fn new(
        layout: Vec<PanelKind>,
        beginner_mode: bool,
        bundle_root: Option<PathBuf>,
    ) -> Self {
        Self {
            data: UiData::default(),
            pending_confirm: None,
            beginner_mode,
            debug_controls: !beginner_mode,
            prompt: PromptState::new(),
            layout,
            focus: None,
            panel_page: 0,
            settings_index: 0,
            menu_index: 0,
            io_index: 0,
            io_value_index: 0,
            io_pending_address: None,
            io_pending_action: None,
            cycle_history: VecDeque::with_capacity(120),
            watch_list: Vec::new(),
            watch_values: Vec::new(),
            forced_io: HashSet::new(),
            alerts: VecDeque::with_capacity(6),
            seen_events: HashSet::new(),
            connected: true,
            bundle_root,
        }
    }
}

#[derive(Default)]
pub(super) struct ConsoleConfig {
    pub(super) layout: Option<Vec<PanelKind>>,
    pub(super) refresh_ms: Option<u64>,
}

pub(super) fn push_alert(state: &mut UiState, text: &str, style: Style) {
    if state.alerts.len() > 4 {
        state.alerts.pop_front();
    }
    state
        .alerts
        .push_back(PromptLine::plain(text.to_string(), style));
}

pub(super) fn update_cycle_history(state: &mut UiState) {
    let status = match state.data.status.as_ref() {
        Some(status) => status,
        None => return,
    };
    let value = (status.cycle_last * 10.0).max(0.0).round() as u64;
    if state.cycle_history.len() >= 120 {
        state.cycle_history.pop_front();
    }
    state.cycle_history.push_back(value.max(1));
}

pub(super) fn update_watch_values(client: &mut ControlClient, state: &mut UiState) {
    if state.watch_list.is_empty() {
        state.watch_values.clear();
        return;
    }
    let mut out = Vec::new();
    for name in state.watch_list.iter() {
        let response = client.request(json!({
            "id": 1,
            "type": "eval",
            "params": { "expr": name }
        }));
        match response {
            Ok(value) => {
                if let Some(result) = value.get("result").and_then(|r| r.get("value")) {
                    out.push((name.clone(), result.to_string()));
                } else if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
                    out.push((name.clone(), format!("error: {err}")));
                } else {
                    out.push((name.clone(), "unknown".to_string()));
                }
            }
            Err(_) => out.push((name.clone(), "unavailable".to_string())),
        }
    }
    state.watch_values = out;
}

pub(super) fn update_event_alerts(state: &mut UiState) {
    let events = state.data.events.clone();
    for event in events {
        if state.seen_events.contains(&event.label) {
            continue;
        }
        state.seen_events.insert(event.label.clone());
        match event.kind {
            EventKind::Fault => push_alert(
                state,
                &format!("[FAULT] {}", event.message),
                Style::default().fg(COLOR_RED),
            ),
            EventKind::Warn => push_alert(
                state,
                &format!("[WARN] {}", event.message),
                Style::default().fg(COLOR_AMBER),
            ),
            EventKind::Info => {}
        }
        if state.seen_events.len() > 400 {
            state.seen_events.clear();
        }
    }
}
