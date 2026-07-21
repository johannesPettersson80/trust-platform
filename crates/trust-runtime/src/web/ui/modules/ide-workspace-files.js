function activeTab() {
  if (!state.activePath) {
    return null;
  }
  return state.openTabs.get(state.activePath) || null;
}

function saveDraft(path, content) {
  try {
    localStorage.setItem(`${DRAFT_PREFIX}${path}`, content);
    return true;
  } catch (error) {
    const message = String(error?.message || error);
    bumpTelemetry("autosave_failures");
    updateSaveBadge("err", "draft full");
    setStatus(`Local draft storage failed: ${message}`);
    return false;
  }
}

function loadDraft(path) {
  return localStorage.getItem(`${DRAFT_PREFIX}${path}`);
}

function clearDraft(path) {
  localStorage.removeItem(`${DRAFT_PREFIX}${path}`);
}

async function saveActiveTab({explicit = false} = {}) {
  const tab = activeTab();
  if (!tab) {
    return;
  }
  if (!state.writeEnabled || tab.readOnly) {
    updateSaveBadge("warn", "read-only");
    return;
  }
  const latestContent = state.editorView.getValue();
  tab.content = latestContent;
  if (tab.content === tab.savedContent && !explicit) {
    updateSaveBadge("ok", "saved");
    return;
  }

  if (!state.online) {
    updateSaveBadge("err", "offline draft");
    saveDraft(tab.path, tab.content);
    updateDraftInfo();
    return;
  }

  updateSaveBadge("warn", "saving...");
  try {
    const result = await apiJson("/api/ide/file", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify({
        path: tab.path,
        expected_version: tab.version,
        content: tab.content,
      }),
    });
    tab.version = result.version;
    tab.savedContent = tab.content;
    tab.dirty = false;
    clearDraft(tab.path);
    renderTabs();
    updateDraftInfo();
    updateSaveBadge("ok", "saved");
    document.dispatchEvent(new CustomEvent("ide-file-saved", {
      detail: {
        path: tab.path,
        version: tab.version,
      },
    }));
    if (state.lastFailedAction?.kind === "save") {
      setRetryAction(null, `Saved ${tab.path}`);
    } else {
      setStatus(`Saved ${tab.path}`);
    }
  } catch (error) {
    const message = String(error.message || error);
    if (message.includes("current version")) {
      updateSaveBadge("err", "conflict");
      setRetryAction({kind: "save", path: tab.path}, `Save conflict on ${tab.path}. Retry after merge/reload.`);
    } else {
      bumpTelemetry("autosave_failures");
      updateSaveBadge("err", "save failed");
      setRetryAction({kind: "save", path: tab.path}, `Save failed: ${message}`);
    }
    saveDraft(tab.path, tab.content);
    updateDraftInfo();
  }
}

function scheduleAutosave() {
  if (state.autosaveTimer) {
    clearTimeout(state.autosaveTimer);
  }
  state.autosaveTimer = setTimeout(() => {
    saveActiveTab().catch(() => {});
  }, 800);
}

async function flushDirtyTabs() {
  for (const [path, tab] of state.openTabs.entries()) {
    if (!tab.dirty) {
      continue;
    }
    const prev = state.activePath;
    if (path !== state.activePath) {
      await switchTab(path, {preserveSelection: true});
    }
    await saveActiveTab();
    if (prev && prev !== state.activePath) {
      await switchTab(prev, {preserveSelection: true});
    }
  }
}

async function formatActiveDocument() {
  const tab = activeTab();
  if (!tab || !state.editorView) {
    return;
  }
  if (!isStructuredTextPath(tab.path)) {
    setStatus("Format document is available for .st files.");
    return;
  }
  const result = await apiJson("/api/ide/format", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify({
      path: tab.path,
      content: editorText(),
    }),
    timeoutMs: 2500,
  });
  if (!result || typeof result.content !== "string") {
    setStatus("Format did not return document content.");
    return;
  }
  setEditorContent(result.content);
  const currentTab = activeTab();
  if (currentTab) {
    currentTab.content = result.content;
    const dirty = currentTab.content !== currentTab.savedContent;
    markTabDirty(currentTab.path, dirty);
    updateDraftInfo();
    if (dirty) {
      saveDraft(currentTab.path, currentTab.content);
      updateSaveBadge("warn", "dirty");
    } else {
      clearDraft(currentTab.path);
      updateSaveBadge("ok", "saved");
    }
  }
  setStatus(result.changed ? `Formatted ${tab.path}` : `No formatting changes for ${tab.path}`);
}

function parentDirectory(path) {
  const parts = String(path || "").split("/").filter(Boolean);
  if (parts.length <= 1) {
    return "";
  }
  parts.pop();
  return parts.join("/");
}

function selectedDirectory() {
  if (state.selectedPath) {
    const selectedNode = state.selectedPath;
    const kind = nodeKindForPath(selectedNode);
    if (kind === "file") {
      return parentDirectory(selectedNode);
    }
    if (kind === "directory") {
      return selectedNode;
    }
  }
  if (state.activePath) {
    return parentDirectory(state.activePath);
  }
  return "";
}

function remapOpenTabs(oldPath, newPath, isDirectory) {
  const next = new Map();
  for (const [path, tab] of state.openTabs.entries()) {
    if (path === oldPath || (isDirectory && path.startsWith(`${oldPath}/`))) {
      const suffix = path.slice(oldPath.length);
      const mapped = `${newPath}${suffix}`;
      next.set(mapped, {...tab, path: mapped});
    } else {
      next.set(path, tab);
    }
  }
  state.openTabs = next;
  // Remap secondaryOpenTabs
  const nextSecondary = new Set();
  for (const path of state.secondaryOpenTabs) {
    if (path === oldPath || (isDirectory && path.startsWith(`${oldPath}/`))) {
      const suffix = path.slice(oldPath.length);
      nextSecondary.add(`${newPath}${suffix}`);
    } else {
      nextSecondary.add(path);
    }
  }
  state.secondaryOpenTabs = nextSecondary;
  if (state.activePath === oldPath || (isDirectory && state.activePath?.startsWith(`${oldPath}/`))) {
    const suffix = state.activePath.slice(oldPath.length);
    state.activePath = `${newPath}${suffix}`;
  }
  if (state.secondaryPath === oldPath || (isDirectory && state.secondaryPath?.startsWith(`${oldPath}/`))) {
    const suffix = state.secondaryPath.slice(oldPath.length);
    state.secondaryPath = `${newPath}${suffix}`;
  }
}

function removeTabsForPath(path, isDirectory) {
  for (const key of [...state.openTabs.keys()]) {
    if (key === path || (isDirectory && key.startsWith(`${path}/`))) {
      state.openTabs.delete(key);
      state.secondaryOpenTabs.delete(key);
    }
  }
  if (state.activePath === path || (isDirectory && state.activePath?.startsWith(`${path}/`))) {
    state.activePath = null;
  }
  if (state.secondaryPath === path || (isDirectory && state.secondaryPath?.startsWith(`${path}/`))) {
    state.secondaryPath = null;
  }
}
async function createPath(kind) {
  const base = selectedDirectory();
  const defaultPath = kind === "directory"
    ? (base ? `${base}/new_folder` : "new_folder")
    : (base ? `${base}/new_file.st` : "new_file.st");
  const input = await idePrompt(kind === "directory" ? "Create folder path:" : "Create file path:", defaultPath);
  if (!input) {
    return;
  }
  const payload = {
    path: input.trim(),
    kind,
  };
  if (kind === "file") {
    payload.content = "";
  }
  await apiJson("/api/ide/fs/create", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify(payload),
  });
  setStatus(`${kind === "directory" ? "Folder" : "File"} created: ${payload.path}`);
  await bootstrapFiles();
  if (kind === "file") {
    selectPath(payload.path);
    await openFile(payload.path);
  } else {
    selectPath(payload.path);
    state.expandedDirs.add(payload.path);
    renderFileTree();
  }
}

async function renameSelectedPath() {
  const sourcePath = state.selectedPath || state.activePath;
  if (!sourcePath) {
    setStatus("Select a file or folder first.");
    return;
  }
  const nextPath = await idePrompt("Rename/move path to:", sourcePath);
  if (!nextPath || nextPath.trim() === sourcePath) {
    return;
  }
  const result = await apiJson("/api/ide/fs/rename", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify({
      path: sourcePath,
      new_path: nextPath.trim(),
    }),
  });
  const isDirectory = result.kind === "directory";
  remapOpenTabs(sourcePath, result.path, isDirectory);
  selectPath(result.path);
  setStatus(`Renamed: ${sourcePath} -> ${result.path}`);
  await bootstrapFiles();
  if (state.activePath && state.openTabs.has(state.activePath)) {
    await switchTab(state.activePath, {preserveSelection: true});
  } else if (state.files.length > 0) {
    await openFile(state.files[0]);
  }
}

async function deleteSelectedPath() {
  const path = state.selectedPath || state.activePath;
  if (!path) {
    setStatus("Select a file or folder first.");
    return;
  }
  const confirmed = await ideConfirm("Delete", `Delete ${path}?`);
  if (!confirmed) {
    return;
  }
  const isDirectory = nodeKindForPath(path) !== "file";
  await apiJson("/api/ide/fs/delete", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify({path}),
  });
  removeTabsForPath(path, isDirectory);
  selectPath(null);
  setStatus(`Deleted: ${path}`);
  await bootstrapFiles();
  if (!state.activePath && state.files.length > 0) {
    await openFile(state.files[0]);
  } else {
    renderTabs();
  }
}

async function openFile(path, {targetPane} = {}) {
  const pane = targetPane || state.activePane;

  // Ensure the file is loaded into openTabs
  if (!state.openTabs.has(path)) {
    setStatus(`Opening ${path}...`);
    const snapshot = await apiJson(`/api/ide/file?path=${encodeURIComponent(path)}`, {
      method: "GET",
      headers: apiHeaders(),
    });
    const draft = loadDraft(path);
    const content = draft ?? snapshot.content;
    state.openTabs.set(path, {
      path,
      version: Number(snapshot.version),
      savedContent: snapshot.content,
      content,
      dirty: draft !== null && draft !== snapshot.content,
      readOnly: Boolean(snapshot.read_only),
    });
    syncDocumentsToWasm();
  }

  // Route to the correct pane
  if (state.splitEnabled && pane === "secondary") {
    const tab = state.openTabs.get(path);
    openInSecondaryPane(path, tab.content);
    renderTabs();
    return;
  }

  await switchTab(path);
}

function showWelcomeScreen() {
  el.editorWelcome.style.display = "";
  el.editorGrid.style.display = "none";
}

async function switchTab(path, {preserveSelection = false} = {}) {
  const tab = state.openTabs.get(path);
  if (!tab) {
    return;
  }

  if (state.activePath && state.editorView) {
    const previous = state.openTabs.get(state.activePath);
    if (previous) {
      previous.content = state.editorView.getValue();
    }
  }

  state.activePath = path;
  state.selectedPath = path;
  document.dispatchEvent(new CustomEvent("ide-active-path-change", {
    detail: {
      path,
    },
  }));
  renderBreadcrumbs(path);
  el.editorTitle.textContent = `Editor - ${path}`;
  el.editorWelcome.style.display = "none";
  el.editorGrid.style.display = "";

  if (!state.editorView) {
    state.editorView = createEditor(tab.content, tab.path);
  } else {
    setEditorContent(tab.content);
    setModelLanguageForPath(activeModel(), tab.path);
  }
  state.editorView.updateOptions({
    readOnly: !state.writeEnabled || Boolean(tab.readOnly),
  });

  if (!preserveSelection) {
    const model = activeModel();
    const firstColumn = model ? model.getLineFirstNonWhitespaceColumn(1) || 1 : 1;
    const position = new monaco.Position(1, firstColumn);
    state.editorView.setPosition(position);
    state.editorView.revealPositionInCenter(position);
  }

  state.editorView.focus();
  renderFileTree();
  renderTabs();
  syncSecondaryEditor();
  updateCursorLabel();
  scheduleCursorInsights(cursorPosition());
  updateDraftInfo();
  updateSaveBadge(tab.dirty ? "warn" : "ok", tab.dirty ? "dirty" : "saved");
  scheduleDiagnostics({immediate: true});
  setStatus(`Active file: ${path}`);
  postPresenceEvent(path);
  refreshMultiTabCollision();
}
