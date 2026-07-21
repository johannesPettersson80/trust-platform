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
