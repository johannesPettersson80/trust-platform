// Contract markers for integration tests:
// hmi.schema.get
// hmi.values.get
// hmi.trends.get
// hmi.alarms.get
// hmi.alarm.ack
// connectWebSocketTransport
// /ws/hmi
// hmi.values.delta
// hmi.schema.revision
// renderProcessPage
// function renderProcessPage
// /hmi/assets/
// section-grid
// section-widget-grid
// createGaugeRenderer
// createSparklineRenderer
// kind === 'sparkline'
// createBarRenderer
// createTankRenderer
// createIndicatorRenderer
// createToggleRenderer
// createSliderRenderer
const POLL_MS = 500;
const CONNECTOR_POLL_MS = 2000;
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
  connectors: [],
  connectorPollHandle: null,
  connectorStatusError: null,
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

const CONNECTOR_STATE_ORDER = [
  'ready',
  'degraded',
  'reconnecting',
  'stale',
  'not_ready',
  'faulted',
  'starting',
  'configured',
  'disabled',
];
const CONNECTOR_HEALTH_ORDER = ['ok', 'degraded', 'faulted', 'unknown'];
const CONNECTOR_CONFIDENCE_ORDER = ['confirmed', 'likely', 'port_reachable', 'unavailable'];

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

function connectorToken(value, fallback = 'unknown') {
  const token = String(value || fallback).trim().toLowerCase();
  return token || fallback;
}

function connectorLabel(value) {
  const token = connectorToken(value);
  if (token === 'not_ready') {
    return 'not ready';
  }
  return token.replace(/_/g, ' ');
}

function connectionDisplayLabel(value) {
  const token = connectorToken(value);
  if (token === 'ready') {
    return 'ready';
  }
  if (['degraded', 'stale', 'not_ready'].includes(token)) {
    return 'needs attention';
  }
  if (token === 'faulted') {
    return 'fault';
  }
  return connectorLabel(token);
}

function verificationDisplayLabel(value) {
  const token = connectorToken(value);
  if (token === 'port_reachable') {
    return 'port reachable only';
  }
  if (token === 'unavailable') {
    return 'not verified';
  }
  return connectorLabel(token);
}

function healthDisplayLabel(value) {
  const token = connectorToken(value);
  if (token === 'ok') {
    return 'OK';
  }
  if (token === 'faulted') {
    return 'fault';
  }
  return connectorLabel(token);
}

function countConnectorField(connectors, field) {
  const counts = new Map();
  for (const connector of connectors) {
    const key = connectorToken(connector?.[field]);
    counts.set(key, (counts.get(key) || 0) + 1);
  }
  return counts;
}

function orderedCountEntries(counts, order) {
  const entries = [];
  for (const key of order) {
    const count = counts.get(key) || 0;
    if (count > 0) {
      entries.push([key, count]);
    }
  }
  const extras = Array.from(counts.entries())
    .filter(([key, count]) => count > 0 && !order.includes(key))
    .sort(([left], [right]) => left.localeCompare(right));
  return [...entries, ...extras];
}

function formatDisplayCounts(prefix, counts, order, labeler) {
  const entries = orderedCountEntries(counts, order);
  if (entries.length === 0) {
    return `${prefix}: none`;
  }
  return `${prefix}: ${entries
    .map(([key, count]) => `${count} ${labeler(key)}`)
    .join(', ')}`;
}

function summarizeConnectionCounts(connectors) {
  let ready = 0;
  let attention = 0;
  for (const connector of connectors) {
    const stateValue = connectorToken(connector?.state);
    const health = connectorToken(connector?.health);
    const confidence = connectorToken(connector?.confidence);
    if (stateValue === 'ready' && health === 'ok' && confidence !== 'port_reachable') {
      ready += 1;
    } else {
      attention += 1;
    }
  }
  if (ready === 0 && attention === 0) {
    return 'Connections: none';
  }
  const parts = [];
  if (ready > 0) {
    parts.push(`${ready} ready`);
  }
  if (attention > 0) {
    parts.push(`${attention} needs attention`);
  }
  return `Connections: ${parts.join(', ')}`;
}

function summarizePointCounts(connectors) {
  let good = 0;
  let degraded = 0;
  let unavailable = 0;
  for (const connector of connectors) {
    const counts = connector && typeof connector.point_counts === 'object'
      ? connector.point_counts
      : {};
    good += Number(counts.good) || 0;
    degraded += Number(counts.degraded) || 0;
    unavailable += Number(counts.unavailable) || 0;
  }
  const issue = degraded + unavailable;
  return {
    text: issue > 0 ? `Signals: ${good} good, ${issue} need attention` : `Signals: ${good} good`,
    severity: issue > 0 ? 'stale' : 'connected',
    title: `Signals: ${good} good, ${issue} need attention`,
  };
}

function connectorSeverity(connectors) {
  if (!Array.isArray(connectors) || connectors.length === 0) {
    return 'stale';
  }
  const hasFault = connectors.some((connector) => {
    const health = connectorToken(connector?.health);
    const stateValue = connectorToken(connector?.state);
    return health === 'faulted' || stateValue === 'faulted';
  });
  if (hasFault) {
    return 'disconnected';
  }
  const hasDegraded = connectors.some((connector) => {
    const health = connectorToken(connector?.health);
    const stateValue = connectorToken(connector?.state);
    return (
      health === 'degraded' ||
      health === 'unknown' ||
      ['degraded', 'reconnecting', 'stale', 'not_ready', 'starting', 'configured'].includes(stateValue)
    );
  });
  return hasDegraded ? 'stale' : 'connected';
}

function setSummaryPill(id, text, severity, title = '') {
  const pill = byId(id);
  if (!pill) {
    return;
  }
  pill.classList.remove('connected', 'stale', 'disconnected');
  if (severity) {
    pill.classList.add(severity);
  }
  pill.textContent = text;
  pill.title = title;
}

function updateConnectorStatusSummary(result) {
  const connectors = Array.isArray(result?.connectors) ? result.connectors : [];
  state.connectors = connectors;
  state.connectorStatusError = null;
  const severity = connectorSeverity(connectors);
  const states = countConnectorField(connectors, 'state');
  const health = countConnectorField(connectors, 'health');
  const confidence = countConnectorField(connectors, 'confidence');
  const pointSummary = summarizePointCounts(connectors);
  setSummaryPill(
    'connectorSummaryState',
    summarizeConnectionCounts(connectors),
    confidence.get('port_reachable') ? 'stale' : severity,
    [
      formatDisplayCounts('Connection', states, CONNECTOR_STATE_ORDER, connectionDisplayLabel),
      formatDisplayCounts('Health', health, CONNECTOR_HEALTH_ORDER, healthDisplayLabel),
      formatDisplayCounts('Verification', confidence, CONNECTOR_CONFIDENCE_ORDER, verificationDisplayLabel),
      pointSummary.text,
    ].join('\n'),
  );
}

function markConnectorStatusUnavailable(error) {
  const detail = error instanceof Error ? error.message : String(error || 'unavailable');
  state.connectors = [];
  state.connectorStatusError = detail;
  setSummaryPill('connectorSummaryState', 'Connections: unavailable', 'stale', detail);
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
