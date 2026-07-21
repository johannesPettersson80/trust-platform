function normalizeTaskLocationPath(path) {
  const raw = String(path || "").trim().replaceAll("\\", "/");
  if (!raw) {
    return "";
  }
  return raw.startsWith("./") ? raw.slice(2) : raw;
}

function setRetryAction(action, message) {
  state.lastFailedAction = action || null;
  el.retryActionBtn.disabled = !state.lastFailedAction;
  if (message) {
    setStatus(message);
  }
}

async function retryLastFailedAction() {
  const action = state.lastFailedAction;
  if (!action) {
    setStatus("No failed action to retry.");
    return;
  }
  if (action.kind === "save") {
    if (action.path && state.activePath !== action.path && state.openTabs.has(action.path)) {
      await switchTab(action.path, {preserveSelection: true});
    }
    await saveActiveTab({explicit: true});
    return;
  }
  if (action.kind === "build" || action.kind === "test" || action.kind === "validate") {
    await startTask(action.kind);
    return;
  }
  setStatus(`Unsupported retry action: ${action.kind}`);
}

function renderTaskLinks(locations) {
  el.taskLinksPanel.innerHTML = "";
  if (!Array.isArray(locations) || locations.length === 0) {
    const empty = document.createElement("div");
    empty.className = "muted";
    empty.textContent = "No source links detected.";
    el.taskLinksPanel.appendChild(empty);
    return;
  }
  for (const location of locations.slice(0, 60)) {
    const path = normalizeTaskLocationPath(location.path);
    const line = Math.max(1, Number(location.line || 1));
    const column = Math.max(1, Number(location.column || 1));
    const button = document.createElement("button");
    button.type = "button";
    button.className = "ide-link-button";
    button.textContent = `${path}:${line}:${column} ${location.message || ""}`.trim();
    button.disabled = !path.toLowerCase().endsWith(".st");
    button.addEventListener("click", async () => {
      if (!path || !path.toLowerCase().endsWith(".st")) {
        return;
      }
      await openFile(path);
      jumpToRange({
        start: {
          line: Math.max(0, line - 1),
          character: Math.max(0, column - 1),
        },
      });
    });
    el.taskLinksPanel.appendChild(button);
  }
}

function renderTaskOutput(task) {
  if (!task) {
    el.taskStatus.textContent = "No task running.";
    el.taskOutput.textContent = "Build/Test/Validate output will appear here.";
    renderTaskLinks([]);
    return;
  }
  const status = task.status || "running";
  const suffix = task.success === true ? "success" : task.success === false ? "failed" : "running";
  const started = formatTimestampMs(task.started_ms);
  const finished = task.finished_ms ? formatTimestampMs(task.finished_ms) : null;
  const timing = finished
    ? `started ${started}, finished ${finished}`
    : `started ${started}`;
  el.taskStatus.textContent = `${task.kind} #${task.job_id}: ${status} (${suffix}) | ${timing}`;
  el.taskOutput.textContent = task.output || "";
  renderTaskLinks(task.locations || []);
}

function stopTaskPolling() {
  if (state.taskPollTimer) {
    clearInterval(state.taskPollTimer);
    state.taskPollTimer = null;
  }
}

async function pollActiveTask() {
  if (!state.activeTaskId) {
    return;
  }
  const task = await apiJson(`/api/ide/task?id=${state.activeTaskId}`, {
    method: "GET",
    headers: apiHeaders(),
    timeoutMs: 3000,
  });
  renderTaskOutput(task);
  const done = task.status === "completed";
  if (done) {
    stopTaskPolling();
    if (typeof document === "object") {
      document.dispatchEvent(new CustomEvent("ide-task-complete", {
        detail: {
          jobId: task.job_id,
          kind: task.kind || "unknown",
          success: !!task.success,
          output: String(task.output || ""),
        },
      }));
    }
    if (task.success) {
      if (typeof document === "object") {
        document.dispatchEvent(new CustomEvent("ide-build-log", {
          detail: {
            message: `${task.kind} completed successfully`,
            level: "info",
          },
        }));
      }
      setRetryAction(null, `Task ${task.kind} finished (ok).`);
      if (typeof showIdeToast === "function") {
        const errorCount = (task.output || "").match(/error/gi)?.length || 0;
        const warnCount = (task.output || "").match(/warning/gi)?.length || 0;
        if (task.kind === "validate") {
          showIdeToast(`Validated: ${errorCount} errors, ${warnCount} warnings`, errorCount > 0 ? "error" : "success");
          if (typeof updateProjectStatusDot === "function") {
            const when = (typeof nowLabel === "function") ? nowLabel() : new Date().toLocaleTimeString();
            updateProjectStatusDot(
              errorCount > 0 ? "fail" : warnCount > 0 ? "warn" : "pass",
              `Validated at ${when}: ${errorCount} errors, ${warnCount} warnings`
            );
          }
          // US-3.2: Auto-expand Problems panel when errors found
          if (errorCount > 0) {
            const problemsSection = el.problemsPanel?.closest(".ide-section");
            if (problemsSection && problemsSection.classList.contains("collapsed")) {
              problemsSection.classList.remove("collapsed");
              const header = problemsSection.querySelector(".ide-section-header");
              if (header) header.setAttribute("aria-expanded", "true");
            }
          }
        } else if (task.kind === "build") {
          // US-3.3: Show bundle size on build success
          const sizeMatch = (task.output || "").match(/(\d+(?:\.\d+)?)\s*(?:KB|kB|bytes?|B)\b/i);
          const sizeText = sizeMatch ? ` (${sizeMatch[0]})` : "";
          showIdeToast(`Build successful${sizeText}`, "success");
        } else if (task.kind === "test") {
          const summaryMatch = (task.output || "").match(/(\d+)\s+passed,\s+(\d+)\s+failed,\s+(\d+)\s+errors?/i);
          if (summaryMatch) {
            const passed = Number(summaryMatch[1] || 0);
            const failed = Number(summaryMatch[2] || 0);
            const errors = Number(summaryMatch[3] || 0);
            const isFailure = failed > 0 || errors > 0;
            showIdeToast(`Tests: ${passed} passed, ${failed} failed, ${errors} errors`, isFailure ? "error" : "success");
          } else {
            showIdeToast("Tests completed", "success");
          }
        } else {
          showIdeToast(`${task.kind} completed`, "success");
        }
      }
    } else {
      if (typeof document === "object") {
        document.dispatchEvent(new CustomEvent("ide-build-log", {
          detail: {
            message: `${task.kind} failed`,
            level: "error",
          },
        }));
      }
      setRetryAction({kind: task.kind}, `Task ${task.kind} finished (failed). Retry is available.`);
      if (typeof showIdeToast === "function") {
        showIdeToast(`${task.kind} failed`, "error");
        if (task.kind === "validate") {
          if (typeof updateProjectStatusDot === "function") {
            const when = (typeof nowLabel === "function") ? nowLabel() : new Date().toLocaleTimeString();
            updateProjectStatusDot("fail", `Validation failed at ${when}`);
          }
          // US-3.2: Auto-expand Problems panel on validation failure
          const problemsSection = el.problemsPanel?.closest(".ide-section");
          if (problemsSection && problemsSection.classList.contains("collapsed")) {
            problemsSection.classList.remove("collapsed");
            const header = problemsSection.querySelector(".ide-section-header");
            if (header) header.setAttribute("aria-expanded", "true");
          }
        }
      }
    }
  }
}

async function startTask(kind) {
  try {
    await flushDirtyTabs();
    // US-3.3: Build gating – abort if diagnostics contain errors
    if (kind === "build") {
      const errorCount = (state.diagnostics || []).filter((d) =>
        String(d.severity || "").toLowerCase().includes("error")
      ).length;
      if (errorCount > 0) {
        const msg = `Fix ${errorCount} error${errorCount !== 1 ? "s" : ""} first`;
        setStatus(msg);
        if (typeof showIdeToast === "function") {
          showIdeToast(msg, "error");
        }
        setRetryAction({kind}, msg);
        return;
      }
    }
    const endpoint = kind === "build"
      ? "/api/ide/build"
      : kind === "validate"
        ? "/api/ide/validate"
        : "/api/ide/test";
    const task = await apiJson(endpoint, {
      method: "POST",
      headers: apiHeaders(),
      body: "{}",
      timeoutMs: 3000,
    });
    state.activeTaskId = task.job_id;
    renderTaskOutput(task);
    stopTaskPolling();
    state.taskPollTimer = setInterval(() => {
      pollActiveTask().catch(() => {});
    }, 700);
    setRetryAction(null, `Task started: ${kind} #${task.job_id}`);
    if (typeof document === "object") {
      document.dispatchEvent(new CustomEvent("ide-build-log", {
        detail: {
          message: `${kind} started (#${task.job_id})`,
          level: "info",
        },
      }));
    }
  } catch (error) {
    const message = String(error?.message || error);
    setRetryAction({kind}, `Task ${kind} failed to start: ${message}`);
    throw error;
  }
}

// ── Search & Symbols ───────────────────────────────────

async function gotoDefinitionAtCursor() {
  const tab = activeTab();
  const position = cursorPosition();
  if (!tab || !position) {
    return;
  }
  if (!isStructuredTextPath(tab.path)) {
    setStatus("Go to definition is available for .st files.");
    return;
  }
  syncDocumentsToWasm();
  if (!wasmClient) {
    setStatus("WASM analysis not available.");
    return;
  }
  const result = await wasmClient.definition(tab.path, position);
  if (!result || !result.uri) {
    setStatus("Definition not found.");
    return;
  }
  await openFile(result.uri);
  jumpToRange(result.range);
  setStatus(`Definition: ${result.uri}`);
}

async function refreshReferencesAtPosition(position, {quiet = false} = {}) {
  const tab = activeTab();
  if (!tab || !position || !isStructuredTextPath(tab.path)) {
    state.references = [];
    renderReferences(state.references);
    return [];
  }
  syncDocumentsToWasm();
  if (!wasmClient) {
    state.references = [];
    renderReferences(state.references);
    return [];
  }
  const result = await wasmClient.references(tab.path, position, true);
  state.references = Array.isArray(result) ? result : [];
  renderReferences(state.references);
  if (!quiet) {
    setStatus(`References: ${state.references.length}`);
  }
  return state.references;
}

function scheduleCursorInsights(position) {
  if (cursorInsightTimer) {
    clearTimeout(cursorInsightTimer);
    cursorInsightTimer = null;
  }
  cursorInsightTimer = setTimeout(() => {
    refreshReferencesAtPosition(position, {quiet: true}).catch((error) => {
      console.warn("[ide] reference refresh failed:", error);
    });
  }, 260);
}

async function findReferencesAtCursor() {
  const position = cursorPosition();
  if (!position) {
    return;
  }
  const tab = activeTab();
  if (!tab || !isStructuredTextPath(tab.path)) {
    setStatus("Find references is available for .st files.");
    return;
  }
  await refreshReferencesAtPosition(position, {quiet: false});
}

async function renameSymbolAtCursor() {
  const tab = activeTab();
  const position = cursorPosition();
  if (!tab || !position) {
    return;
  }
  if (!isStructuredTextPath(tab.path)) {
    setStatus("Rename symbol is available for .st files.");
    return;
  }
  const newName = await idePrompt("Rename symbol to:");
  if (!newName || !newName.trim()) {
    return;
  }
  syncDocumentsToWasm();
  if (!wasmClient) {
    setStatus("WASM analysis not available.");
    return;
  }
  const edits = await wasmClient.rename(tab.path, position, newName.trim());
  if (!Array.isArray(edits) || edits.length === 0) {
    setStatus("Rename produced no edits.");
    return;
  }
  // Group edits by uri and apply them to open tabs
  const editsByUri = new Map();
  for (const edit of edits) {
    if (!edit.uri) {
      continue;
    }
    if (!editsByUri.has(edit.uri)) {
      editsByUri.set(edit.uri, []);
    }
    editsByUri.get(edit.uri).push(edit);
  }
  const changedUris = new Set();
  for (const [uri, fileEdits] of editsByUri) {
    const existing = state.openTabs.get(uri);
    if (!existing) {
      continue;
    }
    // Apply edits in reverse order to preserve offsets
    const sorted = fileEdits.sort((a, b) => {
      const lineDiff = (b.range.start.line || 0) - (a.range.start.line || 0);
      if (lineDiff !== 0) return lineDiff;
      return (b.range.start.character || 0) - (a.range.start.character || 0);
    });
    let content = existing.content;
    for (const edit of sorted) {
      const startOffset = positionToContentOffset(content, edit.range.start);
      const endOffset = positionToContentOffset(content, edit.range.end);
      if (startOffset !== null && endOffset !== null) {
        content = content.slice(0, startOffset) + edit.new_text + content.slice(endOffset);
      }
    }
    existing.content = content;
    existing.dirty = existing.content !== existing.savedContent;
    changedUris.add(uri);
  }
  renderTabs();
  if (state.activePath && state.openTabs.has(state.activePath)) {
    await switchTab(state.activePath, {preserveSelection: true});
  }
  syncDocumentsToWasm();
  scheduleDiagnostics();
  setStatus(`Rename applied across ${changedUris.size} file(s).`);
}
async function workspaceSearchFlow() {
  const query = await idePrompt("Workspace search query:");
  if (!query || !query.trim()) {
    return;
  }
  const include = await idePrompt("Include glob (optional, e.g. **/*.st):", "**/*.st");
  const exclude = await idePrompt("Exclude glob (optional):", "");
  const params = new URLSearchParams({
    q: query.trim(),
    limit: "120",
  });
  if (include && include.trim()) {
    params.set("include", include.trim());
  }
  if (exclude && exclude.trim()) {
    params.set("exclude", exclude.trim());
  }
  const result = await apiJson(`/api/ide/search?${params.toString()}`, {
    method: "GET",
    headers: apiHeaders(),
  });
  state.searchHits = Array.isArray(result) ? result : [];
  renderSearchHits(state.searchHits);
  setStatus(`Search results: ${state.searchHits.length}`);
}

async function fetchSymbols(query = "", path = "") {
  const scoped = path ? `&path=${encodeURIComponent(path)}` : "";
  const result = await apiJson(`/api/ide/symbols?q=${encodeURIComponent(query)}&limit=120${scoped}`, {
    method: "GET",
    headers: apiHeaders(),
  });
  return Array.isArray(result) ? result : [];
}

async function fileSymbolSearchFlow() {
  const path = activeTab()?.path;
  if (!path) {
    setStatus("Open a file first to search file symbols.");
    return;
  }
  if (!isStructuredTextPath(path)) {
    setStatus("File symbol search is available for .st files.");
    return;
  }
  const query = await idePrompt(`File symbol query (${path}):`, "");
  if (query === null) {
    return;
  }
  const symbols = await fetchSymbols(query.trim(), path);
  renderSymbolHits(symbols);
  setStatus(`File symbols: ${symbols.length}`);
}

async function workspaceSymbolSearchFlow() {
  const query = await idePrompt("Workspace symbol query:", "");
  if (query === null) {
    return;
  }
  const symbols = await fetchSymbols(query.trim(), "");
  renderSymbolHits(symbols);
  setStatus(`Workspace symbols: ${symbols.length}`);
}

async function loadFsAuditLog() {
  const events = await apiJson("/api/ide/fs/audit?limit=80", {
    method: "GET",
    headers: apiHeaders(),
  });
  el.searchPanel.innerHTML = "";
  if (!Array.isArray(events) || events.length === 0) {
    const empty = document.createElement("div");
    empty.className = "muted";
    empty.textContent = "No filesystem mutation events.";
    el.searchPanel.appendChild(empty);
    return;
  }
  for (const event of events) {
    const row = document.createElement("div");
    row.className = "muted";
    row.textContent = `${event.ts_secs || 0}  ${event.action || "event"}  ${event.path || ""}`;
    el.searchPanel.appendChild(row);
  }
  setStatus(`Filesystem audit events: ${events.length}`);
}

function idePrompt(title, defaultValue = "") {
  return new Promise((resolve) => {
    el.inputModalTitle.textContent = title;
    el.inputModalField.value = defaultValue;
    el.inputModal.classList.add("open");
    el.inputModalField.focus();
    el.inputModalField.select();

    const cleanup = () => {
      el.inputModal.classList.remove("open");
      el.inputModalOk.removeEventListener("click", onOk);
      el.inputModalCancel.removeEventListener("click", onCancel);
      el.inputModalField.removeEventListener("keydown", onKey);
      el.inputModal.removeEventListener("click", onBackdrop);
    };
    const onOk = () => { cleanup(); resolve(el.inputModalField.value); };
    const onCancel = () => { cleanup(); resolve(null); };
    const onKey = (e) => {
      if (e.key === "Enter") { e.preventDefault(); onOk(); }
      if (e.key === "Escape") { e.preventDefault(); onCancel(); }
    };
    const onBackdrop = (e) => { if (e.target === el.inputModal) onCancel(); };
    el.inputModalOk.addEventListener("click", onOk);
    el.inputModalCancel.addEventListener("click", onCancel);
    el.inputModalField.addEventListener("keydown", onKey);
    el.inputModal.addEventListener("click", onBackdrop);
  });
}

function ideConfirm(title, message) {
  return new Promise((resolve) => {
    el.confirmModalTitle.textContent = title;
    el.confirmModalMessage.textContent = message;
    el.confirmModal.classList.add("open");
    el.confirmModalOk.focus();

    const cleanup = () => {
      el.confirmModal.classList.remove("open");
      el.confirmModalOk.removeEventListener("click", onOk);
      el.confirmModalCancel.removeEventListener("click", onCancel);
      el.confirmModal.removeEventListener("click", onBackdrop);
      document.removeEventListener("keydown", onKey);
    };
    const onOk = () => { cleanup(); resolve(true); };
    const onCancel = () => { cleanup(); resolve(false); };
    const onKey = (e) => {
      if (e.key === "Enter") { e.preventDefault(); onOk(); }
      if (e.key === "Escape") { e.preventDefault(); onCancel(); }
    };
    const onBackdrop = (e) => { if (e.target === el.confirmModal) onCancel(); };
    el.confirmModalOk.addEventListener("click", onOk);
    el.confirmModalCancel.addEventListener("click", onCancel);
    el.confirmModal.addEventListener("click", onBackdrop);
    document.addEventListener("keydown", onKey);
  });
}

// ── Command Palette ────────────────────────────────────

function nextTab() {
  const paths = [...state.openTabs.keys()];
  if (paths.length <= 1 || !state.activePath) {
    return;
  }
  const index = paths.indexOf(state.activePath);
  const next = paths[(index + 1) % paths.length];
  switchTab(next).catch(() => {});
}

function previousTab() {
  const paths = [...state.openTabs.keys()];
  if (paths.length <= 1 || !state.activePath) {
    return;
  }
  const index = paths.indexOf(state.activePath);
  const prev = paths[(index - 1 + paths.length) % paths.length];
  switchTab(prev).catch(() => {});
}

function closePalette() {
  el.commandPalette.classList.remove("open");
  el.commandInput.value = "";
  state.commandFilter = "";
}

function openQuickOpenPalette() {
  if (state.files.length === 0) {
    setStatus("No files available. Open a project folder first.");
    return;
  }
  state.commands = state.files.map((path) => ({
    id: `open:${path}`,
    label: `Open ${path}`,
    run: () => openFile(path),
  }));
  state.commandFilter = "";
  state.selectedCommandIndex = 0;
  renderCommandList();
  el.commandPalette.classList.add("open");
  el.commandInput.focus();
}

function paletteCommands() {
  return [
    {id: "save", label: "Save active file", run: () => saveActiveTab({explicit: true})},
    {id: "save-all", label: "Save all open files", run: () => flushDirtyTabs()},
    {id: "new-project", label: "New project", run: () => newProjectFlow()},
    {id: "open-project", label: "Open project folder", run: () => openProjectFlow()},
    {id: "format-document", label: "Format document", run: () => formatActiveDocument()},
    {id: "quick-open", label: "Quick open file", run: () => openQuickOpenPalette()},
    {id: "file-symbols", label: "File symbols", run: () => fileSymbolSearchFlow()},
    {id: "workspace-symbols", label: "Workspace symbols", run: () => workspaceSymbolSearchFlow()},
    {id: "goto-definition", label: "Go to definition", run: () => gotoDefinitionAtCursor()},
    {id: "find-references", label: "Find references", run: () => findReferencesAtCursor()},
    {id: "rename-symbol", label: "Rename symbol", run: () => renameSymbolAtCursor()},
    {id: "workspace-search", label: "Workspace search", run: () => workspaceSearchFlow()},
    {id: "fs-audit", label: "Filesystem audit log", run: () => loadFsAuditLog()},
    {id: "validate", label: "Validate project", run: () => startTask("validate")},
    {id: "build", label: "Build project", run: () => startTask("build")},
    {id: "test", label: "Run project tests", run: () => startTask("test")},
    {id: "retry-last", label: "Retry last failed action", run: () => retryLastFailedAction()},
    {id: "toggle-split", label: "Toggle split editor", run: () => toggleSplitEditor()},
    {id: "theme", label: "Toggle dark/light mode", run: () => toggleTheme()},
    {id: "next-tab", label: "Next tab", run: () => nextTab()},
    {id: "prev-tab", label: "Previous tab", run: () => previousTab()},
    {id: "refresh-files", label: "Refresh file tree", run: () => bootstrapFiles()},
    {
      id: "recover-analysis",
      label: "Recover analysis mode",
      run: () => {
        state.analysis.degraded = false;
        state.analysis.consecutiveFailures = 0;
        state.analysis.lastNoticeAtMs = 0;
        updateLatencyBadge();
        setStatus("Analysis mode reset.");
      },
    },
    {id: "a11y", label: "Show accessibility baseline path", run: () => setStatus(`Accessibility baseline: ${A11Y_REPORT_LINK}`)},
    {id: "completion", label: "Trigger completion", run: () => state.editorView && startCompletion(state.editorView)},
  ];
}

function renderCommandList() {
  const filter = state.commandFilter.trim().toLowerCase();
  const commands = state.commands.filter((cmd) => {
    if (!filter) return true;
    return cmd.label.toLowerCase().includes(filter);
  });
  if (commands.length === 0) {
    el.commandList.innerHTML = "<div class='muted'>No matching commands.</div>";
    return;
  }
  state.selectedCommandIndex = Math.min(state.selectedCommandIndex, commands.length - 1);
  el.commandList.innerHTML = "";
  commands.forEach((command, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `ide-command${index === state.selectedCommandIndex ? " active" : ""}`;
    button.textContent = command.label;
    button.addEventListener("mouseenter", () => {
      state.selectedCommandIndex = index;
      renderCommandList();
    });
    button.addEventListener("click", async () => {
      closePalette();
      await command.run();
    });
    el.commandList.appendChild(button);
  });
}

function openCommandPalette() {
  state.commands = paletteCommands();
  state.commandFilter = "";
  state.selectedCommandIndex = 0;
  renderCommandList();
  el.commandPalette.classList.add("open");
  el.commandInput.focus();
}

async function runSelectedCommand() {
  const filter = state.commandFilter.trim().toLowerCase();
  const commands = state.commands.filter((cmd) => {
    if (!filter) return true;
    return cmd.label.toLowerCase().includes(filter);
  });
  if (commands.length === 0) {
    return;
  }
  const selected = commands[state.selectedCommandIndex] || commands[0];
  closePalette();
  await selected.run();
}
