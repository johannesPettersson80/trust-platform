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

async function refreshConnectorStatus() {
  try {
    const response = await apiControl('connectors.status');
    if (!response.ok) {
      throw new Error(response.error || 'connectors.status request failed');
    }
    updateConnectorStatusSummary(response.result || {});
  } catch (error) {
    markConnectorStatusUnavailable(error);
  }
}

function ensureConnectorPollingLoop() {
  if (state.connectorPollHandle !== null) {
    return;
  }
  state.connectorPollHandle = window.setInterval(() => {
    void refreshConnectorStatus();
  }, CONNECTOR_POLL_MS);
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
    if (text) {
      text.textContent = top.label || top.path || top.id || 'Alarm active';
      text.title = top.path || top.id || '';
    }
  } else {
    banner.classList.remove('active');
    if (text) {
      text.textContent = 'No alarms';
      text.title = '';
    }
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
          text.title = '';
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
