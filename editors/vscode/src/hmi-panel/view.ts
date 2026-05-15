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
  <title>HMI Preview</title>
  <style>
    :root {
      color-scheme: light dark;
    }
    body {
      margin: 0;
      font-family: var(--vscode-font-family);
      color: var(--vscode-editor-foreground);
      background: var(--vscode-editor-background);
    }
    header {
      position: sticky;
      top: 0;
      z-index: 2;
      display: flex;
      gap: 8px;
      align-items: center;
      padding: 10px;
      border-bottom: 1px solid var(--vscode-panel-border);
      background: var(--vscode-editor-background);
    }
    #status {
      margin-left: auto;
      font-size: 12px;
      opacity: 0.85;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    #tabs {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      padding: 10px;
      border-bottom: 1px solid var(--vscode-panel-border);
    }
    .tab {
      border: 1px solid var(--vscode-panel-border);
      background: transparent;
      color: inherit;
      border-radius: 999px;
      padding: 4px 10px;
      cursor: pointer;
    }
    .tab.active {
      border-color: var(--vscode-focusBorder);
      background: color-mix(in srgb, var(--vscode-focusBorder) 20%, transparent);
    }
    #widgets {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
      gap: 10px;
      padding: 10px;
      padding-bottom: 24px;
    }
    .group {
      grid-column: 1 / -1;
      margin-top: 10px;
      font-weight: 700;
      opacity: 0.9;
    }
    .widget {
      border: 1px solid var(--vscode-panel-border);
      border-radius: 8px;
      padding: 8px;
      background: color-mix(in srgb, var(--vscode-editor-background) 90%, var(--vscode-editor-foreground) 10%);
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .widget-title {
      font-weight: 700;
      border: 0;
      background: transparent;
      color: inherit;
      text-align: left;
      cursor: pointer;
      padding: 0;
    }
    .widget-value {
      font-family: var(--vscode-editor-font-family);
      font-size: 13px;
      opacity: 0.95;
      word-break: break-all;
    }
    .widget-meta {
      font-size: 11px;
      opacity: 0.7;
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
      border: 1px solid var(--vscode-panel-border);
      border-radius: 8px;
      padding: 10px;
      background: color-mix(in srgb, var(--vscode-editor-background) 92%, var(--vscode-editor-foreground) 8%);
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
      opacity: 0.88;
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
      border: 1px solid var(--vscode-panel-border);
      border-radius: 8px;
      padding: 10px;
      background: color-mix(in srgb, var(--vscode-editor-background) 94%, var(--vscode-editor-foreground) 6%);
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .process-svg-host {
      width: 100%;
      overflow: auto;
      border: 1px solid color-mix(in srgb, var(--vscode-panel-border) 70%, transparent);
      border-radius: 6px;
      padding: 8px;
      box-sizing: border-box;
      background: color-mix(in srgb, var(--vscode-editor-background) 96%, var(--vscode-editor-foreground) 4%);
    }
    .process-svg-host svg {
      width: 100%;
      height: auto;
      display: block;
      min-height: 200px;
    }
    .process-meta {
      font-size: 11px;
      opacity: 0.72;
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
      border: 1px solid color-mix(in srgb, var(--vscode-panel-border) 76%, transparent);
      border-radius: 8px;
      background:
        linear-gradient(0deg, color-mix(in srgb, var(--vscode-editor-background) 88%, transparent), color-mix(in srgb, var(--vscode-editor-background) 88%, transparent)),
        repeating-linear-gradient(90deg, transparent 0 47px, color-mix(in srgb, var(--vscode-panel-border) 30%, transparent) 48px),
        repeating-linear-gradient(0deg, transparent 0 47px, color-mix(in srgb, var(--vscode-panel-border) 30%, transparent) 48px);
    }
    .scene3d-node {
      position: absolute;
      min-width: 72px;
      max-width: 150px;
      min-height: 36px;
      padding: 7px 10px;
      border: 1px solid var(--vscode-panel-border);
      border-radius: 6px;
      color: inherit;
      background: color-mix(in srgb, var(--vscode-button-secondaryBackground) 60%, var(--vscode-editor-background));
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
      border-color: var(--vscode-focusBorder);
      background: color-mix(in srgb, var(--vscode-focusBorder) 20%, var(--vscode-button-secondaryBackground));
    }
    .scene3d-meta {
      font-size: 11px;
      opacity: 0.72;
    }
    .empty {
      font-size: 12px;
      opacity: 0.75;
      padding: 6px 0;
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

      for (const section of sections) {
        const card = document.createElement("article");
        card.className = "section-card";
        card.style.gridColumn = "span " + clampSpan(section?.span, 12);

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
        sectionGrid.appendChild(card);
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
        sectionGrid.appendChild(card);
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
        element.dataset.trustTwinNode = nodeId;
        element.textContent =
          typeof node?.label === "string" && node.label.trim() ? node.label.trim() : nodeId;
        const fallback = index * 1.5;
        const x = sceneNodePosition(node, 0, fallback);
        const z = sceneNodePosition(node, 2, 0);
        element.style.left = 8 + ((x - minX) / spanX) * 84 + "%";
        element.style.top = 12 + ((z - minZ) / spanZ) * 76 + "%";
        if (firstInteraction) {
          element.dataset.trustTwinAction = firstInteraction.action;
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
              type: "trustTwinInteraction",
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
        renderProcessPage(page, visible);
        return;
      }
      if (kind === "scene3d") {
        renderScene3dPage(page);
        return;
      }
      renderSectionWidgets(page, visible);
    }

    function render() {
      if (!state.schema) {
        elements.tabs.innerHTML = "";
        elements.widgets.innerHTML = "<div style='padding:10px;'>No HMI schema available.</div>";
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
