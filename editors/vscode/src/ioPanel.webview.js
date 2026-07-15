const vscode =
  typeof acquireVsCodeApi === "function"
    ? acquireVsCodeApi()
    : { postMessage: () => {} };
const sections = document.getElementById("sections");
const status = document.getElementById("status");
const filterInput = document.getElementById("filter");
const forcedFilterBtn = document.getElementById("forcedFilter");
const diagnosticsSummary = document.getElementById("diagnosticsSummary");
const diagnosticsRuntime = document.getElementById("diagnosticsRuntime");
const diagnosticsList = document.getElementById("diagnosticsList");
const diagnosticsPanel = document.getElementById("diagnostics");
const runtimeView = document.getElementById("runtimeView");
const settingsPanel = document.getElementById("settingsPanel");
const settingsSave = document.getElementById("settingsSave");
const settingsCancel = document.getElementById("settingsCancel");
const runtimeStatusText = document.getElementById("runtimeStatusText");
const targetLabel = document.getElementById("targetLabel");
const scanLabel = document.getElementById("scanLabel");
const forcePolicy = document.getElementById("forcePolicy");
const runtimeStart = document.getElementById("runtimeStart");
const modeSimulate = document.getElementById("modeSimulate");
const modeOnline = document.getElementById("modeOnline");
const releaseAllForcesBtn = document.getElementById("releaseAllForces");
const settingsFields = {
  serverPath: document.getElementById("serverPath"),
  traceServer: document.getElementById("traceServer"),
  debugAdapterPath: document.getElementById("debugAdapterPath"),
  debugAdapterArgs: document.getElementById("debugAdapterArgs"),
  debugAdapterEnv: document.getElementById("debugAdapterEnv"),
  runtimeControlEndpoint: document.getElementById("runtimeControlEndpoint"),
  runtimeControlAuthToken: document.getElementById(
    "runtimeControlAuthToken"
  ),
  runtimeInlineValuesEnabled: document.getElementById(
    "runtimeInlineValuesEnabled"
  ),
  runtimeIncludeGlobs: document.getElementById("runtimeIncludeGlobs"),
  runtimeExcludeGlobs: document.getElementById("runtimeExcludeGlobs"),
  runtimeIgnorePragmas: document.getElementById("runtimeIgnorePragmas"),
};
let currentState = { inputs: [], outputs: [], memory: [], ads: [] };
let compileState = null;
let currentFilter = "";
let forcedOnly = false;
const editCache = new Map();
const pendingBooleanForces = new Map();
let settingsOpen = false;
// Current target kind (simulate/online). Force/Unforce work on both now (the adapter forwards
// io.force/io.unforce via attach); kept only so a target flip re-renders the rows.
let currentMode = "simulate";
let currentRuntimeState = "stopped";
let currentTargetKey = "simulate|stopped|";
let forceArmed = false;
let statusClearTimer = undefined;
let numericDisplayBase = "dec";
let actionInteractionActive = false;
let actionInteractionRenderPending = false;
let actionInteractionReleaseTimer = undefined;
let hoveredRowKey = "";
const TRANSIENT_STATUS_CLEAR_MS = 5000;
let currentAccess = {
  allowWrite: true,
  allowForce: true,
  allowRelease: true,
  reason: "",
};

function forcedAddresses(state) {
  const all = [
    ...(state.inputs || []),
    ...(state.outputs || []),
    ...(state.memory || []),
    ...(state.ads || []),
  ];
  return all
    .filter((entry) => entry && entry.forced && entry.address)
    .map((entry) => entry.address);
}

function ioEntries(state) {
  return [
    ...(state.inputs || []),
    ...(state.outputs || []),
    ...(state.memory || []),
    ...(state.ads || []),
  ];
}

function clearPendingBooleanForces() {
  let cleared = false;
  for (const entry of ioEntries(currentState)) {
    const address = String(entry.address || "");
    if (!pendingBooleanForces.has(address)) {
      continue;
    }
    pendingBooleanForces.delete(address);
    editCache.delete([entry.name || "", entry.address || ""].join("|"));
    cleared = true;
  }
  if (pendingBooleanForces.size > 0) {
    pendingBooleanForces.clear();
    cleared = true;
  }
  return cleared;
}

function reconcilePendingBooleanForces(state) {
  for (const entry of ioEntries(state)) {
    const address = String(entry.address || "");
    const pending = pendingBooleanForces.get(address);
    if (pending === undefined || !entry.forced) {
      continue;
    }
    const actual = parseBooleanValue(entry.value);
    const expected = parseBooleanValue(pending);
    if (actual === undefined || actual !== expected) {
      continue;
    }
    pendingBooleanForces.delete(address);
    editCache.delete([entry.name || "", entry.address || ""].join("|"));
  }
}

function updateForcedFilter(state) {
  if (!forcedFilterBtn) {
    return;
  }
  const count = forcedAddresses(state).length;
  if (count === 0) {
    forcedOnly = false;
  }
  forcedFilterBtn.style.display = count > 0 ? "" : "none";
  forcedFilterBtn.textContent = count > 0 ? "Forced (" + count + ")" : "Forced";
  forcedFilterBtn.disabled = count === 0;
  forcedFilterBtn.classList.toggle("active", forcedOnly && count > 0);
  forcedFilterBtn.setAttribute("aria-pressed", forcedOnly && count > 0 ? "true" : "false");
  forcedFilterBtn.title =
    count > 0
      ? forcedOnly
        ? "Showing only forced values"
        : "Show only forced values"
      : "No forced values";
}

function updateReleaseAll(state) {
  if (!releaseAllForcesBtn) {
    return;
  }
  const count = forcedAddresses(state).length;
  releaseAllForcesBtn.style.display = count > 0 ? "" : "none";
  releaseAllForcesBtn.textContent =
    count > 0 ? "Release all forces (" + count + ")" : "Release all forces";
  releaseAllForcesBtn.disabled = count === 0 || !currentAccess.allowRelease;
  releaseAllForcesBtn.title =
    !currentAccess.allowRelease && currentAccess.reason
      ? currentAccess.reason
      : "Release every forced value on this target";
}

function updateForceStatusFromState(state) {
  if (!status) {
    return;
  }
  const addresses = forcedAddresses(state);
  const current = status.textContent || "";
  const preserveStatus = isErrorStatusText(current) || isPermissionGuidanceText(current);
  if (addresses.length > 0 && !preserveStatus) {
    setStatusText(
      addresses.length === 1
        ? "I/O force active at " + addresses[0] + "."
        : addresses.length + " I/O forces active."
    );
    return;
  }
  if (addresses.length === 0 && /I\/O forces? active|force active/i.test(current)) {
    setStatusText("");
  }
}

if (releaseAllForcesBtn) {
  releaseAllForcesBtn.addEventListener("click", () => {
    const addresses = forcedAddresses(currentState);
    if (addresses.length) {
      vscode.postMessage({ type: "releaseAllForces", addresses });
    }
  });
}

if (forcedFilterBtn) {
  forcedFilterBtn.addEventListener("click", () => {
    if (forcedFilterBtn.disabled) {
      return;
    }
    forcedOnly = !forcedOnly;
    render(currentState);
  });
}

function setNumericDisplayBase(next) {
  const normalized = ["dec", "hex", "bin"].includes(next) ? next : "dec";
  numericDisplayBase = normalized;
  document.querySelectorAll("[data-numeric-format]").forEach((button) => {
    const active = button.dataset.numericFormat === normalized;
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", active ? "true" : "false");
  });
  render(currentState);
}

document.addEventListener("click", (event) => {
  const target = event.target;
  const button =
    target && typeof target.closest === "function"
      ? target.closest("[data-numeric-format]")
      : undefined;
  if (!button) {
    return;
  }
  setNumericDisplayBase(button.dataset.numericFormat || "dec");
});

function isActionInteractionTarget(target) {
  return !!(
    target &&
    typeof target.closest === "function" &&
    target.closest(".actions")
  );
}

function beginActionInteraction(event) {
  if (!isActionInteractionTarget(event && event.target)) {
    return;
  }
  if (typeof event.button === "number" && event.button !== 0) {
    return;
  }
  if (actionInteractionReleaseTimer !== undefined) {
    window.clearTimeout(actionInteractionReleaseTimer);
    actionInteractionReleaseTimer = undefined;
  }
  actionInteractionActive = true;
}

function finishActionInteraction() {
  if (!actionInteractionActive) {
    return;
  }
  if (actionInteractionReleaseTimer !== undefined) {
    window.clearTimeout(actionInteractionReleaseTimer);
  }
  // pointerup/keyup happens immediately before the browser's click event. Flush on the next
  // task so the clicked control remains mounted long enough to deliver that click.
  actionInteractionReleaseTimer = window.setTimeout(() => {
    actionInteractionReleaseTimer = undefined;
    actionInteractionActive = false;
    if (actionInteractionRenderPending) {
      actionInteractionRenderPending = false;
      render(currentState);
      updateForceStatusFromState(currentState);
    }
  }, 0);
}

document.addEventListener("pointerdown", beginActionInteraction);
document.addEventListener("pointerup", finishActionInteraction);
document.addEventListener("pointercancel", finishActionInteraction);
document.addEventListener("keydown", (event) => {
  if (event.key === "Enter" || event.key === " ") {
    beginActionInteraction(event);
  }
});
document.addEventListener("keyup", (event) => {
  if (event.key === "Enter" || event.key === " ") {
    finishActionInteraction();
  }
});

function rowForPointerTarget(target) {
  return target && typeof target.closest === "function"
    ? target.closest(".row")
    : null;
}

if (sections) {
  sections.addEventListener("pointerover", (event) => {
    const row = rowForPointerTarget(event.target);
    if (!row || (event.relatedTarget && row.contains(event.relatedTarget))) {
      return;
    }
    const previous = sections.querySelector(".row.pointer-hover");
    if (previous && previous !== row) {
      previous.classList.remove("pointer-hover");
    }
    hoveredRowKey = row.dataset.rowKey || "";
    row.classList.toggle("pointer-hover", !!hoveredRowKey);
  });

  sections.addEventListener("pointerout", (event) => {
    const row = rowForPointerTarget(event.target);
    if (!row || (event.relatedTarget && row.contains(event.relatedTarget))) {
      return;
    }
    row.classList.remove("pointer-hover");
    if (row.dataset.rowKey === hoveredRowKey) {
      hoveredRowKey = "";
    }
  });
}

function setStatusText(message) {
  if (status) {
    clearStatusClearTimer();
    const text = String(message || "");
    const isWarning = isWarningStatusText(text);
    const isError = isErrorStatusText(text) && !isWarning;
    status.textContent = text;
    status.title = text;
    status.classList.toggle("status-error", isError);
    status.classList.toggle("status-warn", isWarning);
    status.classList.toggle(
      "status-ok",
      !isWarning && /queued|released|cleared/i.test(text)
    );
    if (isAutoExpiringStatusText(text)) {
      statusClearTimer = window.setTimeout(() => {
        if (status && status.textContent === text) {
          setStatusText("");
        }
      }, TRANSIENT_STATUS_CLEAR_MS);
    }
  }
}

function isPermissionGuidanceText(message) {
  const text = String(message || "");
  return (
    /viewer role|operator role|engineer token|permissions are unknown/i.test(text) ||
    (/forbidden|requires role|denied/i.test(text) && !/failed|error/i.test(text))
  );
}

function isWarningStatusText(message) {
  const text = String(message || "");
  return /force armed|force active|force remains armed/i.test(text) || isPermissionGuidanceText(text);
}

function isErrorStatusText(message) {
  return /failed|forbidden|requires role|missing|error|denied/i.test(String(message || ""));
}

function clearStatusClearTimer() {
  if (statusClearTimer !== undefined) {
    window.clearTimeout(statusClearTimer);
    statusClearTimer = undefined;
  }
}

function updateScanLabel(state) {
  if (!scanLabel) {
    return;
  }
  const scan = state && Number.isFinite(state.scan) ? state.scan : undefined;
  scanLabel.textContent = scan === undefined ? "scan --" : "scan #" + scan;
  scanLabel.title =
    scan === undefined
      ? "No runtime scan has been received yet"
      : "Rows are from runtime scan #" + scan;
}

function isTransientStatusText(message) {
  return (
    /^Live Values (loading|ready)\.?$/i.test(message) ||
    /^Start the runtime to see live values\.?$/i.test(message) ||
    /^Connect to the selected runtime to see live values\.?$/i.test(message)
  );
}

function isAutoExpiringStatusText(message) {
  return (
    /^Live Values ready\.?$/i.test(message) ||
    /^I\/O write queued for .+\.?$/i.test(message) ||
    /^I\/O force released at .+\.?$/i.test(message) ||
    /^Released \d+ forces?\.?$/i.test(message) ||
    /^No forces to release\.?$/i.test(message)
  );
}

function forceRequiresArming() {
  return currentMode !== "simulate" || currentRuntimeState === "connected";
}

function updateForcePolicy() {
  if (!forcePolicy) {
    return;
  }
  const requiresArm = forceRequiresArming();
  forcePolicy.textContent = requiresArm
    ? "Force policy: this target requires Arm force first; simulator pins immediately."
    : "Force policy: simulator pins immediately; managed/remote targets require Arm force first.";
  forcePolicy.classList.toggle("armed-target", requiresArm);
}

function resetForceArming() {
  forceArmed = false;
}

function normalizeAccess(access, mode, runtimeState) {
  if (access && typeof access === "object") {
    return {
      allowWrite: access.allowWrite === true,
      allowForce: access.allowForce === true,
      allowRelease: access.allowRelease === true,
      reason:
        typeof access.reason === "string" && access.reason.trim()
          ? access.reason.trim()
          : "",
    };
  }
  if (mode === "simulate") {
    return {
      allowWrite: true,
      allowForce: true,
      allowRelease: true,
      reason: "",
    };
  }
  const active = runtimeState === "connected" || runtimeState === "running";
  return {
    allowWrite: false,
    allowForce: false,
    allowRelease: false,
    reason: active
      ? "Write/force permissions are unknown — reconnect with an engineer token."
      : "Connect with an engineer token to write or force.",
  };
}

function accessKey(access) {
  return [
    access.allowWrite ? "w1" : "w0",
    access.allowForce ? "f1" : "f0",
    access.allowRelease ? "r1" : "r0",
    access.reason || "",
  ].join("|");
}

function armForceForTarget() {
  forceArmed = true;
  setStatusText("Force armed for this target. Click Force again to pin a value.");
  render(currentState);
}

function reportWebviewError(message, stack) {
  setStatusText("Live Values error: " + message);
  vscode.postMessage({
    type: "webviewError",
    message,
    stack,
  });
}

window.addEventListener("error", (event) => {
  const message =
    event && typeof event.message === "string"
      ? event.message
      : "Unknown error";
  const stack =
    event && event.error && event.error.stack ? event.error.stack : "";
  reportWebviewError(message, stack);
});

window.addEventListener("unhandledrejection", (event) => {
  const reason = event && event.reason ? event.reason : "Unknown error";
  const message =
    reason && typeof reason.message === "string"
      ? reason.message
      : String(reason);
  const stack = reason && reason.stack ? reason.stack : "";
  reportWebviewError(message, stack);
});

if (runtimeStart) {
  runtimeStart.addEventListener("click", () => {
    vscode.postMessage({ type: "runtimeStart" });
  });
}
if (modeSimulate) {
  modeSimulate.addEventListener("click", () => {
    vscode.postMessage({ type: "runtimeSetMode", mode: "simulate" });
  });
}
if (modeOnline) {
  modeOnline.addEventListener("click", () => {
    vscode.postMessage({ type: "runtimeSetMode", mode: "online" });
  });
}
const settingsButton = document.getElementById("settings");
if (settingsButton) {
  settingsButton.addEventListener("click", () => {
    setSettingsOpen(!settingsOpen);
  });
}
if (settingsSave) {
  settingsSave.addEventListener("click", () => {
    const payload = collectSettingsPayload();
    if (!payload) {
      return;
    }
    vscode.postMessage({ type: "saveSettings", payload });
  });
}
if (settingsCancel) {
  settingsCancel.addEventListener("click", () => {
    setSettingsOpen(false);
  });
}

if (filterInput) {
  filterInput.addEventListener("input", () => {
    currentFilter = filterInput.value;
    render(currentState);
  });
}
setStatusText("Live Values ready.");
vscode.postMessage({ type: "webviewReady" });

function setSettingsOpen(open) {
  settingsOpen = open;
  if (settingsPanel) {
    settingsPanel.classList.toggle("open", open);
  }
  if (runtimeView) {
    runtimeView.classList.toggle("hidden", open);
  }
  if (filterInput) {
    filterInput.disabled = open;
  }
  if (open) {
    vscode.postMessage({ type: "requestSettings" });
  }
}

function getFieldValue(element) {
  if (!element || typeof element.value !== "string") {
    return "";
  }
  return element.value;
}

function setFieldValue(element, value) {
  if (!element || typeof element.value !== "string") {
    return;
  }
  element.value = value == null ? "" : value;
}

function arrayToText(values) {
  if (!Array.isArray(values)) {
    return "";
  }
  return values.join("\n");
}

function textToArray(value) {
  return String(value || "")
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function envToText(env) {
  if (!env || typeof env !== "object") {
    return "";
  }
  return Object.entries(env)
    .map(([key, value]) => key + "=" + (value == null ? "" : value))
    .join("\n");
}

function parseEnv(text) {
  const trimmed = String(text || "").trim();
  if (!trimmed) {
    return {};
  }
  try {
    const parsed = JSON.parse(trimmed);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const env = {};
      Object.entries(parsed).forEach(([key, value]) => {
        env[key] = value === undefined ? "" : String(value);
      });
      return env;
    }
  } catch (err) {
    // Fallback to KEY=VALUE lines.
  }
  const env = {};
  const lines = trimmed.split(/\r?\n/);
  for (const line of lines) {
    if (!line.trim()) {
      continue;
    }
    const eq = line.indexOf("=");
    if (eq <= 0) {
      throw new Error(
        "Env entries must be KEY=VALUE per line or a JSON object."
      );
    }
    const key = line.slice(0, eq).trim();
    const value = line.slice(eq + 1).trim();
    if (!key) {
      throw new Error("Env entries must include a key.");
    }
    env[key] = value;
  }
  return env;
}

function collectSettingsPayload() {
  let debugAdapterEnv = {};
  try {
    debugAdapterEnv = parseEnv(getFieldValue(settingsFields.debugAdapterEnv));
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    setStatusText(message);
    return null;
  }
  return {
    serverPath: getFieldValue(settingsFields.serverPath).trim(),
    traceServer: getFieldValue(settingsFields.traceServer).trim() || "off",
    debugAdapterPath: getFieldValue(settingsFields.debugAdapterPath).trim(),
    debugAdapterArgs: textToArray(getFieldValue(settingsFields.debugAdapterArgs)),
    debugAdapterEnv,
    runtimeControlEndpoint: getFieldValue(
      settingsFields.runtimeControlEndpoint
    ).trim(),
    runtimeControlAuthToken: getFieldValue(
      settingsFields.runtimeControlAuthToken
    ),
    runtimeInlineValuesEnabled: !!settingsFields.runtimeInlineValuesEnabled?.checked,
    runtimeIncludeGlobs: textToArray(
      getFieldValue(settingsFields.runtimeIncludeGlobs)
    ),
    runtimeExcludeGlobs: textToArray(
      getFieldValue(settingsFields.runtimeExcludeGlobs)
    ),
    runtimeIgnorePragmas: textToArray(
      getFieldValue(settingsFields.runtimeIgnorePragmas)
    ),
  };
}

function shortEndpointLabel(endpoint) {
  const text = String(endpoint || "").trim();
  if (!text) {
    return "";
  }
  if (text.startsWith("tcp://")) {
    try {
      const url = new URL(text);
      return url.host || text;
    } catch (err) {
      return text;
    }
  }
  if (text.startsWith("unix://")) {
    return "local control socket";
  }
  return text;
}

function targetLabelForStatus(payload) {
  const label = payload && typeof payload.targetLabel === "string" ? payload.targetLabel.trim() : "";
  if (label) {
    return label;
  }
  const mode = payload && payload.runtimeMode ? payload.runtimeMode : "simulate";
  const runtimeState = payload && payload.runtimeState ? payload.runtimeState : "";
  if (runtimeState === "connected") {
    const endpoint = shortEndpointLabel(payload && payload.endpoint);
    if (endpoint === "local control socket") {
      return "Local runtime (control socket)";
    }
    return endpoint ? "Runtime at " + endpoint : "Connected runtime";
  }
  if (mode === "simulate") {
    return "Simulator";
  }
  const endpoint = shortEndpointLabel(payload && payload.endpoint);
  if (endpoint === "local control socket") {
    return "Local runtime (control socket)";
  }
  return endpoint ? "Runtime at " + endpoint : "Runtime endpoint";
}

function applyRuntimeStatus(payload) {
  if (!payload) {
    return;
  }
  const running = !!payload.running;
  const runtimeState = payload.runtimeState || (running ? "running" : "stopped");
  const connected = runtimeState === "connected";
  const mode = payload.runtimeMode || "simulate";
  const modeChanged = mode !== currentMode;
  const nextTargetKey = [mode, runtimeState, payload.endpoint || ""].join("|");
  const targetChanged = nextTargetKey !== currentTargetKey;
  const nextAccess = normalizeAccess(payload.access, mode, runtimeState);
  const accessChanged = accessKey(nextAccess) !== accessKey(currentAccess);
  currentMode = mode;
  currentRuntimeState = runtimeState;
  currentTargetKey = nextTargetKey;
  currentAccess = nextAccess;
  if (targetChanged || (!running && !connected)) {
    resetForceArming();
  }

  if (modeSimulate) {
    modeSimulate.classList.toggle("active", mode === "simulate");
    modeSimulate.disabled = running || connected;
  }
  if (modeOnline) {
    modeOnline.classList.toggle("active", mode === "online");
    modeOnline.disabled = running || connected;
  }
  // Re-render the rows when the target changes (keeps safety affordances in sync with the target).
  if (modeChanged || targetChanged || accessChanged) {
    render(currentState);
  }
  if (currentAccess.reason && (running || connected)) {
    setStatusText(currentAccess.reason);
  }

  if (runtimeStart) {
    let label = "Start";
    if (runtimeState === "connected") {
      label = "Disconnect";
    } else if (running) {
      label = "Stop";
    }
    runtimeStart.textContent = label;
    runtimeStart.disabled = false;
  }


  if (runtimeStatusText) {
    const isRunning = runtimeState === "running" || runtimeState === "connected";
    const label =
      runtimeState === "connected"
        ? "Connected"
        : runtimeState === "running"
          ? "Running"
          : payload.runtimeMode === "online"
            ? "Not connected"
            : "Stopped";
    runtimeStatusText.textContent = label;
    runtimeStatusText.classList.toggle("running", isRunning);
    runtimeStatusText.classList.toggle("connected", runtimeState === "connected");
    runtimeStatusText.classList.toggle("disconnected", !isRunning);
    runtimeStatusText.title = payload.endpoint || label;
  }
  if (targetLabel) {
    const label = targetLabelForStatus(payload);
    targetLabel.textContent = label;
    targetLabel.title = payload.endpoint || label;
  }
  updateForcePolicy();
}

function clearUnavailableRuntimeStatus(message) {
  if (/Start the runtime to see live values/i.test(message)) {
    applyRuntimeStatus({
      running: false,
      runtimeMode: "simulate",
      runtimeState: "stopped",
      endpoint: "",
      endpointConfigured: false,
      endpointEnabled: true,
      endpointReachable: false
    });
    return;
  }
  if (/Connect to the selected runtime to see live values/i.test(message)) {
    applyRuntimeStatus({
      running: false,
      runtimeMode: "online",
      runtimeState: "stopped",
      endpoint: "",
      endpointConfigured: false,
      endpointEnabled: true,
      endpointReachable: false
    });
  }
}

function applySettingsPayload(payload) {
  if (!payload) {
    return;
  }
  setFieldValue(settingsFields.serverPath, payload.serverPath || "");
  setFieldValue(settingsFields.traceServer, payload.traceServer || "off");
  setFieldValue(settingsFields.debugAdapterPath, payload.debugAdapterPath || "");
  setFieldValue(
    settingsFields.debugAdapterArgs,
    arrayToText(payload.debugAdapterArgs)
  );
  setFieldValue(
    settingsFields.debugAdapterEnv,
    envToText(payload.debugAdapterEnv)
  );
  setFieldValue(
    settingsFields.runtimeControlEndpoint,
    payload.runtimeControlEndpoint || ""
  );
  setFieldValue(
    settingsFields.runtimeControlAuthToken,
    payload.runtimeControlAuthToken || ""
  );
  if (settingsFields.runtimeInlineValuesEnabled) {
    settingsFields.runtimeInlineValuesEnabled.checked =
      payload.runtimeInlineValuesEnabled !== false;
  }
  setFieldValue(
    settingsFields.runtimeIncludeGlobs,
    arrayToText(payload.runtimeIncludeGlobs)
  );
  setFieldValue(
    settingsFields.runtimeExcludeGlobs,
    arrayToText(payload.runtimeExcludeGlobs)
  );
  setFieldValue(
    settingsFields.runtimeIgnorePragmas,
    arrayToText(payload.runtimeIgnorePragmas)
  );
}

function fileLabel(path) {
  if (!path) {
    return "";
  }
  const segments = String(path).split(/[/\\\\]/);
  return segments[segments.length - 1] || path;
}

function renderDiagnostics() {
  if (!diagnosticsSummary || !diagnosticsList || !diagnosticsRuntime) {
    return;
  }
  diagnosticsList.innerHTML = "";
  if (!compileState) {
    if (diagnosticsPanel) {
      diagnosticsPanel.style.display = "none";
    }
    diagnosticsSummary.textContent = "";
    diagnosticsRuntime.textContent = "";
    return;
  }
  if (diagnosticsPanel) {
    diagnosticsPanel.style.display = "";
  }

  const targetLabel = compileState.target ? fileLabel(compileState.target) : "";
  const dirtyLabel = compileState.dirty ? " (unsaved)" : "";
  diagnosticsSummary.textContent =
    (targetLabel || "Unknown target") +
    dirtyLabel +
    " • " +
    compileState.errors +
    " error(s), " +
    compileState.warnings +
    " warning(s)";

  diagnosticsRuntime.textContent =
    compileState.runtimeStatus !== "skipped" && compileState.runtimeMessage
      ? compileState.runtimeMessage
      : "";

  const issues = Array.isArray(compileState.issues)
    ? compileState.issues
    : [];
  if (issues.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "No warnings or errors.";
    diagnosticsList.appendChild(empty);
    return;
  }

  issues.forEach((issue) => {
    const item = document.createElement("div");
    item.className =
      "diagnostic-item " + (issue.severity === "error" ? "error" : "warning");

    const message = document.createElement("div");
    message.className = "diagnostic-message";
    message.textContent = issue.message || "";
    item.appendChild(message);

    const meta = document.createElement("div");
    meta.className = "diagnostic-meta";
    const location = document.createElement("span");
    const locationParts = [];
    if (issue.file) {
      locationParts.push(fileLabel(issue.file));
    }
    if (issue.line) {
      locationParts.push("L" + issue.line);
    }
    if (issue.column) {
      locationParts.push("C" + issue.column);
    }
    location.textContent = locationParts.join(": ");
    meta.appendChild(location);

    if (issue.code) {
      const code = document.createElement("span");
      code.textContent = String(issue.code);
      meta.appendChild(code);
    }

    if (issue.source) {
      const source = document.createElement("span");
      source.textContent = String(issue.source);
      meta.appendChild(source);
    }

    item.appendChild(meta);
    diagnosticsList.appendChild(item);
  });
}

function applyFilter(entries) {
  const filter = currentFilter.trim().toLowerCase();
  return (entries || []).filter((entry) => {
    if (forcedOnly && !entry.forced) {
      return false;
    }
    if (!filter) {
      return true;
    }
    const haystack = [entry.name || "", entry.address || "", entry.source || ""]
      .join(" ")
      .toLowerCase();
    return haystack.includes(filter);
  });
}

function parseBooleanValue(value) {
  const trimmed = String(value || "").trim();
  if (!trimmed) {
    return undefined;
  }
  const normalized = trimmed.toUpperCase();
  const maybeWrapped =
    normalized.startsWith("BOOL(") && normalized.endsWith(")")
      ? normalized.slice(5, -1).trim()
      : normalized;
  if (maybeWrapped === "TRUE" || maybeWrapped === "1") {
    return true;
  }
  if (maybeWrapped === "FALSE" || maybeWrapped === "0") {
    return false;
  }
  return undefined;
}

function defaultNumericValue(value) {
  const trimmed = String(value || "").trim();
  if (!trimmed) {
    return "";
  }
  const numericLiteral = /^(?:0x[0-9a-fA-F_]+|[0-9][0-9_]*|[28]#[0-9A-Fa-f_]+|16#[0-9A-Fa-f_]+)$/;
  if (numericLiteral.test(trimmed)) {
    return trimmed;
  }
  const match = trimmed.match(/\((-?\d+)\)/);
  if (match) {
    return match[1];
  }
  return "";
}

function defaultWriteValue(entry, display) {
  const addressType = typeFromAddress(entry.address);
  const resolvedType = String(display.type || addressType || "").toUpperCase();
  const isBool =
    resolvedType === "BOOL" ||
    /^%[IQM]X/i.test(String(entry.address || "").trim());
  if (isBool) {
    // BOOL: pre-fill with the current TRUE/FALSE so it matches the value badge.
    const booleanValue = parseBooleanValue(entry.value || display.value);
    return booleanValue ? "TRUE" : "FALSE";
  }
  // Numeric (BYTE/WORD/DWORD/INT/REAL): never offer a boolean literal as the default; pre-fill with
  // the current numeric value so the write box is type-consistent with the row.
  const numericValue = defaultNumericValue(display.value || entry.value);
  return numericValue || String(display.value || "").trim();
}

function splitDisplayValue(value) {
  const text = String(value == null ? "" : value);
  const match = text.match(/^([A-Za-z_][A-Za-z0-9_]*)\((.*)\)$/);
  if (!match) {
    return { value: text, type: "" };
  }
  return { value: match[2], type: match[1].toUpperCase() };
}

function integerBitsForType(type) {
  switch (String(type || "").toUpperCase()) {
    case "BYTE":
    case "USINT":
      return 8;
    case "WORD":
    case "UINT":
      return 16;
    case "DWORD":
    case "UDINT":
      return 32;
    case "LWORD":
    case "ULINT":
      return 64;
    default:
      return 0;
  }
}

function parseUnsignedIntegerValue(value) {
  const text = String(value == null ? "" : value).trim().replace(/_/g, "");
  if (!text) {
    return undefined;
  }
  try {
    if (/^0x[0-9a-f]+$/i.test(text)) {
      return BigInt(text);
    }
    if (/^16#[0-9a-f]+$/i.test(text)) {
      return BigInt("0x" + text.slice(3));
    }
    if (/^2#[01]+$/i.test(text)) {
      return BigInt("0b" + text.slice(2));
    }
    if (/^[0-9]+$/.test(text)) {
      return BigInt(text);
    }
  } catch (err) {
    return undefined;
  }
  return undefined;
}

function formatIntegerForBase(value, bits) {
  if (numericDisplayBase === "dec" || bits <= 0) {
    return undefined;
  }
  const numeric = parseUnsignedIntegerValue(value);
  if (numeric === undefined) {
    return undefined;
  }
  const max = 1n << BigInt(bits);
  const normalized = numeric < 0n ? max + numeric : numeric;
  if (normalized < 0n || normalized >= max) {
    return undefined;
  }
  if (numericDisplayBase === "hex") {
    const width = Math.ceil(bits / 4);
    return "16#" + normalized.toString(16).toUpperCase().padStart(width, "0");
  }
  if (numericDisplayBase === "bin") {
    return "2#" + normalized.toString(2).padStart(bits, "0");
  }
  return undefined;
}

function displayValueForEntry(display, displayType) {
  const value = display.value || "";
  const formatted = formatIntegerForBase(value, integerBitsForType(displayType));
  return formatted || value;
}

function typeFromAddress(address) {
  const match = String(address || "").match(/^%[IQM]([XBWDL])/i);
  if (!match) {
    return "";
  }
  switch (match[1].toUpperCase()) {
    case "X":
      return "BOOL";
    case "B":
      return "BYTE";
    case "W":
      return "WORD";
    case "D":
      return "DWORD";
    case "L":
      return "LWORD";
    default:
      return "";
  }
}

function displayTypeForEntry(entry, display) {
  const explicitType = String(
    (entry && (entry.valueType || entry.value_type || entry.type)) || ""
  ).trim();
  return explicitType.toUpperCase() || display.type || typeFromAddress(entry && entry.address);
}

function createNode(title, level, content, open = true) {
  const details = document.createElement("details");
  details.className = "tree-node level-" + level;
  details.open = open;
  const summary = document.createElement("summary");
  summary.textContent = title;
  details.appendChild(summary);
  details.appendChild(content);
  return details;
}

function hasForcedEntry(entries) {
  return (entries || []).some((entry) => entry && entry.forced);
}

function appendIoSection(parent, title, entries, options) {
  if (forcedOnly && !hasForcedEntry(entries)) {
    return;
  }
  parent.appendChild(createNode(title, 2, renderRows(entries, options), true));
}

function renderRows(entries, options = {}) {
  const {
    allowActions = false,
    showAddress = false,
    allowWrite = true,
    allowForce = true,
    allowRelease = true,
    remoteReason = "",
    writeDisabledReason = "",
  } = options;
  const wrapper = document.createElement("div");
  wrapper.className = "rows";

  const filtered = applyFilter(entries);
  if (filtered.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    let message = "No entries";
    if (forcedOnly) {
      message =
        entries && entries.some((entry) => entry && entry.forced)
          ? "No forced matches"
          : "No forced values";
    } else if (entries && entries.length > 0 && currentFilter.trim()) {
      message = "No matches";
    }
    empty.textContent = message;
    wrapper.appendChild(empty);
    return wrapper;
  }

  if (options.allowActions) {
    const header = document.createElement("div");
    header.className = "row-header";
    const signal = document.createElement("div");
    signal.textContent = "Name";
    const value = document.createElement("div");
    value.textContent = "Value";
    const type = document.createElement("div");
    type.textContent = "Type";
    const state = document.createElement("div");
    state.textContent = "State";
    const actions = document.createElement("div");
    actions.className = "actions-heading";
    actions.textContent = "Actions";
    header.appendChild(signal);
    header.appendChild(value);
    header.appendChild(type);
    header.appendChild(state);
    header.appendChild(actions);
    wrapper.appendChild(header);
  }

  filtered.forEach((entry) => {
    const row = document.createElement("div");
    row.className = "row";
    row.dataset.rowKey = [entry.source || "", entry.address || "", entry.name || ""].join("|");
    if (row.dataset.rowKey === hoveredRowKey) {
      row.classList.add("pointer-hover");
    }

    const nameCell = document.createElement("div");
    nameCell.className = "name";
    const nameTitle = [entry.name, entry.address].filter(Boolean).join("\n");
    if (nameTitle) {
      nameCell.title = nameTitle;
    }
    const nameLabel = document.createElement("div");
    nameLabel.textContent = entry.name || "";
    nameCell.appendChild(nameLabel);
    const runtimeDisplay = splitDisplayValue(entry.value || "");
    const displayType = displayTypeForEntry(entry, runtimeDisplay);
    const pendingBooleanForce =
      displayType === "BOOL"
        ? pendingBooleanForces.get(String(entry.address || ""))
        : undefined;
    const display = pendingBooleanForce === undefined
      ? runtimeDisplay
      : { ...runtimeDisplay, value: pendingBooleanForce };
    if (showAddress && entry.address) {
      const address = document.createElement("div");
      address.className = "address";
      address.textContent = entry.address;
      nameCell.appendChild(address);
    }
    const sourceText = String(entry.source || "").trim();
    if (sourceText) {
      const source = document.createElement("div");
      source.className = "source-subtitle";
      source.textContent = sourceText;
      source.title = sourceText;
      nameCell.appendChild(source);
    }

    const valueCell = document.createElement("div");
    valueCell.className = "value";
    const displayValue = displayValueForEntry(display, displayType);
    valueCell.textContent = displayValue;
    if (displayValue && displayValue !== (display.value || "")) {
      valueCell.title = (display.value || "") + " as " + displayValue;
    }

    const typeCell = document.createElement("div");
    typeCell.className = "type-cell";
    typeCell.textContent = displayType || "—";

    const stateCell = document.createElement("div");
    stateCell.className = "state-cell";
    const stateBadge = document.createElement("span");
    const forced = !!entry.forced;
    const readOnly = entry.writable === false;
    // A forced value is ALWAYS visibly marked in its own State column, not inferred from
    // action buttons or hidden inside the value text.
    if (forced) {
      row.classList.add("forced");
      stateBadge.className = "state-badge forced";
      stateBadge.textContent = "FORCED";
    } else if (readOnly) {
      stateBadge.className = "state-badge live";
      stateBadge.textContent = "read only";
    } else {
      stateBadge.className = "state-badge live";
      stateBadge.textContent = "live";
    }
    stateCell.appendChild(stateBadge);

    row.appendChild(nameCell);
    row.appendChild(valueCell);
    row.appendChild(typeCell);
    row.appendChild(stateCell);

    if (allowActions) {
      const actions = document.createElement("div");
      actions.className = "actions";
      const entryReason = readOnly
        ? "Read-only ADS tag. Enable writes when adding the tag, then restart the runtime."
        : "";
      const canWrite = allowWrite && !readOnly && !forced;
      const canForce = allowForce && !readOnly && !forced;
      const canRelease = allowRelease && forced;

      const key = [entry.name || "", entry.address || ""].join("|");
      const defaultValue = editCache.has(key)
        ? editCache.get(key)
        : defaultWriteValue(entry, display);
      const createTextInput = () => {
        const input = document.createElement("input");
        input.className = "value-input";
        input.type = "text";
        input.dataset.key = key;
        input.value = defaultValue;
        input.placeholder = display.value || entry.value || "";
        input.disabled = !(canWrite || canForce);
        input.addEventListener("input", () => {
          editCache.set(key, input.value);
        });
        input.addEventListener("focus", () => {
          editCache.set(key, input.value);
        });
        input.addEventListener("blur", () => {
          editCache.delete(key);
        });
        return input;
      };
      const boolCurrentValue =
        String(display.value || entry.value || "").toUpperCase() === "TRUE";
      // BOOL rows get a TRUE/FALSE chooser in the same slot as the numeric write-box, so the
      // operator explicitly picks the value to write or force (starts at the current value).
      const createBoolToggle = () => {
        const toggle = document.createElement("button");
        toggle.type = "button";
        toggle.className = "value-input bool-toggle";
        const draft = String(defaultValue || "").trim().toUpperCase();
        const initial = draft === "TRUE" || draft === "FALSE"
          ? draft
          : boolCurrentValue ? "TRUE" : "FALSE";
        toggle.value = initial;
        toggle.textContent = initial;
        toggle.dataset.key = key;
        toggle.setAttribute("aria-pressed", initial === "TRUE" ? "true" : "false");
        toggle.title = "Selected " + initial + ". Click to select " +
          (initial === "TRUE" ? "FALSE" : "TRUE") + ".";
        toggle.setAttribute("aria-label", "Value to write or force: " + initial);
        toggle.disabled = !(canWrite || canForce);
        toggle.addEventListener("click", () => {
          const next = toggle.value === "TRUE" ? "FALSE" : "TRUE";
          toggle.value = next;
          toggle.textContent = next;
          toggle.setAttribute("aria-pressed", next === "TRUE" ? "true" : "false");
          toggle.title = "Selected " + next + ". Click to select " +
            (next === "TRUE" ? "FALSE" : "TRUE") + ".";
          toggle.setAttribute("aria-label", "Value to write or force: " + next);
          editCache.set(key, next);
        });
        return toggle;
      };
      const valueControl = displayType === "BOOL" ? createBoolToggle() : createTextInput();

      const sendValue = (action) => {
        if (action === "force" && forceRequiresArming() && !forceArmed) {
          armForceForTarget();
          return;
        }
        if (action !== "release") {
          const raw = String(valueControl.value || "").trim();
          if (!raw) {
    setStatusText("Enter a value.");
            return;
          }
          if (action === "force" && displayType === "BOOL") {
            const booleanValue = parseBooleanValue(raw);
            if (booleanValue !== undefined) {
              const pendingValue = booleanValue ? "TRUE" : "FALSE";
              pendingBooleanForces.set(String(entry.address || ""), pendingValue);
              editCache.set(key, pendingValue);
              valueCell.textContent = pendingValue;
            }
          } else {
            editCache.delete(key);
          }
          vscode.postMessage({
            type: action === "force" ? "forceInput" : "writeInput",
            address: entry.address,
            value: raw,
          });
          return;
        }
        editCache.delete(key);
        vscode.postMessage({
          type: "releaseInput",
          address: entry.address,
        });
      };

      const writeButton = document.createElement("button");
      writeButton.className = "mini-btn";
      writeButton.textContent =
        "Write";
      writeButton.title =
        displayType === "BOOL"
          ? "Write the chosen value once (next cycle, inputs only)"
          : "Write once (next cycle, inputs only)";
      writeButton.setAttribute("aria-label", "Write value once");
      writeButton.disabled = !canWrite;
      if (!canWrite) {
        writeButton.title = forced
          ? "Release force before writing this value."
          : writeDisabledReason || entryReason || remoteReason || "Write is not available for this value.";
      }
      writeButton.addEventListener("click", () => sendValue("write"));

      const forceButton = document.createElement("button");
      forceButton.className = "mini-btn force-slot";
      const isForced = forced;
      const needsForceArm = forceRequiresArming() && !forceArmed && !isForced;
      forceButton.classList.toggle("active", isForced);
      forceButton.classList.toggle("armed", forceRequiresArming() && forceArmed && !isForced);
      forceButton.setAttribute("aria-pressed", isForced ? "true" : "false");
      forceButton.textContent = needsForceArm ? "Arm force" : "Force";
      forceButton.title = isForced
        ? "Force continuously (active)"
        : needsForceArm
          ? "Arm force for this target before pinning a value"
          : displayType === "BOOL"
            ? "Force the chosen value continuously"
            : "Force continuously";
      forceButton.setAttribute(
        "aria-label",
        needsForceArm ? "Arm force for this target" : "Force value continuously"
      );
      forceButton.disabled = !canForce;
      if (!canForce) {
        forceButton.title = entryReason || remoteReason || "Force is not available for this value.";
      }
      forceButton.addEventListener("click", () => sendValue("force"));

      const releaseButton = document.createElement("button");
      releaseButton.className = "mini-btn force-slot release";
      releaseButton.textContent = "Release";
      releaseButton.title = "Release force";
      releaseButton.setAttribute("aria-label", "Release forced value");
      releaseButton.disabled = !canRelease;
      if (!canRelease && remoteReason) {
        releaseButton.title = remoteReason;
      }
      releaseButton.addEventListener("click", () => sendValue("release"));

      actions.appendChild(valueControl);
      actions.appendChild(writeButton);
      if (isForced) {
        actions.appendChild(releaseButton);
      } else {
        actions.appendChild(forceButton);
      }
      row.appendChild(actions);
    }

    wrapper.appendChild(row);
  });

  return wrapper;
}

function captureActiveInput() {
  const active = document.activeElement;
  if (
    active &&
    active.tagName === "INPUT" &&
    active.dataset &&
    active.dataset.key
  ) {
    return {
      key: active.dataset.key,
      value: active.value,
      start:
        typeof active.selectionStart === "number"
          ? active.selectionStart
          : null,
      end:
        typeof active.selectionEnd === "number"
          ? active.selectionEnd
          : null,
    };
  }
  return null;
}

function restoreActiveInput(state) {
  if (!state || !state.key) {
    return;
  }
  const selector = 'input[data-key="' + state.key + '"]';
  const input = document.querySelector(selector);
  if (!input) {
    return;
  }
  input.value = state.value == null ? input.value : state.value;
  input.focus();
  if (
    typeof state.start === "number" &&
    typeof state.end === "number"
  ) {
    input.setSelectionRange(state.start, state.end);
  }
}

function render(state) {
  const activeInput = captureActiveInput();
  updateScanLabel(state);
  updateForcePolicy();
  updateForcedFilter(state);
  sections.innerHTML = "";

  // Read + write + force/release work on the simulator AND on remote attach (the adapter forwards
  // io.force/io.unforce; the runtime authorizes by role and surfaces any error). Outputs/memory stay
  // write-disabled per their I/O semantics, independent of target.
  const ioContent = document.createElement("div");
  const writeHint = document.createElement("div");
  writeHint.className = "write-hint";
  writeHint.textContent = "Outputs and memory are program-driven — use Force to override.";
  ioContent.appendChild(writeHint);
  appendIoSection(ioContent, "Inputs", state.inputs, {
    allowActions: true,
    showAddress: true,
    allowWrite: currentAccess.allowWrite,
    allowForce: currentAccess.allowForce,
    allowRelease: currentAccess.allowRelease,
    remoteReason: currentAccess.reason,
  });
  appendIoSection(ioContent, "Outputs", state.outputs, {
    allowActions: true,
    showAddress: true,
    allowWrite: false,
    allowForce: currentAccess.allowForce,
    allowRelease: currentAccess.allowRelease,
    remoteReason: currentAccess.reason,
    writeDisabledReason: "Program-driven — use Force to override",
  });
  appendIoSection(ioContent, "Memory", state.memory, {
    allowActions: true,
    showAddress: true,
    allowWrite: false,
    allowForce: currentAccess.allowForce,
    allowRelease: currentAccess.allowRelease,
    remoteReason: currentAccess.reason,
    writeDisabledReason: "Program-driven — use Force to override",
  });

  sections.appendChild(createNode("I/O", 0, ioContent, true));
  const adsEntries = state.ads || [];
  if (
    adsEntries.length > 0 &&
    (!forcedOnly || hasForcedEntry(adsEntries))
  ) {
    const adsRows = renderRows(adsEntries, {
      allowActions: true,
      showAddress: true,
      allowWrite: currentAccess.allowWrite,
      allowForce: currentAccess.allowForce,
      allowRelease: currentAccess.allowRelease,
      remoteReason: currentAccess.reason,
    });
    adsRows.classList.add("aligned-root-rows");
    sections.appendChild(
      createNode(
        "ADS tags",
        0,
        adsRows,
        true
      )
    );
  }
  updateReleaseAll(state);
  restoreActiveInput(activeInput);
}

window.addEventListener("message", (event) => {
  const message = event.data;
  if (message.type === "ioState") {
    if (isTransientStatusText(status ? status.textContent || "" : "")) {
      setStatusText("");
    }
    currentState = message.payload || { inputs: [], outputs: [], memory: [], ads: [] };
    reconcilePendingBooleanForces(currentState);
    if (actionInteractionActive) {
      actionInteractionRenderPending = true;
    } else {
      render(currentState);
      updateForceStatusFromState(currentState);
    }
  }
  if (message.type === "status") {
    const payload = String(message.payload || "");
    clearUnavailableRuntimeStatus(payload);
    if (/I\/O force failed:/i.test(payload) && clearPendingBooleanForces()) {
      if (actionInteractionActive) {
        actionInteractionRenderPending = true;
      } else {
        render(currentState);
      }
    }
    if (
      forceRequiresArming() &&
      forceArmed &&
      /released|cleared/i.test(payload) &&
      !/armed/i.test(payload)
    ) {
      setStatusText(payload + " Force remains armed for this target.");
    } else {
      setStatusText(payload);
    }
  }
  if (message.type === "compileResult") {
    compileState = message.payload || null;
    renderDiagnostics();
  }
  if (message.type === "settings") {
    applySettingsPayload(message.payload || {});
  }
  if (message.type === "runtimeStatus") {
    applyRuntimeStatus(message.payload || {});
  }

  if (message.type === "openSettings") {
    setSettingsOpen(true);
  }
});
