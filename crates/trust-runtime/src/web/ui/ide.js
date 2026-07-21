// truST Web IDE – application logic

// ── Constants & Configuration ──────────────────────────

const DRAFT_PREFIX = "trust.ide.draft.";
const THEME_STORAGE_KEY = "trustTheme";
const IDE_LEFT_WIDTH_KEY = "trust.ide.leftWidth";
const IDE_RIGHT_WIDTH_KEY = "trust.ide.rightWidth";
const A11Y_REPORT_LINK = "docs/guides/WEB_IDE_ACCESSIBILITY_BASELINE.md";
const IDE_PRESENCE_CHANNEL = "trust.ide.presence";
const IDE_PRESENCE_STORAGE_KEY = "trust.ide.presence.event";
const IDE_PRESENCE_CLAIM_TTL_MS = 12_000;
const API_DEFAULT_TIMEOUT_MS = 6_000;
const ANALYSIS_TIMEOUT_MS = 3_000;
const SESSION_EXPIRED_TEXT = "invalid or expired session";
const ST_LANGUAGE_ID = "trust-st";
const MONACO_MARKER_OWNER = "trust.ide";
const TAB_ID = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
const RECENT_PROJECTS_KEY = "trust.ide.recentProjects";
const MAX_RECENT_PROJECTS = 10;
const IDE_SESSION_STORAGE_KEY = "trust.ide.session";

// ── State ──────────────────────────────────────────────

let monaco;
let ensureStyleInjected = () => {};
let completionProviderDisposable = null;
let hoverProviderDisposable = null;
let startCompletion = () => {};
let cursorInsightTimer = null;
let completionTriggerTimer = null;
let cursorHoverPopupTimer = null;
let documentHighlightDecorations = [];
let documentHighlightTimer = null;
let wasmClient = null;

const state = {
  tabId: TAB_ID,
  online: navigator.onLine,
  ready: false,
  sessionToken: null,
  writeEnabled: false,
  files: [],
  tree: [],
  activeProject: null,
  startupProject: null,
  fileFilter: "",
  selectedPath: null,
  expandedDirs: new Set([""]),
  openTabs: new Map(),
  activePath: null,
  editorView: null,
  secondaryEditorView: null,
  secondaryPath: null,
  secondaryOpenTabs: new Set(),
  splitEnabled: false,
  activePane: "primary",
  diagnostics: [],
  references: [],
  searchHits: [],
  latencySamples: [],
  diagnosticsTimer: null,
  diagnosticsTicket: 0,
  autosaveTimer: null,
  healthTimer: null,
  telemetryTimer: null,
  taskPollTimer: null,
  suppressEditorChange: false,
  editorDisposables: [],
  activeTaskId: null,
  lastFailedAction: null,
  presenceChannel: null,
  peerClaims: new Map(),
  collisionPath: null,
  analysis: {
    degraded: false,
    consecutiveFailures: 0,
    lastNoticeAtMs: 0,
  },
  telemetry: {
    bootstrap_failures: 0,
    analysis_timeouts: 0,
    worker_restarts: 0,
    autosave_failures: 0,
  },
  uiMode: "runtime",
  standaloneMode: false,
  commandFilter: "",
  commands: [],
  selectedCommandIndex: 0,
  contextPath: null,
  browseVisible: false,
};

// ── DOM References ─────────────────────────────────────

const el = {
  fileTree: document.getElementById("fileTree"),
  fileFilterInput: document.getElementById("fileFilterInput"),
  newFileBtn: document.getElementById("newFileBtn"),
  newFolderBtn: document.getElementById("newFolderBtn"),
  renamePathBtn: document.getElementById("renamePathBtn"),
  deletePathBtn: document.getElementById("deletePathBtn"),
  breadcrumbBar: document.getElementById("breadcrumbBar"),
  sidebarResizeHandle: document.getElementById("sidebarResizeHandle"),
  tabBar: document.getElementById("tabBar"),
  ideTitle: document.getElementById("ideTitle"),
  headerSyncBadge: document.getElementById("headerSyncBadge"),
  headerSyncPopover: document.getElementById("headerSyncPopover"),
  scopeNote: document.getElementById("scopeNote"),
  statusMode: document.getElementById("statusMode"),
  statusProject: document.getElementById("statusProject"),
  connectionPill: document.getElementById("connectionPill"),
  connectionPillText: document.getElementById("connectionPillText"),
  runtimeState: document.getElementById("runtimeState"),
  alarmCount: document.getElementById("alarmCount"),
  statusText: document.getElementById("statusText"),
  draftInfo: document.getElementById("draftInfo"),
  ideToast: document.getElementById("ideToast"),
  editorTitle: document.getElementById("editorTitle"),
  cursorLabel: document.getElementById("cursorLabel"),
  problemsPanel: document.getElementById("problemsPanel"),
  referencesPanel: document.getElementById("referencesPanel"),
  searchPanel: document.getElementById("searchPanel"),
  taskStatus: document.getElementById("taskStatus"),
  retryActionBtn: document.getElementById("retryActionBtn"),
  taskOutput: document.getElementById("taskOutput"),
  taskLinksPanel: document.getElementById("taskLinksPanel"),
  healthPanel: document.getElementById("healthPanel"),
  statusLatency: document.getElementById("statusLatency"),
  editorPanePrimary: document.getElementById("editorPanePrimary"),
  editorPaneSecondary: document.getElementById("editorPaneSecondary"),
  editorMount: document.getElementById("editorMount"),
  editorMountSecondary: document.getElementById("editorMountSecondary"),
  tabBarPrimary: document.getElementById("tabBarPrimary"),
  tabBarSecondary: document.getElementById("tabBarSecondary"),
  insightResizeHandle: document.getElementById("insightResizeHandle"),
  editorWelcome: document.getElementById("editorWelcome"),
  welcomeNewProjectBtn: document.getElementById("welcomeNewProjectBtn"),
  welcomeOpenBtn: document.getElementById("welcomeOpenBtn"),
  welcomeQuickOpenBtn: document.getElementById("welcomeQuickOpenBtn"),
  editorGrid: document.getElementById("editorGrid"),
  saveBtn: document.getElementById("saveBtn"),
  saveAllBtn: document.getElementById("saveAllBtn"),
  validateBtn: document.getElementById("validateBtn"),
  buildBtn: document.getElementById("buildBtn"),
  testBtn: document.getElementById("testBtn"),
  splitBtn: document.getElementById("splitBtn"),
  settingsBackToFormBtn: document.getElementById("settingsBackToFormBtn"),
  newProjectBtn: document.getElementById("newProjectBtn"),
  openProjectBtn: document.getElementById("openProjectBtn"),
  quickOpenBtn: document.getElementById("quickOpenBtn"),
  headerMoreActions: document.getElementById("headerMoreActions"),
  moreActionsBtn: document.getElementById("moreActionsBtn"),
  moreActionsMenu: document.getElementById("moreActionsMenu"),
  themeToggle: document.getElementById("themeToggle"),
  commandPalette: document.getElementById("commandPalette"),
  commandInput: document.getElementById("commandInput"),
  commandList: document.getElementById("commandList"),
  cmdPaletteBtn: document.getElementById("cmdPaletteBtn"),
  treeContextMenu: document.getElementById("treeContextMenu"),
  ctxOpenBtn: document.getElementById("ctxOpenBtn"),
  ctxNewFileBtn: document.getElementById("ctxNewFileBtn"),
  ctxNewFolderBtn: document.getElementById("ctxNewFolderBtn"),
  ctxRenameBtn: document.getElementById("ctxRenameBtn"),
  ctxDeleteBtn: document.getElementById("ctxDeleteBtn"),
  inputModal: document.getElementById("inputModal"),
  inputModalTitle: document.getElementById("inputModalTitle"),
  inputModalField: document.getElementById("inputModalField"),
  inputModalOk: document.getElementById("inputModalOk"),
  inputModalCancel: document.getElementById("inputModalCancel"),
  confirmModal: document.getElementById("confirmModal"),
  confirmModalTitle: document.getElementById("confirmModalTitle"),
  confirmModalMessage: document.getElementById("confirmModalMessage"),
  confirmModalOk: document.getElementById("confirmModalOk"),
  confirmModalCancel: document.getElementById("confirmModalCancel"),
  openProjectPanel: document.getElementById("openProjectPanel"),
  openProjectInput: document.getElementById("openProjectInput"),
  openProjectRecent: document.getElementById("openProjectRecent"),
  openProjectOk: document.getElementById("openProjectOk"),
  openProjectCancel: document.getElementById("openProjectCancel"),
  browseBtn: document.getElementById("browseBtn"),
  browseListing: document.getElementById("browseListing"),
  browseBreadcrumbs: document.getElementById("browseBreadcrumbs"),
  browseEntries: document.getElementById("browseEntries"),
  newProjectModal: document.getElementById("newProjectModal"),
  newProjectName: document.getElementById("newProjectName"),
  newProjectLocation: document.getElementById("newProjectLocation"),
  newProjectBrowseBtn: document.getElementById("newProjectBrowseBtn"),
  newProjectTemplate: document.getElementById("newProjectTemplate"),
  newProjectPreview: document.getElementById("newProjectPreview"),
  newProjectOk: document.getElementById("newProjectOk"),
  newProjectCancel: document.getElementById("newProjectCancel"),
  hardwarePalette: document.getElementById("hardwarePalette"),
  hwWorkspace: document.getElementById("hwWorkspace"),
  hwEmptyState: document.getElementById("hwEmptyState"),
  hwPresets: document.getElementById("hwPresets"),
  hwSummary: document.getElementById("hwSummary"),
  hwCanvas: document.getElementById("hwCanvas"),
  hwAddressTable: document.getElementById("hwAddressTable"),
  hwDriverCards: document.getElementById("hwDriverCards"),
  hwPropertyPanel: document.getElementById("hwPropertyPanel"),
  hwRuntimeSelect: document.getElementById("hwRuntimeSelect"),
  hwTransportPills: document.getElementById("hwTransportPills"),
  hwViewCanvas: document.getElementById("hwViewCanvas"),
  hwViewTable: document.getElementById("hwViewTable"),
  hwFitCanvasBtn: document.getElementById("hwFitCanvasBtn"),
  hwCenterCanvasBtn: document.getElementById("hwCenterCanvasBtn"),
  hwToggleInspectorBtn: document.getElementById("hwToggleInspectorBtn"),
  hwToggleDriversBtn: document.getElementById("hwToggleDriversBtn"),
  hwFullscreenBtn: document.getElementById("hwFullscreenBtn"),
  hwCanvasToolbar: document.getElementById("hwCanvasToolbar"),
  hwLegendToggleBtn: document.getElementById("hwLegendToggleBtn"),
  hwLegend: document.getElementById("hwLegend"),
  hwDriversPanel: document.getElementById("hwDriversPanel"),
  hwDriversPanelToggleBtn: document.getElementById("hwDriversPanelToggleBtn"),
  hwReloadConfigBtn: document.getElementById("hwReloadConfigBtn"),
  hwNodeContextMenu: document.getElementById("hwNodeContextMenu"),
  hwEdgeContextMenu: document.getElementById("hwEdgeContextMenu"),
  hwCtxCreateLinkBtn: document.getElementById("hwCtxCreateLinkBtn"),
  hwCtxRuntimeSettingsBtn: document.getElementById("hwCtxRuntimeSettingsBtn"),
  hwCtxRuntimeCommSettingsBtn: document.getElementById("hwCtxRuntimeCommSettingsBtn"),
  hwCtxCreateLinkFromEdgeBtn: document.getElementById("hwCtxCreateLinkFromEdgeBtn"),
  hwCtxEditLinkBtn: document.getElementById("hwCtxEditLinkBtn"),
  hwCtxDeleteLinkBtn: document.getElementById("hwCtxDeleteLinkBtn"),
  hwCtxOpenLinkSettingsBtn: document.getElementById("hwCtxOpenLinkSettingsBtn"),
  hwCtxOpenTransportSettingsBtn: document.getElementById("hwCtxOpenTransportSettingsBtn"),
  connectionDialog: document.getElementById("connectionDialog"),
  connectionDialogClose: document.getElementById("connectionDialogClose"),
  connectAddress: document.getElementById("connectAddress"),
  connectPort: document.getElementById("connectPort"),
  connectAuthFields: document.getElementById("connectAuthFields"),
  connectUsername: document.getElementById("connectUsername"),
  connectPassword: document.getElementById("connectPassword"),
  connectBtn: document.getElementById("connectBtn"),
  connectStatus: document.getElementById("connectStatus"),
  connectionRetryBtn: document.getElementById("connectionRetryBtn"),
  discoveredRuntimes: document.getElementById("discoveredRuntimes"),
  recentConnections: document.getElementById("recentConnections"),
  deployBtn: document.getElementById("deployBtn"),
  syncBadge: document.getElementById("syncBadge"),
  liveValuesToggle: document.getElementById("liveValuesToggle"),
  debugToolbar: document.getElementById("debugToolbar"),
  debugForceBanner: document.getElementById("debugForceBanner"),
  debugVariablesPanel: document.getElementById("debugVariablesPanel"),
  debugCallStackPanel: document.getElementById("debugCallStackPanel"),
  debugWatchPanel: document.getElementById("debugWatchPanel"),
  settingsCategories: document.getElementById("settingsCategories"),
  settingsFormPanel: document.getElementById("settingsFormPanel"),
  logsSources: document.getElementById("logsSources"),
  logsFilterBar: document.getElementById("logsFilterBar"),
  logsTablePanel: document.getElementById("logsTablePanel"),
};

// ── Utilities ──────────────────────────────────────────

function nowLabel() {
  return new Date().toLocaleTimeString();
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function isStructuredTextPath(path) {
  return String(path || "").toLowerCase().endsWith(".st");
}

function formatTimestampMs(value) {
  const asNumber = Number(value || 0);
  if (!Number.isFinite(asNumber) || asNumber <= 0) {
    return "--";
  }
  return new Date(asNumber).toLocaleTimeString();
}

function setStatus(text) {
  el.statusText.textContent = text;
}

function bumpTelemetry(key, amount = 1) {
  const current = Number(state.telemetry[key] || 0);
  state.telemetry[key] = current + amount;
}

function isTimeoutMessage(message) {
  const text = String(message || "").toLowerCase();
  return text.includes("timeout");
}

function bindAction(element, action, errorLabel) {
  element.addEventListener("click", () => {
    action().catch((error) => {
      if (errorLabel) setStatus(`${errorLabel}: ${error.message || error}`);
    });
  });
}

// ── API Layer ──────────────────────────────────────────

function apiHeaders(extra = {}, includeSession = true) {
  const headers = {
    "Content-Type": "application/json",
    ...extra,
  };
  if (includeSession && state.sessionToken) {
    headers["X-Trust-Ide-Session"] = state.sessionToken;
  }
  return headers;
}

function clearStoredIdeSession() {
  try {
    localStorage.removeItem(IDE_SESSION_STORAGE_KEY);
  } catch {
    // ignore storage failures
  }
}

function loadStoredIdeSession(expectedRole) {
  try {
    const raw = localStorage.getItem(IDE_SESSION_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    const token = typeof parsed?.token === "string" ? parsed.token.trim() : "";
    const role = typeof parsed?.role === "string" ? parsed.role.trim().toLowerCase() : "";
    if (!token || !role) return null;
    if (expectedRole && role !== String(expectedRole).toLowerCase()) return null;
    return { token, role };
  } catch {
    return null;
  }
}

function persistIdeSession(token, role) {
  const normalizedToken = typeof token === "string" ? token.trim() : "";
  const normalizedRole = typeof role === "string" ? role.trim().toLowerCase() : "";
  if (!normalizedToken || !normalizedRole) {
    clearStoredIdeSession();
    return;
  }
  try {
    localStorage.setItem(
      IDE_SESSION_STORAGE_KEY,
      JSON.stringify({
        token: normalizedToken,
        role: normalizedRole,
        saved_at_ms: Date.now(),
      }),
    );
  } catch {
    // ignore storage failures
  }
}

async function requestNewSession(preferredRole) {
  const role = preferredRole || (state.writeEnabled ? "editor" : "viewer");
  const response = await fetch("/api/ide/session", {
    method: "POST",
    headers: apiHeaders({}, false),
    body: JSON.stringify({role}),
  });
  const text = await response.text();
  const payload = text ? JSON.parse(text) : {};
  if (!response.ok || payload.ok === false) {
    const message = payload.error || `session refresh failed (${response.status})`;
    clearStoredIdeSession();
    throw new Error(message);
  }
  const session = payload.result || {};
  state.sessionToken = session.token || null;
  if (state.sessionToken) {
    persistIdeSession(state.sessionToken, session.role || role);
  } else {
    clearStoredIdeSession();
  }
  return payload.result;
}

async function apiJson(url, options = {}) {
  const {
    timeoutMs = API_DEFAULT_TIMEOUT_MS,
    allowSessionRetry = true,
    ...fetchOptions
  } = options;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const opts = {
    method: "GET",
    ...fetchOptions,
    headers: {
      ...(fetchOptions.headers || {}),
    },
    signal: controller.signal,
  };

  try {
    const response = await fetch(url, opts);
    const text = await response.text();
    let payload = {};
    try {
      payload = text ? JSON.parse(text) : {};
    } catch {
      payload = {ok: false, error: text || "invalid response"};
    }
    if (!response.ok || payload.ok === false) {
      const message = payload.error || `request failed (${response.status})`;
      const normalizedMessage = String(message || "").toLowerCase();
      const sessionAuthError = (
        normalizedMessage.includes(SESSION_EXPIRED_TEXT) ||
        normalizedMessage.includes("missing x-trust-ide-session") ||
        normalizedMessage.includes("invalid session") ||
        normalizedMessage.includes("expired session")
      );
      if (allowSessionRetry && state.ready && sessionAuthError) {
        await requestNewSession();
        return await apiJson(url, {
          ...options,
          allowSessionRetry: false,
        });
      }
      throw new Error(message);
    }
    state.online = true;
    updateConnectionBadge();
    if (payload && typeof payload === "object" && Object.prototype.hasOwnProperty.call(payload, "result")) {
      return payload.result;
    }
    return payload;
  } catch (error) {
    if (error?.name === "AbortError") {
      throw new Error(`request timeout after ${timeoutMs}ms`);
    }
    if (error instanceof TypeError) {
      state.online = false;
      updateConnectionBadge();
    }
    throw error;
  } finally {
    clearTimeout(timer);
  }
}



// ── Event Binding ──────────────────────────────────────

function closeMoreActionsMenu() {
  if (!el.moreActionsMenu || !el.moreActionsBtn || !el.headerMoreActions) return;
  el.moreActionsMenu.hidden = true;
  el.moreActionsBtn.setAttribute("aria-expanded", "false");
  el.headerMoreActions.classList.remove("open");
}

function openMoreActionsMenu() {
  if (!el.moreActionsMenu || !el.moreActionsBtn || !el.headerMoreActions) return;
  el.moreActionsMenu.hidden = false;
  el.moreActionsBtn.setAttribute("aria-expanded", "true");
  el.headerMoreActions.classList.add("open");
}

function bindHeaderOverflowMenu() {
  if (!el.moreActionsBtn || !el.moreActionsMenu || !el.headerMoreActions) return;

  el.moreActionsBtn.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    const expanded = el.moreActionsBtn.getAttribute("aria-expanded") === "true";
    if (expanded) {
      closeMoreActionsMenu();
    } else {
      openMoreActionsMenu();
    }
  });

  el.moreActionsMenu.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    if (target.closest("button")) {
      closeMoreActionsMenu();
    }
  });
}

function bindGlobalEvents() {
  bindResizeHandles();
  bindHeaderOverflowMenu();

  // DRY: action bindings
  bindAction(el.saveBtn, () => saveActiveTab({explicit: true}));
  bindAction(el.saveAllBtn, () => flushDirtyTabs());
  bindAction(el.buildBtn, () => startTask("build"), "Build failed");
  bindAction(el.validateBtn, () => startTask("validate"), "Validate failed");
  bindAction(el.testBtn, () => startTask("test"), "Test failed");
  bindAction(el.retryActionBtn, () => retryLastFailedAction(), "Retry failed");
  el.splitBtn.addEventListener("click", () => toggleSplitEditor());
  el.editorPanePrimary.addEventListener("mousedown", () => { if (state.splitEnabled) setActivePane("primary"); });
  el.editorPaneSecondary.addEventListener("mousedown", () => { if (state.splitEnabled) setActivePane("secondary"); });
  bindAction(el.newProjectBtn, () => newProjectFlow(), "New project failed");
  bindAction(el.openProjectBtn, () => openProjectFlow(), "Open folder failed");
  el.quickOpenBtn.addEventListener("click", () => openQuickOpenPalette());
  bindAction(el.welcomeNewProjectBtn, () => newProjectFlow(), "New project failed");
  bindAction(el.welcomeOpenBtn, () => openProjectFlow(), "Open folder failed");
  el.welcomeQuickOpenBtn.addEventListener("click", () => openQuickOpenPalette());
  bindAction(el.newFileBtn, () => createPath("file"), "Create file failed");
  bindAction(el.newFolderBtn, () => createPath("directory"), "Create folder failed");
  bindAction(el.renamePathBtn, () => renameSelectedPath(), "Rename failed");
  bindAction(el.deletePathBtn, () => deleteSelectedPath(), "Delete failed");

  el.fileFilterInput.addEventListener("input", (event) => {
    state.fileFilter = String(event.target.value || "").trim().toLowerCase();
    renderFileTree();
  });
  el.themeToggle.addEventListener("click", () => toggleTheme());
  el.cmdPaletteBtn.addEventListener("click", () => openCommandPalette());

  // Context menu actions
  el.ctxOpenBtn.addEventListener("click", () => {
    const path = state.contextPath;
    closeTreeContextMenu();
    if (!path) return;
    if (nodeKindForPath(path) === "file") {
      openFile(path).catch((error) => setStatus(`Open failed: ${error.message || error}`));
    } else {
      toggleDir(path);
    }
  });
  bindAction(el.ctxNewFileBtn, () => { closeTreeContextMenu(); return createPath("file"); }, "Create file failed");
  bindAction(el.ctxNewFolderBtn, () => { closeTreeContextMenu(); return createPath("directory"); }, "Create folder failed");
  bindAction(el.ctxRenameBtn, () => { closeTreeContextMenu(); return renameSelectedPath(); }, "Rename failed");
  bindAction(el.ctxDeleteBtn, () => { closeTreeContextMenu(); return deleteSelectedPath(); }, "Delete failed");

  // Open project panel
  el.openProjectOk.addEventListener("click", () => {
    const val = el.openProjectInput.value;
    const returnTo = el.openProjectOk.dataset.returnTo;
    delete el.openProjectOk.dataset.returnTo;
    closeOpenProjectPanel();
    if (returnTo === "newProject") {
      openNewProjectModal(val);
      return;
    }
    doOpenProject(val).catch((error) => setStatus(`Open folder failed: ${error.message || error}`));
  });
  el.openProjectCancel.addEventListener("click", () => closeOpenProjectPanel());

  // New project modal
  el.newProjectOk.addEventListener("click", () => {
    submitNewProject().catch((error) => setStatus(`Create project failed: ${error.message || error}`));
  });
  el.newProjectCancel.addEventListener("click", () => closeNewProjectModal());
  el.newProjectName.addEventListener("input", () => updateNewProjectPreview());
  el.newProjectLocation.addEventListener("input", () => updateNewProjectPreview());
  el.newProjectName.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      submitNewProject().catch((error) => setStatus(`Create project failed: ${error.message || error}`));
    } else if (event.key === "Escape") {
      closeNewProjectModal();
    }
  });
  el.newProjectBrowseBtn.addEventListener("click", () => {
    closeNewProjectModal();
    openProjectPanel();
    el.openProjectOk.dataset.returnTo = "newProject";
  });

  // Browse button
  if (el.browseBtn) {
    el.browseBtn.addEventListener("click", () => {
      if (state.browseVisible) {
        hideBrowseListing();
      } else {
        const current = el.openProjectInput.value.trim() || undefined;
        browseTo(current);
      }
    });
  }

  el.openProjectInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      const items = state._recentItems || [];
      const idx = state._recentSelectedIndex ?? -1;
      if (idx >= 0 && idx < items.length) {
        items[idx].click();
      } else {
        const val = el.openProjectInput.value;
        closeOpenProjectPanel();
        doOpenProject(val).catch((error) => setStatus(`Open folder failed: ${error.message || error}`));
      }
    } else if (event.key === "Escape") {
      event.preventDefault();
      closeOpenProjectPanel();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      const items = state._recentItems || [];
      if (items.length > 0) {
        const idx = (state._recentSelectedIndex ?? -1) + 1;
        state._recentSelectedIndex = idx >= items.length ? 0 : idx;
        items.forEach((r, i) => r.classList.toggle("active", i === state._recentSelectedIndex));
      }
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      const items = state._recentItems || [];
      if (items.length > 0) {
        const idx = (state._recentSelectedIndex ?? 0) - 1;
        state._recentSelectedIndex = idx < 0 ? items.length - 1 : idx;
        items.forEach((r, i) => r.classList.toggle("active", i === state._recentSelectedIndex));
      }
    }
  });

  for (const header of document.querySelectorAll(".ide-section-header")) {
    header.addEventListener("click", () => {
      const section = header.closest(".ide-section");
      if (!section) return;
      const collapsed = section.classList.toggle("collapsed");
      header.setAttribute("aria-expanded", String(!collapsed));
    });
  }

  if (typeof BroadcastChannel !== "undefined") {
    try {
      state.presenceChannel = new BroadcastChannel(IDE_PRESENCE_CHANNEL);
      state.presenceChannel.onmessage = (event) => {
        consumePresencePayload(event.data);
      };
    } catch {
      state.presenceChannel = null;
    }
  }

  el.commandInput.addEventListener("input", (event) => {
    state.commandFilter = event.target.value || "";
    state.selectedCommandIndex = 0;
    renderCommandList();
  });

  el.commandInput.addEventListener("keydown", (event) => {
    const filter = state.commandFilter.trim().toLowerCase();
    const commands = state.commands.filter((cmd) => {
      if (!filter) return true;
      return cmd.label.toLowerCase().includes(filter);
    });
    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (commands.length > 0) {
        state.selectedCommandIndex = (state.selectedCommandIndex + 1) % commands.length;
        renderCommandList();
      }
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      if (commands.length > 0) {
        state.selectedCommandIndex = (state.selectedCommandIndex - 1 + commands.length) % commands.length;
        renderCommandList();
      }
    } else if (event.key === "Enter") {
      event.preventDefault();
      runSelectedCommand().catch(() => {});
    } else if (event.key === "Escape") {
      event.preventDefault();
      closePalette();
    }
  });

  el.commandPalette.addEventListener("click", (event) => {
    if (event.target === el.commandPalette) {
      closePalette();
    }
  });

  document.addEventListener("click", (event) => {
    if (el.moreActionsBtn && el.moreActionsBtn.getAttribute("aria-expanded") === "true") {
      const target = event.target;
      if (!(target instanceof Node) || !el.headerMoreActions || !el.headerMoreActions.contains(target)) {
        closeMoreActionsMenu();
      }
    }
    if (!el.treeContextMenu.classList.contains("ide-hidden")) {
      const target = event.target;
      if (target instanceof Node && !el.treeContextMenu.contains(target)) {
        closeTreeContextMenu();
      }
    }
  });

  window.addEventListener("online", () => {
    state.online = true;
    updateConnectionBadge();
    setStatus("Connection restored. Flushing dirty drafts...");
    flushDirtyTabs().catch(() => {});
    flushFrontendTelemetry().catch(() => {});
  });

  window.addEventListener("offline", () => {
    state.online = false;
    updateConnectionBadge();
    updateSaveBadge("err", "offline draft");
    setStatus("Connection lost. Drafts are stored locally.");
  });

  window.addEventListener("storage", (event) => {
    if (event.key !== IDE_PRESENCE_STORAGE_KEY || !event.newValue) {
      return;
    }
    try {
      consumePresencePayload(JSON.parse(event.newValue));
    } catch {
      // no-op
    }
  });

  window.addEventListener("keydown", (event) => {
    const isMod = event.ctrlKey || event.metaKey;
    if (isMod && event.shiftKey && event.key.toLowerCase() === "p") {
      event.preventDefault();
      openCommandPalette();
      return;
    }
    if (isMod && event.shiftKey && event.key.toLowerCase() === "o") {
      event.preventDefault();
      if (typeof openConnectionDialog === "function") {
        openConnectionDialog();
      }
      return;
    }
    if (isMod && !event.shiftKey && event.altKey && event.key.toLowerCase() === "o") {
      event.preventDefault();
      fileSymbolSearchFlow().catch((error) => setStatus(`File symbols failed: ${error.message || error}`));
      return;
    }
    if (isMod && event.shiftKey && event.key.toLowerCase() === "f") {
      event.preventDefault();
      workspaceSearchFlow().catch((error) => setStatus(`Search failed: ${error.message || error}`));
      return;
    }
    if (isMod && !event.shiftKey && event.key.toLowerCase() === "p") {
      event.preventDefault();
      openQuickOpenPalette();
      return;
    }
    if (event.key === "F1") {
      event.preventDefault();
      openCommandPalette();
      return;
    }
    if (event.shiftKey && event.altKey && event.key.toLowerCase() === "f") {
      event.preventDefault();
      formatActiveDocument().catch((error) => setStatus(`Format failed: ${error.message || error}`));
      return;
    }
    if (isMod && !event.shiftKey && event.key.toLowerCase() === "s") {
      event.preventDefault();
      saveActiveTab({explicit: true}).catch(() => {});
      return;
    }
    if (isMod && event.code === "Space") {
      event.preventDefault();
      startCompletion();
      return;
    }
    if (isMod && event.key === "Tab") {
      event.preventDefault();
      if (event.shiftKey) {
        previousTab();
      } else {
        nextTab();
      }
      return;
    }
    if (event.key === "F12" && !event.shiftKey) {
      event.preventDefault();
      gotoDefinitionAtCursor().catch((error) => setStatus(`Definition failed: ${error.message || error}`));
      return;
    }
    if (event.key === "F12" && event.shiftKey) {
      event.preventDefault();
      findReferencesAtCursor().catch((error) => setStatus(`References failed: ${error.message || error}`));
      return;
    }
    if (event.key === "F2") {
      event.preventDefault();
      renameSymbolAtCursor().catch((error) => setStatus(`Rename failed: ${error.message || error}`));
    }
    if (event.key === "Escape" && el.openProjectPanel.classList.contains("open")) {
      closeOpenProjectPanel();
      return;
    }
    if (event.key === "Escape" && el.commandPalette.classList.contains("open")) {
      closePalette();
      return;
    }
    if (event.key === "Escape" && el.moreActionsBtn && el.moreActionsBtn.getAttribute("aria-expanded") === "true") {
      closeMoreActionsMenu();
      return;
    }
    if (event.key === "Escape" && !el.treeContextMenu.classList.contains("ide-hidden")) {
      closeTreeContextMenu();
    }
  });

  window.addEventListener("beforeunload", () => {
    flushFrontendTelemetry().catch(() => {});
    stopTaskPolling();
    disposeEditorDisposables();
    completionProviderDisposable?.dispose();
    hoverProviderDisposable?.dispose();
    if (cursorInsightTimer) {
      clearTimeout(cursorInsightTimer);
      cursorInsightTimer = null;
    }
    if (completionTriggerTimer) {
      clearTimeout(completionTriggerTimer);
      completionTriggerTimer = null;
    }
    if (cursorHoverPopupTimer) {
      clearHoverPopupTimer();
    }
    if (state.editorView) {
      state.editorView.dispose();
    }
    if (state.secondaryEditorView) {
      state.secondaryEditorView.dispose();
    }
    if (state.presenceChannel) {
      state.presenceChannel.close();
    }
  });
}

// ── Bootstrap ──────────────────────────────────────────

async function bootstrapUiMode() {
  try {
    const modePayload = await apiJson("/api/ui/mode", {
      method: "GET",
      timeoutMs: 3000,
    });
    state.uiMode = modePayload.mode || "runtime";
  } catch {
    state.uiMode = "runtime";
  }
  state.standaloneMode = state.uiMode === "standalone-ide";
  if (!state.standaloneMode) {
    return;
  }

  if (el.statusText) {
    el.statusText.textContent = "Standalone mode: runtime not connected";
  }
}

async function bootstrapSession() {
  const caps = await apiJson("/api/ide/capabilities");
  state.writeEnabled = caps.mode === "authoring";
  el.statusMode.textContent = state.writeEnabled ? "Authoring" : "Read-only";
  el.newFileBtn.disabled = !state.writeEnabled;
  el.newFolderBtn.disabled = !state.writeEnabled;
  el.renamePathBtn.disabled = !state.writeEnabled;
  el.deletePathBtn.disabled = !state.writeEnabled;
  el.saveBtn.disabled = !state.writeEnabled;
  el.saveAllBtn.disabled = !state.writeEnabled;
  el.validateBtn.disabled = !state.writeEnabled;
  el.buildBtn.disabled = !state.writeEnabled;
  el.testBtn.disabled = !state.writeEnabled;

  const role = state.writeEnabled ? "editor" : "viewer";
  let session = null;
  const stored = loadStoredIdeSession(role);
  if (stored && stored.token) {
    state.sessionToken = stored.token;
    try {
      await apiJson("/api/ide/project", {
        method: "GET",
        headers: apiHeaders(),
        timeoutMs: 3000,
        allowSessionRetry: false,
      });
      session = { token: stored.token, role: stored.role };
    } catch {
      state.sessionToken = null;
      clearStoredIdeSession();
    }
  }
  if (!session) {
    session = await requestNewSession(role);
  }
  state.sessionToken = session.token;
  persistIdeSession(state.sessionToken, session.role || role);
  setStatus(`Session ${session.role} active. ${state.writeEnabled ? "Autosave enabled." : "Read-only mode."}`);
  await refreshProjectSelection();
  document.dispatchEvent(new CustomEvent("ide-session-ready", {
    detail: {
      token: state.sessionToken,
      activeProject: state.activeProject,
      startupProject: state.startupProject,
    },
  }));
}

async function bootstrap() {
  updateConnectionBadge();
  applyWorkbenchSizing();
  const storedTheme = localStorage.getItem(THEME_STORAGE_KEY);
  if (!storedTheme) {
    const preferred = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
    applyTheme(preferred);
  } else {
    applyTheme(storedTheme);
  }

  bindGlobalEvents();
  try {
    await bootstrapUiMode();
    const modulesLoaded = await loadEditorModules();
    if (!modulesLoaded) {
      bumpTelemetry("bootstrap_failures");
      flushFrontendTelemetry().catch(() => {});
      return;
    }
    await bootstrapSession();
    await loadPresenceModel();
    await bootstrapFiles();
    await initWasmAnalysis();
    syncDocumentsToWasm();
    await pollHealth();
    scheduleHealthPoll();
    scheduleTelemetryFlush();
    renderReferences([]);
    renderSearchHits([]);
    renderTaskOutput(null);
    setRetryAction(null, null);
    el.splitBtn.title = "Split";
    if (typeof onlineState === "object" && onlineState && onlineState.connected) {
      setStatus("IDE ready.");
    } else {
      setStatus("No runtime connected");
    }
    updateSaveBadge("ok", state.writeEnabled ? "saved" : "read-only");
    state.ready = true;
  } catch (error) {
    bumpTelemetry("bootstrap_failures");
    const reason = String(error?.message || error);
    if (reason.toLowerCase().includes("too many active ide sessions")) {
      setStatus("IDE bootstrap failed: session limit reached. Close inactive tabs or restart runtime.");
    } else {
      setStatus(`IDE bootstrap failed: ${reason}`);
    }
    updateSaveBadge("err", "error");
    flushFrontendTelemetry().catch(() => {});
  }
}

bootstrap();
