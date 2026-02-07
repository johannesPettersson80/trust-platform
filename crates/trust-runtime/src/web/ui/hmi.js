const POLL_MS = 500;

const state = {
  schema: null,
  cards: new Map(),
  pollHandle: null,
  currentPage: null,
  responsiveMode: 'auto',
  ackInFlight: new Set(),
};

function byId(id) {
  return document.getElementById(id);
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

function applyTheme(theme) {
  if (!theme || typeof theme !== 'object') {
    return;
  }
  const root = document.documentElement;
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
  if (typeof theme.style === 'string') {
    const label = byId('themeLabel');
    if (label) {
      label.textContent = `theme: ${theme.style}`;
    }
  }
}

function parseResponsiveOverride() {
  const params = new URLSearchParams(window.location.search);
  const value = params.get('mode');
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

  const entries = pages();
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
      renderSidebar();
      renderCurrentPage();
      await refreshActivePage();
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
}

function visibleWidgets() {
  if (!state.schema || !Array.isArray(state.schema.widgets)) {
    return [];
  }
  if (!state.currentPage) {
    return state.schema.widgets;
  }
  return state.schema.widgets.filter((widget) => widget.page === state.currentPage);
}

function renderWidgets() {
  const groupsRoot = byId('hmiGroups');
  if (!groupsRoot) {
    return;
  }

  groupsRoot.classList.remove('hidden');
  groupsRoot.innerHTML = '';
  state.cards.clear();

  const widgets = visibleWidgets();
  if (!widgets.length) {
    setEmptyMessage('No user-visible variables discovered for this page.');
    return;
  }
  hideEmptyMessage();

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
      const card = document.createElement('article');
      card.className = 'card';
      card.dataset.id = widget.id;
      card.dataset.quality = 'stale';

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
      value.textContent = '--';

      const meta = document.createElement('div');
      meta.className = 'card-meta';
      meta.textContent = widgetMeta(widget);

      card.appendChild(head);
      card.appendChild(value);
      card.appendChild(meta);
      grid.appendChild(card);

      state.cards.set(widget.id, { card, value, widget });
    }

    section.appendChild(grid);
    groupsRoot.appendChild(section);
  }
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
  for (const [id, refs] of state.cards.entries()) {
    const entry = values[id];
    if (!entry || typeof entry !== 'object') {
      refs.card.dataset.quality = 'stale';
      refs.value.textContent = '--';
      refs.value.classList.remove('indicator-true', 'indicator-false');
      continue;
    }

    const quality = typeof entry.q === 'string' ? entry.q : 'stale';
    refs.card.dataset.quality = quality;

    refs.value.textContent = formatValue(entry.v);
    refs.value.classList.remove('indicator-true', 'indicator-false');

    if (refs.widget.widget === 'indicator') {
      const truthy = entry.v === true;
      refs.value.classList.add(truthy ? 'indicator-true' : 'indicator-false');
    }
  }
}

async function refreshValues() {
  const ids = Array.from(state.cards.keys());
  if (!ids.length) {
    setConnection('stale');
    setFreshness(null);
    return;
  }
  try {
    const response = await apiControl('hmi.values.get', { ids });
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
    return '<svg class="trend-svg" viewBox="0 0 320 120"></svg>';
  }
  const width = 320;
  const height = 120;
  const values = points.flatMap((point) => [Number(point.min), Number(point.max), Number(point.value)]);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = Math.max(1e-9, max - min);
  const toY = (value) => {
    const normalized = (value - min) / span;
    return Math.round((height - 8) - normalized * (height - 16));
  };
  const toX = (index) => {
    if (points.length <= 1) {
      return 0;
    }
    return Math.round((index / (points.length - 1)) * width);
  };

  const avg = points
    .map((point, idx) => `${toX(idx)},${toY(Number(point.value))}`)
    .join(' ');
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

  for (const entry of series) {
    const card = document.createElement('article');
    card.className = 'trend-card';

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
  const params = {
    duration_ms: Number(page?.duration_ms) || 10 * 60 * 1000,
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
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = 'No active alarms.';
    panel.appendChild(empty);
  } else {
    const table = document.createElement('table');
    table.className = 'alarm-table';
    table.innerHTML = '<thead><tr><th>State</th><th>Signal</th><th>Value</th><th>Range</th><th>Action</th></tr></thead>';
    const body = document.createElement('tbody');

    for (const alarm of active) {
      const row = document.createElement('tr');

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
    setConnection(result.connected ? 'connected' : 'stale');
    setFreshness(result.timestamp_ms || null);
    renderAlarmTable(result);
  } catch (_error) {
    setConnection('disconnected');
    setFreshness(null);
    setEmptyMessage('Alarm data unavailable.');
  }
}

function renderCurrentPage() {
  hideContentPanels();
  ensureCurrentPage();

  if (!state.currentPage) {
    setEmptyMessage('No pages configured.');
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
    return;
  }

  if (kind === 'alarm') {
    const panel = byId('alarmPanel');
    if (panel) {
      panel.classList.remove('hidden');
      panel.innerHTML = '<h2 class="panel-head">Alarms</h2><div class="empty">Loading alarms...</div>';
    }
    return;
  }

  if (kind === 'process') {
    setEmptyMessage('Process view is configured but SVG binding is not enabled in this build.');
    return;
  }

  renderWidgets();
}

async function refreshActivePage() {
  if (!state.schema) {
    return;
  }
  const page = currentPage();
  const kind = currentPageKind();

  if (kind === 'trend') {
    await refreshTrends(page);
    return;
  }
  if (kind === 'alarm') {
    await refreshAlarms();
    return;
  }
  await refreshValues();
}

function renderSchema(schema) {
  state.schema = schema;
  const resource = byId('resourceName');
  if (resource) {
    resource.textContent = `resource: ${schema.resource}`;
  }

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
  renderSidebar();
  renderCurrentPage();
}

async function init() {
  try {
    const response = await apiControl('hmi.schema.get');
    if (!response.ok) {
      throw new Error(response.error || 'schema request failed');
    }
    renderSchema(response.result);
    await refreshActivePage();
    state.pollHandle = window.setInterval(refreshActivePage, POLL_MS);
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
