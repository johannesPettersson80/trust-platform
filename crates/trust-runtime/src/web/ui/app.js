let authToken = localStorage.getItem('trustToken');
let currentState = 'unknown';
let setupDismissed = localStorage.getItem('trustSetupDismissed') === 'true';
let needsSetup = false;
let connectionOk = true;
let theme = localStorage.getItem('trustTheme');
let tourIndex = 0;
let tourActive = false;
let paletteVisible = false;
let tourPreviousFocus = null;
let palettePreviousFocus = null;
let shortcutsVisible = false;
let shortcutsPreviousFocus = null;
let debugEnabled = true;
let lastUptimeMs = null;
let lastWallClockMs = null;
let lastUpdateMs = null;
let currentPlcName = 'PLC';
let ioConfigState = {
  driver: 'loopback',
  params: {},
  drivers: [],
  supported_drivers: [],
  safe_state: [],
  use_system_io: false,
  source: 'project',
};
let ioConfigOriginal = null;
let meshPublishState = [];
let meshSubscribeState = [];
let programLoaded = false;
let watchList = [];
let watchValues = {};
let trendWindowMs = 60000;
let cycleTrendSamples = [];
let variableTrendSamples = {};
let selectedTrendVar = '';
let eventHistory = [];
let meshValueCache = {};
let lastFaultText = null;
let currentSourceName = '';
let currentPcLine = null;
let refreshTimer = null;
let discoveryTimer = null;
let initialLoad = true;

const fallbackSupportedIoDrivers = ['gpio', 'loopback', 'modbus-tcp', 'simulated', 'mqtt'];

const pageTitles = {
  overview: 'PLC Overview',
  io: 'I/O',
  logs: 'Logs',
  program: 'Program',
  deploy: 'Deploy',
  settings: 'Settings',
  network: 'Network',
};

const tabGroups = new Map();

function uniqueIoDrivers(drivers) {
  const names = Array.isArray(drivers)
    ? drivers.map(value => String(value || '').trim()).filter(Boolean)
    : [];
  const seen = new Set();
  const deduped = [];
  for (const name of names) {
    if (seen.has(name)) continue;
    seen.add(name);
    deduped.push(name);
  }
  return deduped.length ? deduped : fallbackSupportedIoDrivers;
}

function populateDriverSelect(selectId, drivers, includeAuto = false) {
  const select = document.getElementById(selectId);
  if (!select) return;
  const options = uniqueIoDrivers(drivers);
  const current = select.value;
  const values = includeAuto ? ['auto', ...options] : [...options];
  if (current && !values.includes(current)) {
    values.push(current);
  }
  select.innerHTML = values
    .map(value => `<option value="${value}">${value}</option>`)
    .join('');
  if (current) {
    select.value = current;
  }
}

function normalizeIoDriverConfigs(drivers, fallbackName = 'loopback') {
  const normalized = Array.isArray(drivers)
    ? drivers
      .map(entry => ({
        name: String(entry?.name || '').trim(),
        params: (entry?.params && typeof entry.params === 'object' && !Array.isArray(entry.params))
          ? entry.params
          : {},
      }))
      .filter(entry => entry.name)
    : [];
  if (!normalized.length) {
    return [{ name: fallbackName, params: {} }];
  }
  return normalized;
}

function driverParamsText(params) {
  return JSON.stringify(params || {}, null, 2);
}

function renderAdditionalIoDrivers() {
  const target = document.getElementById('ioAdditionalDrivers');
  if (!target) return;
  const entries = (ioConfigState.drivers || []).slice(1);
  if (!entries.length) {
    target.innerHTML = '<div class="empty">No additional drivers configured.</div>';
    return;
  }
  const supported = uniqueIoDrivers(ioConfigState.supported_drivers || fallbackSupportedIoDrivers);
  target.innerHTML = entries.map((entry, idx) => {
    const values = supported.includes(entry.name) ? [...supported] : [...supported, entry.name];
    const options = values
      .map(value => `<option value="${value}" ${value === entry.name ? 'selected' : ''}>${value}</option>`)
      .join('');
    return `
      <div class="card" style="padding:10px; margin-bottom:8px;">
        <div class="table-row" style="grid-template-columns: 1fr auto;">
          <select id="ioExtraDriverName${idx}">${options}</select>
          <button class="btn ghost" onclick="removeAdditionalIoDriver(${idx})">Remove</button>
        </div>
        <div class="field" style="margin-top:8px;">
          <label class="muted">Params (JSON object)</label>
          <textarea id="ioExtraDriverParams${idx}" rows="6">${escapeHtml(driverParamsText(entry.params))}</textarea>
        </div>
      </div>
    `;
  }).join('');
}

function addAdditionalIoDriver() {
  if (!Array.isArray(ioConfigState.drivers) || !ioConfigState.drivers.length) {
    ioConfigState.drivers = [{ name: ioConfigState.driver || 'loopback', params: ioConfigState.params || {} }];
  }
  const primary = String(ioConfigState.driver || 'loopback');
  const fallback = fallbackSupportedIoDrivers.find(name => name !== primary) || 'simulated';
  ioConfigState.drivers.push({ name: fallback, params: {} });
  renderAdditionalIoDrivers();
}

function removeAdditionalIoDriver(index) {
  const offset = Number(index) + 1;
  if (!Array.isArray(ioConfigState.drivers) || offset < 1 || offset >= ioConfigState.drivers.length) {
    return;
  }
  ioConfigState.drivers.splice(offset, 1);
  renderAdditionalIoDrivers();
}

function escapeHtml(value) {
  return String(value || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function showToast(message, type = 'info') {
  const toast = document.getElementById('toast');
  if (!toast) return;
  toast.textContent = message;
  toast.className = `toast ${type}`.trim();
  toast.hidden = false;
  clearTimeout(showToast._timer);
  showToast._timer = setTimeout(() => {
    toast.hidden = true;
  }, 3000);
}

function setHtml(id, html) {
  const el = document.getElementById(id);
  if (!el) return;
  el.classList.remove('skeleton');
  el.innerHTML = html;
}

function applySkeleton(id, lines = 3) {
  const el = document.getElementById(id);
  if (!el) return;
  el.classList.add('skeleton');
  el.innerHTML = Array.from({ length: lines }).map(() => '<div class="skeleton-line"></div>').join('');
}

function debounce(fn, wait = 200) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), wait);
  };
}

async function runBatched(tasks, size = 4) {
  for (let i = 0; i < tasks.length; i += size) {
    const batch = tasks.slice(i, i + size).map(task => task());
    await Promise.all(batch);
  }
}

async function withLoadingState(buttonId, statusId, label, action) {
  setLoading(buttonId, true, label);
  if (statusId) setStatus(statusId, '', '');
  try {
    return await action();
  } finally {
    setLoading(buttonId, false);
  }
}

function formatMs(value) {
  if (value === null || value === undefined) return '-';
  const num = Number(value);
  if (!Number.isFinite(num)) return '-';
  return `${num.toFixed(2)} ms`;
}

function formatEventTimestamp(event) {
  if (!event || event.time_ns == null || lastUptimeMs == null || lastWallClockMs == null) {
    return '';
  }
  const eventUptimeMs = Number(event.time_ns) / 1e6;
  if (!Number.isFinite(eventUptimeMs)) return '';
  const delta = lastUptimeMs - eventUptimeMs;
  if (!Number.isFinite(delta)) return '';
  const wallTime = lastWallClockMs - delta;
  if (!Number.isFinite(wallTime)) return '';
  return new Date(wallTime).toLocaleTimeString();
}

function applyTheme(next) {
  theme = next || 'light';
  document.body.dataset.theme = theme;
  const toggle = document.getElementById('themeToggle');
  if (toggle) {
    toggle.textContent = theme === 'dark' ? 'Light mode' : 'Dark mode';
  }
  localStorage.setItem('trustTheme', theme);
}

function toggleTheme() {
  applyTheme(theme === 'dark' ? 'light' : 'dark');
}

function applyInitialSkeletons() {
  applySkeleton('health', 4);
  applySkeleton('metrics', 6);
  applySkeleton('tasks', 4);
  applySkeleton('ioInputs', 4);
  applySkeleton('ioOutputs', 4);
  applySkeleton('ioDrivers', 3);
  applySkeleton('events', 4);
  applySkeleton('eventsSummary', 3);
  applySkeleton('faults', 3);
  applySkeleton('eventHistory', 5);
  applySkeleton('programMeta', 3);
  applySkeleton('sourceList', 3);
  applySkeleton('deployHistory', 3);
}

if (!theme) {
  theme = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches
    ? 'dark'
    : 'light';
}
applyTheme(theme);

function setStatus(id, message, type) {
  const el = document.getElementById(id);
  if (!el) return;
  if (!message) {
    el.hidden = true;
    el.textContent = '';
    return;
  }
  el.hidden = false;
  el.textContent = message;
  el.className = `status ${type || ''}`.trim();
  if (type === 'error') {
    el.setAttribute('role', 'alert');
  } else {
    el.setAttribute('role', 'status');
  }
  el.setAttribute('aria-live', 'polite');
}

function setLoading(id, loading, label) {
  const el = document.getElementById(id);
  if (!el) return;
  if (!el.dataset.label) {
    el.dataset.label = el.textContent.trim();
  }
  el.disabled = loading;
  const base = label || el.dataset.label;
  if (loading) {
    el.innerHTML = `<span class="spinner"></span>${base}`;
  } else {
    el.textContent = el.dataset.label;
  }
}

async function apiRequest(type, params) {
  const payload = { id: 1, type, params };
  try {
    const res = await fetch('/api/control', {
      method: 'POST',
      headers: Object.assign(
        { 'Content-Type': 'application/json' },
        authToken ? { 'X-Trust-Token': authToken } : {},
      ),
      body: JSON.stringify(payload),
    });
    if (res.status === 401) {
      return { ok: false, error: 'unauthorized' };
    }
    return res.json();
  } catch (err) {
    return { ok: false, error: 'offline' };
  }
}

function formatDuration(ms) {
  const total = Math.max(0, Math.round(ms));
  const seconds = Math.floor(total / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${seconds % 60}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

function formatIoValue(entry) {
  const raw = entry?.value;
  const type = entry?.type || entry?.data_type || entry?.kind;
  const suffix = type ? ` <span class="muted">(${escapeHtml(type)})</span>` : '';
  if (typeof raw === 'boolean') return `${raw ? 'TRUE' : 'FALSE'}${suffix}`;
  if (typeof raw === 'number') {
    const num = Number.isInteger(raw) ? raw : raw.toFixed(2);
    return `${num}${suffix}`;
  }
  const text = String(raw ?? '');
  const lower = text.toLowerCase();
  if (lower === 'true' || lower === 'false') {
    return `${lower === 'true' ? 'TRUE' : 'FALSE'}${suffix}`;
  }
  const parsed = Number(text);
  if (!Number.isNaN(parsed) && text.trim() !== '') {
    const formatted = Number.isInteger(parsed) ? parsed : parsed.toFixed(2);
    return `${formatted}${suffix}`;
  }
  return `${escapeHtml(text)}${suffix}`;
}

function formatEvent(event) {
  const type = event.type || 'event';
  const timestamp = formatEventTimestamp(event);
  const prefix = timestamp ? `${timestamp} · ` : '';
  return `${prefix}${formatEventBody(event)}`;
}

function formatEventBody(event) {
  const type = event.type || 'event';
  switch (type) {
    case 'task_overrun':
      return `Task overrun: ${event.name || 'task'} missed ${event.missed ?? '?'} cycle(s)`;
    case 'task_start':
      return `Task start: ${event.name || 'task'}`;
    case 'task_end':
      return `Task end: ${event.name || 'task'}`;
    case 'fault':
      return `Fault: ${event.error || 'unknown error'}`;
    case 'cycle_start':
      return `Cycle ${event.cycle ?? '?'} start`;
    case 'cycle_end':
      return `Cycle ${event.cycle ?? '?'} end`;
    default:
      return `${type}`;
  }
}

function eventWallTimeMs(event) {
  if (!event || event.time_ns == null || lastUptimeMs == null || lastWallClockMs == null) {
    return null;
  }
  const eventUptimeMs = Number(event.time_ns) / 1e6;
  if (!Number.isFinite(eventUptimeMs)) return null;
  const delta = lastUptimeMs - eventUptimeMs;
  if (!Number.isFinite(delta)) return null;
  const wallTime = lastWallClockMs - delta;
  return Number.isFinite(wallTime) ? wallTime : null;
}

function eventCategory(event) {
  if (!event) return 'info';
  if (event.type === 'fault') return 'fault';
  if (event.type === 'task_overrun') return 'warn';
  return 'info';
}

function faultSuggestion(text) {
  const lower = String(text || '').toLowerCase();
  if (lower.includes('modbus')) {
    return 'Verify Modbus address, unit ID, and network reachability.';
  }
  if (lower.includes('watchdog')) {
    return 'Increase watchdog timeout or reduce PLC cycle load.';
  }
  if (lower.includes('overrun')) {
    return 'Check task timing and reduce cycle interval.';
  }
  if (lower.includes('io')) {
    return 'Verify I/O mapping and driver configuration.';
  }
  return 'Review logs for details and restart the PLC when safe.';
}

function loadEventHistory() {
  try {
    eventHistory = JSON.parse(localStorage.getItem('trustEventHistory') || '[]');
  } catch {
    eventHistory = [];
  }
}

function saveEventHistory() {
  localStorage.setItem('trustEventHistory', JSON.stringify(eventHistory.slice(0, 500)));
}

function eventIdFor(event) {
  const id = [
    event.type || 'event',
    event.time_ns ?? '',
    event.name ?? '',
    event.cycle ?? '',
    event.error ?? '',
  ].join('|');
  return id;
}

function mergeEventHistory(list) {
  if (!Array.isArray(list) || !list.length) return;
  const existing = new Set(eventHistory.map(item => item.id));
  list.forEach(event => {
    const id = eventIdFor(event);
    if (existing.has(id)) return;
    const tsMs = eventWallTimeMs(event) || Date.now();
    eventHistory.unshift({
      id,
      type: event.type || 'event',
      message: escapeHtml(formatEventBody(event)),
      raw: event,
      ts_ms: tsMs,
      ack: false,
    });
    existing.add(id);
  });
  eventHistory = eventHistory.slice(0, 500);
  saveEventHistory();
}

function ackEvent(id) {
  const decoded = decodeURIComponent(id);
  const entry = eventHistory.find(item => item.id === decoded);
  if (!entry) return;
  entry.ack = true;
  saveEventHistory();
  renderEventHistory();
  renderFaultsPanel();
}

function exportEventCsv() {
  const rows = [['timestamp', 'type', 'message', 'ack']];
  eventHistory.forEach(entry => {
    rows.push([
      new Date(entry.ts_ms).toISOString(),
      entry.type,
      entry.message.replace(/,/g, ' '),
      entry.ack ? 'ack' : '',
    ]);
  });
  const csv = rows.map(row => row.join(',')).join('\n');
  const blob = new Blob([csv], { type: 'text/csv' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = 'trust-events.csv';
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

function renderEventHistory() {
  const list = document.getElementById('eventHistory');
  if (!list) return;
  list.classList.remove('skeleton');
  const typeFilter = document.getElementById('eventFilterType')?.value || 'all';
  const windowFilter = document.getElementById('eventFilterWindow')?.value || 'all';
  const query = (document.getElementById('eventSearch')?.value || '').toLowerCase();
  const now = Date.now();
  let cutoff = 0;
  if (windowFilter === '1h') cutoff = now - 60 * 60 * 1000;
  if (windowFilter === '24h') cutoff = now - 24 * 60 * 60 * 1000;
  if (windowFilter === '7d') cutoff = now - 7 * 24 * 60 * 60 * 1000;
  const filtered = eventHistory.filter(entry => {
    if (typeFilter !== 'all' && eventCategory(entry.raw) !== typeFilter) return false;
    if (cutoff && entry.ts_ms < cutoff) return false;
    if (query && !entry.message.toLowerCase().includes(query)) return false;
    return true;
  });
  if (!filtered.length) {
    list.innerHTML = '<div class="empty">No events match the current filters.</div>';
    return;
  }
  list.innerHTML = filtered.map(entry => {
    const time = new Date(entry.ts_ms).toLocaleString();
    const badge = eventCategory(entry.raw);
    const ack = entry.ack ? '<span class="tag">ack</span>' : '';
    const encoded = encodeURIComponent(entry.id);
    const action = entry.type === 'fault' && !entry.ack
      ? `<button class="btn ghost" onclick="ackEvent('${encoded}')">Acknowledge</button>`
      : '';
    return `
      <div class="row">
        <span>
          <span class="tag">${badge}</span>
          <span class="muted">${time}</span>
          ${entry.message}
        </span>
        <span>${ack}${action}</span>
      </div>
    `;
  }).join('');
}

function renderFaultsPanel() {
  const target = document.getElementById('faults');
  if (!target) return;
  target.classList.remove('skeleton');
  const faults = eventHistory.filter(entry => entry.type === 'fault' && !entry.ack);
  if (!faults.length) {
    if (lastFaultText) {
      target.innerHTML = `<div class="row"><span>${escapeHtml(lastFaultText)}</span></div>`;
    } else {
      target.innerHTML = '<div class="empty">No faults. System is healthy.</div>';
    }
    return;
  }
  target.innerHTML = faults.map(entry => {
    const suggestion = faultSuggestion(entry.message);
    const encoded = encodeURIComponent(entry.id);
    return `
      <div class="row">
        <span>${entry.message}</span>
        <button class="btn ghost" onclick="ackEvent('${encoded}')">Acknowledge</button>
      </div>
      <div class="note">Suggested fix: ${escapeHtml(suggestion)} <button class="btn ghost" onclick="sendControl('restart', {mode:'warm'})">Restart warm</button></div>
    `;
  }).join('');
}

function updateStatusPill(state) {
  const pill = document.getElementById('statusPill');
  if (!pill) return;
  pill.textContent = state;
  pill.dataset.state = state;
}

function updateLastUpdateLabel() {
  const label = document.getElementById('statusUpdated');
  if (!label) return;
  if (!lastUpdateMs) {
    label.textContent = 'Last update: --';
    return;
  }
  const diff = Math.max(0, Date.now() - lastUpdateMs);
  const seconds = Math.floor(diff / 1000);
  const suffix = connectionOk ? '' : ' (offline)';
  label.textContent = seconds <= 1
    ? `Last update: just now${suffix}`
    : `Last update: ${seconds}s ago${suffix}`;
}

function setConnectionState(ok) {
  connectionOk = ok;
  const meta = document.getElementById('statusMeta');
  const badge = document.getElementById('connectionBadge');
  if (badge) {
    badge.textContent = ok ? 'online' : 'offline';
    badge.dataset.state = ok ? 'online' : 'offline';
  }
  if (!ok) {
    updateStatusPill('offline');
    if (meta) meta.textContent = 'connection lost';
  }
  updateLastUpdateLabel();
  renderMeshConnections();
}

function updateSetupBanner() {
  const banner = document.getElementById('setupBanner');
  if (!banner) return;
  banner.hidden = setupDismissed || !needsSetup;
}

function updateControlAvailability(enabled) {
  debugEnabled = enabled;
  const toggle = document.getElementById('runToggle');
  if (toggle) {
    toggle.disabled = !enabled;
    toggle.classList.toggle('disabled', !enabled);
    toggle.title = enabled ? '' : 'Enable debug mode in runtime.toml to pause or resume.';
  }
  const hint = document.getElementById('debugHint');
  if (hint) {
    if (!enabled) {
      hint.hidden = false;
      hint.textContent = 'Pause/Resume disabled in production. Enable runtime.control.debug_enabled to use controls.';
    } else {
      hint.hidden = true;
      hint.textContent = '';
    }
  }
}

function calculateHealthScore(statusResult) {
  let score = 100;
  if (!statusResult) return score;
  if (statusResult.fault) score -= 40;
  const metrics = statusResult.metrics || {};
  const overruns = metrics.overruns || 0;
  score -= Math.min(overruns * 5, 30);
  const drivers = statusResult.io_drivers || [];
  const degraded = drivers.filter(d => d.status === 'degraded').length;
  const faulted = drivers.filter(d => d.status === 'faulted').length;
  score -= degraded * 5 + faulted * 15;
  return Math.max(0, Math.min(100, score));
}

async function refreshStatus() {
  const status = await apiRequest('status');
  if (!status.ok && status.error === 'offline') {
    setConnectionState(false);
    document.getElementById('runtimeMeta').textContent = 'waiting for PLC connection';
    return null;
  }
  setConnectionState(true);
  if (!status.ok && status.error === 'unauthorized') {
    updateStatusPill('auth');
    document.getElementById('statusMeta').textContent = 'auth required';
    document.getElementById('runtimeMeta').textContent = 'Add a pairing token to access this PLC.';
    return null;
  }
  if (!status.ok) return null;
  const result = status.result;
  lastUptimeMs = result.uptime_ms || 0;
  lastWallClockMs = Date.now();
  lastUpdateMs = lastWallClockMs;
  updateLastUpdateLabel();
  const plcName = result.plc_name || result.resource || '-';
  currentPlcName = plcName;
  const fault = result.fault || null;
  lastFaultText = fault;
  currentState = result.state || 'unknown';
  const simulationMode = result.simulation_mode || 'production';
  const simulationScale = Number(result.simulation_time_scale || 1);
  const simulationWarning = result.simulation_warning || '';
  updateControlAvailability(result.debug_enabled !== false);
  updateStatusPill(currentState);
  document.getElementById('statusMeta').textContent = `PLC name: ${plcName}`;
  document.getElementById('runtimeMeta').textContent = `${plcName} | ${simulationMode} x${simulationScale} | uptime ${formatDuration(result.uptime_ms || 0)}`;
  const drivers = result.io_drivers || [];
  const okDrivers = drivers.filter(d => d.status === 'ok').length;
  const degraded = drivers.filter(d => d.status === 'degraded').length;
  const faulted = drivers.filter(d => d.status === 'faulted').length;
  const metrics = result.metrics || {};
  const profiling = metrics.profiling || {};
  const topContributors = Array.isArray(profiling.top) ? profiling.top : [];
  const leadContributor = topContributors[0] || null;
  const leadContributorLabel = leadContributor
    ? `${leadContributor.kind}:${leadContributor.name} (${formatMs(leadContributor.avg_cycle_ms)} / ${Number(leadContributor.cycle_pct || 0).toFixed(1)}%)`
    : 'n/a';
  const cpu = Number(metrics.cpu_pct ?? metrics.cpu);
  const memBytes = Number(metrics.memory_bytes ?? metrics.mem_bytes);
  const memMb = Number.isFinite(memBytes) ? memBytes / (1024 * 1024) : Number(metrics.memory_mb ?? metrics.mem_mb);
  const cpuLabel = Number.isFinite(cpu) ? `${cpu.toFixed(1)}%` : 'n/a';
  const memLabel = Number.isFinite(memMb) ? `${memMb.toFixed(1)} MB` : 'n/a';
  setHtml('health', `
    <div class="row"><span>State</span><span class="stat">${currentState}</span></div>
    <div class="row"><span>Uptime</span><span>${formatDuration(result.uptime_ms || 0)}</span></div>
    <div class="row"><span>Mode</span><span>${escapeHtml(simulationMode)} (x${simulationScale})</span></div>
    <div class="row"><span>Fault</span><span>${fault || 'none'}</span></div>
    <div class="row"><span>I/O drivers</span><span>${okDrivers} ok | ${degraded} degraded | ${faulted} faulted</span></div>
    <div class="row"><span>CPU / memory</span><span>${cpuLabel} / ${memLabel}</span></div>
    ${simulationMode === 'simulation' && simulationWarning ? `<div class="row"><span>Warning</span><span>${escapeHtml(simulationWarning)}</span></div>` : ''}
  `);
  const cycle = metrics.cycle_ms || {};
  setHtml('metrics', `
    <div class="row"><span>last</span><span>${formatMs(cycle.last)}</span></div>
    <div class="row"><span>avg</span><span>${formatMs(cycle.avg)}</span></div>
    <div class="row"><span>min / max</span><span>${formatMs(cycle.min)} / ${formatMs(cycle.max)}</span></div>
    <div class="row"><span>overruns</span><span>${metrics.overruns ?? 0}</span></div>
    <div class="row"><span>profiling</span><span>${profiling.enabled ? 'on' : 'off'}</span></div>
    <div class="row"><span>top contributor</span><span>${escapeHtml(leadContributorLabel)}</span></div>
  `);
  const lastCycle = Number(cycle.last);
  if (Number.isFinite(lastCycle)) {
    cycleTrendSamples.push({ t: Date.now(), v: lastCycle });
  }
  updateTrends();
  const toggle = document.getElementById('runToggle');
  if (toggle) {
    const paused = currentState === 'paused';
    toggle.textContent = paused ? 'Resume' : 'Pause';
    toggle.setAttribute('aria-label', paused ? 'Resume PLC' : 'Pause PLC');
    toggle.onclick = () => sendControl(paused ? 'resume' : 'pause');
  }
  const score = calculateHealthScore(result);
  const scoreLabel = document.getElementById('healthScore');
  if (scoreLabel) {
    scoreLabel.textContent = `Health score: ${score}%`;
  }
  return result;
}

async function refreshTasks() {
  const tasks = await apiRequest('tasks.stats');
  if (!tasks.ok) return;
  const list = tasks.result.tasks || [];
  const topContributors = tasks.result.top_contributors || [];
  if (!list.length) {
    setHtml('tasks', '<div class="empty">No tasks configured yet. Add a task in runtime.toml.</div>');
    return;
  }
  const rows = list.map(t => `
    <tr>
      <td>${escapeHtml(t.name)}</td>
      <td>${t.avg_ms.toFixed(2)} ms</td>
      <td>${t.max_ms.toFixed(2)} ms</td>
      <td>${t.overruns}</td>
    </tr>
  `).join('');
  const contributorRows = topContributors.slice(0, 5).map((entry) => `
    <tr>
      <td>${escapeHtml(entry.kind || '-')}</td>
      <td>${escapeHtml(entry.name || '-')}</td>
      <td>${formatMs(entry.avg_cycle_ms)}</td>
      <td>${Number(entry.cycle_pct || 0).toFixed(1)}%</td>
    </tr>
  `).join('');
  setHtml('tasks', `
    <table class="data-table" aria-label="Task timings">
      <thead>
        <tr><th>task</th><th>avg</th><th>max</th><th>overrun</th></tr>
      </thead>
      <tbody>${rows}</tbody>
    </table>
    <div class="muted" style="margin:10px 0 6px;">Top cycle-budget contributors</div>
    ${contributorRows ? `
      <table class="data-table" aria-label="Top cycle contributors">
        <thead>
          <tr><th>kind</th><th>name</th><th>avg/cycle</th><th>share</th></tr>
        </thead>
        <tbody>${contributorRows}</tbody>
      </table>
    ` : '<div class="empty">No profiling contributors captured yet.</div>'}
  `);
}

async function refreshIo(statusResult) {
  const io = await apiRequest('io.list');
  if (!io.ok) return;
  const inputs = io.result.inputs || [];
  const outputs = io.result.outputs || [];
  setHtml('ioInputs', inputs.length
    ? inputs.map(i => `<div class="row"><span>${escapeHtml(i.name || i.address)}</span><span>${formatIoValue(i)}</span></div>`).join('')
    : '<div class="empty">No inputs mapped yet. Check io.toml.</div>');
  setHtml('ioOutputs', outputs.length
    ? outputs.map(o => `<div class="row"><span>${escapeHtml(o.name || o.address)}</span><span>${formatIoValue(o)}</span></div>`).join('')
    : '<div class="empty">No outputs mapped yet. Check io.toml.</div>');
  const drivers = statusResult ? (statusResult.io_drivers || []) : [];
  setHtml('ioDrivers', drivers.length
    ? drivers.map(d => `<div class="row"><span>${d.name}</span><span>${d.status}${d.error ? ` - ${d.error}` : ''}</span></div>`).join('')
    : '<div class="empty">No drivers active. Check io.toml.</div>');
  renderSimulation(inputs, outputs, drivers);
}

async function refreshEvents() {
  const events = await apiRequest('events.tail', { limit: 50 });
  if (!events.ok) return;
  const list = (events.result.events || []).filter(Boolean);
  mergeEventHistory(list);
  renderEventHistory();
  renderFaultsPanel();
  setHtml('events', list.length
    ? list.map(e => `<div>${escapeHtml(formatEvent(e))}</div>`).join('')
    : '<div class="empty">All clear. No events recorded yet.</div>');
  setHtml('eventsSummary', list.length
    ? list.slice(0, 4).map(e => `<div>${escapeHtml(formatEvent(e))}</div>`).join('')
    : '<div class="empty">All clear. No recent events.</div>');
}

async function refresh() {
  const statusResult = await refreshStatus();
  updateSetupBanner();
  if (!statusResult) return;
  await Promise.all([
    refreshTasks(),
    refreshIo(statusResult),
    refreshEvents(),
    refreshWatchValues(),
    refreshMeshValues(),
    updatePcIndicator(),
  ]);
  initialLoad = false;
}

function scheduleRefresh() {
  if (refreshTimer) clearTimeout(refreshTimer);
  const delay = document.hidden ? 4000 : 1000;
  refreshTimer = setTimeout(async () => {
    await refresh();
    scheduleRefresh();
  }, delay);
}

function scheduleDiscovery() {
  if (discoveryTimer) clearTimeout(discoveryTimer);
  const delay = document.hidden ? 15000 : 5000;
  discoveryTimer = setTimeout(async () => {
    await refreshDiscovery();
    scheduleDiscovery();
  }, delay);
}

function renderSimulation(inputs, outputs, drivers) {
  const loopback = drivers.some(d => (d.name || '').toLowerCase() === 'loopback');
  const panel = document.getElementById('simPanel');
  const list = document.getElementById('simInputs');
  if (!panel || !list) return;
  if (!loopback || !inputs.length) {
    panel.hidden = true;
    return;
  }
  panel.hidden = false;
  list.innerHTML = inputs.map((input) => {
    const value = String(input.value || '').toLowerCase().includes('true');
    const label = input.name || input.address;
    const action = value ? 'Set FALSE' : 'Set TRUE';
    return `<div class="row"><span>${escapeHtml(label)}</span><button class="btn secondary" onclick="writeInput('${input.address}', ${!value})">${action}</button></div>`;
  }).join('');
}

function normalizeNumber(value, fallback) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  return parsed;
}

function normalizeEnum(value) {
  return String(value || '').toLowerCase();
}

function loadWatchList() {
  try {
    watchList = JSON.parse(localStorage.getItem('trustWatchList') || '[]');
  } catch {
    watchList = [];
  }
  if (!Array.isArray(watchList)) watchList = [];
}

function saveWatchList() {
  localStorage.setItem('trustWatchList', JSON.stringify(watchList));
}

function addWatch() {
  watchList.push({ expr: '', scope: 'global', forceValue: '', forced: false });
  saveWatchList();
  renderWatchList();
}

function addWatchFromSearch() {
  const input = document.getElementById('watchSearch');
  const expr = input?.value.trim();
  if (!expr) return;
  watchList.push({ expr, scope: 'global', forceValue: '', forced: false });
  input.value = '';
  saveWatchList();
  renderWatchList();
}

function removeWatch(index) {
  watchList.splice(index, 1);
  saveWatchList();
  renderWatchList();
}

function updateWatchExpr(index, value) {
  watchList[index].expr = value;
  saveWatchList();
}

function updateWatchScope(index, value) {
  watchList[index].scope = value;
  saveWatchList();
}

function updateWatchForceValue(index, value) {
  watchList[index].forceValue = value;
  saveWatchList();
}

async function toggleWatchForce(index, checked) {
  const entry = watchList[index];
  if (!entry || !entry.expr) return;
  const target = `${entry.scope}:${entry.expr}`;
  if (checked) {
    const value = entry.forceValue || 'TRUE';
    await apiRequest('var.force', { target, value });
    entry.forced = true;
  } else {
    await apiRequest('var.unforce', { target });
    entry.forced = false;
  }
  saveWatchList();
  renderWatchList();
}

async function releaseAllForces() {
  if (!debugEnabled) return;
  for (const entry of watchList) {
    if (entry.forced && entry.expr) {
      await apiRequest('var.unforce', { target: `${entry.scope}:${entry.expr}` });
      entry.forced = false;
    }
  }
  saveWatchList();
  renderWatchList();
}

function renderWatchList() {
  const target = document.getElementById('watchList');
  if (!target) return;
  if (!watchList.length) {
    target.innerHTML = '<div class="empty">No variables watched yet. Add one below.</div>';
    return;
  }
  target.innerHTML = watchList.map((entry, idx) => {
    const value = watchValues[entry.expr];
    const label = value?.error ? `<span class="error-text">${escapeHtml(value.error)}</span>` : escapeHtml(value?.value || '--');
    const type = value?.type ? `<span class="muted">(${escapeHtml(value.type)})</span>` : '';
    const forced = entry.forced ? 'checked' : '';
    const disabled = debugEnabled ? '' : 'disabled';
    return `
      <div class="table-row" style="grid-template-columns:1.2fr 0.7fr 0.6fr 0.6fr 0.4fr;">
        <input class="field-input" placeholder="Counter" value="${escapeHtml(entry.expr)}" oninput="updateWatchExpr(${idx}, this.value)"/>
        <span>${label} ${type}</span>
        <select ${disabled} onchange="updateWatchScope(${idx}, this.value)">
          <option value="global" ${entry.scope === 'global' ? 'selected' : ''}>global</option>
          <option value="retain" ${entry.scope === 'retain' ? 'selected' : ''}>retain</option>
        </select>
        <input class="field-input" ${disabled} placeholder="force value" value="${escapeHtml(entry.forceValue || '')}" oninput="updateWatchForceValue(${idx}, this.value)"/>
        <div style="display:flex; gap:6px;">
          <label class="toggle-inline"><input type="checkbox" ${forced} ${disabled} onchange="toggleWatchForce(${idx}, this.checked)"/> force</label>
          <button class="btn ghost" onclick="removeWatch(${idx})">Remove</button>
        </div>
      </div>
    `;
  }).join('');
  const trendSelect = document.getElementById('trendVariable');
  if (trendSelect) {
    const options = watchList
      .filter(entry => entry.expr)
      .map(entry => `<option value="${escapeHtml(entry.expr)}">${escapeHtml(entry.expr)}</option>`)
      .join('');
    trendSelect.innerHTML = `<option value="">cycle only</option>${options}`;
    if (selectedTrendVar && trendSelect.value !== selectedTrendVar) {
      trendSelect.value = selectedTrendVar;
    }
    if (!selectedTrendVar && watchList[0] && watchList[0].expr) {
      selectedTrendVar = watchList[0].expr;
      trendSelect.value = selectedTrendVar;
    }
  }
}

async function refreshWatchValues() {
  if (!watchList.length) return;
  if (!debugEnabled) {
    watchValues = {};
    renderWatchList();
    return;
  }
  const forced = await apiRequest('var.forced');
  if (forced.ok) {
    const forcedTargets = new Set((forced.result.vars || []).map(item => item.target));
    watchList.forEach(entry => {
      entry.forced = forcedTargets.has(`${entry.scope}:${entry.expr}`);
    });
    saveWatchList();
  }
  const tasks = watchList
    .filter(entry => entry.expr)
    .map(entry => async () => {
      const res = await apiRequest('debug.evaluate', { expression: entry.expr });
      if (res.ok) {
        watchValues[entry.expr] = { value: res.result.result, type: res.result.type };
        const numeric = Number(res.result.result);
        if (Number.isFinite(numeric)) {
          const list = variableTrendSamples[entry.expr] || [];
          list.push({ t: Date.now(), v: numeric });
          variableTrendSamples[entry.expr] = list;
        }
      } else {
        watchValues[entry.expr] = { error: res.error || 'unavailable' };
      }
    });
  await runBatched(tasks, 4);
  renderWatchList();
  updateTrends();
}

function setTrendWindow(windowMs) {
  trendWindowMs = windowMs;
  document.querySelectorAll('[data-trend]').forEach(btn => {
    btn.classList.toggle('active', Number(btn.dataset.trend) === windowMs);
  });
  updateTrends();
}

function setTrendVariable(value) {
  selectedTrendVar = value;
  updateTrends();
}

function drawTrend(canvas, samples) {
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  const style = getComputedStyle(document.body);
  const labelColor = style.getPropertyValue('--muted').trim() || '#6a6a74';
  const lineColor = style.getPropertyValue('--accent').trim() || '#14b8a6';
  const unit = canvas.dataset.unit || '';
  if (!samples.length) {
    ctx.fillStyle = labelColor;
    ctx.font = '12px sans-serif';
    ctx.fillText('No data', 8, 16);
    return;
  }
  const values = samples.map(s => s.v);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const padding = { left: 36, right: 8, top: 10, bottom: 18 };
  const width = Math.max(1, canvas.width - padding.left - padding.right);
  const height = Math.max(1, canvas.height - padding.top - padding.bottom);
  const formatLabel = (value) => {
    const text = Number.isFinite(value) ? value.toFixed(2) : '--';
    return unit ? `${text} ${unit}` : text;
  };
  ctx.fillStyle = labelColor;
  ctx.font = '11px sans-serif';
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillText(formatLabel(max), 6, padding.top - 6);
  ctx.textBaseline = 'bottom';
  ctx.fillText(formatLabel(min), 6, canvas.height - 4);
  ctx.strokeStyle = lineColor;
  ctx.lineWidth = 2;
  ctx.beginPath();
  samples.forEach((sample, idx) => {
    const denom = samples.length > 1 ? (samples.length - 1) : 1;
    const x = padding.left + (idx / denom) * width;
    const y = padding.top + (1 - ((sample.v - min) / range)) * height;
    if (idx === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();
}

function updateTrends() {
  const now = Date.now();
  cycleTrendSamples = cycleTrendSamples.filter(sample => now - sample.t <= trendWindowMs);
  const cycleCanvas = document.getElementById('cycleTrend');
  drawTrend(cycleCanvas, cycleTrendSamples);
  const varCanvas = document.getElementById('varTrend');
  if (!selectedTrendVar) {
    drawTrend(varCanvas, []);
    return;
  }
  const list = (variableTrendSamples[selectedTrendVar] || []).filter(sample => now - sample.t <= trendWindowMs);
  variableTrendSamples[selectedTrendVar] = list;
  drawTrend(varCanvas, list);
}

function updateIoDriverPanels() {
  const driver = document.getElementById('ioDriverSelect')?.value;
  const modbus = document.getElementById('ioDriverModbus');
  const gpio = document.getElementById('ioDriverGpio');
  const mqtt = document.getElementById('ioDriverMqtt');
  if (modbus) modbus.hidden = driver !== 'modbus-tcp';
  if (gpio) gpio.hidden = driver !== 'gpio';
  if (mqtt) mqtt.hidden = driver !== 'mqtt';
  if (driver === 'gpio') {
    ioConfigState.params = ioConfigState.params || {};
    ioConfigState.params.inputs = ioConfigState.params.inputs || [];
    ioConfigState.params.outputs = ioConfigState.params.outputs || [];
    renderGpioInputs();
    renderGpioOutputs();
  }
}

function setIoConfigDisabled(disabled) {
  const section = document.getElementById('ioConfig');
  if (!section) return;
  section.querySelectorAll('input, select, textarea, button').forEach(el => {
    if (el.id === 'ioUseSystem') return;
    if (el.id === 'ioConfigApply') return;
    el.disabled = disabled;
  });
}

function renderGridList(targetId, items, emptyText, gridTemplate, rowRenderer) {
  const target = document.getElementById(targetId);
  if (!target) return;
  if (!items.length) {
    target.innerHTML = `<div class="empty">${emptyText}</div>`;
    return;
  }
  target.innerHTML = items.map((entry, idx) => `
    <div class="table-row" style="grid-template-columns:${gridTemplate};">
      ${rowRenderer(entry, idx)}
    </div>
  `).join('');
}

function renderGpioInputs() {
  const inputs = (ioConfigState.params.inputs || []);
  renderGridList(
    'gpioInputs',
    inputs,
    'No inputs mapped yet.',
    '1.2fr 0.8fr 0.8fr 0.4fr',
    (entry, idx) => `
      <input class="field-input" placeholder="%IX0.0" value="${entry.address || ''}" oninput="updateGpioInput(${idx}, 'address', this.value)"/>
      <input class="field-input" placeholder="GPIO pin" value="${entry.line ?? ''}" oninput="updateGpioInput(${idx}, 'line', this.value)"/>
      <input class="field-input" placeholder="debounce ms" value="${entry.debounce_ms ?? ''}" oninput="updateGpioInput(${idx}, 'debounce_ms', this.value)"/>
      <button class="btn ghost" onclick="removeGpioInput(${idx})">Remove</button>
    `,
  );
}

function renderGpioOutputs() {
  const outputs = (ioConfigState.params.outputs || []);
  renderGridList(
    'gpioOutputs',
    outputs,
    'No outputs mapped yet.',
    '1.2fr 0.8fr 0.8fr 0.4fr',
    (entry, idx) => `
      <input class="field-input" placeholder="%QX0.0" value="${entry.address || ''}" oninput="updateGpioOutput(${idx}, 'address', this.value)"/>
      <input class="field-input" placeholder="GPIO pin" value="${entry.line ?? ''}" oninput="updateGpioOutput(${idx}, 'line', this.value)"/>
      <input class="field-input" placeholder="initial" value="${entry.initial ?? ''}" oninput="updateGpioOutput(${idx}, 'initial', this.value)"/>
      <button class="btn ghost" onclick="removeGpioOutput(${idx})">Remove</button>
    `,
  );
}

function renderSafeState() {
  const safe = ioConfigState.safe_state || [];
  renderGridList(
    'ioSafeState',
    safe,
    'No safe state outputs configured.',
    '1.2fr 1fr 0.4fr',
    (entry, idx) => `
      <input class="field-input" placeholder="%QX0.0" value="${entry.address || ''}" oninput="updateSafeState(${idx}, 'address', this.value)"/>
      <input class="field-input" placeholder="FALSE" value="${entry.value || ''}" oninput="updateSafeState(${idx}, 'value', this.value)"/>
      <button class="btn ghost" onclick="removeSafeState(${idx})">Remove</button>
    `,
  );
}

function updateGpioInput(index, field, value) {
  ioConfigState.params.inputs[index][field] = value;
}

function updateGpioOutput(index, field, value) {
  ioConfigState.params.outputs[index][field] = value;
}

function updateSafeState(index, field, value) {
  ioConfigState.safe_state[index][field] = value;
}

function addGpioInput() {
  if (!ioConfigState.params.inputs) ioConfigState.params.inputs = [];
  ioConfigState.params.inputs.push({ address: '', line: '', debounce_ms: '' });
  renderGpioInputs();
}

function removeGpioInput(index) {
  ioConfigState.params.inputs.splice(index, 1);
  renderGpioInputs();
}

function addGpioOutput() {
  if (!ioConfigState.params.outputs) ioConfigState.params.outputs = [];
  ioConfigState.params.outputs.push({ address: '', line: '', initial: '' });
  renderGpioOutputs();
}

function removeGpioOutput(index) {
  ioConfigState.params.outputs.splice(index, 1);
  renderGpioOutputs();
}

function addSafeState() {
  if (!ioConfigState.safe_state) ioConfigState.safe_state = [];
  ioConfigState.safe_state.push({ address: '', value: '' });
  renderSafeState();
}

function removeSafeState(index) {
  ioConfigState.safe_state.splice(index, 1);
  renderSafeState();
}

async function loadIoConfig() {
  let supportedDrivers = fallbackSupportedIoDrivers;
  let configuredDrivers = [{ name: 'loopback', params: {} }];
  try {
    const res = await fetch('/api/io/config');
    const data = await res.json();
    supportedDrivers = uniqueIoDrivers(data?.supported_drivers);
    const fallbackLegacy = {
      name: String(data?.driver || 'loopback'),
      params: (data?.params && typeof data.params === 'object' && !Array.isArray(data.params)) ? data.params : {},
    };
    configuredDrivers = normalizeIoDriverConfigs(data?.drivers, fallbackLegacy.name);
    if (!Array.isArray(data?.drivers) || !data.drivers.length) {
      configuredDrivers = normalizeIoDriverConfigs([fallbackLegacy], fallbackLegacy.name);
    }
    ioConfigState = Object.assign({
      driver: configuredDrivers[0].name,
      params: configuredDrivers[0].params || {},
      drivers: configuredDrivers,
      supported_drivers: supportedDrivers,
      safe_state: [],
      use_system_io: false,
      source: 'project',
    }, data || {});
    ioConfigState.supported_drivers = supportedDrivers;
    ioConfigState.drivers = configuredDrivers;
    ioConfigState.driver = configuredDrivers[0].name;
    ioConfigState.params = configuredDrivers[0].params || {};
  } catch (err) {
    configuredDrivers = [{ name: 'loopback', params: {} }];
    ioConfigState = {
      driver: 'loopback',
      params: {},
      drivers: configuredDrivers,
      supported_drivers: supportedDrivers,
      safe_state: [],
      use_system_io: false,
      source: 'default',
    };
  }
  populateDriverSelect('ioDriverSelect', [...supportedDrivers, ...configuredDrivers.map(entry => entry.name)]);
  ioConfigOriginal = JSON.parse(JSON.stringify(ioConfigState));
  const driverSelect = document.getElementById('ioDriverSelect');
  if (driverSelect) {
    driverSelect.value = ioConfigState.driver || 'loopback';
    driverSelect.onchange = () => {
      ioConfigState.driver = driverSelect.value;
      if (!Array.isArray(ioConfigState.drivers) || !ioConfigState.drivers.length) {
        ioConfigState.drivers = [{ name: ioConfigState.driver, params: {} }];
      }
      ioConfigState.drivers[0].name = ioConfigState.driver;
      ioConfigState.params = ioConfigState.drivers[0].params || {};
      updateIoDriverPanels();
    };
  }
  const useSystem = document.getElementById('ioUseSystem');
  if (useSystem) {
    useSystem.value = String(ioConfigState.use_system_io);
    useSystem.onchange = () => {
      ioConfigState.use_system_io = useSystem.value === 'true';
      setIoConfigDisabled(ioConfigState.use_system_io);
    };
  }
  if (!ioConfigState.params || typeof ioConfigState.params !== 'object') {
    ioConfigState.params = {};
  }
  if (!Array.isArray(ioConfigState.drivers) || !ioConfigState.drivers.length) {
    ioConfigState.drivers = [{ name: ioConfigState.driver || 'loopback', params: ioConfigState.params }];
  } else {
    ioConfigState.drivers[0] = {
      name: ioConfigState.driver || ioConfigState.drivers[0].name || 'loopback',
      params: ioConfigState.params,
    };
  }
  if (ioConfigState.driver === 'gpio') {
    ioConfigState.params.inputs = ioConfigState.params.inputs || [];
    ioConfigState.params.outputs = ioConfigState.params.outputs || [];
  }
  const modbusAddress = document.getElementById('modbusAddress');
  const modbusPort = document.getElementById('modbusPort');
  const modbusUnit = document.getElementById('modbusUnitId');
  const modbusTimeout = document.getElementById('modbusTimeout');
  const modbusInput = document.getElementById('modbusInputStart');
  const modbusOutput = document.getElementById('modbusOutputStart');
  const modbusError = document.getElementById('modbusOnError');
  const addressValue = String(ioConfigState.params.address || '');
  if (modbusAddress) modbusAddress.value = addressValue.includes(':') ? addressValue.split(':')[0] : addressValue;
  if (modbusPort) modbusPort.value = addressValue.includes(':')
    ? (addressValue.split(':')[1] || 502)
    : (ioConfigState.params.port || 502);
  if (modbusUnit) modbusUnit.value = ioConfigState.params.unit_id ?? 1;
  if (modbusTimeout) modbusTimeout.value = ioConfigState.params.timeout_ms ?? 500;
  if (modbusInput) modbusInput.value = ioConfigState.params.input_start ?? 0;
  if (modbusOutput) modbusOutput.value = ioConfigState.params.output_start ?? 0;
  if (modbusError) modbusError.value = ioConfigState.params.on_error || 'fault';
  const gpioBackend = document.getElementById('gpioBackend');
  if (gpioBackend) gpioBackend.value = ioConfigState.params.backend || 'sysfs';
  const mqttBroker = document.getElementById('mqttBroker');
  if (mqttBroker) mqttBroker.value = ioConfigState.params.broker || '127.0.0.1:1883';
  const mqttClientId = document.getElementById('mqttClientId');
  if (mqttClientId) mqttClientId.value = ioConfigState.params.client_id || '';
  const mqttTopicIn = document.getElementById('mqttTopicIn');
  if (mqttTopicIn) mqttTopicIn.value = ioConfigState.params.topic_in || 'trust/io/in';
  const mqttTopicOut = document.getElementById('mqttTopicOut');
  if (mqttTopicOut) mqttTopicOut.value = ioConfigState.params.topic_out || 'trust/io/out';
  const mqttReconnectMs = document.getElementById('mqttReconnectMs');
  if (mqttReconnectMs) mqttReconnectMs.value = ioConfigState.params.reconnect_ms ?? 500;
  const mqttKeepAliveS = document.getElementById('mqttKeepAliveS');
  if (mqttKeepAliveS) mqttKeepAliveS.value = ioConfigState.params.keep_alive_s ?? 5;
  const mqttAllowInsecure = document.getElementById('mqttAllowInsecureRemote');
  if (mqttAllowInsecure) {
    mqttAllowInsecure.value = String(ioConfigState.params.allow_insecure_remote ?? false);
  }
  const mqttUsername = document.getElementById('mqttUsername');
  if (mqttUsername) mqttUsername.value = ioConfigState.params.username || '';
  const mqttPassword = document.getElementById('mqttPassword');
  if (mqttPassword) mqttPassword.value = ioConfigState.params.password || '';
  renderAdditionalIoDrivers();
  renderGpioInputs();
  renderGpioOutputs();
  renderSafeState();
  updateIoDriverPanels();
  setIoConfigDisabled(ioConfigState.use_system_io);
}

async function saveIoConfig() {
  const driver = document.getElementById('ioDriverSelect')?.value || 'loopback';
  const useSystem = document.getElementById('ioUseSystem')?.value === 'true';
  let params = {};
  if (driver === 'modbus-tcp') {
    const host = document.getElementById('modbusAddress')?.value.trim() || '';
    const port = document.getElementById('modbusPort')?.value || '502';
    if (!host) {
      setStatus('ioConfigStatus', 'Modbus address is required before saving.', 'error');
      return;
    }
    const address = host.includes(':') ? host : `${host}:${port}`;
    params = {
      address,
      unit_id: normalizeNumber(document.getElementById('modbusUnitId')?.value, 1),
      input_start: normalizeNumber(document.getElementById('modbusInputStart')?.value, 0),
      output_start: normalizeNumber(document.getElementById('modbusOutputStart')?.value, 0),
      timeout_ms: normalizeNumber(document.getElementById('modbusTimeout')?.value, 500),
      on_error: document.getElementById('modbusOnError')?.value || 'fault',
    };
  } else if (driver === 'gpio') {
    params = {
      backend: document.getElementById('gpioBackend')?.value.trim() || 'sysfs',
      inputs: (ioConfigState.params.inputs || []).map(entry => ({
        address: entry.address,
        line: normalizeNumber(entry.line, 0),
        debounce_ms: normalizeNumber(entry.debounce_ms || 0, 0),
      })).filter(entry => entry.address),
      outputs: (ioConfigState.params.outputs || []).map(entry => ({
        address: entry.address,
        line: normalizeNumber(entry.line, 0),
        initial: String(entry.initial || '').toLowerCase() === 'true' || entry.initial === true,
      })).filter(entry => entry.address),
    };
  } else if (driver === 'mqtt') {
    const broker = document.getElementById('mqttBroker')?.value.trim() || '127.0.0.1:1883';
    const topicIn = document.getElementById('mqttTopicIn')?.value.trim() || 'trust/io/in';
    const topicOut = document.getElementById('mqttTopicOut')?.value.trim() || 'trust/io/out';
    const username = document.getElementById('mqttUsername')?.value.trim() || '';
    const password = document.getElementById('mqttPassword')?.value || '';
    if (!broker) {
      setStatus('ioConfigStatus', 'MQTT broker is required before saving.', 'error');
      return;
    }
    if ((username && !password) || (!username && password)) {
      setStatus('ioConfigStatus', 'MQTT username/password must both be set or both be empty.', 'error');
      return;
    }
    params = {
      broker,
      topic_in: topicIn,
      topic_out: topicOut,
      reconnect_ms: normalizeNumber(document.getElementById('mqttReconnectMs')?.value, 500),
      keep_alive_s: normalizeNumber(document.getElementById('mqttKeepAliveS')?.value, 5),
      allow_insecure_remote: document.getElementById('mqttAllowInsecureRemote')?.value === 'true',
    };
    const clientId = document.getElementById('mqttClientId')?.value.trim() || '';
    if (clientId) params.client_id = clientId;
    if (username && password) {
      params.username = username;
      params.password = password;
    }
  } else {
    params = {};
  }
  const extraConfigured = (ioConfigState.drivers || []).slice(1);
  const extraDrivers = [];
  for (let idx = 0; idx < extraConfigured.length; idx += 1) {
    const name = String(document.getElementById(`ioExtraDriverName${idx}`)?.value || '').trim();
    if (!name) {
      setStatus('ioConfigStatus', `Additional driver #${idx + 1} requires a name.`, 'error');
      return;
    }
    const raw = document.getElementById(`ioExtraDriverParams${idx}`)?.value.trim() || '{}';
    let parsed;
    try {
      parsed = raw ? JSON.parse(raw) : {};
    } catch (err) {
      setStatus('ioConfigStatus', `Additional driver #${idx + 1} params are invalid JSON.`, 'error');
      return;
    }
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      setStatus('ioConfigStatus', `Additional driver #${idx + 1} params must be a JSON object.`, 'error');
      return;
    }
    extraDrivers.push({ name, params: parsed });
  }
  const drivers = [{ name: driver, params }, ...extraDrivers];
  const payload = {
    driver,
    params,
    drivers,
    safe_state: (ioConfigState.safe_state || []).filter(entry => entry.address && entry.value),
    use_system_io: useSystem,
  };
  const previousSafe = JSON.stringify((ioConfigOriginal && ioConfigOriginal.safe_state) || []);
  const nextSafe = JSON.stringify(payload.safe_state || []);
  if (previousSafe !== nextSafe && payload.safe_state.length) {
    const summary = payload.safe_state.map(entry => `${entry.address} → ${entry.value}`).join('\n');
    const ok = confirm(`Apply safe-state outputs?\n\n${summary}\n\nThese values will be forced on fault/watchdog.`);
    if (!ok) return;
  }
  await withLoadingState('ioConfigApply', 'ioConfigStatus', 'Saving...', async () => {
    try {
      const res = await fetch('/api/io/config', {
        method: 'POST',
        headers: Object.assign(
          { 'Content-Type': 'application/json' },
          authToken ? { 'X-Trust-Token': authToken } : {},
        ),
        body: JSON.stringify(payload),
      });
      const text = await res.text();
      const isError = text.startsWith('error');
      setStatus('ioConfigStatus', text, isError ? 'error' : 'success');
      if (!isError) {
        showToast('I/O configuration saved.', 'success');
      }
    } catch (err) {
      setStatus('ioConfigStatus', 'Failed to save I/O config (offline).', 'error');
    }
  });
}

async function testModbus() {
  await withLoadingState('modbusTest', 'modbusStatus', 'Testing...', async () => {
    const address = document.getElementById('modbusAddress')?.value.trim() || '';
    const port = normalizeNumber(document.getElementById('modbusPort')?.value, 502);
    const timeout_ms = normalizeNumber(document.getElementById('modbusTimeout')?.value, 500);
    if (!address) {
      setStatus('modbusStatus', 'Enter a server address first.', 'error');
      return;
    }
    try {
      const res = await fetch('/api/io/modbus-test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ address, port, timeout_ms }),
      });
      const data = await res.json();
      if (data.ok) {
        setStatus('modbusStatus', 'Connected.', 'success');
        showToast('Modbus connection verified.', 'success');
      } else {
        setStatus('modbusStatus', data.error || 'Connection failed.', 'error');
      }
    } catch (err) {
      setStatus('modbusStatus', 'Connection failed (offline).', 'error');
    }
  });
}

function renderMeshPublish() {
  if (!meshPublishState.length) {
    renderGridList('meshPublishList', [], 'No published variables yet.', '1fr', () => '');
    renderMeshConnections();
    return;
  }
  renderGridList(
    'meshPublishList',
    meshPublishState,
    'No published variables yet.',
    '1.2fr 0.8fr 0.6fr 0.4fr',
    (value, idx) => `
      <input class="field-input" placeholder="Status.RunState" value="${escapeHtml(value)}" oninput="updateMeshPublish(${idx}, this.value)"/>
      <span>${escapeHtml(meshValueCache[value] ?? (debugEnabled ? '--' : 'debug off'))}</span>
      <span class="muted">—</span>
      <button class="btn ghost" onclick="removeMeshPublish(${idx})">Remove</button>
    `,
  );
  renderMeshConnections();
}

async function refreshMeshValues() {
  if (!meshPublishState.length) {
    meshValueCache = {};
    renderMeshPublish();
    return;
  }
  if (!debugEnabled) {
    meshValueCache = {};
    renderMeshPublish();
    return;
  }
  const tasks = meshPublishState
    .filter(name => name)
    .map(name => async () => {
      const res = await apiRequest('debug.evaluate', { expression: name });
      meshValueCache[name] = res.ok ? res.result.result : (res.error || 'error');
    });
  await runBatched(tasks, 4);
  renderMeshPublish();
}

function renderMeshSubscribe() {
  if (!meshSubscribeState.length) {
    renderGridList('meshSubscribeList', [], 'No subscriptions configured.', '1fr', () => '');
    renderMeshConnections();
    return;
  }
  renderGridList(
    'meshSubscribeList',
    meshSubscribeState,
    'No subscriptions configured.',
    '1.2fr 1.2fr 0.4fr',
    (entry, idx) => `
      <input class="field-input" placeholder="PLC-1:TempA" value="${escapeHtml(entry.remote)}" oninput="updateMeshSubscribe(${idx}, 'remote', this.value)"/>
      <input class="field-input" placeholder="RemoteTemp" value="${escapeHtml(entry.local)}" oninput="updateMeshSubscribe(${idx}, 'local', this.value)"/>
      <button class="btn ghost" onclick="removeMeshSubscribe(${idx})">Remove</button>
    `,
  );
  renderMeshConnections();
}

function renderMeshConnections() {
  const target = document.getElementById('meshConnections');
  if (!target) return;
  if (!meshPublishState.length && !meshSubscribeState.length) {
    target.innerHTML = '<div class="empty">No mesh connections configured.</div>';
    return;
  }
  const publishRows = meshPublishState.map(value => {
    const status = connectionOk ? (debugEnabled ? 'live' : 'configured') : 'stale';
    return `<div class="row"><span>Publish ${escapeHtml(value || '-')}</span><span class="stat">${status}</span></div>`;
  }).join('');
  const subscribeRows = meshSubscribeState.map(entry => {
    const status = connectionOk ? 'configured' : 'stale';
    return `<div class="row"><span>Subscribe ${escapeHtml(entry.remote || '-')} → ${escapeHtml(entry.local || '-')}</span><span class="stat">${status}</span></div>`;
  }).join('');
  target.innerHTML = publishRows + subscribeRows;
}

function renderSourceViewer(text) {
  const target = document.getElementById('sourceViewer');
  if (!target) return;
  if (!text) {
    target.textContent = '';
    return;
  }
  const lines = text.split('\n');
  target.innerHTML = lines.map((line, idx) => `
    <div class="code-line${currentPcLine === idx + 1 ? ' pc' : ''}" data-line="${idx + 1}"><span class="line-no">${idx + 1}</span><span>${escapeHtml(line)}</span></div>
  `).join('');
}

async function updatePcIndicator() {
  if (!debugEnabled) {
    currentPcLine = null;
    document.querySelectorAll('.code-line.pc').forEach(el => el.classList.remove('pc'));
    return;
  }
  const state = await apiRequest('debug.state');
  if (!state.ok || !state.result?.paused || !state.result?.last_stop) {
    currentPcLine = null;
    document.querySelectorAll('.code-line.pc').forEach(el => el.classList.remove('pc'));
    return;
  }
  const stop = state.result.last_stop;
  const path = stop.path || '';
  const file = path.split('/').pop();
  if (!file || file !== currentSourceName || !stop.line) {
    currentPcLine = null;
    document.querySelectorAll('.code-line.pc').forEach(el => el.classList.remove('pc'));
    return;
  }
  currentPcLine = stop.line;
  document.querySelectorAll('.code-line.pc').forEach(el => el.classList.remove('pc'));
  const lineEl = document.querySelector(`.code-line[data-line="${stop.line}"]`);
  if (lineEl) lineEl.classList.add('pc');
}

async function loadProgram() {
  try {
    const res = await fetch('/api/program');
    const data = await res.json();
    const meta = document.getElementById('programMeta');
    if (meta) {
      const updated = data.updated_ms ? new Date(Number(data.updated_ms)).toLocaleString() : '-';
      setHtml('programMeta', `
        <div class="row"><span>Program</span><span>${data.program || '-'}</span></div>
        <div class="row"><span>Last updated</span><span>${updated}</span></div>
        <div class="row"><span>Sources</span><span>${(data.sources || []).length}</span></div>
      `);
    }
    const list = document.getElementById('sourceList');
    if (list) {
      if (!data.sources || !data.sources.length) {
        setHtml('sourceList', '<div class="empty">No source files found.</div>');
      } else {
        setHtml('sourceList', data.sources.map(file => `
          <button class="btn ghost" onclick="openSource('${encodeURIComponent(file)}')">${escapeHtml(file)}</button>
        `).join(''));
      }
    }
    if (data.sources && data.sources.length) {
      const first = data.sources[0];
      await openSource(first);
    } else {
      renderSourceViewer('');
    }
    programLoaded = true;
  } catch (err) {
    setHtml('programMeta', '<div class="empty">Program metadata unavailable.</div>');
    setHtml('sourceList', '<div class="empty">Sources unavailable.</div>');
    renderSourceViewer('');
  }
}

async function openSource(name) {
  const decoded = decodeURIComponent(name);
  const res = await fetch(`/api/source?file=${encodeURIComponent(decoded)}`);
  if (!res.ok) {
    renderSourceViewer('Unable to load source.');
    return;
  }
  const text = await res.text();
  currentSourceName = decoded;
  currentPcLine = null;
  renderSourceViewer(text);
  await updatePcIndicator();
}

function addMeshPublish() {
  meshPublishState.push('');
  renderMeshPublish();
}

function removeMeshPublish(index) {
  meshPublishState.splice(index, 1);
  renderMeshPublish();
}

function updateMeshPublish(index, value) {
  meshPublishState[index] = value;
}

function addMeshSubscribe() {
  meshSubscribeState.push({ remote: '', local: '' });
  renderMeshSubscribe();
}

function addMeshQuick() {
  const remote = document.getElementById('meshQuickRemote')?.value.trim() || '';
  const local = document.getElementById('meshQuickLocal')?.value.trim() || '';
  if (!remote || !local) return;
  meshSubscribeState.push({ remote, local });
  document.getElementById('meshQuickRemote').value = '';
  document.getElementById('meshQuickLocal').value = '';
  renderMeshSubscribe();
}

function removeMeshSubscribe(index) {
  meshSubscribeState.splice(index, 1);
  renderMeshSubscribe();
}

function updateMeshSubscribe(index, field, value) {
  meshSubscribeState[index][field] = value;
}

async function writeInput(address, value) {
  const res = await apiRequest('io.force', { address, value: value ? 'TRUE' : 'FALSE' });
  if (!res.ok) {
    showToast(res.error || 'Unable to write input.', 'error');
    return;
  }
  showToast(`Set ${address} to ${value ? 'TRUE' : 'FALSE'}.`, 'success');
  refresh();
}

function getManual() {
  try { return JSON.parse(localStorage.getItem('trustManual') || '[]'); }
  catch { return []; }
}

function setManual(list) {
  localStorage.setItem('trustManual', JSON.stringify(list));
}

function addManual() {
  const input = document.getElementById('manualEndpoint');
  const value = input.value.trim();
  if (!value) {
    showToast('Enter a PLC URL first.', 'warn');
    return;
  }
  try {
    new URL(value);
  } catch {
    showToast('Invalid URL. Use http://host:port', 'error');
    return;
  }
  const list = getManual();
  if (!list.includes(value)) list.push(value);
  setManual(list);
  input.value = '';
  showToast('Manual PLC added.', 'success');
  refreshDiscovery();
}

function scrollToManual() {
  setPage('network');
  activateTab('network', 'network-discovery');
  const input = document.getElementById('manualEndpoint');
  if (input) {
    input.scrollIntoView({ behavior: 'smooth', block: 'center' });
    input.focus();
  }
}

function renderTopology(items) {
  const target = document.getElementById('topology');
  if (!target) return;
  target.classList.remove('skeleton');
  const nodes = [];
  nodes.push({
    name: currentPlcName || 'This PLC',
    status: currentState || 'unknown',
    addr: 'local',
    url: '',
  });
  items.forEach(item => {
    const addr = item.addresses?.[0] || item.url || 'unknown';
    const url = item.web_port ? `http://${addr}:${item.web_port}` : item.url || '';
    const status = item.status || (item.web_port ? 'online' : 'manual');
    nodes.push({
      name: item.name || 'PLC',
      status,
      addr,
      url,
    });
  });
  if (!nodes.length) {
    target.innerHTML = '<div class="empty">No topology data yet.</div>';
    return;
  }
  target.innerHTML = nodes.map(node => {
    const encoded = node.url ? encodeURIComponent(node.url) : '';
    return `
      <div class="row">
        <span>${escapeHtml(node.name)} <span class="muted">(${escapeHtml(node.addr)})</span></span>
        <span class="stat">${escapeHtml(node.status)}</span>
        ${node.url ? `<button class="btn ghost" onclick="window.open(decodeURIComponent('${encoded}'), '_blank')">Open</button>` : ''}
      </div>
    `;
  }).join('');
}

async function refreshDiscovery() {
  let list = [];
  let discoveryError = null;
  try {
    const res = await fetch('/api/discovery');
    const data = await res.json();
    list = data.items || [];
  } catch (err) {
    discoveryError = 'Discovery unavailable (offline).';
  }
  let manual = getManual().map(url => ({ name: 'manual', url }));
  const localResults = await discoverLocalPeers(list, manual);
  list = list.concat(localResults.discovered);
  manual = manual.concat(localResults.manual);
  const count = list.length + manual.length;
  const countLabel = document.getElementById('connectionCount');
  if (countLabel) {
    countLabel.textContent = `Active connections: ${count}`;
  }
  const probedManual = await Promise.all(manual.map(async item => {
    const probe = await probeRemote(item.url);
    if (probe.ok) {
      return {
        ...item,
        name: probe.name || item.name,
        status: probe.state || 'online',
      };
    }
    if (probe.error === 'auth_required') {
      return { ...item, status: 'auth required' };
    }
    return { ...item, status: probe.error || 'manual' };
  }));
  const html = list.map(item => {
    const addr = item.addresses?.[0] || 'unknown';
    const web = item.web_port ? `http://${addr}:${item.web_port}` : '';
    return `<div class="row"><span>${item.name}</span><span>${web || 'no web'}</span></div>`;
  }).concat(probedManual.map(item => `<div class="row"><span>${item.name || 'manual'}</span><span>${item.url}</span></div>`)).join('');
  const discoveryList = document.getElementById('discovery');
  if (discoveryList) {
    if (html) {
      discoveryList.innerHTML = `${discoveryError ? `<div class="note muted">${discoveryError}</div>` : ''}${html}`;
    } else if (discoveryError) {
      discoveryList.innerHTML = `<div class="empty">${discoveryError}</div>`;
    } else {
      discoveryList.innerHTML = '<div class="empty">No PLCs found on the LAN.</div>';
    }
  }
  renderTopology(list.concat(probedManual));
}

async function refreshDiscoveryWithFeedback() {
  await withLoadingState('refreshDiscoveryBtn', null, 'Refreshing...', async () => {
    await refreshDiscovery();
    showToast('Discovery refreshed.', 'success');
  });
}

async function probeRemote(url) {
  try {
    const res = await fetch(`/api/probe?url=${encodeURIComponent(url)}`);
    if (!res.ok) return { ok: false, error: 'unreachable' };
    const data = await res.json();
    if (data.ok) {
      return { ok: true, name: data.name, state: data.state };
    }
    return { ok: false, error: data.error || 'unreachable' };
  } catch (err) {
    return { ok: false, error: 'unreachable' };
  }
}

async function discoverLocalPeers(discovered, manual) {
  const localHost = window.location.hostname;
  if (localHost !== 'localhost' && localHost !== '127.0.0.1') {
    return { discovered: [], manual: [] };
  }
  const currentPort = window.location.port || '80';
  const knownUrls = new Set([
    ...discovered.map(item => {
      const addr = item.addresses?.[0] || '';
      return item.web_port ? `http://${addr}:${item.web_port}` : '';
    }).filter(Boolean),
    ...manual.map(item => item.url),
  ]);
  const ports = [8080, 8081, 8082, 8083, 8084];
  const candidates = ports
    .filter(port => String(port) !== currentPort)
    .map(port => `http://localhost:${port}`)
    .filter(url => !knownUrls.has(url));
  const discoveredPeers = [];
  for (const url of candidates) {
    const probe = await probeRemote(url);
    if (probe.ok) {
      discoveredPeers.push({
        id: `local-${url}`,
        name: probe.name || 'PLC',
        status: probe.state || 'online',
        addresses: ['localhost'],
        web_port: Number(url.split(':').pop()),
      });
    }
  }
  return { discovered: discoveredPeers, manual: [] };
}

async function loadSettings() {
  const cfg = await apiRequest('config.get');
  if (!cfg.ok) return;
  document.getElementById('logLevel').value = cfg.result['log.level'];
  document.getElementById('watchdogEnabled').value = String(cfg.result['watchdog.enabled']);
  document.getElementById('watchdogTimeout').value = cfg.result['watchdog.timeout_ms'];
  document.getElementById('watchdogAction').value = normalizeEnum(cfg.result['watchdog.action']) || 'halt';
  document.getElementById('faultPolicy').value = normalizeEnum(cfg.result['fault.policy']) || 'halt';
  document.getElementById('retainMode').value = normalizeEnum(cfg.result['retain.mode']) || 'none';
  document.getElementById('retainSaveInterval').value = cfg.result['retain.save_interval_ms'] ?? '';
  document.getElementById('controlMode').value = normalizeEnum(cfg.result['control.mode']) || 'production';
  document.getElementById('debugEnabled').value = String(cfg.result['control.debug_enabled']);
  document.getElementById('webEnabled').value = String(cfg.result['web.enabled']);
  document.getElementById('webListen').value = cfg.result['web.listen'] || '';
  document.getElementById('webAuth').value = normalizeEnum(cfg.result['web.auth']) || 'local';
  document.getElementById('discoveryEnabled').value = String(cfg.result['discovery.enabled']);
  document.getElementById('discoveryServiceName').value = cfg.result['discovery.service_name'] || '';
  document.getElementById('discoveryAdvertise').value = String(cfg.result['discovery.advertise']);
  const interfaces = cfg.result['discovery.interfaces'] || [];
  document.getElementById('discoveryInterfaces').value = Array.isArray(interfaces) ? interfaces.join(',') : '';
  document.getElementById('meshEnabled').value = String(cfg.result['mesh.enabled']);
  document.getElementById('meshListen').value = cfg.result['mesh.listen'] || '';
  const meshAuthSet = cfg.result['mesh.auth_token_set'];
  const meshToken = document.getElementById('meshAuthToken');
  if (meshToken) {
    meshToken.value = '';
    meshToken.placeholder = meshAuthSet ? 'token set (leave blank to keep)' : 'set token';
  }
  meshPublishState = (cfg.result['mesh.publish'] || []).map(value => String(value));
  const subscribe = cfg.result['mesh.subscribe'] || {};
  meshSubscribeState = Object.entries(subscribe).map(([remote, local]) => ({
    remote,
    local: String(local || ''),
  }));
  renderMeshPublish();
  renderMeshSubscribe();
}

async function loadSetupDefaults() {
  const res = await fetch('/api/setup/defaults');
  if (!res.ok) return;
  const defaults = await res.json();
  if (!defaults || !(defaults.project_path || defaults.bundle_path)) return;
  populateDriverSelect('setupDriver', [...uniqueIoDrivers(defaults.supported_drivers), String(defaults.driver || '')], true);
  needsSetup = defaults.needs_setup === true;
  document.getElementById('setupProjectPath').value = defaults.project_path || defaults.bundle_path || '';
  document.getElementById('setupPlcName').value = defaults.resource_name || '';
  document.getElementById('setupCycle').value = defaults.cycle_ms || 100;
  document.getElementById('setupDriver').value = defaults.driver || 'auto';
  document.getElementById('setupUseSystem').value = String(defaults.use_system_io ?? true);
  document.getElementById('setupWriteSystem').value = String(defaults.write_system_io ?? false);
  document.getElementById('setupOverwriteSystem').value = 'false';
  setStatus(
    'setupNote',
    defaults.system_io_exists ? 'System-wide I/O config detected.' : 'No system-wide I/O config found.',
    defaults.system_io_exists ? 'success' : 'warn',
  );
}

function renderQrImage(targetId, text, placeholderId) {
  const el = document.getElementById(targetId);
  const placeholder = placeholderId ? document.getElementById(placeholderId) : null;
  if (!el) return;
  if (!text) {
    el.hidden = true;
    el.removeAttribute('src');
    if (placeholder) placeholder.hidden = false;
    return;
  }
  el.hidden = false;
  if (placeholder) placeholder.hidden = true;
  const encoded = encodeURIComponent(text);
  el.onerror = () => {
    el.hidden = true;
    if (placeholder) placeholder.hidden = false;
  };
  el.onload = () => {
    if (placeholder) placeholder.hidden = true;
  };
  el.setAttribute('src', `/api/qr?text=${encoded}`);
}

async function showInvite() {
  const res = await fetch('/api/invite');
  const data = await res.json();
  if (data.token) {
    navigator.clipboard?.writeText(JSON.stringify(data, null, 2));
    showToast('Invite copied to clipboard.', 'success');
    renderQrImage('inviteQr', JSON.stringify(data), 'inviteQrPlaceholder');
  } else {
    showToast('Invite unavailable. Set a control auth token first.', 'warn');
    renderQrImage('inviteQr', '', 'inviteQrPlaceholder');
  }
}

async function startPairing() {
  await withLoadingState('pairStart', 'pairStatus', 'Generating...', async () => {
    try {
      const res = await fetch('/api/pair/start', { method: 'POST' });
      const data = await res.json();
      if (data.code) {
        document.getElementById('pairCode').textContent = data.code;
        renderQrImage('pairQr', data.code, 'pairQrPlaceholder');
        setStatus('pairStatus', 'Pair code generated (expires in 5 minutes).', 'success');
        showToast('Pair code generated.', 'success');
      } else {
        setStatus('pairStatus', data.error || 'Pairing unavailable.', 'error');
        renderQrImage('pairQr', '', 'pairQrPlaceholder');
      }
    } catch (err) {
      setStatus('pairStatus', 'Pairing unavailable (offline).', 'error');
      renderQrImage('pairQr', '', 'pairQrPlaceholder');
    }
  });
}

async function claimPairing() {
  await withLoadingState('pairClaimButton', 'pairStatus', 'Claiming...', async () => {
    const code = document.getElementById('pairClaim').value.trim();
    if (!code) {
      setStatus('pairStatus', 'Enter a pairing code first.', 'error');
      return;
    }
    try {
      const res = await fetch('/api/pair/claim', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code }),
      });
      const data = await res.json();
      if (data.token) {
        authToken = data.token;
        localStorage.setItem('trustToken', authToken);
        setStatus('pairStatus', 'Paired successfully.', 'success');
        showToast('Pairing complete.', 'success');
        document.getElementById('pairClaim').value = '';
        refresh();
      } else {
        setStatus('pairStatus', data.error || 'Invalid code.', 'error');
      }
    } catch (err) {
      setStatus('pairStatus', 'Pairing unavailable (offline).', 'error');
    }
  });
}

function getDeployHistory() {
  try { return JSON.parse(localStorage.getItem('trustDeployHistory') || '[]'); }
  catch { return []; }
}

function setDeployHistory(list) {
  localStorage.setItem('trustDeployHistory', JSON.stringify(list));
}

function addDeployHistory(entry) {
  const list = getDeployHistory();
  list.unshift(entry);
  setDeployHistory(list.slice(0, 10));
  renderDeployHistory();
}

function renderDeployHistory() {
  const list = getDeployHistory();
  const target = document.getElementById('deployHistory');
  const last = document.getElementById('deployLast');
  if (!target) return;
  target.classList.remove('skeleton');
  if (last) {
    if (list.length) {
      last.textContent = `Last deploy: ${new Date(list[0].ts).toLocaleString()}`;
    } else {
      last.textContent = 'Last deploy: --';
    }
  }
  if (!list.length) {
    target.innerHTML = '<div class="empty">No deployments yet.</div>';
    return;
  }
  target.innerHTML = list.map(item => {
    const files = (item.written || []).join(', ') || 'no files';
    return `<div class="row"><span>${new Date(item.ts).toLocaleString()}</span><span>${item.restart || 'no restart'}</span><span class="muted">${files}</span></div>`;
  }).join('');
}

function clearDeployHistory() {
  localStorage.removeItem('trustDeployHistory');
  renderDeployHistory();
}

async function deployBundle() {
  const runtimeFile = document.getElementById('deployRuntime').files[0];
  const ioFile = document.getElementById('deployIo').files[0];
  const programFile = document.getElementById('deployProgram').files[0];
  const sources = Array.from(document.getElementById('deploySources').files || []);
  const restart = document.getElementById('deployRestart').value || null;
  if (restart === 'cold' && !confirm('Deploy and restart cold?')) {
    return;
  }
  await withLoadingState('deployButton', 'deployStatus', 'Deploying...', async () => {
    const payload = {
      runtime_toml: runtimeFile ? await runtimeFile.text() : null,
      io_toml: ioFile ? await ioFile.text() : null,
      program_stbc_b64: programFile ? await readBase64(programFile) : null,
      sources: sources.length
        ? await Promise.all(sources.map(async (file) => ({
            path: file.webkitRelativePath || file.name,
            content: await file.text(),
          })))
        : null,
      restart,
    };
    try {
      const res = await fetch('/api/deploy', {
        method: 'POST',
        headers: Object.assign(
          { 'Content-Type': 'application/json' },
          authToken ? { 'X-Trust-Token': authToken } : {},
        ),
        body: JSON.stringify(payload),
      });
      const data = await res.json();
      if (data.ok) {
        setStatus('deployStatus', `Deployed: ${data.written.join(', ')}`, 'success');
        addDeployHistory({ ts: Date.now(), restart, written: data.written || [] });
        showToast('Deployment complete.', 'success');
      } else {
        setStatus('deployStatus', data.error || 'Deploy failed.', 'error');
      }
    } catch (err) {
      setStatus('deployStatus', 'Deploy failed (offline).', 'error');
    }
  });
}

async function readBase64(file) {
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

async function applySettings() {
  const discoveryInterfaces = document.getElementById('discoveryInterfaces').value
    .split(',')
    .map(item => item.trim())
    .filter(Boolean);
  const meshPublish = meshPublishState.map(item => String(item).trim()).filter(Boolean);
  const meshSubscribe = meshSubscribeState.reduce((acc, entry) => {
    const remote = String(entry.remote || '').trim();
    const local = String(entry.local || '').trim();
    if (remote && local) acc[remote] = local;
    return acc;
  }, {});
  const params = {
    'log.level': document.getElementById('logLevel').value,
    'watchdog.enabled': document.getElementById('watchdogEnabled').value === 'true',
    'watchdog.timeout_ms': Number(document.getElementById('watchdogTimeout').value || 0),
    'watchdog.action': document.getElementById('watchdogAction').value,
    'fault.policy': document.getElementById('faultPolicy').value,
    'retain.mode': document.getElementById('retainMode').value,
    'retain.save_interval_ms': Number(document.getElementById('retainSaveInterval').value || 0),
    'control.mode': document.getElementById('controlMode').value,
    'control.debug_enabled': document.getElementById('debugEnabled').value === 'true',
    'web.enabled': document.getElementById('webEnabled').value === 'true',
    'web.listen': document.getElementById('webListen').value.trim(),
    'web.auth': document.getElementById('webAuth').value,
    'discovery.enabled': document.getElementById('discoveryEnabled').value === 'true',
    'discovery.service_name': document.getElementById('discoveryServiceName').value.trim(),
    'discovery.advertise': document.getElementById('discoveryAdvertise').value === 'true',
    'discovery.interfaces': discoveryInterfaces,
    'mesh.enabled': document.getElementById('meshEnabled').value === 'true',
    'mesh.listen': document.getElementById('meshListen').value.trim(),
    'mesh.publish': meshPublish,
    'mesh.subscribe': meshSubscribe,
  };
  const meshToken = document.getElementById('meshAuthToken').value.trim();
  if (meshToken) {
    params['mesh.auth_token'] = meshToken;
  }
  await withLoadingState('settingsApply', 'settingsNote', 'Applying...', async () => {
    const res = await apiRequest('config.set', params);
    if (!res.ok) {
      setStatus('settingsNote', res.error || 'Failed to apply settings.', 'error');
      return;
    }
    if (res.result.restart_required?.length) {
      setStatus('settingsNote', 'Restart required to apply some settings.', 'warn');
      showToast('Settings saved. Restart required.', 'warn');
    } else {
      setStatus('settingsNote', 'Settings applied.', 'success');
      showToast('Settings saved.', 'success');
    }
  });
}

async function applySetup() {
  const payload = {
    project_path: document.getElementById('setupProjectPath').value.trim() || null,
    resource_name: document.getElementById('setupPlcName').value.trim() || null,
    cycle_ms: Number(document.getElementById('setupCycle').value || 0) || null,
    driver: document.getElementById('setupDriver').value || null,
    use_system_io: document.getElementById('setupUseSystem').value === 'true',
    write_system_io: document.getElementById('setupWriteSystem').value === 'true',
    overwrite_system_io: document.getElementById('setupOverwriteSystem').value === 'true',
  };
  if (payload.overwrite_system_io && !confirm('Overwrite system-wide I/O config?')) {
    return;
  }
  await withLoadingState('setupApply', 'setupNote', 'Applying...', async () => {
    try {
      const res = await fetch('/api/setup/apply', {
        method: 'POST',
        headers: Object.assign(
          { 'Content-Type': 'application/json' },
          authToken ? { 'X-Trust-Token': authToken } : {},
        ),
        body: JSON.stringify(payload),
      });
      const text = await res.text();
      const isError = text.startsWith('error');
      setStatus('setupNote', text, isError ? 'error' : 'success');
      if (!isError) {
        showToast('Setup saved.', 'success');
        setupDismissed = true;
        localStorage.setItem('trustSetupDismissed', 'true');
        needsSetup = false;
      }
    } catch (err) {
      setStatus('setupNote', 'Setup failed (offline).', 'error');
    }
  });
}

function toggleAdvancedSettings() {
  const section = document.getElementById('advancedSettings');
  const toggle = document.getElementById('toggleAdvanced');
  if (!section || !toggle) return;
  const hidden = section.hasAttribute('hidden');
  if (hidden) {
    section.removeAttribute('hidden');
    toggle.textContent = 'Hide advanced';
  } else {
    section.setAttribute('hidden', '');
    toggle.textContent = 'Show advanced';
  }
}

async function sendControl(type, params) {
  await apiRequest(type, params);
  refresh();
}

function toggleRun() {
  if (!debugEnabled) {
    updateControlAvailability(false);
    return;
  }
  if (currentState === 'paused') {
    sendControl('resume');
  } else {
    sendControl('pause');
  }
}

function confirmRestartCold() {
  if (confirm('Cold restart? This resets retain values.')) {
    sendControl('restart', { mode: 'cold' });
  }
}

function confirmShutdown() {
  if (confirm('Shutdown runtime?')) {
    sendControl('shutdown');
  }
}

function confirmRollback() {
  if (confirm('Rollback to the previous project version and restart warm?')) {
    rollbackDeploy('warm');
  }
}

async function rollbackDeploy(mode) {
  await withLoadingState('rollbackButton', 'deployStatus', 'Rolling back...', async () => {
    try {
      const res = await fetch('/api/rollback', {
        method: 'POST',
        headers: Object.assign(
          { 'Content-Type': 'application/json' },
          authToken ? { 'X-Trust-Token': authToken } : {},
        ),
        body: JSON.stringify({ restart: mode }),
      });
      const data = await res.json();
      if (data.ok) {
        setStatus('deployStatus', `Rolled back to ${data.current}`, 'success');
        addDeployHistory({ ts: Date.now(), restart: mode, written: ['rollback'] });
        showToast('Rollback complete.', 'success');
      } else {
        setStatus('deployStatus', data.error || 'Rollback failed.', 'error');
      }
    } catch (err) {
      setStatus('deployStatus', 'Rollback failed (offline).', 'error');
    }
  });
}

function setPage(page) {
  navButtons.forEach(other => other.classList.remove('active'));
  const active = Array.from(navButtons).find(btn => btn.dataset.page === page);
  if (active) active.classList.add('active');
  sections.forEach(section => {
    section.hidden = section.dataset.page !== page;
  });
  const title = document.getElementById('pageTitle');
  if (title) title.textContent = pageTitles[page] || 'PLC Overview';
  if (page === 'program' && !programLoaded) {
    loadProgram();
  }
  const group = tabGroups.get(page);
  if (group && group.active) {
    group.activate(group.active);
  }
}

function openSetup() {
  setPage('settings');
  activateTab('settings', 'settings-setup');
  const wizard = document.getElementById('setupWizard');
  if (wizard) {
    wizard.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
}

function dismissSetupBanner() {
  setupDismissed = true;
  localStorage.setItem('trustSetupDismissed', 'true');
  updateSetupBanner();
}

async function runHealthCheck() {
  await withLoadingState('healthCheck', 'healthStatus', 'Checking...', async () => {
    const status = await apiRequest('status');
    const tasks = await apiRequest('tasks.stats');
    const io = await apiRequest('io.list');
    if (!status.ok || !tasks.ok || !io.ok) {
      const error = status.error || tasks.error || io.error || 'unknown error';
      setStatus('healthStatus', `Health check failed: ${error}`, 'error');
      return;
    }
    setStatus('healthStatus', 'Health check passed. Runtime and I/O are reachable.', 'success');
    showToast('Health check passed.', 'success');
  });
}

const navButtons = document.querySelectorAll('.nav button');
const sections = document.querySelectorAll('section[data-page]');
navButtons.forEach(btn => {
  btn.addEventListener('click', () => {
    setPage(btn.dataset.page);
  });
});

const tourSteps = [
  {
    title: 'Overview',
    body: 'This page shows your PLC health, cycle timing, and recent events.',
    page: 'overview',
    target: 'healthCard',
  },
  {
    title: 'Setup wizard',
    body: 'Configure PLC name, cycle time, and driver here.',
    page: 'settings',
    target: 'setupWizard',
  },
  {
    title: 'I/O page',
    body: 'View inputs/outputs and simulate loopback I/O.',
    page: 'io',
    target: 'ioInputs',
  },
];

function clearHighlights() {
  document.querySelectorAll('.tour-highlight').forEach(el => el.classList.remove('tour-highlight'));
}

function showTourStep(index) {
  const step = tourSteps[index];
  if (!step) return;
  setPage(step.page);
  clearHighlights();
  const target = document.getElementById(step.target);
  if (target) {
    target.classList.add('tour-highlight');
    target.scrollIntoView({ behavior: 'smooth', block: 'center' });
  }
  document.getElementById('tourTitle').textContent = step.title;
  document.getElementById('tourBody').textContent = step.body;
  document.getElementById('tourProgress').textContent = `Step ${index + 1} of ${tourSteps.length}`;
  document.getElementById('tourBack').disabled = index === 0;
  document.getElementById('tourNext').textContent = index === tourSteps.length - 1 ? 'Finish' : 'Next';
}

function startTour() {
  tourActive = true;
  tourIndex = 0;
  tourPreviousFocus = document.activeElement;
  const overlay = document.getElementById('onboarding');
  overlay.hidden = false;
  showTourStep(tourIndex);
  document.getElementById('tourNext').focus();
}

function nextTour() {
  if (!tourActive) return;
  if (tourIndex >= tourSteps.length - 1) {
    closeTour();
    return;
  }
  tourIndex += 1;
  showTourStep(tourIndex);
}

function prevTour() {
  if (!tourActive || tourIndex === 0) return;
  tourIndex -= 1;
  showTourStep(tourIndex);
}

function closeTour() {
  tourActive = false;
  clearHighlights();
  document.getElementById('onboarding').hidden = true;
  localStorage.setItem('trustOnboardingDone', 'true');
  if (tourPreviousFocus && typeof tourPreviousFocus.focus === 'function') {
    tourPreviousFocus.focus();
  }
}

const paletteCommands = [
  { label: 'Go to Overview', action: () => setPage('overview') },
  { label: 'Go to I/O', action: () => setPage('io') },
  { label: 'Go to Logs', action: () => setPage('logs') },
  { label: 'Go to Program', action: () => setPage('program') },
  { label: 'Go to Deploy', action: () => setPage('deploy') },
  { label: 'Go to Settings', action: () => setPage('settings') },
  { label: 'Go to Network', action: () => setPage('network') },
  { label: 'Open setup wizard', action: () => openSetup() },
  { label: 'Pause PLC', action: () => sendControl('pause') },
  { label: 'Resume PLC', action: () => sendControl('resume') },
  { label: 'Restart warm', action: () => sendControl('restart', { mode: 'warm' }) },
  { label: 'Restart cold', action: () => confirmRestartCold() },
  { label: 'Shutdown', action: () => confirmShutdown() },
];

function openPalette() {
  const palette = document.getElementById('commandPalette');
  palette.hidden = false;
  paletteVisible = true;
  palettePreviousFocus = document.activeElement;
  renderPalette('');
  const input = document.getElementById('paletteInput');
  input.value = '';
  input.focus();
}

function closePalette() {
  const palette = document.getElementById('commandPalette');
  palette.hidden = true;
  paletteVisible = false;
  if (palettePreviousFocus && typeof palettePreviousFocus.focus === 'function') {
    palettePreviousFocus.focus();
  }
}

function renderPalette(filter) {
  const list = document.getElementById('paletteList');
  const term = filter.toLowerCase();
  const items = paletteCommands.filter(cmd => cmd.label.toLowerCase().includes(term));
  if (!items.length) {
    list.innerHTML = '<div class="empty">No matches</div>';
    return;
  }
  list.innerHTML = items.map(cmd => `<button class="btn secondary" onclick="runPaletteCommand('${cmd.label.replace(/'/g, "\'")}')">${cmd.label}</button>`).join('');
}

function runPaletteCommand(label) {
  const cmd = paletteCommands.find(item => item.label === label);
  if (cmd) cmd.action();
  closePalette();
}

function openShortcuts() {
  const overlay = document.getElementById('shortcutsOverlay');
  if (!overlay) return;
  shortcutsVisible = true;
  shortcutsPreviousFocus = document.activeElement;
  overlay.hidden = false;
  const close = document.getElementById('shortcutsClose');
  if (close) close.focus();
}

function closeShortcuts() {
  const overlay = document.getElementById('shortcutsOverlay');
  if (!overlay) return;
  shortcutsVisible = false;
  overlay.hidden = true;
  if (shortcutsPreviousFocus && typeof shortcutsPreviousFocus.focus === 'function') {
    shortcutsPreviousFocus.focus();
  }
}

function setupTabs() {
  document.querySelectorAll('.tabs').forEach(tabs => {
    const page = tabs.closest('.page');
    const pageKey = page?.dataset.page;
    const buttons = Array.from(tabs.querySelectorAll('[data-tab]'));
    const panels = Array.from(page?.querySelectorAll('.tab-panel') || []);
    const activate = (tabId) => {
      buttons.forEach(btn => btn.classList.toggle('active', btn.dataset.tab === tabId));
      panels.forEach(panel => {
        panel.hidden = panel.dataset.tab !== tabId;
      });
      if (pageKey && tabGroups.has(pageKey)) {
        const group = tabGroups.get(pageKey);
        group.active = tabId;
      }
    };
    buttons.forEach(btn => {
      btn.addEventListener('click', () => activate(btn.dataset.tab));
    });
    const defaultTab = tabs.dataset.defaultTab || buttons[0]?.dataset.tab;
    if (defaultTab) activate(defaultTab);
    if (pageKey) {
      tabGroups.set(pageKey, { activate, active: defaultTab });
    }
  });
}

function activateTab(pageKey, tabId) {
  const group = tabGroups.get(pageKey);
  if (group) {
    group.activate(tabId);
  }
}

window.addEventListener('keydown', (event) => {
  const tag = event.target?.tagName?.toLowerCase();
  const typing = tag === 'input' || tag === 'textarea';
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    openPalette();
  }
  if (!typing && event.key === '?' && !event.ctrlKey && !event.metaKey) {
    event.preventDefault();
    openShortcuts();
  }
  if (event.key === 'Escape' && paletteVisible) {
    closePalette();
  }
  if (event.key === 'Escape' && shortcutsVisible) {
    closeShortcuts();
  }
});

const paletteInput = document.getElementById('paletteInput');
if (paletteInput) {
  paletteInput.addEventListener('input', (event) => {
    renderPalette(event.target.value);
  });
}

const paletteOverlay = document.getElementById('commandPalette');
if (paletteOverlay) {
  paletteOverlay.addEventListener('click', (event) => {
    if (event.target === paletteOverlay) {
      closePalette();
    }
  });
}

setupTabs();

const shortcutsOverlay = document.getElementById('shortcutsOverlay');
if (shortcutsOverlay) {
  shortcutsOverlay.addEventListener('click', (event) => {
    if (event.target === shortcutsOverlay) {
      closeShortcuts();
    }
  });
}

const eventSearch = document.getElementById('eventSearch');
if (eventSearch) {
  eventSearch.addEventListener('input', debounce(() => renderEventHistory(), 200));
}
const eventFilterType = document.getElementById('eventFilterType');
if (eventFilterType) {
  eventFilterType.addEventListener('change', () => renderEventHistory());
}
const eventFilterWindow = document.getElementById('eventFilterWindow');
if (eventFilterWindow) {
  eventFilterWindow.addEventListener('change', () => renderEventHistory());
}
const trendSelect = document.getElementById('trendVariable');
if (trendSelect) {
  trendSelect.addEventListener('change', (event) => setTrendVariable(event.target.value));
}

applyInitialSkeletons();
loadSettings();
loadSetupDefaults();
loadIoConfig();
loadWatchList();
renderWatchList();
setTrendWindow(trendWindowMs);
loadEventHistory();
renderEventHistory();
renderFaultsPanel();
renderDeployHistory();
refresh().finally(scheduleRefresh);
refreshDiscovery().finally(scheduleDiscovery);
setInterval(updateLastUpdateLabel, 1000);

document.addEventListener('visibilitychange', () => {
  scheduleRefresh();
  scheduleDiscovery();
});

const initial = window.location.hash.replace('#', '');
if (window.location.pathname === '/setup') {
  openSetup();
  window.history.replaceState(null, '', '/');
} else if (initial === 'setup') {
  openSetup();
} else if (initial) {
  setPage(initial);
}

if (localStorage.getItem('trustOnboardingDone') !== 'true') {
  setTimeout(() => startTour(), 500);
}
