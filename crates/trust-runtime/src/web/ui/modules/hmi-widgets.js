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

function widgetWritePolicy(widget) {
  if (state.schema?.read_only === true) {
    return {
      locked: true,
      label: 'HMI read-only',
      reason: 'Writes are disabled for this HMI session.',
    };
  }
  if (widget?.writable !== true) {
    return {
      locked: true,
      label: 'Read-only',
      reason: 'This value can be watched but not changed from HMI.',
    };
  }
  return { locked: false };
}

function widgetRendererCarriesUnit(widget) {
  const kind = String(widget?.widget || '').toLowerCase();
  return ['bar', 'gauge', 'module', 'slider', 'sparkline', 'tank'].includes(kind);
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
  const tagStack = document.createElement('div');
  tagStack.className = 'widget-tag-stack';
  tagStack.appendChild(tag);
  const writePolicy = widgetWritePolicy(widget);
  if (writePolicy.locked) {
    card.classList.add('card-read-only');
    const writeBadge = document.createElement('span');
    writeBadge.className = 'widget-policy-badge';
    writeBadge.textContent = writePolicy.label;
    tagStack.appendChild(writeBadge);
  }

  head.appendChild(titleWrap);
  head.appendChild(tagStack);

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
  if (widget.unit && !widgetRendererCarriesUnit(widget)) {
    const unitEl = document.createElement('div');
    unitEl.className = 'card-unit';
    unitEl.textContent = widget.unit;
    card.appendChild(unitEl);
  }
  if (writePolicy.locked) {
    const writeReason = document.createElement('div');
    writeReason.className = 'card-policy-note';
    writeReason.textContent = writePolicy.reason;
    card.appendChild(writeReason);
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
