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
  if (widget.unit) {
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
