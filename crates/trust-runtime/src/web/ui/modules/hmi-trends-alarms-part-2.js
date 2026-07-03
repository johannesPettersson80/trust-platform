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
      signalCell.title = alarm.path || alarm.id || '';

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
