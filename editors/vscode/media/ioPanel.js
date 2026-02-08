const vscode =
  typeof acquireVsCodeApi === "function"
    ? acquireVsCodeApi()
    : { postMessage: () => {} };
const sections = document.getElementById("sections");
const status = document.getElementById("status");
const filterInput = document.getElementById("filter");
const diagnosticsSummary = document.getElementById("diagnosticsSummary");
const diagnosticsRuntime = document.getElementById("diagnosticsRuntime");
const diagnosticsList = document.getElementById("diagnosticsList");
const runtimeView = document.getElementById("runtimeView");
const settingsPanel = document.getElementById("settingsPanel");
const settingsSave = document.getElementById("settingsSave");
const settingsCancel = document.getElementById("settingsCancel");
const runtimeStatusText = document.getElementById("runtimeStatusText");
const runtimeStart = document.getElementById("runtimeStart");
const modeSimulate = document.getElementById("modeSimulate");
const modeOnline = document.getElementById("modeOnline");
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
let currentState = { inputs: [], outputs: [], memory: [] };
let compileState = null;
let currentFilter = "";
const editCache = new Map();
let settingsOpen = false;

function setStatusText(message) {
  if (status) {
    status.textContent = message;
  }
}

function reportWebviewError(message, stack) {
  setStatusText("Runtime panel error: " + message);
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
setStatusText("Runtime panel ready.");
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
    status.textContent = message;
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

function applyRuntimeStatus(payload) {
  if (!payload) {
    return;
  }
  const running = !!payload.running;
  const runtimeState = payload.runtimeState || (running ? "running" : "stopped");
  const connected = runtimeState === "connected";
  const mode = payload.runtimeMode || "simulate";

  if (modeSimulate) {
    modeSimulate.classList.toggle("active", mode === "simulate");
    modeSimulate.disabled = running || connected;
  }
  if (modeOnline) {
    modeOnline.classList.toggle("active", mode === "online");
    modeOnline.disabled = running || connected;
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
    const label = isRunning ? "Running" : "Stopped";
    runtimeStatusText.textContent = label;
    runtimeStatusText.classList.toggle("running", isRunning);
    runtimeStatusText.classList.toggle("connected", runtimeState === "connected");
    runtimeStatusText.classList.toggle("disconnected", !isRunning);
    runtimeStatusText.title = payload.endpoint || "";
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
    diagnosticsSummary.textContent = "No compile run yet";
    diagnosticsRuntime.textContent = "";
    return;
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
  if (!filter) {
    return entries || [];
  }
  return (entries || []).filter((entry) => {
    const haystack = ((entry.name || "") + " " + (entry.address || "")).toLowerCase();
    return haystack.includes(filter);
  });
}

function isBooleanValue(value) {
  const normalized = String(value || "").trim().toUpperCase();
  return normalized === "TRUE" || normalized === "FALSE";
}

function nextBooleanValue(value) {
  return String(value || "").trim().toUpperCase() === "TRUE"
    ? "FALSE"
    : "TRUE";
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
  const match = trimmed.match(/\\(([-\\d]+)\\)/);
  if (match) {
    return match[1];
  }
  return "";
}

function splitDisplayValue(value) {
  const text = String(value == null ? "" : value);
  const match = text.match(/^([A-Za-z_][A-Za-z0-9_]*)\\((.*)\\)$/);
  if (!match) {
    return { value: text, type: "" };
  }
  return { value: match[2], type: match[1].toUpperCase() };
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

function renderRows(entries, options = {}) {
  const {
    allowActions = false,
    showAddress = false,
    allowWrite = true,
    allowForce = true,
    allowRelease = true,
  } = options;
  const wrapper = document.createElement("div");
  wrapper.className = "rows";

  const filtered = applyFilter(entries);
  if (filtered.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    const message =
      entries && entries.length > 0 && currentFilter.trim()
        ? "No matches"
        : "No entries";
    empty.textContent = message;
    wrapper.appendChild(empty);
    return wrapper;
  }

  filtered.forEach((entry) => {
    const row = document.createElement("div");
    row.className = "row";

    const nameCell = document.createElement("div");
    nameCell.className = "name";
    const nameLabel = document.createElement("div");
    nameLabel.textContent = entry.name || "";
    nameCell.appendChild(nameLabel);
    const display = splitDisplayValue(entry.value || "");
    if (display.type) {
      const typeLabel = document.createElement("div");
      typeLabel.className = "type";
      typeLabel.textContent = display.type;
      nameCell.appendChild(typeLabel);
    }
    if (showAddress && entry.address) {
      const address = document.createElement("div");
      address.className = "address";
      address.textContent = entry.address;
      nameCell.appendChild(address);
    }

    const valueCell = document.createElement("div");
    valueCell.className = "value";
    valueCell.textContent = display.value || "";

    row.appendChild(nameCell);
    row.appendChild(valueCell);

    if (allowActions) {
      const actions = document.createElement("div");
      actions.className = "actions";
      const canWrite = allowWrite;
      const canForce = allowForce;
      const canRelease = allowRelease;

      const input = document.createElement("input");
      input.className = "value-input";
      input.type = "text";
      const key = [entry.name || "", entry.address || ""].join("|");
      input.dataset.key = key;
      if (isBooleanValue(entry.value)) {
        input.value = editCache.has(key)
          ? editCache.get(key)
          : nextBooleanValue(entry.value);
      } else {
        input.value = editCache.has(key)
          ? editCache.get(key)
          : defaultNumericValue(entry.value);
      }
      input.placeholder = entry.value || "";
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

      const sendValue = (action) => {
        if (action !== "release") {
          const raw = input.value.trim();
          if (!raw) {
            status.textContent = "Enter a value.";
            return;
          }
          editCache.delete(key);
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
      writeButton.textContent = "W";
      writeButton.title = "Write once (next cycle, inputs only)";
      writeButton.disabled = !canWrite;
      writeButton.addEventListener("click", () => sendValue("write"));

      const forceButton = document.createElement("button");
      forceButton.className = "mini-btn";
      const isForced = !!entry.forced;
      forceButton.classList.toggle("active", isForced);
      forceButton.setAttribute("aria-pressed", isForced ? "true" : "false");
      forceButton.textContent = isForced ? "F*" : "F";
      forceButton.title = isForced
        ? "Force continuously (active)"
        : "Force continuously";
      forceButton.disabled = !canForce;
      forceButton.addEventListener("click", () => sendValue("force"));

      const releaseButton = document.createElement("button");
      releaseButton.className = "mini-btn";
      releaseButton.textContent = "R";
      releaseButton.title = "Release force";
      releaseButton.disabled = !canRelease;
      releaseButton.addEventListener("click", () => sendValue("release"));

      actions.appendChild(input);
      actions.appendChild(writeButton);
      actions.appendChild(forceButton);
      actions.appendChild(releaseButton);
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
  sections.innerHTML = "";

  const ioContent = document.createElement("div");
  ioContent.appendChild(
    createNode(
      "Inputs",
      2,
      renderRows(state.inputs, {
        allowActions: true,
        showAddress: true,
      }),
      true
    )
  );
  ioContent.appendChild(
    createNode(
      "Outputs",
      2,
      renderRows(state.outputs, {
        allowActions: true,
        showAddress: true,
        allowWrite: false,
      }),
      true
    )
  );
  ioContent.appendChild(
    createNode(
      "Memory",
      2,
      renderRows(state.memory, {
        allowActions: true,
        showAddress: true,
        allowWrite: false,
      }),
      true
    )
  );

  sections.appendChild(createNode("I/O", 0, ioContent, true));
  restoreActiveInput(activeInput);
}

window.addEventListener("message", (event) => {
  const message = event.data;
  if (message.type === "ioState") {
    status.textContent = "";
    currentState = message.payload || { inputs: [], outputs: [], memory: [] };
    render(currentState);
  }
  if (message.type === "status") {
    status.textContent = message.payload;
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
