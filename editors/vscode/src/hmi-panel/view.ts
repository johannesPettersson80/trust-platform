import * as vscode from "vscode";

function nonce(): string {
  const chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let result = "";
  for (let index = 0; index < 32; index += 1) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}

export function getHtml(webview: vscode.Webview): string {
  const scriptNonce = nonce();
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta
    http-equiv="Content-Security-Policy"
    content="default-src 'none'; img-src ${webview.cspSource} https: data:; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${scriptNonce}';"
  />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>HMI</title>
  <style>
    :root {
      color-scheme: light dark;
      --trust-canvas: var(--vscode-editor-background, #0f1116);
      --trust-surface: var(--vscode-editorWidget-background, #1b1f28);
      --trust-surface-raised: var(--vscode-editorHoverWidget-background, #222732);
      --trust-overlay: var(--vscode-editorHoverWidget-background, #12151c);
      --trust-text: var(--vscode-foreground, #cfd6e0);
      --trust-text-muted: var(--vscode-descriptionForeground, #949cab);
      --trust-text-subtle: var(--vscode-disabledForeground, #6b7480);
      --trust-on-accent: var(--vscode-button-foreground, #ffffff);
      --trust-mono: var(--vscode-editor-font-family, ui-monospace, SFMono-Regular, Menlo, monospace);
      --trust-border: var(--vscode-editorWidget-border, var(--vscode-panel-border, #2a2f3a));
      --trust-border-subtle: var(--vscode-panel-border, #23272f);
      --trust-accent: var(--vscode-focusBorder, #4a9eff);
      --trust-ok: var(--vscode-charts-green, var(--vscode-testing-iconPassed, #46c265));
      --trust-warn: var(--vscode-charts-yellow, var(--vscode-editorWarning-foreground, #e0b341));
      --trust-danger: var(--vscode-charts-red, var(--vscode-errorForeground, #f0584f));
      --trust-input-bg: var(--vscode-input-background, #10141b);
      --trust-input-border: var(--vscode-input-border, var(--vscode-editorWidget-border, #343b47));
      --trust-grid-line: color-mix(in srgb, var(--trust-border) 62%, transparent);
      --trust-selected-bg: color-mix(in srgb, var(--trust-accent) 18%, transparent);
      --trust-selected-strong-bg: color-mix(in srgb, var(--trust-accent) 28%, transparent);
      --trust-radius-sm: 4px;
      --trust-radius: 6px;
      --trust-radius-lg: 8px;
      --trust-pill: 999px;
      --trust-ease: 150ms cubic-bezier(.4, 0, .2, 1);
      --trust-shadow: 0 1px 2px rgba(0, 0, 0, .14), 0 3px 10px rgba(0, 0, 0, .10);
    }
    * {
      box-sizing: border-box;
    }
    body {
      margin: 0;
      font-family: var(--vscode-font-family);
      color: var(--trust-text);
      background: var(--trust-canvas);
    }
    header {
      position: sticky;
      top: 0;
      z-index: 2;
      display: flex;
      gap: 8px;
      align-items: center;
      padding: 10px 12px;
      border-bottom: 1px solid var(--trust-border);
      background: var(--trust-overlay);
    }
    #status {
      margin-left: auto;
      font-size: 12px;
      color: var(--trust-text-muted);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    #tabs {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      padding: 10px 12px;
      border-bottom: 1px solid var(--trust-border);
    }
    .tab {
      border-radius: var(--trust-pill);
    }
    .tab.active {
      background: var(--trust-selected-bg);
      border-color: var(--trust-accent);
      color: var(--trust-text);
      font-weight: 650;
    }
    #widgets {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
      gap: 10px;
      padding: 12px;
      padding-bottom: 24px;
    }
    .hmi-empty {
      grid-column: 1 / -1;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 6px;
      min-height: 40vh;
      text-align: center;
      padding: 24px;
    }
    .hmi-empty--state {
      min-height: 48vh;
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius-lg);
      background: var(--trust-surface);
      margin: 12px;
    }
    .hmi-empty-title {
      font-size: 14px;
      font-weight: 600;
      color: var(--trust-text);
    }
    .hmi-empty-sub {
      font-size: 12px;
      color: var(--trust-text-muted);
      max-width: 320px;
    }
    .group {
      grid-column: 1 / -1;
      margin-top: 10px;
      font-weight: 700;
      color: var(--trust-text-muted);
    }
    .widget {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius-lg);
      padding: 8px;
      background: var(--trust-surface);
      display: flex;
      flex-direction: column;
      gap: 8px;
      min-width: 0;
    }
    .widget-title {
      font-weight: 700;
      border: 0;
      background: transparent;
      color: var(--trust-text);
      text-align: left;
      cursor: pointer;
      padding: 0;
      min-height: 0;
      justify-content: flex-start;
    }
    .widget-title:hover:not(:disabled) {
      background: transparent;
      border-color: transparent;
      color: var(--trust-accent);
    }
    .widget-value {
      font-family: var(--trust-mono);
      font-size: 13px;
      color: var(--trust-text);
      word-break: break-all;
    }
    .widget-meta {
      font-size: 11px;
      color: var(--trust-text-muted);
    }
    .edit-row {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 6px;
    }
    .edit-row input {
      width: 100%;
      box-sizing: border-box;
    }
    .section-grid {
      grid-column: 1 / -1;
      display: grid;
      grid-template-columns: repeat(12, minmax(0, 1fr));
      gap: 10px;
      width: 100%;
    }
    .section-card {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius-lg);
      padding: 10px;
      background: var(--trust-surface);
      display: flex;
      flex-direction: column;
      gap: 8px;
      min-width: 0;
    }
    .section-title {
      margin: 0;
      font-size: 12px;
      font-weight: 700;
      letter-spacing: 0.02em;
      color: var(--trust-text-muted);
      text-transform: uppercase;
    }
    .section-widget-grid {
      display: grid;
      grid-template-columns: repeat(12, minmax(0, 1fr));
      gap: 8px;
      width: 100%;
    }
    .process-panel {
      grid-column: 1 / -1;
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius-lg);
      padding: 10px;
      background: var(--trust-surface);
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .process-svg-host {
      width: 100%;
      overflow: auto;
      border: 1px solid color-mix(in srgb, var(--trust-border) 70%, transparent);
      border-radius: var(--trust-radius);
      padding: 8px;
      box-sizing: border-box;
      background: var(--trust-surface-raised);
    }
    .process-svg-host svg {
      width: 100%;
      height: auto;
      display: block;
      min-height: 200px;
    }
    .process-meta {
      font-size: 11px;
      color: var(--trust-text-muted);
    }
    .scene3d-panel {
      grid-column: 1 / -1;
      display: flex;
      flex-direction: column;
      gap: 8px;
      min-width: 0;
    }
    .scene3d-surface {
      position: relative;
      min-height: 360px;
      overflow: hidden;
      border: 1px solid color-mix(in srgb, var(--trust-border) 76%, transparent);
      border-radius: var(--trust-radius-lg);
      background:
        linear-gradient(0deg, color-mix(in srgb, var(--trust-canvas) 88%, transparent), color-mix(in srgb, var(--trust-canvas) 88%, transparent)),
        repeating-linear-gradient(90deg, transparent 0 47px, color-mix(in srgb, var(--trust-grid-line) 70%, transparent) 48px),
        repeating-linear-gradient(0deg, transparent 0 47px, color-mix(in srgb, var(--trust-grid-line) 70%, transparent) 48px);
    }
    .scene3d-node {
      position: absolute;
      min-width: 72px;
      max-width: 150px;
      min-height: 36px;
      padding: 7px 10px;
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      color: var(--trust-text);
      background: var(--trust-surface-raised);
      box-sizing: border-box;
      transform: translate(-50%, -50%);
      font: inherit;
      font-size: 12px;
      text-align: center;
      overflow-wrap: anywhere;
    }
    button.scene3d-node {
      cursor: pointer;
    }
    button.scene3d-node:hover {
      border-color: var(--trust-accent);
      background: var(--trust-selected-bg);
    }
    .scene3d-meta {
      font-size: 11px;
      color: var(--trust-text-muted);
    }
    .empty {
      font-size: 12px;
      color: var(--trust-text-muted);
      padding: 6px 0;
    }
    input {
      background: var(--trust-input-bg);
      border: 1px solid var(--trust-input-border);
      border-radius: var(--trust-radius);
      color: var(--vscode-input-foreground, var(--trust-text));
      font-family: var(--vscode-font-family);
      font-size: 12px;
      padding: 6px 8px;
    }
    input:focus {
      border-color: var(--trust-accent);
      outline: 1px solid var(--trust-accent);
      outline-offset: -1px;
    }
    button {
      align-items: center;
      background: transparent;
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      color: var(--trust-text);
      cursor: pointer;
      display: inline-flex;
      font-family: var(--vscode-font-family);
      font-size: 12px;
      font-weight: 500;
      justify-content: center;
      line-height: 1.25;
      min-height: 30px;
      padding: 7px 10px;
      text-align: center;
      transition: background var(--trust-ease), border-color var(--trust-ease), color var(--trust-ease);
      white-space: nowrap;
    }
    button:hover:not(:disabled) {
      background: var(--trust-selected-bg);
      border-color: var(--trust-accent);
    }
    button:disabled {
      color: var(--trust-text-subtle);
      cursor: not-allowed;
      opacity: 0.6;
    }
    label {
      color: var(--trust-text);
      font-size: 12px;
    }
    @media (max-width: 900px) {
      .section-grid {
        grid-template-columns: repeat(6, minmax(0, 1fr));
      }
      .section-widget-grid {
        grid-template-columns: repeat(6, minmax(0, 1fr));
      }
    }
  </style>
</head>
<body>
  <header>
    <button id="refresh">Refresh</button>
    <label><input id="editMode" type="checkbox" /> Edit layout</label>
    <button id="save" disabled>Save layout</button>
    <span id="status">Loading HMI preview...</span>
  </header>
  <div id="tabs"></div>
  <div id="widgets"></div>
  <script nonce="${scriptNonce}">
    const vscode = acquireVsCodeApi();
    const state = {
      schema: null,
      values: null,
      selectedPage: null,
      editMode: false,
      overrides: {},
    };
    const elements = {
      status: document.getElementById("status"),
      tabs: document.getElementById("tabs"),
      widgets: document.getElementById("widgets"),
      refresh: document.getElementById("refresh"),
      editMode: document.getElementById("editMode"),
      save: document.getElementById("save"),
    };

    function setStatus(text) {
      elements.status.textContent = String(text || "");
      if (!state.schema && text) {
        renderSystemState(String(text));
      }
    }

    function isFiniteNumber(value) {
      return typeof value === "number" && Number.isFinite(value);
    }

    function recordOverride(path, key, value) {
      if (!state.overrides[path]) {
        state.overrides[path] = {};
      }
      if (value === "" || value === null || value === undefined) {
        delete state.overrides[path][key];
      } else {
        state.overrides[path][key] = value;
      }
      if (Object.keys(state.overrides[path]).length === 0) {
        delete state.overrides[path];
      }
      elements.save.disabled = Object.keys(state.overrides).length === 0;
    }

    function toDisplayValue(record) {
      if (!record) {
        return "n/a";
      }
      const value = record.v;
      if (typeof value === "string") {
        return value;
      }
      return JSON.stringify(value);
    }

    function currentPage() {
      const pages = Array.isArray(state.schema?.pages) ? state.schema.pages : [];
      if (pages.length === 0) {
        return null;
      }
      return pages.find((page) => page.id === state.selectedPage) || pages[0];
    }

    function currentPageKind() {
      const page = currentPage();
      const kind = typeof page?.kind === "string" ? page.kind.trim().toLowerCase() : "";
      if (kind === "process" || kind === "trend" || kind === "alarm" || kind === "scene3d") {
        return kind;
      }
      return "dashboard";
    }

    function clampSpan(value, fallback) {
      const numeric = Number(value);
      if (!Number.isFinite(numeric)) {
        return fallback;
      }
      return Math.max(1, Math.min(12, Math.trunc(numeric)));
    }

    function renderTabs() {
      const pages = Array.isArray(state.schema?.pages) ? state.schema.pages : [];
      if (!state.selectedPage && pages.length > 0) {
        state.selectedPage = pages[0].id;
      }
      const validSelected = pages.some((page) => page.id === state.selectedPage);
      if (!validSelected && pages.length > 0) {
        state.selectedPage = pages[0].id;
      }
      elements.tabs.innerHTML = "";
      for (const page of pages) {
        const button = document.createElement("button");
        button.className = "tab" + (page.id === state.selectedPage ? " active" : "");
        button.textContent = page.title || page.id;
        button.addEventListener("click", () => {
          state.selectedPage = page.id;
          render();
        });
        elements.tabs.appendChild(button);
      }
    }

    function createWidgetCard(widget) {
      const card = document.createElement("article");
      card.className = "widget";
      card.style.gridColumn = "span " + clampSpan(widget.widget_span, 12);

      const title = document.createElement("button");
      title.className = "widget-title";
      title.textContent = widget.label;
      title.title = "Open declaration";
      title.addEventListener("click", () => {
        vscode.postMessage({ type: "navigateWidget", payload: { id: widget.id } });
      });
      card.appendChild(title);

      const value = document.createElement("div");
      value.className = "widget-value";
      value.textContent = toDisplayValue(state.values?.values?.[widget.id]);
      card.appendChild(value);

      const meta = document.createElement("div");
      meta.className = "widget-meta";
      meta.textContent =
        widget.path +
        " | " +
        widget.data_type +
        (widget.unit ? " (" + widget.unit + ")" : "");
      card.appendChild(meta);

      if (state.editMode) {
        const rowA = document.createElement("div");
        rowA.className = "edit-row";
        const labelInput = document.createElement("input");
        labelInput.placeholder = "Label";
        labelInput.value = widget.label || "";
        labelInput.addEventListener("change", () => {
          const text = labelInput.value.trim();
          recordOverride(widget.path, "label", text || null);
        });
        const pageInput = document.createElement("input");
        pageInput.placeholder = "Page ID";
        pageInput.value = widget.page || "";
        pageInput.addEventListener("change", () => {
          const text = pageInput.value.trim();
          recordOverride(widget.path, "page", text || null);
        });
        rowA.appendChild(labelInput);
        rowA.appendChild(pageInput);
        card.appendChild(rowA);

        const rowB = document.createElement("div");
        rowB.className = "edit-row";
        const groupInput = document.createElement("input");
        groupInput.placeholder = "Group";
        groupInput.value = widget.group || "";
        groupInput.addEventListener("change", () => {
          const text = groupInput.value.trim();
          recordOverride(widget.path, "group", text || null);
        });
        const orderInput = document.createElement("input");
        orderInput.type = "number";
        orderInput.placeholder = "Order";
        orderInput.value = isFiniteNumber(widget.order) ? String(widget.order) : "";
        orderInput.addEventListener("change", () => {
          const text = orderInput.value.trim();
          if (!text) {
            recordOverride(widget.path, "order", null);
            return;
          }
          const numeric = Number(text);
          if (!Number.isFinite(numeric)) {
            return;
          }
          recordOverride(widget.path, "order", Math.trunc(numeric));
        });
        rowB.appendChild(groupInput);
        rowB.appendChild(orderInput);
        card.appendChild(rowB);
      }

      return card;
    }

    function renderGroupedWidgets(widgets) {
      let lastGroup = "";
      for (const widget of widgets) {
        if (widget.group !== lastGroup) {
          const group = document.createElement("div");
          group.className = "group";
          group.textContent = widget.group;
          elements.widgets.appendChild(group);
          lastGroup = widget.group;
        }
        elements.widgets.appendChild(createWidgetCard(widget));
      }
    }

    function renderSectionWidgets(page, widgets) {
      const sections = Array.isArray(page?.sections) ? page.sections : [];
      if (!sections.length) {
        renderGroupedWidgets(widgets);
        return;
      }
      const byId = new Map(widgets.map((widget) => [widget.id, widget]));
      const used = new Set();
      const sectionGrid = document.createElement("section");
      sectionGrid.className = "section-grid";

      const built = [];

      for (const section of sections) {
        const card = document.createElement("article");
        card.className = "section-card";
        const span = clampSpan(section?.span, 12);
        card.style.gridColumn = "span " + span;

        const title = document.createElement("h3");
        title.className = "section-title";
        title.textContent =
          typeof section?.title === "string" && section.title.trim()
            ? section.title.trim()
            : "Section";
        card.appendChild(title);

        const grid = document.createElement("div");
        grid.className = "section-widget-grid";
        const widgetIds = Array.isArray(section?.widget_ids) ? section.widget_ids : [];
        for (const widgetId of widgetIds) {
          const widget = byId.get(widgetId);
          if (!widget) {
            continue;
          }
          used.add(widget.id);
          grid.appendChild(createWidgetCard(widget));
        }

        if (!grid.children.length) {
          const empty = document.createElement("div");
          empty.className = "empty";
          empty.textContent = "No widgets are mapped to this section.";
          card.appendChild(empty);
        } else {
          card.appendChild(grid);
        }
        built.push({ card, span });
      }

      const unassigned = widgets.filter((widget) => !used.has(widget.id));
      if (unassigned.length) {
        const card = document.createElement("article");
        card.className = "section-card";
        card.style.gridColumn = "span 12";
        const title = document.createElement("h3");
        title.className = "section-title";
        title.textContent = "Other";
        card.appendChild(title);
        const grid = document.createElement("div");
        grid.className = "section-widget-grid";
        for (const widget of unassigned) {
          grid.appendChild(createWidgetCard(widget));
        }
        card.appendChild(grid);
        built.push({ card, span: 12 });
      }

      // A section-card that ends up alone on its 12-column row is stretched to fill the row,
      // so a solo section (e.g. a lone "Key metrics") doesn't leave dead space beside it.
      let packed = 0;
      while (packed < built.length) {
        const rowStart = packed;
        let rowSpan = 0;
        while (packed < built.length && rowSpan + built[packed].span <= 12) {
          rowSpan += built[packed].span;
          packed += 1;
        }
        if (packed === rowStart) {
          packed += 1;
        }
        if (packed - rowStart === 1) {
          built[rowStart].card.style.gridColumn = "span 12";
        }
      }

      for (const entry of built) {
        sectionGrid.appendChild(entry.card);
      }

      elements.widgets.appendChild(sectionGrid);
    }

    function isSafeProcessSelector(selector) {
      return typeof selector === "string" && /^#[A-Za-z0-9_.:-]{1,127}$/.test(selector);
    }

    function isSafeProcessAttribute(attribute) {
      return (
        typeof attribute === "string" &&
        /^(text|fill|stroke|opacity|x|y|width|height|class|transform|data-value)$/.test(attribute)
      );
    }

    function formatProcessRawValue(value) {
      if (value === null || value === undefined) {
        return "--";
      }
      if (typeof value === "number") {
        return Number.isFinite(value) ? String(value) : "--";
      }
      if (typeof value === "boolean") {
        return value ? "true" : "false";
      }
      if (typeof value === "string") {
        return value;
      }
      try {
        return JSON.stringify(value);
      } catch {
        return String(value);
      }
    }

    function scaleProcessValue(value, scale) {
      const numeric = Number(value);
      if (!Number.isFinite(numeric) || !scale || typeof scale !== "object") {
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
      return outputMin + (outputMax - outputMin) * ratio;
    }

    function formatProcessValue(value, format) {
      if (typeof format !== "string" || !format.trim()) {
        return formatProcessRawValue(value);
      }
      const pattern = format.trim();
      const fixedMatch = pattern.match(/\{:\.(\d+)f\}/);
      if (fixedMatch && Number.isFinite(Number(value))) {
        const precision = Number(fixedMatch[1]);
        const formatted = Number(value).toFixed(precision);
        return pattern.replace(/\{:\.(\d+)f\}/, formatted);
      }
      if (pattern.includes("{}")) {
        return pattern.replace("{}", formatProcessRawValue(value));
      }
      return (pattern + " " + formatProcessRawValue(value)).trim();
    }

    function renderProcessPage(page, widgets) {
      const panel = document.createElement("section");
      panel.className = "process-panel";
      if (state.editMode) {
        const note = document.createElement("div");
        note.className = "process-meta";
        note.textContent = "Layout edit mode is disabled for process pages.";
        panel.appendChild(note);
      }

      const svgContent = typeof page?.svg_content === "string" ? page.svg_content.trim() : "";
      if (!svgContent) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent =
          "Process SVG is not available. Add the asset under hmi/ and refresh.";
        panel.appendChild(empty);
        elements.widgets.appendChild(panel);
        return;
      }

      const parser = new DOMParser();
      const doc = parser.parseFromString(svgContent, "image/svg+xml");
      const svgRoot = doc.documentElement;
      if (!svgRoot || String(svgRoot.tagName).toLowerCase() !== "svg") {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = "Invalid process SVG content.";
        panel.appendChild(empty);
        elements.widgets.appendChild(panel);
        return;
      }

      for (const tag of ["script", "foreignObject"]) {
        for (const node of Array.from(svgRoot.querySelectorAll(tag))) {
          node.remove();
        }
      }

      const byPath = new Map(widgets.map((widget) => [widget.path, widget]));
      const bindings = Array.isArray(page?.bindings) ? page.bindings : [];
      let applied = 0;
      for (const binding of bindings) {
        const selector =
          typeof binding?.selector === "string" ? binding.selector.trim() : "";
        const attribute =
          typeof binding?.attribute === "string"
            ? binding.attribute.trim().toLowerCase()
            : "";
        const source = typeof binding?.source === "string" ? binding.source.trim() : "";
        if (!isSafeProcessSelector(selector) || !isSafeProcessAttribute(attribute) || !source) {
          continue;
        }
        const target = svgRoot.querySelector(selector);
        if (!target) {
          continue;
        }
        const widget = byPath.get(source);
        if (!widget) {
          continue;
        }
        const entry = state.values?.values?.[widget.id];
        if (!entry || typeof entry !== "object") {
          continue;
        }
        let resolved = entry.v;
        const mapTable =
          binding?.map && typeof binding.map === "object" ? binding.map : null;
        if (mapTable) {
          const key = formatProcessRawValue(resolved);
          if (Object.prototype.hasOwnProperty.call(mapTable, key)) {
            resolved = mapTable[key];
          }
        }
        resolved = scaleProcessValue(resolved, binding?.scale);
        const text = formatProcessValue(resolved, binding?.format);
        if (attribute === "text") {
          target.textContent = text;
        } else {
          target.setAttribute(attribute, text);
        }
        applied += 1;
      }

      const host = document.createElement("div");
      host.className = "process-svg-host";
      host.appendChild(svgRoot);
      panel.appendChild(host);

      const meta = document.createElement("div");
      meta.className = "process-meta";
      const fileName =
        typeof page?.svg === "string" && page.svg.trim() ? page.svg.trim() : "inline";
      meta.textContent = "SVG: " + fileName + " | active bindings: " + applied;
      panel.appendChild(meta);

      elements.widgets.appendChild(panel);
    }

    function sceneNodeInteractions(node) {
      if (Array.isArray(node?.interaction)) {
        return node.interaction;
      }
      if (Array.isArray(node?.interactions)) {
        return node.interactions;
      }
      return [];
    }

    function sceneNodePosition(node, axis, fallback) {
      const position = node?.transform?.position;
      if (!Array.isArray(position) || position.length < 3) {
        return fallback;
      }
      const value = Number(position[axis]);
      return Number.isFinite(value) ? value : fallback;
    }

    function renderScene3dPage(page) {
      const panel = document.createElement("section");
      panel.className = "scene3d-panel";

      const nodes = Array.isArray(page?.scene_view?.node) ? page.scene_view.node : [];
      if (!nodes.length) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = "3D view payload is not available.";
        panel.appendChild(empty);
        elements.widgets.appendChild(panel);
        return;
      }

      let minX = Infinity;
      let maxX = -Infinity;
      let minZ = Infinity;
      let maxZ = -Infinity;
      nodes.forEach((node, index) => {
        const fallback = index * 1.5;
        const x = sceneNodePosition(node, 0, fallback);
        const z = sceneNodePosition(node, 2, 0);
        minX = Math.min(minX, x);
        maxX = Math.max(maxX, x);
        minZ = Math.min(minZ, z);
        maxZ = Math.max(maxZ, z);
      });
      const spanX = Math.max(1, maxX - minX);
      const spanZ = Math.max(1, maxZ - minZ);

      const surface = document.createElement("div");
      surface.className = "scene3d-surface";
      let interactionCount = 0;
      nodes.forEach((node, index) => {
        const interactions = sceneNodeInteractions(node).filter(
          (interaction) => interaction && typeof interaction.action === "string",
        );
        interactionCount += interactions.length;
        const firstInteraction = interactions.find(
          (interaction) => interaction.action.trim().toLowerCase() === "hmi.write",
        );
        const element = firstInteraction
          ? document.createElement("button")
          : document.createElement("div");
        element.className = "scene3d-node";
        const nodeId = typeof node?.id === "string" && node.id.trim() ? node.id.trim() : "node-" + index;
        element.dataset.sceneNode = nodeId;
        element.textContent =
          typeof node?.label === "string" && node.label.trim() ? node.label.trim() : nodeId;
        const fallback = index * 1.5;
        const x = sceneNodePosition(node, 0, fallback);
        const z = sceneNodePosition(node, 2, 0);
        element.style.left = 8 + ((x - minX) / spanX) * 84 + "%";
        element.style.top = 12 + ((z - minZ) / spanZ) * 76 + "%";
        if (firstInteraction) {
          element.dataset.sceneAction = firstInteraction.action;
          element.addEventListener("click", () => {
            const confirmation = firstInteraction.confirmation;
            if (
              confirmation &&
              typeof confirmation.message === "string" &&
              confirmation.message.trim() &&
              !window.confirm(confirmation.message.trim())
            ) {
              return;
            }
            vscode.postMessage({
              type: "sceneInteraction",
              payload: {
                page: page?.id,
                node: nodeId,
                interaction: firstInteraction,
              },
            });
          });
        }
        surface.appendChild(element);
      });
      panel.appendChild(surface);

      const meta = document.createElement("div");
      meta.className = "scene3d-meta";
      const fileName =
        typeof page?.view === "string" && page.view.trim() ? page.view.trim() : "inline";
      meta.textContent =
        "View: " + fileName + " | nodes: " + nodes.length + " | interactions: " + interactionCount;
      panel.appendChild(meta);
      elements.widgets.appendChild(panel);
    }

    function renderWidgets() {
      elements.widgets.innerHTML = "";
      if (!state.schema) {
        return;
      }
      const page = currentPage();
      const kind = currentPageKind();
      const allWidgets = Array.isArray(state.schema.widgets) ? state.schema.widgets : [];
      const visible = state.selectedPage
        ? allWidgets.filter((widget) => widget.page === state.selectedPage)
        : allWidgets;
      if (kind === "process") {
        renderProcessPage(page, allWidgets);
        return;
      }
      if (kind === "scene3d") {
        renderScene3dPage(page);
        return;
      }
      // Every page gets a designed state — never a blank body. Trend/alarm pages with nothing mapped
      // (or any empty page) explain what to do instead of rendering an empty void.
      if (visible.length === 0) {
        renderEmptyPage(kind, page);
        return;
      }
      renderSectionWidgets(page, visible);
    }

    function renderSystemState(statusText) {
      if (state.schema) {
        return;
      }
      elements.tabs.innerHTML = "";
      elements.widgets.innerHTML = "";
      const wrap = document.createElement("div");
      wrap.className = "hmi-empty hmi-empty--state";
      const heading = document.createElement("div");
      heading.className = "hmi-empty-title";
      const sub = document.createElement("div");
      sub.className = "hmi-empty-sub";
      const normalized = String(statusText || "").toLowerCase();
      if (normalized.includes("start the runtime")) {
        heading.textContent = "Start the runtime to see live HMI data";
        sub.textContent = "Use Start in the truST sidebar, then return here to watch the operator view update.";
      } else if (normalized.includes("could not load")) {
        heading.textContent = "HMI preview could not load";
        sub.textContent = String(statusText || "Check the runtime connection and try Refresh.");
      } else {
        heading.textContent = "Loading HMI preview";
        sub.textContent = String(statusText || "Reading the HMI layout and live values.");
      }
      wrap.appendChild(heading);
      wrap.appendChild(sub);
      elements.widgets.appendChild(wrap);
    }

    // A page with no widgets to render still gets a designed, HONEST state — never a blank void.
    // Trend/alarm pages are usually configured (signals / alarm defs) but the preview doesn't draw
    // time-series charts or the alarm list, so we say exactly that (and name the tracked signals)
    // rather than implying nothing is set up.
    function renderEmptyPage(kind, page) {
      const signals = page && Array.isArray(page.signals) ? page.signals : [];
      const wrap = document.createElement("div");
      wrap.className = "hmi-empty";
      const heading = document.createElement("div");
      heading.className = "hmi-empty-title";
      const sub = document.createElement("div");
      sub.className = "hmi-empty-sub";
      if (kind === "trend") {
        heading.textContent = "Live trend charts aren't shown in this preview";
        sub.textContent = signals.length
          ? "Tracked signals: " + signals.map((s) => s.replace(/^global\./, "")).join(", ") + "."
          : "Add trend signals to this page to chart values over time.";
      } else if (kind === "alarm") {
        heading.textContent = "The alarm list isn't shown in this preview";
        sub.textContent = "This preview renders live values; open the runtime's web HMI for alarms.";
      } else {
        heading.textContent = "Nothing on this page yet";
        sub.textContent = "Map a widget to this page in your HMI layout to see it here.";
      }
      wrap.appendChild(heading);
      wrap.appendChild(sub);
      elements.widgets.appendChild(wrap);
    }

    function render() {
      if (!state.schema) {
        elements.tabs.innerHTML = "";
        renderSystemState("Loading HMI preview");
        return;
      }
      renderTabs();
      renderWidgets();
    }

    window.addEventListener("message", (event) => {
      const message = event.data;
      if (!message || typeof message.type !== "string") {
        return;
      }
      if (message.type === "schema") {
        state.schema = message.payload || null;
        state.overrides = {};
        elements.save.disabled = true;
        render();
        return;
      }
      if (message.type === "values") {
        state.values = message.payload || null;
        renderWidgets();
        return;
      }
      if (message.type === "status") {
        setStatus(message.payload);
        return;
      }
      if (message.type === "layoutSaved") {
        if (message.payload && message.payload.ok) {
          state.overrides = {};
          elements.save.disabled = true;
        }
      }
    });

    elements.refresh.addEventListener("click", () => {
      vscode.postMessage({ type: "refreshSchema" });
    });

    elements.editMode.addEventListener("change", () => {
      state.editMode = Boolean(elements.editMode.checked);
      if (!state.editMode) {
        state.overrides = {};
        elements.save.disabled = true;
      }
      render();
    });

    elements.save.addEventListener("click", () => {
      vscode.postMessage({
        type: "saveLayout",
        payload: { widgets: state.overrides },
      });
    });

    vscode.postMessage({ type: "ready" });
  </script>
</body>
</html>`;
}
