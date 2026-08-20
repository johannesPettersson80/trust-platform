function syncDocumentsToWasm() {
  if (!wasmClient) {
    return;
  }
  const documents = [];
  for (const [path, tab] of state.openTabs) {
    if (isStructuredTextPath(path)) {
      documents.push({ uri: path, text: tab.content });
    }
  }
  if (documents.length === 0) {
    return;
  }
  wasmClient.applyDocuments(documents).catch((error) => {
    console.warn("[IDE] WASM document sync failed:", error);
  });
}

function editorText() {
  return state.editorView ? state.editorView.getValue() : "";
}

function setActiveContent(content) {
  if (!state.editorView) {
    return;
  }
  const current = state.editorView.getValue();
  if (current === content) {
    return;
  }
  state.suppressEditorChange = true;
  state.editorView.setValue(content);
  state.suppressEditorChange = false;
}

function updateCursorLabel() {
  if (!state.editorView) {
    return;
  }
  const pos = fromMonacoPosition(state.editorView.getPosition());
  el.cursorLabel.textContent = `Ln ${pos.line + 1}, Col ${pos.character + 1}`;
}

function cursorPosition() {
  if (!state.editorView) {
    return null;
  }
  return fromMonacoPosition(state.editorView.getPosition());
}

function applyMonacoMarkers(items, model) {
  if (!monaco || !model) {
    return;
  }
  const markers = Array.isArray(items)
    ? items.map((item) => {
      const range = toMonacoRange(item.range || {}, model);
      return {
        startLineNumber: range.startLineNumber,
        startColumn: range.startColumn,
        endLineNumber: range.endLineNumber,
        endColumn: Math.max(range.startColumn + 1, range.endColumn),
        severity: monacoMarkerSeverity(item.severity),
        message: item.message || "diagnostic",
        code: item.code ? String(item.code) : undefined,
      };
    })
    : [];
  monaco.editor.setModelMarkers(model, MONACO_MARKER_OWNER, markers);
}

function setModelLanguageForPath(model, path) {
  if (!monaco || !model) {
    return;
  }
  monaco.editor.setModelLanguage(model, monacoLanguageForPath(path));
}

function disposeEditorDisposables() {
  for (const disposable of state.editorDisposables) {
    try {
      disposable.dispose();
    } catch {
      // no-op
    }
  }
  state.editorDisposables = [];
}

function scheduleAutoCompletionTrigger() {
  if (completionTriggerTimer) {
    clearTimeout(completionTriggerTimer);
    completionTriggerTimer = null;
  }
  completionTriggerTimer = setTimeout(() => {
    startCompletion();
  }, 120);
}

function maybeTriggerCompletionOnEdit(event) {
  const tab = activeTab();
  if (!tab || !isStructuredTextPath(tab.path) || !state.editorView) {
    return;
  }
  if (!Array.isArray(event?.changes) || event.changes.length !== 1) {
    return;
  }
  const change = event.changes[0];
  if (!change || typeof change.text !== "string") {
    return;
  }
  if (change.text.length !== 1) {
    return;
  }
  if (!/[A-Za-z0-9_.]/.test(change.text)) {
    return;
  }
  scheduleAutoCompletionTrigger();
}

function clearHoverPopupTimer() {
  if (cursorHoverPopupTimer) {
    clearTimeout(cursorHoverPopupTimer);
    cursorHoverPopupTimer = null;
  }
}

function scheduleHoverPopupOnMouse(event) {
  clearHoverPopupTimer();
  const tab = activeTab();
  if (!tab || !isStructuredTextPath(tab.path) || !state.editorView) {
    return;
  }
  const target = event?.target;
  const position = target?.position;
  if (!position) {
    return;
  }
  if (monaco?.editor?.MouseTargetType && typeof target?.type === "number") {
    const type = target.type;
    const allowed = new Set([
      monaco.editor.MouseTargetType.CONTENT_TEXT,
      monaco.editor.MouseTargetType.CONTENT_EMPTY,
    ]);
    if (!allowed.has(type)) {
      return;
    }
  }
  cursorHoverPopupTimer = setTimeout(() => {
    if (!state.editorView) {
      return;
    }
    state.editorView.trigger("mouse", "editor.action.showHover", {
      lineNumber: position.lineNumber,
      column: position.column,
    });
  }, 260);
}

function scheduleDocumentHighlight(editor) {
  if (documentHighlightTimer) {
    clearTimeout(documentHighlightTimer);
    documentHighlightTimer = null;
  }
  documentHighlightTimer = setTimeout(() => {
    updateDocumentHighlights(editor);
  }, 150);
}

async function updateDocumentHighlights(editor) {
  if (!wasmClient || !editor) {
    return;
  }
  const model = editor.getModel();
  if (!model) {
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
    return;
  }
  const tab = activeTab();
  if (!tab || !isStructuredTextPath(tab.path)) {
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
    return;
  }
  const position = fromMonacoPosition(editor.getPosition());
  try {
    const highlights = await wasmClient.documentHighlight(tab.path, position);
    if (!Array.isArray(highlights) || highlights.length === 0) {
      documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
      return;
    }
    const decorations = highlights.map((h) => ({
      range: toMonacoRange(h.range, model),
      options: {
        className: h.kind === "write" ? "ide-document-highlight-write" : "ide-document-highlight-read",
        overviewRuler: {color: "#14b8a680", position: monaco.editor.OverviewRulerLane.Center},
      },
    }));
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, decorations);
  } catch {
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
  }
}

function createEditor(initialContent, path) {
  const model = monaco.editor.createModel(initialContent, monacoLanguageForPath(path));
  const view = monaco.editor.create(el.editorMount, {
    model,
    readOnly: !state.writeEnabled,
    automaticLayout: true,
    minimap: {enabled: true, scale: 1, showSlider: "mouseover"},
    lineNumbers: "on",
    scrollBeyondLastLine: false,
    fontFamily: "JetBrains Mono, Fira Code, IBM Plex Mono, monospace",
    fontSize: 13,
    lineHeight: 20,
    tabSize: 2,
    insertSpaces: true,
    quickSuggestions: {other: true, comments: false, strings: true},
    quickSuggestionsDelay: 120,
    suggestOnTriggerCharacters: true,
    wordBasedSuggestions: "off",
    parameterHints: {enabled: true},
    snippetSuggestions: "inline",
    hover: {enabled: "on", delay: 250, sticky: true},
    occurrencesHighlight: "singleFile",
    selectionHighlight: true,
    bracketPairColorization: {enabled: true},
    smoothScrolling: true,
    renderLineHighlight: "all",
    padding: {top: 8, bottom: 8},
    theme: document.body.dataset.theme === "dark" ? "trust-dark" : "trust-light",
  });

  disposeEditorDisposables();

  state.editorDisposables.push(view.onDidChangeModelContent((event) => {
    if (state.suppressEditorChange) {
      return;
    }
    const tab = activeTab();
    if (!tab) {
      return;
    }
    tab.content = view.getValue();
    const dirty = tab.content !== tab.savedContent;
    markTabDirty(tab.path, dirty);
    updateDraftInfo();
    if (dirty) {
      const draftStored = saveDraft(tab.path, tab.content);
      scheduleAutosave();
      if (draftStored) {
        updateSaveBadge(state.online ? "warn" : "err", state.online ? "dirty" : "offline draft");
      }
    } else {
      clearDraft(tab.path);
      updateSaveBadge("ok", "saved");
    }
    syncSecondaryEditor();
    updateCursorLabel();
    syncDocumentsToWasm();
    scheduleDiagnostics();
    maybeTriggerCompletionOnEdit(event);
  }));

  state.editorDisposables.push(view.onDidType((text) => {
    const tab = activeTab();
    if (!tab || !isStructuredTextPath(tab.path)) {
      return;
    }
    const char = String(text || "").slice(-1);
    if (/[A-Za-z0-9_.]/.test(char)) {
      scheduleAutoCompletionTrigger();
    }
  }));

  state.editorDisposables.push(view.onDidChangeCursorPosition((event) => {
    updateCursorLabel();
    scheduleCursorInsights(fromMonacoPosition(event.position));
    scheduleDocumentHighlight(view);
  }));

  state.editorDisposables.push(view.onMouseMove((event) => {
    scheduleHoverPopupOnMouse(event);
  }));

  state.editorDisposables.push(view.onMouseLeave(() => {
    clearHoverPopupTimer();
  }));

  view.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
    saveActiveTab({explicit: true}).catch(() => {});
  });
  view.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Space, () => {
    startCompletion();
  });
  view.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyP, () => {
    openCommandPalette();
  });

  return view;
}

function createSecondaryEditor(initialContent, path) {
  const model = monaco.editor.createModel(initialContent, monacoLanguageForPath(path));
  return monaco.editor.create(el.editorMountSecondary, {
    model,
    readOnly: true,
    automaticLayout: true,
    minimap: {enabled: false},
    lineNumbers: "on",
    scrollBeyondLastLine: false,
    fontFamily: "JetBrains Mono, Fira Code, IBM Plex Mono, monospace",
    fontSize: 13,
    lineHeight: 20,
    renderLineHighlight: "none",
    padding: {top: 8, bottom: 8},
    theme: document.body.dataset.theme === "dark" ? "trust-dark" : "trust-light",
  });
}

function setSecondaryEditorContent(text, path) {
  if (!state.secondaryEditorView) {
    state.secondaryEditorView = createSecondaryEditor(text, path);
    return;
  }
  setModelLanguageForPath(state.secondaryEditorView.getModel(), path);
  const current = state.secondaryEditorView.getValue();
  if (current === text) {
    return;
  }
  state.secondaryEditorView.setValue(text);
}

function setActivePane(pane) {
  state.activePane = pane;
  el.editorPanePrimary.classList.toggle("pane-active", pane === "primary");
  el.editorPaneSecondary.classList.toggle("pane-active", pane === "secondary");
}

function syncSecondaryEditor() {
  if (!state.splitEnabled || !state.editorView) {
    return;
  }
  const path = state.secondaryPath;
  if (!path) {
    return;
  }
  const tab = state.openTabs.get(path);
  if (tab) {
    setSecondaryEditorContent(tab.content, tab.path);
  }
}

function openInSecondaryPane(path, content) {
  const tab = state.openTabs.get(path);
  if (!tab && !content) {
    return;
  }
  state.secondaryPath = path;
  state.secondaryOpenTabs.add(path);
  setSecondaryEditorContent(content || tab.content, path);
}

function toggleSplitEditor() {
  state.splitEnabled = !state.splitEnabled;
  el.editorGrid.classList.toggle("split", state.splitEnabled);
  el.editorPaneSecondary.classList.toggle("ide-hidden", !state.splitEnabled);
  el.splitBtn.setAttribute("aria-label", state.splitEnabled ? "Single editor" : "Toggle split editor");
  el.splitBtn.title = state.splitEnabled ? "Single" : "Split";
  if (state.splitEnabled) {
    // Show per-pane tab bars, hide the shared tab bar
    el.tabBar.classList.add("ide-hidden");
    el.tabBarPrimary.classList.remove("ide-hidden");

    setActivePane("primary");
    if (!state.secondaryPath || state.secondaryPath === state.activePath) {
      for (const [p] of state.openTabs) {
        if (p !== state.activePath) {
          state.secondaryPath = p;
          break;
        }
      }
    }
    // Seed secondary tab set
    if (state.secondaryPath) {
      state.secondaryOpenTabs.add(state.secondaryPath);
    }
    syncSecondaryEditor();
    renderTabs();
  } else {
    // Restore shared tab bar, hide per-pane tab bars
    el.tabBar.classList.remove("ide-hidden");
    el.tabBarPrimary.classList.add("ide-hidden");

    // Merge secondary tabs back into shared openTabs (they already share the Map)
    state.secondaryOpenTabs.clear();
    setActivePane("primary");
    renderTabs();
  }
}

function setEditorContent(text) {
  if (!state.editorView) {
    return;
  }
  const current = state.editorView.getValue();
  if (current === text) {
    return;
  }
  state.suppressEditorChange = true;
  state.editorView.setValue(text);
  state.suppressEditorChange = false;
  syncSecondaryEditor();
  scheduleDiagnostics({immediate: true});
}

