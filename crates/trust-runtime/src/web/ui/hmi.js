const POLL_MS = 500;
const WS_ROUTE = '/ws/hmi';
const WS_MAX_FAILURES_BEFORE_POLL = 3;
const WS_RECONNECT_BASE_MS = 500;
const WS_RECONNECT_MAX_MS = 5000;
const HMI_MODE_STORAGE_KEY = 'trust.hmi.mode';
const ROUTE_PAGE_PARAM = 'page';
const ROUTE_SIGNAL_PARAM = 'signal';
const ROUTE_FOCUS_PARAM = 'focus';
const ROUTE_TARGET_PARAM = 'target';
const ROUTE_VIEWPORT_PARAM = 'viewport';

const state = {
  schema: null,
  descriptor: null,
  cards: new Map(),
  moduleCards: new Map(),
  sparklines: new Map(),
  latestValues: new Map(),
  pollHandle: null,
  ws: null,
  wsConnected: false,
  wsFailures: 0,
  wsReconnectHandle: null,
  schemaRevision: 0,
  schemaRefreshInFlight: false,
  lastAlarmResult: null,
  processView: null,
  processSvgCache: new Map(),
  processRenderSeq: 0,
  descriptorError: null,
  currentPage: null,
  routeSignal: null,
  routeFocus: null,
  routeTarget: null,
  trendDurationMs: null,
  processBindingMisses: 0,
  presentationMode: 'operator',
  layoutEditMode: false,
  responsiveMode: 'auto',
  ackInFlight: new Set(),
};

/* Dark mode — matches runtime styles.css body[data-theme="dark"] */
const CONTROL_ROOM_THEME = Object.freeze({
  '--bg': '#0f1115',
  '--bg-2': '#141821',
  '--bg-3': '#11151d',
  '--surface': '#171a21',
  '--surface-soft': '#1f2430',
  '--text': '#f2f2f2',
  '--muted': '#9ca3af',
  '--muted-strong': '#cbd5f5',
  '--border': 'rgba(255, 255, 255, 0.08)',
  '--accent': '#14b8a6',
  '--accent-strong': '#0d9488',
  '--accent-soft': 'rgba(20, 184, 166, 0.18)',
  '--ok': '#14b8a6',
  '--warn': '#f97316',
  '--bad': '#f87171',
  '--danger': '#f87171',
  '--mix-base': '#0f1115',
  '--shadow-sm': '0 1px 3px rgba(0,0,0,0.3)',
  '--shadow-md': '0 4px 12px rgba(0,0,0,0.4)',
  '--shadow-lg': '0 18px 40px rgba(0,0,0,0.45)',
});

const THEME_CYCLE = ['dark', 'light'];
const THEME_STORAGE_KEY = 'trust.hmi.theme';

function byId(id) {
  return document.getElementById(id);
}

function parseRouteState() {
  const params = new URLSearchParams(window.location.search);
  const page = params.get(ROUTE_PAGE_PARAM);
  const signal = params.get(ROUTE_SIGNAL_PARAM);
  const focus = params.get(ROUTE_FOCUS_PARAM);
  const target = params.get(ROUTE_TARGET_PARAM);
  return {
    page: page && page.trim() ? page.trim() : null,
    signal: signal && signal.trim() ? signal.trim() : null,
    focus: focus && focus.trim() ? focus.trim() : null,
    target: target && target.trim() ? target.trim() : null,
  };
}

function syncStateFromRoute() {
  const route = parseRouteState();
  state.routeSignal = route.signal;
  state.routeFocus = route.focus;
  state.routeTarget = route.target;
  if (route.page) {
    state.currentPage = route.page;
  }
}

function applyRoute(next, replace = false) {
  const params = new URLSearchParams(window.location.search);
  const setParam = (key, value) => {
    if (value === null || value === undefined || value === '') {
      params.delete(key);
    } else {
      params.set(key, String(value));
    }
  };
  setParam(ROUTE_PAGE_PARAM, next.page ?? state.currentPage);
  setParam(ROUTE_SIGNAL_PARAM, next.signal);
  setParam(ROUTE_FOCUS_PARAM, next.focus);
  setParam(ROUTE_TARGET_PARAM, next.target);
  const query = params.toString();
  const url = `${window.location.pathname}${query ? `?${query}` : ''}`;
  const historyApi = window.history;
  if (historyApi && typeof historyApi.replaceState === 'function' && typeof historyApi.pushState === 'function') {
    if (replace) {
      historyApi.replaceState({}, '', url);
    } else {
      historyApi.pushState({}, '', url);
    }
  }
  syncStateFromRoute();
}

function setConnection(status) {
  const pill = byId('connectionState');
  if (!pill) {
    return;
  }
  pill.classList.remove('connected', 'stale', 'disconnected');
  if (status === 'connected') {
    pill.classList.add('connected');
    pill.textContent = 'Connected';
  } else if (status === 'stale') {
    pill.classList.add('stale');
    pill.textContent = 'Stale';
  } else {
    pill.classList.add('disconnected');
    pill.textContent = 'Disconnected';
  }
}

function setFreshness(timestampMs) {
  const freshness = byId('freshnessState');
  if (!freshness) {
    return;
  }
  if (!timestampMs) {
    freshness.textContent = 'freshness: n/a';
    return;
  }
  const age = Math.max(0, Date.now() - Number(timestampMs));
  freshness.textContent = `freshness: ${age} ms`;
}

function updateDiagnosticsPill() {
  const pill = byId('diagnosticState');
  if (!pill) {
    return;
  }
  const descriptorError = typeof state.descriptorError === 'string' && state.descriptorError.trim()
    ? state.descriptorError.trim()
    : null;
  if (state.presentationMode !== 'engineering' && !descriptorError) {
    pill.classList.add('hidden');
    pill.title = '';
    return;
  }
  let stale = 0;
  let bad = 0;
  for (const refs of state.cards.values()) {
    const quality = refs?.card?.dataset?.quality;
    if (quality === 'stale') {
      stale += 1;
    } else if (quality === 'bad') {
      bad += 1;
    }
  }
  const missing = Number(state.processBindingMisses) || 0;
  pill.classList.remove('hidden');
  if (state.presentationMode === 'engineering') {
    pill.textContent = descriptorError
      ? `diag: stale ${stale} · bad ${bad} · bind-miss ${missing} · descriptor error`
      : `diag: stale ${stale} · bad ${bad} · bind-miss ${missing}`;
  } else {
    pill.textContent = 'descriptor error';
  }
  pill.title = descriptorError || '';
}

function setEmptyMessage(text) {
  const empty = byId('emptyState');
  if (!empty) {
    return;
  }
  empty.classList.remove('hidden');
  empty.textContent = text;
}

function hideEmptyMessage() {
  const empty = byId('emptyState');
  if (empty) {
    empty.classList.add('hidden');
  }
}

function setThemeVariables(root, values) {
  if (!root || !root.style || typeof root.style.setProperty !== 'function') {
    return;
  }
  for (const [key, value] of Object.entries(values)) {
    root.style.setProperty(key, value);
  }
}

function removeThemeVariables(root, keys) {
  if (!root || !root.style || typeof root.style.removeProperty !== 'function') {
    return;
  }
  for (const key of keys) {
    root.style.removeProperty(key);
  }
}

function isControlRoomTheme(theme) {
  if (!theme || typeof theme !== 'object') {
    return false;
  }
  const style = typeof theme.style === 'string' ? theme.style.trim().toLowerCase() : '';
  return style === 'control-room' || style === 'dark';
}

function flashValueUpdate(element) {
  if (!element || !element.classList) {
    return;
  }
  element.classList.remove('value-updated');
  if (typeof element.offsetWidth === 'number') {
    void element.offsetWidth;
  }
  element.classList.add('value-updated');
}

function applyTheme(theme) {
  if (!theme || typeof theme !== 'object') {
    return;
  }
  const root = document.documentElement;
  const controlRoom = isControlRoomTheme(theme);
  if (controlRoom) {
    setThemeVariables(root, CONTROL_ROOM_THEME);
    root.style.colorScheme = 'dark';
  } else {
    removeThemeVariables(root, Object.keys(CONTROL_ROOM_THEME));
    root.style.colorScheme = 'light';
    root.style.setProperty('--mix-base', '#ffffff');
    if (typeof theme.background === 'string') {
      root.style.setProperty('--bg', theme.background);
    }
    if (typeof theme.surface === 'string') {
      root.style.setProperty('--surface', theme.surface);
    }
    if (typeof theme.text === 'string') {
      root.style.setProperty('--text', theme.text);
    }
    if (typeof theme.accent === 'string') {
      root.style.setProperty('--accent', theme.accent);
    }
  }
  document.body.classList.toggle('theme-dark', controlRoom);
  document.body.dataset.theme = controlRoom ? 'dark' : 'light';
  if (typeof theme.style === 'string') {
    const label = byId('themeLabel');
    if (label) {
      label.textContent = controlRoom ? 'Dark mode' : 'Light mode';
    }
  }
}

function parsePresentationOverride() {
  const params = new URLSearchParams(window.location.search);
  const value = params.get('mode');
  if (!value) {
    return undefined;
  }
  const lower = value.trim().toLowerCase();
  if (lower === 'engineering' || lower === 'operator') {
    return lower;
  }
  return undefined;
}

function readStoredPresentationMode() {
  try {
    const value = window.localStorage.getItem(HMI_MODE_STORAGE_KEY);
    if (!value) {
      return undefined;
    }
    const lower = value.trim().toLowerCase();
    if (lower === 'engineering' || lower === 'operator') {
      return lower;
    }
  } catch (_error) {
    return undefined;
  }
  return undefined;
}

function persistPresentationMode(mode) {
  try {
    window.localStorage.setItem(HMI_MODE_STORAGE_KEY, mode);
  } catch (_error) {
    // ignore local storage failures
  }
}

function applyPresentationMode(mode) {
  state.presentationMode = mode === 'engineering' ? 'engineering' : 'operator';
  document.body.classList.remove('operator-mode', 'engineering-mode');
  document.body.classList.add(`${state.presentationMode}-mode`);
  if (state.presentationMode !== 'engineering') {
    state.layoutEditMode = false;
    document.body.classList.remove('layout-edit-mode');
  }
  const toggle = byId('modeToggle');
  if (toggle) {
    toggle.textContent = state.presentationMode === 'engineering' ? 'Operator Mode' : 'Engineering Mode';
  }
  const layoutToggle = byId('layoutToggle');
  if (layoutToggle) {
    if (state.presentationMode === 'engineering') {
      layoutToggle.classList.remove('hidden');
      layoutToggle.textContent = state.layoutEditMode ? 'Done Editing' : 'Edit Layout';
    } else {
      layoutToggle.classList.add('hidden');
    }
  }
  const addSignalButton = byId('addSignalButton');
  if (addSignalButton) {
    if (state.presentationMode === 'engineering' && state.layoutEditMode) {
      addSignalButton.classList.remove('hidden');
    } else {
      addSignalButton.classList.add('hidden');
    }
  }
  const resetLayoutButton = byId('resetLayoutButton');
  if (resetLayoutButton) {
    if (state.presentationMode === 'engineering') {
      resetLayoutButton.classList.remove('hidden');
    } else {
      resetLayoutButton.classList.add('hidden');
    }
  }
  const backButton = byId('backButton');
  if (backButton) {
    const historyLength = Number(window.history && window.history.length);
    backButton.classList.toggle('hidden', !Number.isFinite(historyLength) || historyLength <= 1);
  }
  updateDiagnosticsPill();
}

function togglePresentationMode() {
  const next = state.presentationMode === 'engineering' ? 'operator' : 'engineering';
  persistPresentationMode(next);
  applyPresentationMode(next);
  renderCurrentPage();
}

function cycleTheme() {
  const currentStyle = state.schema?.theme?.style || 'dark';
  const currentLower = currentStyle.trim().toLowerCase();
  const idx = THEME_CYCLE.indexOf(currentLower);
  const next = THEME_CYCLE[(idx + 1) % THEME_CYCLE.length];
  if (state.schema) {
    if (!state.schema.theme) { state.schema.theme = {}; }
    state.schema.theme.style = next;
  }
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, next);
  } catch (_err) { /* ignore */ }
  applyTheme({ style: next, accent: '', background: '', surface: '', text: '' });
  void apiControl('hmi.descriptor.update', {
    theme: { style: next },
  });
}

function toggleLayoutMode() {
  if (state.presentationMode !== 'engineering') {
    return;
  }
  state.layoutEditMode = !state.layoutEditMode;
  document.body.classList.toggle('layout-edit-mode', state.layoutEditMode);
  applyPresentationMode(state.presentationMode);
}

function parseResponsiveOverride() {
  const params = new URLSearchParams(window.location.search);
  const value = params.get(ROUTE_VIEWPORT_PARAM);
  if (!value) {
    return undefined;
  }
  const lower = value.trim().toLowerCase();
  if (lower === 'auto' || lower === 'mobile' || lower === 'tablet' || lower === 'kiosk') {
    return lower;
  }
  return undefined;
}

function viewportForWidth(width, mobileMax, tabletMax) {
  if (width <= mobileMax) {
    return 'mobile';
  }
  if (width <= tabletMax) {
    return 'tablet';
  }
  return 'desktop';
}

function applyResponsiveLayout() {
  const responsive = state.schema?.responsive ?? {};
  const configured = (typeof responsive.mode === 'string' ? responsive.mode.toLowerCase() : 'auto');
  const override = parseResponsiveOverride();
  const mode = override || configured;
  state.responsiveMode = mode;

  document.body.classList.remove('viewport-mobile', 'viewport-tablet', 'viewport-kiosk');
  if (mode === 'kiosk') {
    document.body.classList.add('viewport-kiosk');
    return;
  }
  const mobileMax = Number(responsive.mobile_max_px) || 680;
  const tabletMax = Number(responsive.tablet_max_px) || 1024;
  const resolved = mode === 'auto' ? viewportForWidth(window.innerWidth, mobileMax, tabletMax) : mode;
  if (resolved === 'mobile') {
    document.body.classList.add('viewport-mobile');
  } else if (resolved === 'tablet') {
    document.body.classList.add('viewport-tablet');
  }
}

function initModeControls() {
  const fromQuery = parsePresentationOverride();
  const fromStorage = readStoredPresentationMode();
  const mode = fromQuery || fromStorage || 'operator';
  if (fromQuery) {
    persistPresentationMode(fromQuery);
  }
  applyPresentationMode(mode);

  const modeToggle = byId('modeToggle');
  if (modeToggle) {
    modeToggle.addEventListener('click', () => {
      togglePresentationMode();
    });
  }
  const layoutToggle = byId('layoutToggle');
  if (layoutToggle) {
    layoutToggle.addEventListener('click', () => {
      toggleLayoutMode();
      renderCurrentPage();
    });
  }
  const addSignalButton = byId('addSignalButton');
  if (addSignalButton) {
    addSignalButton.addEventListener('click', () => {
      void addUnplacedSignalToCurrentPage();
    });
  }
  const resetLayoutButton = byId('resetLayoutButton');
  if (resetLayoutButton) {
    resetLayoutButton.addEventListener('click', () => {
      void resetDescriptorToScaffoldDefaults();
    });
  }
  const backButton = byId('backButton');
  if (backButton) {
    backButton.addEventListener('click', () => {
      if (window.history && typeof window.history.back === 'function') {
        window.history.back();
      }
    });
  }
  const themeLabel = byId('themeLabel');
  if (themeLabel) {
    themeLabel.style.cursor = 'pointer';
    themeLabel.addEventListener('click', () => {
      cycleTheme();
    });
  }
  window.addEventListener('popstate', () => {
    syncStateFromRoute();
    ensureCurrentPage();
    renderSidebar();
    renderCurrentPage();
    void refreshActivePage({ forceValues: true });
    applyPresentationMode(state.presentationMode);
  });
  window.addEventListener('keydown', (event) => {
    if (event.defaultPrevented) {
      return;
    }
    if (event.key && event.key.toLowerCase() === 'g') {
      togglePresentationMode();
    }
  });
}

async function apiControl(type, params) {
  const payload = { id: Date.now(), type };
  if (params !== undefined) {
    payload.params = params;
  }
  const response = await fetch('/api/control', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}`);
  }
  return response.json();
}

function ensurePollingLoop() {
  if (state.pollHandle !== null) {
    return;
  }
  state.pollHandle = window.setInterval(() => {
    refreshActivePage();
  }, POLL_MS);
}

function websocketUrl() {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}${WS_ROUTE}`;
}

function clearWsReconnect() {
  if (state.wsReconnectHandle === null) {
    return;
  }
  window.clearTimeout(state.wsReconnectHandle);
  state.wsReconnectHandle = null;
}

function scheduleWsReconnect() {
  clearWsReconnect();
  const attempt = Math.max(0, state.wsFailures - 1);
  const delay = Math.min(WS_RECONNECT_MAX_MS, WS_RECONNECT_BASE_MS * (2 ** attempt));
  state.wsReconnectHandle = window.setTimeout(() => {
    state.wsReconnectHandle = null;
    connectWebSocketTransport();
  }, delay);
}

function valueSignature(value) {
  if (value === null) {
    return 'null';
  }
  if (value === undefined) {
    return 'undefined';
  }
  const valueType = typeof value;
  if (valueType === 'string') {
    return `s:${value}`;
  }
  if (valueType === 'number') {
    return Number.isFinite(value) ? `n:${value}` : 'n:NaN';
  }
  if (valueType === 'boolean') {
    return `b:${value}`;
  }
  if (valueType === 'object') {
    try {
      return `j:${JSON.stringify(value)}`;
    } catch (_error) {
      return 'j:[unserializable]';
    }
  }
  return `${valueType}:${String(value)}`;
}

function applyCardEntry(refs, entry) {
  if (!refs || !refs.card) {
    return;
  }
  if (!entry || typeof entry !== 'object') {
    refs.card.dataset.quality = 'stale';
    if (typeof refs.apply === 'function') {
      refs.apply(null);
    } else if (refs.value) {
      refs.value.textContent = '--';
    }
    refs.lastValueSignature = undefined;
    return;
  }

  let quality = typeof entry.q === 'string' ? entry.q : 'stale';
  const entryTs = Number(entry.ts_ms);
  if (Number.isFinite(entryTs) && entryTs > 1_000_000_000_000) {
    const age = Math.max(0, Date.now() - entryTs);
    if (age >= 10_000) {
      quality = 'bad';
    } else if (age >= 5_000) {
      quality = 'stale';
    }
  }
  refs.card.dataset.quality = quality;
  if (typeof refs.apply === 'function') {
    refs.apply(entry);
  } else if (refs.value) {
    refs.value.textContent = formatValue(entry.v);
  }

  const signature = valueSignature(entry.v);
  if (refs.lastValueSignature !== undefined && signature !== refs.lastValueSignature) {
    flashValueUpdate(refs.value);
  }
  refs.lastValueSignature = signature;
}

function applyValueDelta(payload) {
  if (!payload || typeof payload !== 'object') {
    return;
  }
  const connected = payload.connected === true;
  setConnection(connected ? 'connected' : 'stale');
  setFreshness(payload.timestamp_ms);

  const values = payload.values && typeof payload.values === 'object' ? payload.values : {};
  for (const [id, entry] of Object.entries(values)) {
    state.latestValues.set(id, entry);
  }
  applyProcessValueEntries(values, payload.timestamp_ms);
  for (const [id, entry] of Object.entries(values)) {
    const refs = state.cards.get(id);
    if (refs) applyCardEntry(refs, entry);
    const moduleRefs = state.moduleCards.get(id);
    if (moduleRefs) applyCardEntry(moduleRefs, entry);
  }
  updateDiagnosticsPill();
  updateAlarmBannerFromValues(values);
}

function updateAlarmBanner() {
  const banner = byId('alarmBanner');
  if (!banner) return;
  const text = byId('alarmBannerText');
  const active = state.lastAlarmResult?.active;
  if (Array.isArray(active) && active.length > 0) {
    const raised = active.filter(a => a.state === 'raised');
    const top = raised.length > 0 ? raised[0] : active[0];
    banner.classList.add('active');
    if (text) text.textContent = top.label || top.path || top.id || 'Alarm active';
  } else {
    banner.classList.remove('active');
    if (text) text.textContent = 'No alarms';
  }
}

function updateAlarmBannerFromValues(values) {
  if (!values || typeof values !== 'object') return;
  for (const [id, entry] of Object.entries(values)) {
    if (id.endsWith('.AlarmMessage') || id.endsWith('.AlarmMessage"')) {
      const val = entry?.value;
      if (typeof val === 'string' && val.trim()) {
        const banner = byId('alarmBanner');
        const text = byId('alarmBannerText');
        if (banner && text) {
          banner.classList.add('active');
          text.textContent = val.trim();
        }
        return;
      }
    }
  }
}

async function refreshSchemaForRevision(revision) {
  const nextRevision = Number(revision);
  if (!Number.isFinite(nextRevision) || nextRevision <= state.schemaRevision) {
    return;
  }
  if (state.schemaRefreshInFlight) {
    return;
  }
  state.schemaRefreshInFlight = true;
  try {
    const response = await apiControl('hmi.schema.get');
    if (!response.ok) {
      throw new Error(response.error || 'schema refresh failed');
    }
    renderSchema(response.result || {});
    await refreshDescriptorModel();
    await refreshActivePage({ forceValues: true });
  } catch (_error) {
    setConnection('stale');
  } finally {
    state.schemaRefreshInFlight = false;
  }
}

async function handleWebSocketEvent(message) {
  if (!message || typeof message !== 'object') {
    return;
  }
  const type = typeof message.type === 'string' ? message.type : '';
  const payload = message.result;
  if (type === 'hmi.values.delta') {
    applyValueDelta(payload);
    return;
  }
  if (type === 'hmi.schema.revision') {
    await refreshSchemaForRevision(payload?.schema_revision);
    return;
  }
  if (type === 'hmi.alarms.event') {
    state.lastAlarmResult = payload || null;
    updateAlarmBanner();
    if (currentPageKind() === 'alarm') {
      renderAlarmTable(payload || {});
    }
  }
}

function isSafeProcessSelector(selector) {
  return typeof selector === 'string' && /^#[A-Za-z0-9_.:-]{1,127}$/.test(selector);
}

function isSafeProcessAttribute(attribute) {
  return typeof attribute === 'string'
    && /^(text|fill|stroke|opacity|x|y|width|height|class|transform|data-value)$/.test(attribute);
}

function formatProcessRawValue(value) {
  if (value === null || value === undefined) {
    return '--';
  }
  if (typeof value === 'number') {
    return Number.isFinite(value) ? String(value) : '--';
  }
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }
  if (typeof value === 'string') {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch (_error) {
    return String(value);
  }
}

function scaleProcessValue(value, scale) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || !scale || typeof scale !== 'object') {
    return value;
  }
  const min = Number(scale.min);
  const max = Number(scale.max);
  const outputMin = Number(scale.output_min);
  const outputMax = Number(scale.output_max);
  if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) {
    return value;
  }
  if (!Number.isFinite(outputMin) || !Number.isFinite(outputMax)) {
    return value;
  }
  const ratio = (numeric - min) / (max - min);
  return outputMin + ((outputMax - outputMin) * ratio);
}

function formatProcessValue(value, format) {
  if (typeof format !== 'string' || !format.trim()) {
    return formatProcessRawValue(value);
  }
  const pattern = format.trim();
  const fixedMatch = pattern.match(/\{:\.(\d+)f\}/);
  if (fixedMatch && Number.isFinite(Number(value))) {
    const precision = Number(fixedMatch[1]);
    const formatted = Number(value).toFixed(precision);
    return pattern.replace(/\{:\.(\d+)f\}/, formatted);
  }
  if (pattern.includes('{}')) {
    return pattern.replace('{}', formatProcessRawValue(value));
  }
  return `${pattern} ${formatProcessRawValue(value)}`.trim();
}

function applyProcessValueEntries(values, payloadTimestampMs) {
  if (!state.processView || !values || typeof values !== 'object') {
    return;
  }
  if (payloadTimestampMs !== undefined) {
    setFreshness(payloadTimestampMs);
  }
  for (const [id, entry] of Object.entries(values)) {
    const bindings = state.processView.bindingsByWidgetId.get(id);
    if (!bindings || !bindings.length || !entry || typeof entry !== 'object') {
      continue;
    }
    for (const binding of bindings) {
      let resolved = entry.v;
      const mapTable = binding.map && typeof binding.map === 'object' ? binding.map : null;
      if (mapTable) {
        const key = formatProcessRawValue(resolved);
        if (Object.prototype.hasOwnProperty.call(mapTable, key)) {
          resolved = mapTable[key];
        }
      }
      resolved = scaleProcessValue(resolved, binding.scale);
      const text = formatProcessValue(resolved, binding.format);
      if (binding.attribute === 'text') {
        binding.target.textContent = text;
      } else {
        binding.target.setAttribute(binding.attribute, text);
      }
    }
  }
}

function connectWebSocketTransport() {
  if (!('WebSocket' in window)) {
    return;
  }
  if (state.ws && (state.ws.readyState === WebSocket.OPEN || state.ws.readyState === WebSocket.CONNECTING)) {
    return;
  }
  let socket;
  try {
    socket = new WebSocket(websocketUrl());
  } catch (_error) {
    state.wsFailures += 1;
    scheduleWsReconnect();
    return;
  }

  state.ws = socket;
  socket.addEventListener('open', () => {
    if (state.ws !== socket) {
      return;
    }
    state.wsConnected = true;
    state.wsFailures = 0;
    clearWsReconnect();
    setConnection('connected');
  });

  socket.addEventListener('message', (event) => {
    let payload;
    try {
      payload = JSON.parse(event.data);
    } catch (_error) {
      return;
    }
    void handleWebSocketEvent(payload);
  });

  socket.addEventListener('close', () => {
    if (state.ws !== socket) {
      return;
    }
    state.ws = null;
    state.wsConnected = false;
    state.wsFailures += 1;
    if (state.wsFailures >= WS_MAX_FAILURES_BEFORE_POLL) {
      setConnection('stale');
    } else {
      setConnection('disconnected');
    }
    scheduleWsReconnect();
  });

  socket.addEventListener('error', () => {
    if (state.ws !== socket) {
      return;
    }
    socket.close();
  });
}

function formatValue(value) {
  if (value === null || value === undefined) {
    return '--';
  }
  if (typeof value === 'boolean') {
    return value ? 'TRUE' : 'FALSE';
  }
  if (typeof value === 'number') {
    return Number.isInteger(value)
      ? String(value)
      : value.toFixed(3).replace(/0+$/, '').replace(/\.$/, '');
  }
  if (typeof value === 'string') {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch (_error) {
    return String(value);
  }
}

function widgetMeta(widget) {
  const parts = [`${widget.data_type} · ${widget.access}`];
  if (widget.inferred_interface === true) {
    parts.push('inferred interface');
  }
  if (widget.unit) {
    parts.push(widget.unit);
  }
  if (typeof widget.min === 'number' || typeof widget.max === 'number') {
    const min = typeof widget.min === 'number' ? widget.min : '-∞';
    const max = typeof widget.max === 'number' ? widget.max : '+∞';
    parts.push(`[${min}..${max}]`);
  }
  return parts.join(' · ');
}

function clamp01(value) {
  return Math.max(0, Math.min(1, value));
}

function numericRange(widget) {
  const min = Number.isFinite(widget?.min) ? Number(widget.min) : 0;
  const rawMax = Number.isFinite(widget?.max) ? Number(widget.max) : 100;
  const max = rawMax <= min ? min + 1 : rawMax;
  return { min, max };
}

function numericFromEntry(entry) {
  if (!entry || typeof entry !== 'object') {
    return null;
  }
  const numeric = Number(entry.v);
  return Number.isFinite(numeric) ? numeric : null;
}

function zoneColorForValue(widget, value, fallback) {
  if (!Array.isArray(widget?.zones) || value === null) {
    return fallback;
  }
  const match = widget.zones.find((zone) => Number(zone.from) <= value && value <= Number(zone.to));
  if (match && typeof match.color === 'string' && match.color.trim()) {
    return match.color.trim();
  }
  return fallback;
}

function writeWidgetValue(widget, value) {
  return apiControl('hmi.write', { id: widget.id, value })
    .then((response) => response && response.ok === true)
    .catch(() => false);
}

function polarPoint(cx, cy, radius, angleDeg) {
  const radians = (angleDeg * Math.PI) / 180;
  return {
    x: cx + radius * Math.cos(radians),
    y: cy + radius * Math.sin(radians),
  };
}

function describeArc(cx, cy, radius, startAngle, endAngle) {
  const start = polarPoint(cx, cy, radius, startAngle);
  const end = polarPoint(cx, cy, radius, endAngle);
  const largeArc = Math.abs(endAngle - startAngle) > 180 ? 1 : 0;
  const sweep = endAngle > startAngle ? 1 : 0;
  return `M ${start.x.toFixed(3)} ${start.y.toFixed(3)} A ${radius} ${radius} 0 ${largeArc} ${sweep} ${end.x.toFixed(3)} ${end.y.toFixed(3)}`;
}

function domSafeToken(value, fallback = 'widget') {
  const token = String(value || '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return token || fallback;
}

function createDefaultRenderer(host) {
  return (entry) => {
    host.textContent = entry ? formatValue(entry.v) : '--';
    host.classList.remove('indicator-true', 'indicator-false');
  };
}

function createIndicatorRenderer(widget, host) {
  host.classList.add('widget-indicator');
  const dot = document.createElement('span');
  dot.className = 'widget-indicator-dot';
  const label = document.createElement('span');
  label.className = 'widget-indicator-label';
  host.appendChild(dot);
  host.appendChild(label);
  const onColor = typeof widget.on_color === 'string' ? widget.on_color : 'var(--ok)';
  const offColor = typeof widget.off_color === 'string' ? widget.off_color : 'var(--bad)';
  return (entry) => {
    const active = entry && entry.v === true;
    dot.style.background = active ? onColor : offColor;
    dot.style.color = active ? onColor : offColor;
    dot.classList.toggle('active', active);
    label.textContent = entry ? (active ? 'ON' : 'OFF') : '--';
  };
}

function createGaugeRenderer(widget, host) {
  host.classList.add('widget-gauge');
  const ns = 'http://www.w3.org/2000/svg';
  const centerX = 100;
  const centerY = 88;
  const radius = 56;
  const startAngle = 205;
  const endAngle = 335;

  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('class', 'widget-gauge-svg');
  svg.setAttribute('viewBox', '0 0 200 120');

  const defs = document.createElementNS(ns, 'defs');
  const grad = document.createElementNS(ns, 'linearGradient');
  grad.id = `gauge-grad-${domSafeToken(widget?.id || Math.random().toString(36))}`;
  grad.setAttribute('x1', '0%');
  grad.setAttribute('y1', '0%');
  grad.setAttribute('x2', '100%');
  grad.setAttribute('y2', '0%');
  const stop1 = document.createElementNS(ns, 'stop');
  stop1.setAttribute('offset', '0%');
  stop1.setAttribute('stop-color', 'var(--accent)');
  stop1.setAttribute('stop-opacity', '0.7');
  const stop2 = document.createElementNS(ns, 'stop');
  stop2.setAttribute('offset', '100%');
  stop2.setAttribute('stop-color', 'var(--accent)');
  stop2.setAttribute('stop-opacity', '1');
  grad.appendChild(stop1);
  grad.appendChild(stop2);
  defs.appendChild(grad);
  svg.appendChild(defs);

  const arcBase = document.createElementNS(ns, 'path');
  arcBase.setAttribute('class', 'widget-gauge-base');
  arcBase.setAttribute('d', describeArc(centerX, centerY, radius, startAngle, endAngle));

  const arcValue = document.createElementNS(ns, 'path');
  arcValue.setAttribute('class', 'widget-gauge-value');

  const centerValue = document.createElementNS(ns, 'text');
  centerValue.setAttribute('class', 'widget-gauge-center-value');
  centerValue.setAttribute('x', String(centerX));
  centerValue.setAttribute('y', '72');
  centerValue.textContent = '--';

  const unitText = document.createElementNS(ns, 'text');
  unitText.setAttribute('class', 'widget-gauge-unit');
  unitText.setAttribute('x', String(centerX));
  unitText.setAttribute('y', '88');
  unitText.setAttribute('text-anchor', 'middle');
  unitText.setAttribute('fill', 'var(--muted)');
  unitText.setAttribute('font-size', '9');
  unitText.setAttribute('font-family', 'var(--font-data)');
  unitText.textContent = widget.unit || '';

  svg.appendChild(arcBase);
  svg.appendChild(arcValue);
  svg.appendChild(centerValue);
  svg.appendChild(unitText);

  const label = document.createElement('div');
  label.className = 'widget-gauge-label';
  label.textContent = widget?.label || widget?.path || 'Gauge';
  host.appendChild(svg);
  host.appendChild(label);

  const range = numericRange(widget);
  return (entry) => {
    const numeric = numericFromEntry(entry);
    if (numeric === null) {
      arcValue.setAttribute('d', '');
      centerValue.textContent = '--';
      return;
    }
    const norm = clamp01((numeric - range.min) / (range.max - range.min));
    const angle = startAngle + (norm * (endAngle - startAngle));
    const color = zoneColorForValue(widget, numeric, `url(#${grad.id})`);
    arcValue.setAttribute('d', describeArc(centerX, centerY, radius, startAngle, angle));
    arcValue.setAttribute('stroke', color);
    centerValue.textContent = formatValue(numeric);
  };
}

function createSparklineRenderer(widget, host) {
  host.classList.add('widget-sparkline');
  const ns = 'http://www.w3.org/2000/svg';
  const svgW = 200;
  const svgH = 72;
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('class', 'widget-sparkline-svg');
  svg.setAttribute('viewBox', `0 0 ${svgW} ${svgH}`);

  const defs = document.createElementNS(ns, 'defs');
  const gradId = `spark-grad-${domSafeToken(widget?.id || Math.random().toString(36))}`;
  const grad = document.createElementNS(ns, 'linearGradient');
  grad.id = gradId;
  grad.setAttribute('x1', '0');
  grad.setAttribute('y1', '0');
  grad.setAttribute('x2', '0');
  grad.setAttribute('y2', '1');
  const s1 = document.createElementNS(ns, 'stop');
  s1.setAttribute('offset', '0%');
  s1.setAttribute('stop-color', 'var(--accent)');
  s1.setAttribute('stop-opacity', '0.25');
  const s2 = document.createElementNS(ns, 'stop');
  s2.setAttribute('offset', '100%');
  s2.setAttribute('stop-color', 'var(--accent)');
  s2.setAttribute('stop-opacity', '0.02');
  grad.appendChild(s1);
  grad.appendChild(s2);
  defs.appendChild(grad);
  svg.appendChild(defs);

  const area = document.createElementNS(ns, 'polygon');
  area.setAttribute('class', 'widget-sparkline-area');
  area.setAttribute('fill', `url(#${gradId})`);
  svg.appendChild(area);

  const polyline = document.createElementNS(ns, 'polyline');
  polyline.setAttribute('class', 'widget-sparkline-line');
  svg.appendChild(polyline);
  const label = document.createElement('div');
  label.className = 'widget-sparkline-label';
  host.appendChild(svg);
  host.appendChild(label);

  if (!state.sparklines.has(widget.id)) {
    state.sparklines.set(widget.id, []);
  }

  const padTop = 6;
  const plotH = svgH - padTop - 8;

  return (entry) => {
    const samples = state.sparklines.get(widget.id) || [];
    const numeric = numericFromEntry(entry);
    if (numeric !== null) {
      samples.push(numeric);
      if (samples.length > 64) {
        samples.shift();
      }
      state.sparklines.set(widget.id, samples);
    }

    if (!samples.length) {
      polyline.setAttribute('points', '');
      area.setAttribute('points', '');
      label.textContent = '--';
      return;
    }

    const min = Math.min(...samples);
    const max = Math.max(...samples);
    const span = Math.max(1e-9, max - min);
    const coords = samples.map((sample, index) => {
      const x = samples.length <= 1 ? 0 : (index / (samples.length - 1)) * svgW;
      const y = padTop + plotH - (((sample - min) / span) * plotH);
      return [x, y];
    });
    const linePoints = coords.map(([x, y]) => `${x.toFixed(2)},${y.toFixed(2)}`).join(' ');
    polyline.setAttribute('points', linePoints);

    const lastX = coords[coords.length - 1][0];
    const firstX = coords[0][0];
    const areaPoints = linePoints + ` ${lastX.toFixed(2)},${svgH} ${firstX.toFixed(2)},${svgH}`;
    area.setAttribute('points', areaPoints);

    label.textContent = `${formatValue(samples[samples.length - 1])}${widget.unit ? ` ${widget.unit}` : ''}`;
  };
}

function createBarRenderer(widget, host) {
  host.classList.add('widget-bar');
  const track = document.createElement('div');
  track.className = 'widget-bar-track';
  const fill = document.createElement('div');
  fill.className = 'widget-bar-fill';
  track.appendChild(fill);
  const label = document.createElement('div');
  label.className = 'widget-bar-label';
  host.appendChild(track);
  host.appendChild(label);

  const range = numericRange(widget);
  return (entry) => {
    const numeric = numericFromEntry(entry);
    if (numeric === null) {
      fill.style.width = '0%';
      label.textContent = '--';
      return;
    }
    const norm = clamp01((numeric - range.min) / (range.max - range.min));
    fill.style.width = `${(norm * 100).toFixed(2)}%`;
    fill.style.background = zoneColorForValue(widget, numeric, 'var(--accent)');
    label.textContent = `${formatValue(numeric)}${widget.unit ? ` ${widget.unit}` : ''}`;
  };
}

function createTankRenderer(widget, host) {
  host.classList.add('widget-tank');
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('class', 'widget-tank-svg');
  svg.setAttribute('viewBox', '0 0 100 116');

  const defs = document.createElementNS(ns, 'defs');
  const gradId = `tank-grad-${domSafeToken(widget?.id || Math.random().toString(36))}`;
  const grad = document.createElementNS(ns, 'linearGradient');
  grad.id = gradId;
  grad.setAttribute('x1', '0');
  grad.setAttribute('y1', '0');
  grad.setAttribute('x2', '0');
  grad.setAttribute('y2', '1');
  const ts1 = document.createElementNS(ns, 'stop');
  ts1.setAttribute('offset', '0%');
  ts1.setAttribute('stop-color', 'var(--accent)');
  ts1.setAttribute('stop-opacity', '0.65');
  const ts2 = document.createElementNS(ns, 'stop');
  ts2.setAttribute('offset', '100%');
  ts2.setAttribute('stop-color', 'var(--accent)');
  ts2.setAttribute('stop-opacity', '0.95');
  grad.appendChild(ts1);
  grad.appendChild(ts2);
  defs.appendChild(grad);
  svg.appendChild(defs);

  const frame = document.createElementNS(ns, 'rect');
  frame.setAttribute('class', 'widget-tank-frame');
  frame.setAttribute('x', '28');
  frame.setAttribute('y', '8');
  frame.setAttribute('width', '42');
  frame.setAttribute('height', '96');
  frame.setAttribute('rx', '4');

  const fill = document.createElementNS(ns, 'rect');
  fill.setAttribute('class', 'widget-tank-fill');
  fill.setAttribute('x', '28');
  fill.setAttribute('y', '104');
  fill.setAttribute('width', '42');
  fill.setAttribute('height', '0');
  fill.setAttribute('rx', '2');
  fill.setAttribute('fill', `url(#${gradId})`);

  svg.appendChild(frame);
  svg.appendChild(fill);
  const label = document.createElement('div');
  label.className = 'widget-tank-label';
  host.appendChild(svg);
  host.appendChild(label);

  const range = numericRange(widget);
  return (entry) => {
    const numeric = numericFromEntry(entry);
    if (numeric === null) {
      fill.setAttribute('y', '104');
      fill.setAttribute('height', '0');
      label.textContent = '--';
      return;
    }
    const norm = clamp01((numeric - range.min) / (range.max - range.min));
    const height = 96 * norm;
    const y = 104 - height;
    fill.setAttribute('y', y.toFixed(3));
    fill.setAttribute('height', height.toFixed(3));
    label.textContent = `${formatValue(numeric)}${widget.unit ? ` ${widget.unit}` : ''}`;
  };
}

function createToggleRenderer(widget, host) {
  host.classList.add('widget-toggle');
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'widget-toggle-control';
  const stateLabel = document.createElement('span');
  stateLabel.className = 'widget-toggle-label';
  host.appendChild(button);
  host.appendChild(stateLabel);

  let current = false;
  const writable = widget.writable === true && state.schema?.read_only !== true;
  const requiresConfirm = commandKeywordMatch(`${widget.path || ''} ${widget.label || ''}`);

  button.disabled = !writable;
  button.addEventListener('click', async () => {
    if (!writable) {
      return;
    }
    button.disabled = true;
    const next = !current;
    if (requiresConfirm) {
      const verb = next ? 'enable' : 'disable';
      const label = widget.label || widget.path || 'this command';
      if (!window.confirm(`Confirm ${verb} ${label}?`)) {
        button.disabled = !writable;
        return;
      }
    }
    const ok = await writeWidgetValue(widget, next);
    if (ok) {
      current = next;
      button.classList.toggle('active', current);
      stateLabel.textContent = current ? 'ON' : 'OFF';
    }
    button.disabled = !writable;
  });

  return (entry) => {
    current = entry && entry.v === true;
    button.classList.toggle('active', current);
    stateLabel.textContent = entry ? (current ? 'ON' : 'OFF') : '--';
    button.disabled = !writable;
  };
}

function createSliderRenderer(widget, host) {
  host.classList.add('widget-slider');
  const range = numericRange(widget);
  const input = document.createElement('input');
  input.type = 'range';
  input.className = 'widget-slider-control';
  input.min = String(range.min);
  input.max = String(range.max);
  input.step = /REAL|LREAL/i.test(String(widget.data_type || '')) ? '0.1' : '1';
  const label = document.createElement('div');
  label.className = 'widget-slider-label';
  const pvLabel = document.createElement('div');
  pvLabel.className = 'widget-slider-label';
  host.appendChild(input);
  host.appendChild(label);
  host.appendChild(pvLabel);

  let lastValue = range.min;
  const writable = widget.writable === true && state.schema?.read_only !== true;
  const peerId = setpointPeerWidgetId(widget);
  input.disabled = !writable;

  input.addEventListener('input', () => {
    label.textContent = `${formatValue(Number(input.value))}${widget.unit ? ` ${widget.unit}` : ''}`;
  });
  input.addEventListener('change', async () => {
    if (!writable) {
      return;
    }
    const next = Number(input.value);
    const ok = await writeWidgetValue(widget, next);
    if (!ok) {
      input.value = String(lastValue);
      label.textContent = `${formatValue(lastValue)}${widget.unit ? ` ${widget.unit}` : ''}`;
    }
  });

  return (entry) => {
    const numeric = numericFromEntry(entry);
    if (numeric === null) {
      label.textContent = '--';
      pvLabel.textContent = peerId ? 'PV: --' : '';
      input.disabled = !writable;
      return;
    }
    lastValue = numeric;
    input.value = String(numeric);
    label.textContent = `${formatValue(numeric)}${widget.unit ? ` ${widget.unit}` : ''}`;
    if (peerId) {
      const peerEntry = state.latestValues.get(peerId);
      pvLabel.textContent = `PV: ${peerEntry ? formatValue(peerEntry.v) : '--'}${widget.unit ? ` ${widget.unit}` : ''}`;
    } else {
      pvLabel.textContent = '';
    }
    input.disabled = !writable;
  };
}

function createModuleRenderer(widget, host) {
  host.classList.add('widget-module');
  const header = document.createElement('div');
  header.className = 'widget-module-header';
  const dot = document.createElement('span');
  dot.className = 'widget-module-status';
  dot.style.background = 'var(--muted)';
  const nameEl = document.createElement('span');
  nameEl.textContent = widget.label || widget.path || 'Module';
  header.appendChild(dot);
  header.appendChild(nameEl);
  host.appendChild(header);

  const metrics = document.createElement('div');
  metrics.className = 'widget-module-metrics';
  const metric1 = document.createElement('div');
  metric1.className = 'widget-module-metric';
  const val1 = document.createElement('span');
  val1.className = 'widget-module-metric-value';
  val1.textContent = '--';
  const lbl1 = document.createElement('span');
  lbl1.className = 'widget-module-metric-label';
  lbl1.textContent = widget.unit || 'value';
  metric1.appendChild(val1);
  metric1.appendChild(lbl1);
  metrics.appendChild(metric1);
  host.appendChild(metrics);

  return (entry) => {
    const active = entry && entry.v !== null && entry.v !== undefined;
    const isBool = entry && typeof entry.v === 'boolean';
    if (isBool) {
      dot.style.background = entry.v ? 'var(--ok)' : 'var(--muted)';
      dot.style.color = entry.v ? 'var(--ok)' : 'var(--muted)';
      dot.classList.toggle('active', entry.v === true);
      val1.textContent = entry.v ? 'Running' : 'Stopped';
    } else {
      dot.style.background = active ? 'var(--ok)' : 'var(--muted)';
      dot.style.color = active ? 'var(--ok)' : 'var(--muted)';
      dot.classList.toggle('active', active);
      val1.textContent = entry ? formatValue(entry.v) : '--';
    }
    const card = host.closest('.card');
    if (card) {
      const alarm = entry && (entry.q === 'bad' || entry.v === false);
      card.dataset.alarm = alarm ? 'true' : 'false';
    }
  };
}

function createWidgetRenderer(widget, host) {
  const kind = String(widget?.widget || '').toLowerCase();
  if (kind === 'gauge') {
    return createGaugeRenderer(widget, host);
  }
  if (kind === 'sparkline') {
    return createSparklineRenderer(widget, host);
  }
  if (kind === 'bar') {
    return createBarRenderer(widget, host);
  }
  if (kind === 'tank') {
    return createTankRenderer(widget, host);
  }
  if (kind === 'indicator') {
    return createIndicatorRenderer(widget, host);
  }
  if (kind === 'toggle') {
    return createToggleRenderer(widget, host);
  }
  if (kind === 'slider') {
    return createSliderRenderer(widget, host);
  }
  if (kind === 'module') {
    return createModuleRenderer(widget, host);
  }
  return createDefaultRenderer(host);
}

function pages() {
  const value = state.schema?.pages;
  return Array.isArray(value) ? value : [];
}

function currentPage() {
  return pages().find((page) => page.id === state.currentPage);
}

function currentPageKind() {
  return (currentPage()?.kind || 'dashboard').toLowerCase();
}

function ensureCurrentPage() {
  const entries = pages();
  if (!entries.length) {
    state.currentPage = null;
    return;
  }
  const exists = entries.some((page) => page.id === state.currentPage);
  if (!exists) {
    state.currentPage = entries[0].id;
  }
}

function renderSidebar() {
  const sidebar = byId('pageSidebar');
  if (!sidebar) {
    return;
  }
  sidebar.innerHTML = '';
  ensureCurrentPage();

  const entries = pages().filter((p) => !p.hidden);
  if (!entries.length) {
    sidebar.classList.add('hidden');
    return;
  }
  sidebar.classList.remove('hidden');

  for (const page of entries) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'page-button';
    if (page.id === state.currentPage) {
      button.classList.add('active');
    }

    const title = document.createElement('span');
    title.className = 'page-title';
    title.textContent = page.title || page.id;

    const kind = document.createElement('span');
    kind.className = 'page-kind';
    kind.textContent = page.kind || 'dashboard';

    button.appendChild(title);
    button.appendChild(kind);
    button.addEventListener('click', async () => {
      state.currentPage = page.id;
      applyRoute({ page: page.id, signal: null, focus: null, target: null });
      renderSidebar();
      renderCurrentPage();
      applyPresentationMode(state.presentationMode);
      await refreshActivePage({ forceValues: true });
    });
    sidebar.appendChild(button);
  }
}

function hideContentPanels() {
  const groups = byId('hmiGroups');
  const trend = byId('trendPanel');
  const alarm = byId('alarmPanel');
  if (groups) {
    groups.classList.add('hidden');
    groups.innerHTML = '';
  }
  if (trend) {
    trend.classList.add('hidden');
    trend.innerHTML = '';
  }
  if (alarm) {
    alarm.classList.add('hidden');
    alarm.innerHTML = '';
  }
  state.cards.clear();
  state.moduleCards.clear();
  state.sparklines.clear();
  state.processView = null;
  state.processBindingMisses = 0;
}

function visibleWidgets() {
  if (!state.schema || !Array.isArray(state.schema.widgets)) {
    return [];
  }
  if (!state.currentPage) {
    return state.schema.widgets;
  }
  // Include widgets referenced by this page's sections (e.g. shared
  // between overview and hidden equipment detail pages).
  const page = currentPage();
  const sectionIds = new Set();
  if (page && Array.isArray(page.sections)) {
    for (const s of page.sections) {
      if (Array.isArray(s.widget_ids)) {
        for (const id of s.widget_ids) sectionIds.add(id);
      }
    }
  }
  return state.schema.widgets.filter(
    (widget) => widget.page === state.currentPage || sectionIds.has(widget.id),
  );
}

function cloneJson(value) {
  try {
    return JSON.parse(JSON.stringify(value));
  } catch (_error) {
    return null;
  }
}

function descriptorWidgetFromSchema(widget) {
  return {
    widget_type: String(widget.widget || 'value'),
    bind: String(widget.path || ''),
    label: widget.label || undefined,
    unit: widget.unit || undefined,
    min: Number.isFinite(widget.min) ? Number(widget.min) : undefined,
    max: Number.isFinite(widget.max) ? Number(widget.max) : undefined,
    span: Number.isFinite(widget.widget_span) ? Math.max(1, Math.min(12, Math.trunc(Number(widget.widget_span)))) : undefined,
    on_color: widget.on_color || undefined,
    off_color: widget.off_color || undefined,
    inferred_interface: widget.inferred_interface === true ? true : undefined,
    zones: Array.isArray(widget.zones) ? widget.zones : [],
  };
}

function descriptorPageFromSchema(page, allWidgets) {
  const widgets = allWidgets.filter((widget) => widget.page === page.id);
  const widgetsById = new Map(widgets.map((widget) => [widget.id, widget]));
  const sections = [];
  if (Array.isArray(page.sections) && page.sections.length) {
    for (const section of page.sections) {
      const sectionWidgets = [];
      for (const id of Array.isArray(section.widget_ids) ? section.widget_ids : []) {
        const widget = widgetsById.get(id);
        if (!widget) {
          continue;
        }
        sectionWidgets.push(descriptorWidgetFromSchema(widget));
      }
      if (!sectionWidgets.length) {
        continue;
      }
      sections.push({
        title: section.title || 'Section',
        span: Number.isFinite(section.span) ? Math.max(1, Math.min(12, Math.trunc(Number(section.span)))) : 12,
        widgets: sectionWidgets,
      });
    }
  }
  if (!sections.length) {
    const grouped = new Map();
    for (const widget of widgets) {
      const group = widget.group || 'General';
      if (!grouped.has(group)) {
        grouped.set(group, []);
      }
      grouped.get(group).push(descriptorWidgetFromSchema(widget));
    }
    for (const [group, groupWidgets] of grouped.entries()) {
      sections.push({
        title: group,
        span: 12,
        widgets: groupWidgets,
      });
    }
  }
  return {
    id: page.id,
    title: page.title || page.id,
    icon: page.icon || undefined,
    order: Number.isFinite(page.order) ? Number(page.order) : 0,
    kind: page.kind || 'dashboard',
    duration_ms: Number.isFinite(page.duration_ms) ? Number(page.duration_ms) : undefined,
    svg: page.svg || undefined,
    signals: Array.isArray(page.signals) ? page.signals.filter((entry) => typeof entry === 'string' && entry.trim()) : [],
    sections,
    bindings: Array.isArray(page.bindings) ? page.bindings : [],
  };
}

function descriptorFromSchema(schema) {
  const widgets = Array.isArray(schema?.widgets) ? schema.widgets : [];
  const pages = Array.isArray(schema?.pages) ? schema.pages : [];
  return {
    config: {
      theme: {
        style: schema?.theme?.style || 'classic',
        accent: schema?.theme?.accent || '#0ea5b7',
      },
      layout: {},
      write: {},
      alarm: [],
    },
    pages: pages.map((page) => descriptorPageFromSchema(page, widgets)),
  };
}

function ensureDescriptorModel() {
  if (state.descriptor && typeof state.descriptor === 'object') {
    const cloned = cloneJson(state.descriptor);
    if (cloned) {
      return cloned;
    }
  }
  return descriptorFromSchema(state.schema);
}

function normalizeDescriptorPage(page, schemaPage) {
  if (!Array.isArray(page.sections)) {
    page.sections = [];
  }
  if (!Array.isArray(page.bindings)) {
    page.bindings = Array.isArray(schemaPage?.bindings) ? schemaPage.bindings : [];
  }
  if (!Array.isArray(page.signals)) {
    page.signals = Array.isArray(schemaPage?.signals) ? schemaPage.signals : [];
  }
  if (!page.kind) {
    page.kind = schemaPage?.kind || 'dashboard';
  }
  if (!Number.isFinite(page.order)) {
    page.order = Number.isFinite(schemaPage?.order) ? Number(schemaPage.order) : 0;
  }
  if (!page.title) {
    page.title = schemaPage?.title || page.id;
  }
  return page;
}

function ensurePageDescriptor(descriptor, pageId) {
  if (!descriptor || !Array.isArray(descriptor.pages)) {
    return null;
  }
  const existing = descriptor.pages.find((page) => page.id === pageId);
  if (existing) {
    return normalizeDescriptorPage(existing, pages().find((entry) => entry.id === pageId));
  }
  const schemaPage = pages().find((entry) => entry.id === pageId);
  const created = normalizeDescriptorPage({
    id: pageId,
    title: schemaPage?.title || pageId,
    icon: schemaPage?.icon || undefined,
    order: Number.isFinite(schemaPage?.order) ? Number(schemaPage.order) : (descriptor.pages.length * 10),
    kind: schemaPage?.kind || 'dashboard',
    duration_ms: Number.isFinite(schemaPage?.duration_ms) ? Number(schemaPage.duration_ms) : undefined,
    svg: schemaPage?.svg || undefined,
    signals: Array.isArray(schemaPage?.signals) ? schemaPage.signals.slice() : [],
    sections: [],
    bindings: Array.isArray(schemaPage?.bindings) ? schemaPage.bindings.slice() : [],
  }, schemaPage);
  descriptor.pages.push(created);
  return created;
}

function trimEmptySections(descriptor) {
  if (!descriptor || !Array.isArray(descriptor.pages)) {
    return;
  }
  for (const page of descriptor.pages) {
    if (!Array.isArray(page.sections)) {
      page.sections = [];
      continue;
    }
    page.sections = page.sections.filter((section) => Array.isArray(section.widgets) && section.widgets.length > 0);
  }
}

function removeWidgetPlacements(descriptor, path) {
  for (const page of descriptor.pages || []) {
    for (const section of page.sections || []) {
      if (!Array.isArray(section.widgets)) {
        section.widgets = [];
      }
      section.widgets = section.widgets.filter((widget) => widget.bind !== path);
    }
  }
  trimEmptySections(descriptor);
}

function removeWidgetPlacementFromPage(descriptor, pageId, path) {
  for (const page of descriptor.pages || []) {
    if (page.id !== pageId) {
      continue;
    }
    for (const section of page.sections || []) {
      if (!Array.isArray(section.widgets)) {
        section.widgets = [];
      }
      section.widgets = section.widgets.filter((widget) => widget.bind !== path);
    }
  }
  trimEmptySections(descriptor);
}

function ensureSection(page, title) {
  if (!Array.isArray(page.sections)) {
    page.sections = [];
  }
  let section = page.sections.find((entry) => entry.title === title);
  if (!section) {
    section = {
      title,
      span: 12,
      widgets: [],
    };
    page.sections.push(section);
  }
  if (!Array.isArray(section.widgets)) {
    section.widgets = [];
  }
  return section;
}

function addWidgetPlacement(descriptor, pageId, widget, sectionTitle) {
  const page = ensurePageDescriptor(descriptor, pageId);
  if (!page) {
    return;
  }
  const section = ensureSection(page, sectionTitle || widget.group || 'Process Variables');
  if (section.widgets.some((entry) => entry.bind === widget.path)) {
    return;
  }
  section.widgets.push(descriptorWidgetFromSchema(widget));
}

function updateWidgetPlacements(descriptor, path, updater) {
  for (const page of descriptor.pages || []) {
    for (const section of page.sections || []) {
      for (const widget of section.widgets || []) {
        if (widget.bind !== path) {
          continue;
        }
        updater(widget, page, section);
      }
    }
  }
}

function widgetPinnedOnOverview(descriptor, widgetPath) {
  const overview = (descriptor.pages || []).find((page) => page.id === 'overview');
  if (!overview) {
    return false;
  }
  for (const section of overview.sections || []) {
    for (const widget of section.widgets || []) {
      if (widget.bind === widgetPath) {
        return true;
      }
    }
  }
  return false;
}

function placedWidgetPaths(descriptor) {
  const paths = new Set();
  for (const page of descriptor.pages || []) {
    for (const section of page.sections || []) {
      for (const widget of section.widgets || []) {
        if (widget && typeof widget.bind === 'string' && widget.bind.trim()) {
          paths.add(widget.bind.trim());
        }
      }
    }
  }
  return paths;
}

async function refreshDescriptorModel() {
  try {
    const response = await apiControl('hmi.descriptor.get');
    if (response.ok) {
      state.descriptor = response.result || null;
    }
  } catch (_error) {
    // descriptor curation stays disabled when endpoint is unavailable
  }
}

async function saveDescriptorAndRefresh(descriptor) {
  const response = await apiControl('hmi.descriptor.update', { descriptor });
  if (!response.ok) {
    throw new Error(response.error || 'descriptor update failed');
  }
  state.descriptor = descriptor;
  const nextRevision = Number(response.result?.schema_revision);
  if (Number.isFinite(nextRevision)) {
    await refreshSchemaForRevision(nextRevision);
  } else {
    await refreshActivePage({ forceValues: true });
  }
}

function promptChoice(title, values, current) {
  const normalized = Array.from(new Set(values.filter((value) => typeof value === 'string' && value.trim())))
    .map((value) => value.trim());
  const suggestion = normalized.join(', ');
  const value = window.prompt(`${title}\nOptions: ${suggestion}`, current || normalized[0] || '');
  if (!value) {
    return null;
  }
  return value.trim();
}

function widgetTypeOptionsFor(widget) {
  const dataType = String(widget?.data_type || '').toUpperCase();
  const options = new Set(['value', 'readout', 'text']);
  if (dataType.includes('BOOL')) {
    options.add('indicator');
    options.add('toggle');
  }
  if (/REAL|LREAL|INT|DINT|UDINT|UINT|SINT|USINT|LINT|ULINT|TIME|LTIME/.test(dataType)) {
    options.add('gauge');
    options.add('sparkline');
    options.add('bar');
    options.add('tank');
    options.add('slider');
  }
  if (Array.isArray(widget?.enum_values) && widget.enum_values.length) {
    options.add('selector');
  }
  return Array.from(options);
}

function setpointPeerWidgetId(widget) {
  const path = String(widget?.path || '');
  if (!path) {
    return null;
  }
  const candidates = [
    path.replace(/setpoint/gi, '').replace(/__+/g, '_').replace(/\._/g, '.').replace(/_$/g, ''),
    path.replace(/_setpoint/gi, '_pv'),
    path.replace(/setpoint/gi, 'pv'),
    path.replace(/_sp\b/gi, '_pv'),
    path.replace(/\.sp\b/gi, '.pv'),
  ]
    .map((value) => value.replace(/__+/g, '_').replace(/_\./g, '.').trim())
    .filter((value) => value && value !== path);
  if (!candidates.length) {
    return null;
  }
  const byPath = new Map((state.schema?.widgets || []).map((entry) => [entry.path, entry.id]));
  for (const candidate of candidates) {
    const id = byPath.get(candidate);
    if (id) {
      return id;
    }
  }
  return null;
}

function commandKeywordMatch(text) {
  const normalized = String(text || '').toLowerCase();
  return /(start|stop|reset|enable|disable|bypass|trip|shutdown)/.test(normalized);
}

async function runWidgetLayoutAction(widget, action) {
  const descriptor = ensureDescriptorModel();
  if (!descriptor || !Array.isArray(descriptor.pages)) {
    return;
  }

  if (action === 'hide') {
    if (!window.confirm(`Hide "${widget.label || widget.path}" from this page?`)) {
      return;
    }
    removeWidgetPlacementFromPage(descriptor, widget.page, widget.path);
  } else if (action === 'move') {
    const target = promptChoice('Move widget to page', pages().map((page) => page.id), widget.page);
    if (!target) {
      return;
    }
    removeWidgetPlacements(descriptor, widget.path);
    addWidgetPlacement(descriptor, target, widget, widget.group || 'Process Variables');
  } else if (action === 'pin') {
    if (widgetPinnedOnOverview(descriptor, widget.path)) {
      const overview = ensurePageDescriptor(descriptor, 'overview');
      if (overview) {
        for (const section of overview.sections || []) {
          section.widgets = (section.widgets || []).filter((entry) => entry.bind !== widget.path);
        }
        trimEmptySections(descriptor);
      }
    } else {
      addWidgetPlacement(descriptor, 'overview', widget, 'Pinned');
    }
  } else if (action === 'label') {
    const nextLabel = window.prompt('Widget label', widget.label || widget.path);
    if (!nextLabel) {
      return;
    }
    updateWidgetPlacements(descriptor, widget.path, (entry) => {
      entry.label = nextLabel.trim();
    });
  } else if (action === 'type') {
    const allowed = widgetTypeOptionsFor(widget);
    const nextType = promptChoice('Widget type', allowed, widget.widget || 'value');
    if (!nextType) {
      return;
    }
    updateWidgetPlacements(descriptor, widget.path, (entry) => {
      entry.widget_type = nextType;
    });
  } else if (action === 'span') {
    const preset = promptChoice('Widget size', ['small', 'medium', 'large'], 'medium');
    if (!preset) {
      return;
    }
    const span = preset.toLowerCase() === 'small' ? 3 : (preset.toLowerCase() === 'large' ? 8 : 5);
    updateWidgetPlacements(descriptor, widget.path, (entry) => {
      entry.span = span;
    });
  } else {
    return;
  }

  try {
    await saveDescriptorAndRefresh(descriptor);
    await refreshDescriptorModel();
    renderCurrentPage();
  } catch (error) {
    setEmptyMessage(`Layout update failed: ${error}`);
  }
}

function schemaWidgetByPath(path) {
  return (state.schema?.widgets || []).find((widget) => widget.path === path || widget.id === path);
}

function unplacedSchemaWidgets(descriptor) {
  const placed = placedWidgetPaths(descriptor);
  return (state.schema?.widgets || []).filter((widget) => !placed.has(widget.path));
}

function promptSignalPath(candidates) {
  if (!Array.isArray(candidates) || !candidates.length) {
    return null;
  }
  const options = candidates.slice(0, 24).map((widget) => widget.path);
  return promptChoice('Signal path to place', options, options[0]);
}

async function addUnplacedSignalToCurrentPage() {
  const pageId = state.currentPage;
  if (!pageId) {
    return;
  }
  const descriptor = ensureDescriptorModel();
  const page = ensurePageDescriptor(descriptor, pageId);
  if (!page) {
    return;
  }
  if (!Array.isArray(page.sections) || page.sections.length === 0) {
    page.sections = [{ title: 'Process Variables', span: 12, widgets: [] }];
  }
  const sectionTitles = page.sections.map((section) => section.title || 'Section');
  const targetSectionTitle = promptChoice('Section', sectionTitles, sectionTitles[0]);
  if (!targetSectionTitle) {
    return;
  }
  const candidates = unplacedSchemaWidgets(descriptor);
  if (!candidates.length) {
    setEmptyMessage('All discovered signals are already placed.');
    return;
  }
  const selectedPath = promptSignalPath(candidates);
  if (!selectedPath) {
    return;
  }
  const schemaWidget = schemaWidgetByPath(selectedPath);
  if (!schemaWidget) {
    setEmptyMessage(`Unknown signal "${selectedPath}".`);
    return;
  }
  addWidgetPlacement(descriptor, pageId, schemaWidget, targetSectionTitle);
  try {
    await saveDescriptorAndRefresh(descriptor);
    await refreshDescriptorModel();
    renderCurrentPage();
  } catch (error) {
    setEmptyMessage(`Layout update failed: ${error}`);
  }
}

async function runSectionLayoutAction(pageId, sectionIndex, action) {
  const descriptor = ensureDescriptorModel();
  const page = ensurePageDescriptor(descriptor, pageId);
  if (!page || !Array.isArray(page.sections)) {
    return;
  }
  const index = Number(sectionIndex);
  if (!Number.isInteger(index) || index < 0 || index >= page.sections.length) {
    return;
  }
  const section = page.sections[index];
  if (!section) {
    return;
  }

  if (action === 'rename') {
    const title = window.prompt('Section title', section.title || 'Section');
    if (!title || !title.trim()) {
      return;
    }
    section.title = title.trim();
  } else if (action === 'up') {
    if (index === 0) {
      return;
    }
    const previous = page.sections[index - 1];
    page.sections[index - 1] = section;
    page.sections[index] = previous;
  } else if (action === 'down') {
    if (index >= page.sections.length - 1) {
      return;
    }
    const next = page.sections[index + 1];
    page.sections[index + 1] = section;
    page.sections[index] = next;
  } else if (action === 'add') {
    const candidates = unplacedSchemaWidgets(descriptor);
    if (!candidates.length) {
      setEmptyMessage('All discovered signals are already placed.');
      return;
    }
    const selectedPath = promptSignalPath(candidates);
    if (!selectedPath) {
      return;
    }
    const schemaWidget = schemaWidgetByPath(selectedPath);
    if (!schemaWidget) {
      setEmptyMessage(`Unknown signal "${selectedPath}".`);
      return;
    }
    section.widgets = Array.isArray(section.widgets) ? section.widgets : [];
    section.widgets.push(descriptorWidgetFromSchema(schemaWidget));
  } else {
    return;
  }

  try {
    await saveDescriptorAndRefresh(descriptor);
    await refreshDescriptorModel();
    renderCurrentPage();
  } catch (error) {
    setEmptyMessage(`Section update failed: ${error}`);
  }
}

async function resetDescriptorToScaffoldDefaults() {
  if (!window.confirm('Reset HMI descriptors to scaffold defaults? A backup snapshot will be created.')) {
    return;
  }
  try {
    const response = await apiControl('hmi.scaffold.reset', { mode: 'reset' });
    if (!response.ok) {
      throw new Error(response.error || 'reset failed');
    }
    const nextRevision = Number(response.result?.schema_revision);
    if (Number.isFinite(nextRevision)) {
      await refreshSchemaForRevision(nextRevision);
    } else {
      const schema = await apiControl('hmi.schema.get');
      if (schema.ok) {
        renderSchema(schema.result || {});
      }
    }
    await refreshDescriptorModel();
    renderSidebar();
    renderCurrentPage();
    await refreshActivePage({ forceValues: true });
  } catch (error) {
    setEmptyMessage(`Reset failed: ${error}`);
  }
}

function pageIdByKind(kind) {
  const match = pages().find((page) => String(page.kind || '').toLowerCase() === kind);
  return match ? match.id : null;
}

function navigateToPage(pageId, route = {}, replace = false) {
  if (!pageId) {
    return;
  }
  state.currentPage = pageId;
  applyRoute(
    {
      page: pageId,
      signal: route.signal ?? null,
      focus: route.focus ?? null,
      target: route.target ?? null,
    },
    replace,
  );
  renderSidebar();
  renderCurrentPage();
  void refreshActivePage({ forceValues: true });
  applyPresentationMode(state.presentationMode);
}

function isLikelySetpoint(widget) {
  const value = `${widget?.path || ''} ${widget?.label || ''}`.toLowerCase();
  return /(setpoint|_sp\b|\.sp\b|\bsp\b)/.test(value);
}

function isLikelyKpi(widget) {
  if (!widget) {
    return false;
  }
  const dataType = String(widget.data_type || '').toUpperCase();
  if (!/REAL|LREAL|INT|DINT|UDINT|UINT|SINT|USINT|LINT|ULINT/.test(dataType)) {
    return false;
  }
  const value = `${widget.path || ''} ${widget.label || ''}`.toLowerCase();
  return /(flow|pressure|level|temp|temperature|speed|rpm|deviation|power|current|voltage)/.test(value);
}

function handleCardDrilldown(widget) {
  if (!widget || state.layoutEditMode || state.presentationMode !== 'operator') {
    return;
  }
  const currentId = state.currentPage;
  if (currentId === 'overview' && isLikelyKpi(widget)) {
    const trendsPage = pageIdByKind('trend') || 'trends';
    navigateToPage(trendsPage, { signal: widget.id });
    return;
  }
  if (currentId !== 'control' && isLikelySetpoint(widget)) {
    const controlPage = pages().find((page) => page.id === 'control')
      || pages().find((page) => String(page.title || '').toLowerCase() === 'control');
    if (controlPage) {
      navigateToPage(controlPage.id, { target: widget.path || widget.id });
    }
  }
}

function createEquipmentBlock(widget) {
  const block = document.createElement('div');
  block.className = 'equipment-block';
  block.dataset.id = widget.id;
  block.dataset.status = 'off';

  const nameRow = document.createElement('div');
  nameRow.className = 'equipment-block-name';
  const dot = document.createElement('span');
  dot.className = 'equipment-block-status-dot';
  const nameEl = document.createElement('span');
  nameEl.textContent = widget.label || widget.path || 'Equipment';
  nameRow.appendChild(dot);
  nameRow.appendChild(nameEl);
  block.appendChild(nameRow);

  const valueEl = document.createElement('div');
  valueEl.className = 'equipment-block-value';
  valueEl.textContent = '--';
  block.appendChild(valueEl);

  const labelEl = document.createElement('div');
  labelEl.className = 'equipment-block-label';
  labelEl.textContent = widget.unit || '';
  block.appendChild(labelEl);

  const detailPage = widget.detail_page;
  if (detailPage) {
    block.addEventListener('click', () => {
      applyRoute({ page: detailPage });
      syncStateFromRoute();
      void renderCurrentPage();
    });
  }

  const apply = (entry) => {
    const active = entry && entry.v !== null && entry.v !== undefined;
    const isBool = entry && typeof entry.v === 'boolean';
    if (isBool) {
      const isOn = entry.v === true;
      dot.style.background = isOn ? 'var(--ok)' : 'var(--muted)';
      block.dataset.status = isOn ? 'ok' : 'off';
      valueEl.textContent = isOn ? 'Running' : 'Stopped';
    } else {
      dot.style.background = active ? 'var(--ok)' : 'var(--muted)';
      block.dataset.status = active ? 'ok' : 'off';
      valueEl.textContent = entry ? formatValue(entry.v) : '--';
    }
    if (entry && (entry.q === 'bad' || entry.v === false)) {
      block.dataset.status = 'alarm';
      dot.style.background = 'var(--bad)';
    }
  };

  state.moduleCards.set(widget.id, {
    card: block,
    value: valueEl,
    widget,
    apply,
    lastValueSignature: undefined,
  });

  return block;
}

function createWidgetCard(widget) {
  const card = document.createElement('article');
  card.className = 'card';
  card.classList.add(`card-widget-${domSafeToken(widget?.widget, 'value')}`);
  if (state.presentationMode === 'operator' && !state.layoutEditMode) {
    card.classList.add('is-drilldown');
  }
  card.dataset.id = widget.id;
  card.dataset.quality = 'stale';
  if (state.routeTarget && (state.routeTarget === widget.id || state.routeTarget === widget.path)) {
    card.classList.add('card-focus-target');
  }

  if (Number.isFinite(widget.widget_span)) {
    const span = Math.max(1, Math.min(12, Math.trunc(Number(widget.widget_span))));
    card.style.setProperty('--widget-span', String(span));
  }

  const head = document.createElement('div');
  head.className = 'card-head';

  const titleWrap = document.createElement('div');
  titleWrap.className = 'card-title-wrap';

  const title = document.createElement('h3');
  title.className = 'card-title';
  title.textContent = widget.label || widget.path;

  const path = document.createElement('p');
  path.className = 'card-path';
  path.textContent = widget.path;

  titleWrap.appendChild(title);
  titleWrap.appendChild(path);

  const tag = document.createElement('span');
  tag.className = 'widget-tag';
  tag.textContent = widget.widget;

  head.appendChild(titleWrap);
  head.appendChild(tag);

  const value = document.createElement('div');
  value.className = 'card-value';
  const apply = createWidgetRenderer(widget, value);

  const meta = document.createElement('div');
  meta.className = 'card-meta';
  meta.textContent = widgetMeta(widget);

  const actions = document.createElement('div');
  actions.className = 'card-actions';
  for (const action of [
    { id: 'move', label: 'Move' },
    { id: 'pin', label: 'Pin' },
    { id: 'hide', label: 'Hide' },
    { id: 'label', label: 'Label' },
    { id: 'type', label: 'Widget' },
    { id: 'span', label: 'Size' },
  ]) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'card-action';
    button.textContent = action.label;
    button.addEventListener('click', async (event) => {
      event.stopPropagation();
      await runWidgetLayoutAction(widget, action.id);
    });
    actions.appendChild(button);
  }

  card.appendChild(head);
  card.appendChild(value);
  if (widget.unit) {
    const unitEl = document.createElement('div');
    unitEl.className = 'card-unit';
    unitEl.textContent = widget.unit;
    card.appendChild(unitEl);
  }
  card.appendChild(meta);
  card.appendChild(actions);
  card.addEventListener('click', () => {
    handleCardDrilldown(widget);
  });

  state.cards.set(widget.id, {
    card,
    value,
    widget,
    apply,
    lastValueSignature: undefined,
  });
  return card;
}

function renderGroupedWidgets(groupsRoot, widgets) {
  const grouped = new Map();
  for (const widget of widgets) {
    const group = widget.group || 'General';
    if (!grouped.has(group)) {
      grouped.set(group, []);
    }
    grouped.get(group).push(widget);
  }

  for (const [groupName, entries] of grouped.entries()) {
    const section = document.createElement('section');
    section.className = 'group-section';

    const heading = document.createElement('h2');
    heading.className = 'group-title';
    heading.textContent = groupName;
    section.appendChild(heading);

    const grid = document.createElement('div');
    grid.className = 'grid';

    for (const widget of entries) {
      grid.appendChild(createWidgetCard(widget));
    }

    section.appendChild(grid);
    groupsRoot.appendChild(section);
  }
}

function renderSectionWidgets(groupsRoot, widgets, page) {
  const sectionDefs = Array.isArray(page?.sections) ? page.sections : [];
  if (!sectionDefs.length) {
    renderGroupedWidgets(groupsRoot, widgets);
    return;
  }

  const widgetById = new Map(widgets.map((widget) => [widget.id, widget]));
  const used = new Set();
  const sectionGrid = document.createElement('div');
  sectionGrid.className = 'section-grid';

  const isDashboard = (page?.kind || 'dashboard').toLowerCase() === 'dashboard';

  for (let sectionIndex = 0; sectionIndex < sectionDefs.length; sectionIndex += 1) {
    const sectionDef = sectionDefs[sectionIndex];

    // On dashboard pages, hide sections where every widget is inferred
    if (isDashboard) {
      const ids = Array.isArray(sectionDef?.widget_ids) ? sectionDef.widget_ids : [];
      const resolved = ids.map((id) => widgetById.get(id)).filter(Boolean);
      if (resolved.length > 0 && resolved.every((w) => w.inferred_interface === true)) {
        continue;
      }
    }

    const section = document.createElement('section');
    section.className = 'group-section hmi-section';
    const span = Number.isFinite(sectionDef?.span)
      ? Math.max(1, Math.min(12, Math.trunc(Number(sectionDef.span))))
      : 12;
    section.style.setProperty('--section-span', String(span));
    if (sectionDef?.tier) {
      section.dataset.tier = sectionDef.tier;
    }

    const head = document.createElement('div');
    head.className = 'section-head';
    const heading = document.createElement('h2');
    heading.className = 'group-title';
    heading.textContent = sectionDef?.title || 'Section';
    head.appendChild(heading);

    const actions = document.createElement('div');
    actions.className = 'section-actions';
    for (const action of [
      { id: 'rename', label: 'Rename' },
      { id: 'up', label: 'Up' },
      { id: 'down', label: 'Down' },
      { id: 'add', label: 'Add' },
    ]) {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'section-action';
      button.textContent = action.label;
      button.addEventListener('click', async (event) => {
        event.stopPropagation();
        await runSectionLayoutAction(page?.id, sectionIndex, action.id);
      });
      actions.appendChild(button);
    }
    head.appendChild(actions);
    section.appendChild(head);

    const widgetIds = Array.isArray(sectionDef?.widget_ids) ? sectionDef.widget_ids : [];
    const isModuleStrip = sectionDef?.tier === 'module';

    if (isModuleStrip) {
      const strip = document.createElement('div');
      strip.className = 'equipment-strip';
      const meta = Array.isArray(sectionDef?.module_meta) ? sectionDef.module_meta : [];
      const metaById = new Map(meta.map((m) => [m.id, m]));
      let blockCount = 0;
      for (const id of widgetIds) {
        if (typeof id !== 'string') continue;
        const widget = widgetById.get(id);
        if (!widget) continue;
        used.add(id);
        if (blockCount > 0) {
          const arrow = document.createElement('span');
          arrow.className = 'equipment-strip-arrow';
          arrow.textContent = '\u2192';
          strip.appendChild(arrow);
        }
        const m = metaById.get(id);
        const displayWidget = m
          ? { ...widget, label: m.label || widget.label, detail_page: m.detail_page || widget.detail_page, unit: m.unit || widget.unit }
          : widget;
        strip.appendChild(createEquipmentBlock(displayWidget));
        blockCount += 1;
      }
      if (!strip.childElementCount) continue;
      section.appendChild(strip);
    } else {
      const grid = document.createElement('div');
      grid.className = 'section-widget-grid';
      for (const id of widgetIds) {
        if (typeof id !== 'string') continue;
        const widget = widgetById.get(id);
        if (!widget) continue;
        used.add(id);
        grid.appendChild(createWidgetCard(widget));
      }
      if (!grid.childElementCount) continue;
      section.appendChild(grid);
    }
    sectionGrid.appendChild(section);
  }

  if (!sectionGrid.childElementCount || used.size === 0) {
    renderGroupedWidgets(groupsRoot, widgets);
    return;
  }

  groupsRoot.appendChild(sectionGrid);
}

function renderWidgets() {
  const groupsRoot = byId('hmiGroups');
  if (!groupsRoot) {
    return;
  }

  groupsRoot.classList.remove('hidden');
  groupsRoot.innerHTML = '';
  state.cards.clear();
  state.moduleCards.clear();

  const widgets = visibleWidgets();
  if (!widgets.length) {
    setEmptyMessage('No user-visible variables discovered for this page.');
    return;
  }
  hideEmptyMessage();

  renderSectionWidgets(groupsRoot, widgets, currentPage());
}

function applyValues(payload) {
  if (!payload || typeof payload !== 'object') {
    setConnection('disconnected');
    setFreshness(null);
    return;
  }

  const connected = payload.connected === true;
  setConnection(connected ? 'connected' : 'stale');
  setFreshness(payload.timestamp_ms);

  const values = payload.values && typeof payload.values === 'object' ? payload.values : {};
  state.latestValues.clear();
  for (const [id, entry] of Object.entries(values)) {
    state.latestValues.set(id, entry);
  }
  for (const [id, refs] of state.cards.entries()) {
    const entry = values[id];
    applyCardEntry(refs, entry);
  }
  for (const [id, refs] of state.moduleCards.entries()) {
    const entry = values[id];
    applyCardEntry(refs, entry);
  }
  updateDiagnosticsPill();
}

async function refreshValues() {
  const ids = Array.from(new Set([...state.cards.keys(), ...state.moduleCards.keys()]));
  const extraIds = [];
  for (const refs of state.cards.values()) {
    const peerId = setpointPeerWidgetId(refs.widget);
    if (peerId && !ids.includes(peerId) && !extraIds.includes(peerId)) {
      extraIds.push(peerId);
    }
  }
  const requestIds = ids.concat(extraIds);
  if (!requestIds.length) {
    setConnection('stale');
    setFreshness(null);
    return;
  }
  try {
    const response = await apiControl('hmi.values.get', { ids: requestIds });
    if (!response.ok) {
      throw new Error(response.error || 'values request failed');
    }
    applyValues(response.result);
  } catch (_error) {
    setConnection('disconnected');
    setFreshness(null);
  }
}

function resolveTrendIds(page) {
  const focusedSignal = state.routeSignal;
  if (focusedSignal) {
    const byPath = new Map((state.schema?.widgets || []).map((widget) => [widget.path, widget.id]));
    const focusedId = byPath.get(focusedSignal) || focusedSignal;
    return [focusedId];
  }
  if (!Array.isArray(page?.signals) || !page.signals.length) {
    return undefined;
  }
  const byPath = new Map((state.schema?.widgets || []).map((widget) => [widget.path, widget.id]));
  const ids = page.signals
    .map((signal) => {
      if (typeof signal !== 'string') {
        return undefined;
      }
      return byPath.get(signal) || signal;
    })
    .filter((value) => typeof value === 'string' && value.length > 0);
  return ids.length ? ids : undefined;
}

function trendSvg(points) {
  if (!Array.isArray(points) || !points.length) {
    return '<svg class="trend-svg" viewBox="0 0 300 92"></svg>';
  }
  const width = 300;
  const height = 92;
  const values = points.flatMap((point) => [Number(point.min), Number(point.max), Number(point.value)]);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = Math.max(1e-9, max - min);
  const toY = (value) => {
    const normalized = (value - min) / span;
    return Math.round((height - 6) - normalized * (height - 14));
  };
  const toX = (index) => {
    if (points.length <= 1) {
      return 0;
    }
    return Math.round((index / (points.length - 1)) * width);
  };

  const avgPoints = points.map((point, idx) => ({
    x: toX(idx),
    y: toY(Number(point.value)),
  }));
  const avg = avgPoints.map((point) => `${point.x},${point.y}`).join(' ');
  const upper = points
    .map((point, idx) => `${toX(idx)},${toY(Number(point.max))}`)
    .join(' ');
  const lower = [...points]
    .reverse()
    .map((point, idx) => {
      const x = toX(points.length - 1 - idx);
      return `${x},${toY(Number(point.min))}`;
    })
    .join(' ');
  const band = `${upper} ${lower}`;
  return `<svg class="trend-svg" viewBox="0 0 ${width} ${height}" preserveAspectRatio="none"><polygon class="trend-band" points="${band}"></polygon><polyline class="trend-line" points="${avg}"></polyline></svg>`;
}

function renderTrends(page, result) {
  const panel = byId('trendPanel');
  if (!panel) {
    return;
  }
  panel.classList.remove('hidden');
  panel.innerHTML = '';

  const title = document.createElement('h2');
  title.className = 'panel-head';
  title.textContent = page?.title || 'Trends';
  panel.appendChild(title);

  const presetWrap = document.createElement('div');
  presetWrap.className = 'trend-presets';
  for (const preset of [
    { label: '1m', ms: 60 * 1000 },
    { label: '10m', ms: 10 * 60 * 1000 },
    { label: '1h', ms: 60 * 60 * 1000 },
  ]) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'trend-preset';
    button.textContent = preset.label;
    const activeDuration = Number.isFinite(state.trendDurationMs) && state.trendDurationMs > 0
      ? state.trendDurationMs
      : (Number(page?.duration_ms) || 10 * 60 * 1000);
    if (activeDuration === preset.ms) {
      button.classList.add('active');
    }
    button.addEventListener('click', () => {
      state.trendDurationMs = preset.ms;
      void refreshTrends(page);
    });
    presetWrap.appendChild(button);
  }
  panel.appendChild(presetWrap);

  const series = Array.isArray(result?.series) ? result.series : [];
  if (!series.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = 'No numeric signals available for trend visualization.';
    panel.appendChild(empty);
    return;
  }

  const grid = document.createElement('div');
  grid.className = 'trend-grid';
  const focusedSignal = state.routeSignal;
  const focusedWidgetId = focusedSignal
    ? (state.schema?.widgets || []).find((widget) => widget.path === focusedSignal || widget.id === focusedSignal)?.id || focusedSignal
    : null;

  for (const entry of series) {
    const card = document.createElement('article');
    card.className = 'trend-card';
    if (focusedWidgetId && entry.id === focusedWidgetId) {
      card.classList.add('focused');
    }

    const heading = document.createElement('h3');
    heading.textContent = entry.label || entry.id;

    const meta = document.createElement('p');
    meta.className = 'trend-meta';
    const last = Array.isArray(entry.points) && entry.points.length
      ? Number(entry.points[entry.points.length - 1].value)
      : undefined;
    meta.textContent = `last: ${last === undefined ? '--' : formatValue(last)}${entry.unit ? ` ${entry.unit}` : ''}`;

    const svgHost = document.createElement('div');
    svgHost.innerHTML = trendSvg(Array.isArray(entry.points) ? entry.points : []);

    card.appendChild(heading);
    card.appendChild(meta);
    card.appendChild(svgHost);
    grid.appendChild(card);
  }

  panel.appendChild(grid);
}

async function refreshTrends(page) {
  const selectedDuration = Number.isFinite(state.trendDurationMs) && state.trendDurationMs > 0
    ? state.trendDurationMs
    : (Number(page?.duration_ms) || 10 * 60 * 1000);
  const params = {
    duration_ms: selectedDuration,
    buckets: 120,
  };
  const ids = resolveTrendIds(page);
  if (ids) {
    params.ids = ids;
  }
  try {
    const response = await apiControl('hmi.trends.get', params);
    if (!response.ok) {
      throw new Error(response.error || 'trends request failed');
    }
    const result = response.result || {};
    setConnection(result.connected ? 'connected' : 'stale');
    setFreshness(result.timestamp_ms || null);
    renderTrends(page, result);
  } catch (_error) {
    setConnection('disconnected');
    setFreshness(null);
    setEmptyMessage('Trend data unavailable.');
  }
}

function renderAlarmTable(result) {
  const panel = byId('alarmPanel');
  if (!panel) {
    return;
  }
  panel.classList.remove('hidden');
  panel.innerHTML = '';

  const title = document.createElement('h2');
  title.className = 'panel-head';
  title.textContent = 'Alarms';
  panel.appendChild(title);

  const active = Array.isArray(result?.active) ? result.active : [];
  if (!active.length) {
    const emptyState = document.createElement('section');
    emptyState.className = 'alarm-empty-state';

    const emptyTitle = document.createElement('p');
    emptyTitle.className = 'alarm-empty-title';
    emptyTitle.textContent = 'No active alarms';

    const emptyBody = document.createElement('div');
    emptyBody.className = 'empty alarm-empty-body';
    emptyBody.textContent = 'Alarm thresholds are configured and being monitored in real time.';

    emptyState.appendChild(emptyTitle);
    emptyState.appendChild(emptyBody);
    panel.appendChild(emptyState);
  } else {
    const table = document.createElement('table');
    table.className = 'alarm-table';
    table.innerHTML = '<thead><tr><th>State</th><th>Signal</th><th>Value</th><th>Range</th><th>Action</th></tr></thead>';
    const body = document.createElement('tbody');

    for (const alarm of active) {
      const row = document.createElement('tr');
      const focusTarget = alarm.path || alarm.widget_id || alarm.id;
      if (focusTarget) {
        row.dataset.focus = focusTarget;
        row.addEventListener('click', (event) => {
          if (event.target && event.target.closest && event.target.closest('button')) {
            return;
          }
          const processPage = pageIdByKind('process');
          if (processPage) {
            navigateToPage(processPage, { focus: focusTarget });
          }
        });
      }

      const stateCell = document.createElement('td');
      const chip = document.createElement('span');
      chip.className = `alarm-chip ${alarm.state || 'raised'}`;
      chip.textContent = alarm.state || 'raised';
      stateCell.appendChild(chip);

      const signalCell = document.createElement('td');
      signalCell.textContent = alarm.label || alarm.path || alarm.id;

      const valueCell = document.createElement('td');
      valueCell.textContent = formatValue(alarm.value);

      const rangeCell = document.createElement('td');
      const min = typeof alarm.min === 'number' ? alarm.min : '-∞';
      const max = typeof alarm.max === 'number' ? alarm.max : '+∞';
      rangeCell.textContent = `[${min}..${max}]`;

      const actionCell = document.createElement('td');
      const ack = document.createElement('button');
      ack.type = 'button';
      ack.className = 'alarm-ack';
      ack.textContent = 'Acknowledge';
      const alarmKey = String(alarm.id || '');
      ack.disabled = alarm.acknowledged === true || state.ackInFlight.has(alarmKey);
      ack.addEventListener('click', async () => {
        await acknowledgeAlarm(alarmKey);
      });
      actionCell.appendChild(ack);

      row.appendChild(stateCell);
      row.appendChild(signalCell);
      row.appendChild(valueCell);
      row.appendChild(rangeCell);
      row.appendChild(actionCell);
      body.appendChild(row);
    }

    table.appendChild(body);
    panel.appendChild(table);
  }

  const history = Array.isArray(result?.history) ? result.history : [];
  if (history.length) {
    const historyWrap = document.createElement('section');
    historyWrap.className = 'alarm-history';
    const heading = document.createElement('h3');
    heading.className = 'panel-head';
    heading.textContent = 'Recent History';
    const list = document.createElement('ul');

    for (const item of history) {
      const line = document.createElement('li');
      const ts = item.timestamp_ms ? new Date(Number(item.timestamp_ms)).toLocaleTimeString() : '--:--:--';
      line.textContent = `${ts} · ${item.event || 'event'} · ${item.label || item.path || item.id}`;
      list.appendChild(line);
    }

    historyWrap.appendChild(heading);
    historyWrap.appendChild(list);
    panel.appendChild(historyWrap);
  }
}

async function acknowledgeAlarm(id) {
  if (!id) {
    return;
  }
  if (state.ackInFlight.has(id)) {
    return;
  }
  state.ackInFlight.add(id);
  try {
    const response = await apiControl('hmi.alarm.ack', { id });
    if (!response.ok) {
      throw new Error(response.error || 'ack failed');
    }
    renderAlarmTable(response.result || {});
  } catch (_error) {
    await refreshAlarms();
  } finally {
    state.ackInFlight.delete(id);
  }
}

async function refreshAlarms() {
  try {
    const response = await apiControl('hmi.alarms.get', { limit: 50 });
    if (!response.ok) {
      throw new Error(response.error || 'alarms request failed');
    }
    const result = response.result || {};
    state.lastAlarmResult = result;
    updateAlarmBanner();
    setConnection(result.connected ? 'connected' : 'stale');
    setFreshness(result.timestamp_ms || null);
    renderAlarmTable(result);
  } catch (_error) {
    setConnection('disconnected');
    setFreshness(null);
    setEmptyMessage('Alarm data unavailable.');
  }
}

async function fetchProcessSvg(page) {
  if (!page || typeof page.svg !== 'string' || !page.svg.trim()) {
    throw new Error('process page missing svg');
  }
  const key = page.svg.trim();
  if (state.processSvgCache.has(key)) {
    return state.processSvgCache.get(key);
  }
  const response = await fetch(`/hmi/assets/${encodeURIComponent(key)}`);
  if (!response.ok) {
    throw new Error(`svg fetch failed (${response.status})`);
  }
  const text = await response.text();
  state.processSvgCache.set(key, text);
  return text;
}

function resolveProcessAssetPath(pageSvg, reference) {
  if (typeof reference !== 'string') {
    return null;
  }
  const trimmed = reference.trim();
  if (!trimmed || trimmed.startsWith('#') || trimmed.startsWith('/')) {
    return null;
  }
  if (/^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(trimmed)) {
    return null;
  }
  const pathOnly = trimmed.split('#', 1)[0].split('?', 1)[0];
  if (!pathOnly) {
    return null;
  }

  const resolved = [];
  if (typeof pageSvg === 'string' && pageSvg.trim()) {
    const baseSegments = pageSvg.trim().split('/');
    for (const segmentRaw of baseSegments) {
      const segment = segmentRaw.trim();
      if (!segment || segment === '.') {
        continue;
      }
      if (segment === '..') {
        if (!resolved.length) {
          return null;
        }
        resolved.pop();
        continue;
      }
      resolved.push(segment);
    }
    if (resolved.length) {
      resolved.pop();
    }
  }

  for (const segmentRaw of pathOnly.split('/')) {
    const segment = segmentRaw.trim();
    if (!segment || segment === '.') {
      continue;
    }
    if (segment === '..') {
      if (!resolved.length) {
        return null;
      }
      resolved.pop();
      continue;
    }
    resolved.push(segment);
  }

  if (!resolved.length) {
    return null;
  }
  return resolved.join('/');
}

function rewriteProcessAssetReferences(svgRoot, pageSvg) {
  if (!svgRoot || typeof svgRoot.querySelectorAll !== 'function') {
    return;
  }
  const nodes = Array.from(svgRoot.querySelectorAll('[href], [xlink\\:href]'));
  for (const node of nodes) {
    if (!node || typeof node.getAttribute !== 'function' || typeof node.setAttribute !== 'function') {
      continue;
    }
    for (const attributeName of ['href', 'xlink:href']) {
      const current = node.getAttribute(attributeName);
      const path = resolveProcessAssetPath(pageSvg, current);
      if (!path) {
        continue;
      }
      const assetRoute = `/hmi/assets/${encodeURIComponent(path)}`;
      node.setAttribute(attributeName, assetRoute);
    }
  }
}

function buildProcessBindings(page, svgRoot) {
  const byPath = new Map((state.schema?.widgets || []).map((widget) => [widget.path, widget.id]));
  const bindingsByWidgetId = new Map();
  const widgetIds = [];
  let missingBindings = 0;
  const bindings = Array.isArray(page?.bindings) ? page.bindings : [];
  for (const binding of bindings) {
    if (!binding || typeof binding !== 'object') {
      missingBindings += 1;
      continue;
    }
    const selector = typeof binding.selector === 'string' ? binding.selector.trim() : '';
    const attribute = typeof binding.attribute === 'string' ? binding.attribute.trim().toLowerCase() : '';
    const source = typeof binding.source === 'string' ? binding.source.trim() : '';
    if (!isSafeProcessSelector(selector) || !isSafeProcessAttribute(attribute) || !source) {
      missingBindings += 1;
      continue;
    }
    const target = svgRoot.querySelector(selector);
    if (!target) {
      missingBindings += 1;
      continue;
    }
    const widgetId = byPath.get(source) || source;
    if (!byPath.has(source) && !source.startsWith('resource/')) {
      missingBindings += 1;
    }
    if (!bindingsByWidgetId.has(widgetId)) {
      bindingsByWidgetId.set(widgetId, []);
      widgetIds.push(widgetId);
    }
    bindingsByWidgetId.get(widgetId).push({
      widgetId,
      target,
      selector,
      attribute,
      source,
      format: typeof binding.format === 'string' ? binding.format : null,
      map: binding.map && typeof binding.map === 'object' ? binding.map : null,
      scale: binding.scale && typeof binding.scale === 'object' ? binding.scale : null,
    });
  }
  return {
    widgetIds,
    bindingsByWidgetId,
    missingBindings,
  };
}

function applyProcessFocusTarget(processView, focus) {
  if (!processView || !focus) {
    return;
  }
  const normalized = String(focus).trim();
  if (!normalized) {
    return;
  }
  for (const bindings of processView.bindingsByWidgetId.values()) {
    for (const binding of bindings) {
      if (binding.target && binding.target.classList) {
        binding.target.classList.remove('process-focus');
      }
      const widgetMatches = binding.widgetId === normalized || binding.source === normalized;
      if (widgetMatches && binding.target && binding.target.classList) {
        binding.target.classList.add('process-focus');
      }
    }
  }
}

async function renderProcessPage(page) {
  const groups = byId('hmiGroups');
  if (!groups) {
    return;
  }
  const renderSeq = state.processRenderSeq + 1;
  state.processRenderSeq = renderSeq;
  groups.classList.remove('hidden');
  groups.innerHTML = '<section class="process-panel"><div class="empty">Loading process view...</div></section>';
  hideEmptyMessage();

  try {
    const svgText = await fetchProcessSvg(page);
    if (renderSeq !== state.processRenderSeq || state.currentPage !== page?.id) {
      return;
    }
    const parser = new DOMParser();
    const doc = parser.parseFromString(svgText, 'image/svg+xml');
    const parseError = doc.querySelector('parsererror');
    if (parseError) {
      throw new Error('invalid SVG payload');
    }
    const svgRoot = doc.documentElement;
    if (!svgRoot || svgRoot.tagName.toLowerCase() !== 'svg') {
      throw new Error('missing svg root');
    }
    for (const tag of ['script', 'foreignObject', 'iframe', 'object', 'embed']) {
      for (const node of Array.from(svgRoot.querySelectorAll(tag))) {
        node.remove();
      }
    }
    rewriteProcessAssetReferences(svgRoot, page.svg);

    const bindings = buildProcessBindings(page, svgRoot);
    state.processView = {
      pageId: page.id,
      widgetIds: bindings.widgetIds,
      bindingsByWidgetId: bindings.bindingsByWidgetId,
    };
    state.processBindingMisses = bindings.missingBindings;
    applyProcessFocusTarget(state.processView, state.routeFocus);
    updateDiagnosticsPill();

    const panel = document.createElement('section');
    panel.className = 'process-panel';
    const heading = document.createElement('h2');
    heading.className = 'panel-head';
    heading.textContent = page?.title || 'Process';
    const host = document.createElement('div');
    host.className = 'process-svg-host';
    host.appendChild(svgRoot);
    panel.appendChild(heading);
    panel.appendChild(host);
    groups.innerHTML = '';
    groups.appendChild(panel);
    await refreshProcessValues();
  } catch (error) {
    state.processView = null;
    if (renderSeq !== state.processRenderSeq || state.currentPage !== page?.id) {
      return;
    }
    setEmptyMessage(`Process view unavailable: ${error}`);
  }
}

async function refreshProcessValues() {
  const processView = state.processView;
  if (!processView || !Array.isArray(processView.widgetIds) || !processView.widgetIds.length) {
    setConnection('stale');
    return;
  }
  try {
    const response = await apiControl('hmi.values.get', { ids: processView.widgetIds });
    if (!response.ok) {
      throw new Error(response.error || 'process values request failed');
    }
    const result = response.result || {};
    setConnection(result.connected ? 'connected' : 'stale');
    setFreshness(result.timestamp_ms || null);
    const values = result.values && typeof result.values === 'object' ? result.values : {};
    applyProcessValueEntries(values, result.timestamp_ms || null);
  } catch (_error) {
    setConnection('disconnected');
    setFreshness(null);
  }
}

function renderCurrentPage() {
  hideContentPanels();
  ensureCurrentPage();

  if (!state.currentPage) {
    setEmptyMessage('No pages configured.');
    updateDiagnosticsPill();
    return;
  }

  hideEmptyMessage();
  const page = currentPage();
  const kind = currentPageKind();

  if (kind === 'trend') {
    const panel = byId('trendPanel');
    if (panel) {
      panel.classList.remove('hidden');
      panel.innerHTML = `<h2 class="panel-head">${page?.title || 'Trends'}</h2><div class="empty">Collecting trend samples...</div>`;
    }
    updateDiagnosticsPill();
    return;
  }

  if (kind === 'alarm') {
    const panel = byId('alarmPanel');
    if (panel) {
      if (state.lastAlarmResult) {
        renderAlarmTable(state.lastAlarmResult);
      } else {
        panel.classList.remove('hidden');
        panel.innerHTML = '<h2 class="panel-head">Alarms</h2><div class="empty">Loading alarms...</div>';
      }
    }
    updateDiagnosticsPill();
    return;
  }

  if (kind === 'process') {
    void renderProcessPage(page);
    updateDiagnosticsPill();
    return;
  }

  renderWidgets();
  updateDiagnosticsPill();
}

async function refreshActivePage(options = {}) {
  if (!state.schema) {
    return;
  }
  const page = currentPage();
  const kind = currentPageKind();
  const forceValues = options.forceValues === true;

  if (kind === 'trend') {
    await refreshTrends(page);
    return;
  }
  if (kind === 'alarm') {
    await refreshAlarms();
    return;
  }
  if (kind === 'process') {
    await refreshProcessValues();
    return;
  }
  if (state.wsConnected && !forceValues) {
    return;
  }
  await refreshValues();
}

function renderSchema(schema) {
  state.schema = schema;
  state.schemaRevision = Number(schema?.schema_revision) || 0;
  state.descriptorError = typeof schema?.descriptor_error === 'string'
    ? schema.descriptor_error
    : null;
  const mode = byId('modeLabel');
  if (mode) {
    mode.textContent = schema.read_only ? 'read-only' : 'read-write';
  }

  const exportLink = byId('exportLink');
  if (exportLink) {
    if (schema.export && schema.export.enabled && typeof schema.export.route === 'string') {
      exportLink.href = schema.export.route;
      exportLink.classList.remove('hidden');
    } else {
      exportLink.classList.add('hidden');
    }
  }

  applyTheme(schema.theme);
  applyResponsiveLayout();
  ensureCurrentPage();
  applyRoute(
    {
      page: state.currentPage,
      signal: state.routeSignal,
      focus: state.routeFocus,
      target: state.routeTarget,
    },
    true,
  );
  renderSidebar();
  renderCurrentPage();
}

async function init() {
  syncStateFromRoute();
  initModeControls();
  try {
    const response = await apiControl('hmi.schema.get');
    if (!response.ok) {
      throw new Error(response.error || 'schema request failed');
    }
    renderSchema(response.result);
    await refreshDescriptorModel();
    await refreshActivePage({ forceValues: true });
    ensurePollingLoop();
    connectWebSocketTransport();
  } catch (error) {
    setEmptyMessage(`HMI unavailable: ${error}`);
    setConnection('disconnected');
    setFreshness(null);
  }
}

window.addEventListener('resize', () => {
  if (!state.schema) {
    return;
  }
  if (state.responsiveMode === 'auto') {
    applyResponsiveLayout();
  }
});

window.addEventListener('DOMContentLoaded', init);
