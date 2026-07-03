import { getVsCodeApi } from "./vscodeApi";

interface IoEntry {
  name?: string;
  address: string;
  source?: string;
  valueType?: string;
  writeTarget?: string;
  value: string;
  forced?: boolean;
  operationStatus?: "pending" | "error";
  operationError?: string;
}

interface IoState {
  inputs: IoEntry[];
  outputs: IoEntry[];
  memory: IoEntry[];
}

const vscode = getVsCodeApi();

interface SettingsPayload {
  serverPath?: string;
  traceServer?: string;
  debugAdapterPath?: string;
  debugAdapterArgs?: string[];
  debugAdapterEnv?: Record<string, string>;
  runtimeControlEndpoint?: string;
  runtimeControlAuthToken?: string;
  runtimeInlineValuesEnabled?: boolean;
  runtimeIncludeGlobs?: string[];
  runtimeExcludeGlobs?: string[];
  runtimeIgnorePragmas?: string[];
}

interface CompileIssue {
  file: string;
  line: number;
  column: number;
  severity: "error" | "warning";
  message: string;
  code?: string;
  source?: string;
}

interface CompileResult {
  target: string;
  dirty: boolean;
  errors: number;
  warnings: number;
  issues: CompileIssue[];
  runtimeStatus: "ok" | "error" | "skipped";
  runtimeMessage?: string;
}

interface RuntimeStatusPayload {
  running: boolean;
  inlineValuesEnabled: boolean;
  runtimeMode: "simulate" | "online";
  runtimeState: "running" | "connected" | "stopped";
  endpoint: string;
  endpointConfigured: boolean;
  endpointEnabled: boolean;
  endpointReachable: boolean;
}

type SettingsFields = {
  serverPath: HTMLInputElement | null;
  traceServer: HTMLSelectElement | null;
  debugAdapterPath: HTMLInputElement | null;
  debugAdapterArgs: HTMLTextAreaElement | null;
  debugAdapterEnv: HTMLTextAreaElement | null;
  runtimeControlEndpoint: HTMLInputElement | null;
  runtimeControlAuthToken: HTMLInputElement | null;
  runtimeInlineValuesEnabled: HTMLInputElement | null;
  runtimeIncludeGlobs: HTMLTextAreaElement | null;
  runtimeExcludeGlobs: HTMLTextAreaElement | null;
  runtimeIgnorePragmas: HTMLTextAreaElement | null;
};

interface RuntimePanelMountOptions {
  initialSettingsOpen?: boolean;
  enableSettingsButtonToggle?: boolean;
  onSettingsOpenChange?: (open: boolean) => void;
}

export function mountStRuntimePanel(
  root: HTMLElement,
  options: RuntimePanelMountOptions = {}
): () => void {
  const byId = <T extends Element>(id: string): T | null =>
    root.querySelector(`[data-st-runtime-id="${id}"]`) as T | null;

  const sections = byId<HTMLDivElement>("sections");
  const status = byId<HTMLDivElement>("status");
  const filterInput = byId<HTMLInputElement>("filter");
  const diagnosticsSummary = byId<HTMLDivElement>("diagnosticsSummary");
  const diagnosticsRuntime = byId<HTMLDivElement>("diagnosticsRuntime");
  const diagnosticsList = byId<HTMLDivElement>("diagnosticsList");
  const runtimeView = byId<HTMLDivElement>("runtimeView");
  const settingsPanel = byId<HTMLDivElement>("settingsPanel");
  const settingsSave = byId<HTMLButtonElement>("settingsSave");
  const settingsCancel = byId<HTMLButtonElement>("settingsCancel");
  const runtimeStatusText = byId<HTMLSpanElement>("runtimeStatusText");
  const runtimeStart = byId<HTMLButtonElement>("runtimeStart");
  const modeSimulate = byId<HTMLButtonElement>("modeSimulate");
  const modeOnline = byId<HTMLButtonElement>("modeOnline");
  const settingsButton = byId<HTMLButtonElement>("settings");

  const settingsFields: SettingsFields = {
    serverPath: byId<HTMLInputElement>("serverPath"),
    traceServer: byId<HTMLSelectElement>("traceServer"),
    debugAdapterPath: byId<HTMLInputElement>("debugAdapterPath"),
    debugAdapterArgs: byId<HTMLTextAreaElement>("debugAdapterArgs"),
    debugAdapterEnv: byId<HTMLTextAreaElement>("debugAdapterEnv"),
    runtimeControlEndpoint: byId<HTMLInputElement>("runtimeControlEndpoint"),
    runtimeControlAuthToken: byId<HTMLInputElement>("runtimeControlAuthToken"),
    runtimeInlineValuesEnabled: byId<HTMLInputElement>(
      "runtimeInlineValuesEnabled"
    ),
    runtimeIncludeGlobs: byId<HTMLTextAreaElement>("runtimeIncludeGlobs"),
    runtimeExcludeGlobs: byId<HTMLTextAreaElement>("runtimeExcludeGlobs"),
    runtimeIgnorePragmas: byId<HTMLTextAreaElement>("runtimeIgnorePragmas"),
  };

  let currentState: IoState = { inputs: [], outputs: [], memory: [] };
  let compileState: CompileResult | null = null;
  let currentFilter = "";
  const editCache = new Map<string, string>();
  let settingsOpen = false;
  let currentRuntimeState: "running" | "connected" | "stopped" = "stopped";
  let statusClearTimer: ReturnType<typeof window.setTimeout> | undefined;
  const TRANSIENT_STATUS_CLEAR_MS = 5000;

  const removeListeners: Array<() => void> = [];

  const clearStatusClearTimer = () => {
    if (statusClearTimer !== undefined) {
      window.clearTimeout(statusClearTimer);
      statusClearTimer = undefined;
    }
  };

  const setStatusText = (message: string) => {
    if (status) {
      clearStatusClearTimer();
      status.textContent = message;
      if (isAutoExpiringStatusText(message)) {
        statusClearTimer = window.setTimeout(() => {
          if (status?.textContent === message) {
            setStatusText("");
          }
        }, TRANSIENT_STATUS_CLEAR_MS);
      }
    }
  };

  const isTransientStatusText = (message: string) =>
    /^Live Values (loading|ready)\.?$/i.test(message) ||
    /^Start the runtime to see live values\.?$/i.test(message) ||
    /^Connect to the selected runtime to see live values\.?$/i.test(message);

  const isAutoExpiringStatusText = (message: string) =>
    /^Live Values ready\.?$/i.test(message) ||
    /^I\/O write queued for .+\.?$/i.test(message) ||
    /^I\/O force released at .+\.?$/i.test(message) ||
    /^Released \d+ forces?\.?$/i.test(message) ||
    /^No forces to release\.?$/i.test(message);

  const reportWebviewError = (message: string, stack: string) => {
    setStatusText(`Live Values error: ${message}`);
    vscode.postMessage({
      type: "webviewError",
      message,
      stack,
    });
  };

  const setSettingsOpen = (open: boolean) => {
    settingsOpen = open;
    settingsPanel?.classList.toggle("open", open);
    runtimeView?.classList.toggle("hidden", open);
    if (filterInput) {
      filterInput.disabled = open;
    }
    options.onSettingsOpenChange?.(open);
    if (open) {
      vscode.postMessage({ type: "requestSettings" });
    }
  };

  const addListener = (
    target: EventTarget | null,
    type: string,
    handler: EventListenerOrEventListenerObject
  ) => {
    if (!target) {
      return;
    }
    target.addEventListener(type, handler);
    removeListeners.push(() => {
      target.removeEventListener(type, handler);
    });
  };

  const getFieldValue = (
    element:
      | HTMLInputElement
      | HTMLSelectElement
      | HTMLTextAreaElement
      | null
      | undefined
  ): string => {
    if (!element) {
      return "";
    }
    return element.value ?? "";
  };

  const setFieldValue = (
    element:
      | HTMLInputElement
      | HTMLSelectElement
      | HTMLTextAreaElement
      | null
      | undefined,
    value: string
  ) => {
    if (!element) {
      return;
    }
    element.value = value ?? "";
  };

  const arrayToText = (values: string[] | undefined): string =>
    Array.isArray(values) ? values.join("\n") : "";

  const textToArray = (value: string): string[] =>
    String(value || "")
      .split(/\r?\n/)
      .map((item) => item.trim())
      .filter((item) => item.length > 0);

  const envToText = (env: Record<string, string> | undefined): string => {
    if (!env || typeof env !== "object") {
      return "";
    }
    return Object.entries(env)
      .map(([key, value]) => `${key}=${value == null ? "" : value}`)
      .join("\n");
  };

  const parseEnv = (text: string): Record<string, string> => {
    const trimmed = String(text || "").trim();
    if (!trimmed) {
      return {};
    }
    try {
      const parsed = JSON.parse(trimmed) as unknown;
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        const env: Record<string, string> = {};
        for (const [key, value] of Object.entries(parsed)) {
          env[key] = value == null ? "" : String(value);
        }
        return env;
      }
    } catch {
      // Fall through to KEY=VALUE lines.
    }

    const env: Record<string, string> = {};
    const lines = trimmed.split(/\r?\n/);
    for (const line of lines) {
      if (!line.trim()) {
        continue;
      }
      const separator = line.indexOf("=");
      if (separator <= 0) {
        throw new Error(
          "Env entries must be KEY=VALUE per line or a JSON object."
        );
      }
      const key = line.slice(0, separator).trim();
      const value = line.slice(separator + 1).trim();
      if (!key) {
        throw new Error("Env entries must include a key.");
      }
      env[key] = value;
    }
    return env;
  };

  const collectSettingsPayload = (): SettingsPayload | null => {
    let debugAdapterEnv: Record<string, string> = {};
    try {
      debugAdapterEnv = parseEnv(getFieldValue(settingsFields.debugAdapterEnv));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
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
      runtimeInlineValuesEnabled:
        settingsFields.runtimeInlineValuesEnabled?.checked !== false,
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
  };

  const applyRuntimeStatus = (payload: RuntimeStatusPayload) => {
    if (!payload) {
      return;
    }

    const running = Boolean(payload.running);
    const runtimeState =
      payload.runtimeState || (running ? "running" : "stopped");
    
    // Update current runtime state
    currentRuntimeState = runtimeState;
    
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
      runtimeStatusText.textContent =
        runtimeState === "connected" ? "Connected" : isRunning ? "Running" : "Stopped";
      runtimeStatusText.classList.toggle("running", isRunning);
      runtimeStatusText.classList.toggle("connected", runtimeState === "connected");
      runtimeStatusText.classList.toggle("disconnected", !isRunning);
      runtimeStatusText.title = payload.endpoint || "";
    }
  };

  const applySettingsPayload = (payload: SettingsPayload) => {
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
  };

  const fileLabel = (filePath: string): string => {
    if (!filePath) {
      return "";
    }
    const parts = filePath.split(/[/\\]/);
    return parts[parts.length - 1] || filePath;
  };

  const renderDiagnostics = () => {
    if (!diagnosticsSummary || !diagnosticsRuntime || !diagnosticsList) {
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
      `${targetLabel || "Unknown target"}${dirtyLabel} • ${compileState.errors}` +
      ` error(s), ${compileState.warnings} warning(s)`;

    diagnosticsRuntime.textContent =
      compileState.runtimeStatus !== "skipped" && compileState.runtimeMessage
        ? compileState.runtimeMessage
        : "";

    const issues = Array.isArray(compileState.issues) ? compileState.issues : [];
    if (issues.length === 0) {
      const empty = document.createElement("div");
      empty.className = "empty";
      empty.textContent = "No warnings or errors.";
      diagnosticsList.appendChild(empty);
      return;
    }

    for (const issue of issues) {
      const item = document.createElement("div");
      item.className = `diagnostic-item ${
        issue.severity === "error" ? "error" : "warning"
      }`;

      const message = document.createElement("div");
      message.className = "diagnostic-message";
      message.textContent = issue.message || "";
      item.appendChild(message);

      const meta = document.createElement("div");
      meta.className = "diagnostic-meta";
      const location = document.createElement("span");
      const locationParts: string[] = [];
      if (issue.file) {
        locationParts.push(fileLabel(issue.file));
      }
      if (issue.line) {
        locationParts.push(`L${issue.line}`);
      }
      if (issue.column) {
        locationParts.push(`C${issue.column}`);
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
    }
  };

  const applyFilter = (entries: IoEntry[]): IoEntry[] => {
    const filter = currentFilter.trim().toLowerCase();
    if (!filter) {
      return entries || [];
    }
    return (entries || []).filter((entry) => {
      const haystack = `${entry.name || ""} ${entry.address || ""} ${
        entry.source || ""
      }`.toLowerCase();
      return haystack.includes(filter);
    });
  };

  const parseBooleanValue = (value: string): boolean | undefined => {
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
  };

  const defaultNumericValue = (value: string): string => {
    const trimmed = String(value || "").trim();
    if (!trimmed) {
      return "";
    }
    const numericLiteral =
      /^(?:0x[0-9a-fA-F_]+|[0-9][0-9_]*|[28]#[0-9A-Fa-f_]+|16#[0-9A-Fa-f_]+)$/;
    if (numericLiteral.test(trimmed)) {
      return trimmed;
    }
    const match = trimmed.match(/\(([-\d]+)\)/);
    return match ? match[1] : "";
  };

  const defaultWriteValue = (
    entry: IoEntry,
    display: { value: string; type: string }
  ): string => {
    const booleanValue = parseBooleanValue(entry.value || display.value);
    if (booleanValue !== undefined) {
      // Pre-fill with the current value. Rendering a BOOL row must not silently choose
      // the opposite force/write value before the user clicks the toggle.
      return booleanValue ? "TRUE" : "FALSE";
    }

    const numericValue = defaultNumericValue(display.value || entry.value);
    if (numericValue) {
      return numericValue;
    }

    if (
      display.type === "BOOL" ||
      /^%[IQM]X/i.test(String(entry.address || "").trim())
    ) {
      return "TRUE";
    }

    return "0";
  };

  const splitDisplayValue = (value: string): { value: string; type: string } => {
    const text = String(value == null ? "" : value);
    const match = text.match(/^([A-Za-z_][A-Za-z0-9_]*)\((.*)\)$/);
    if (!match) {
      return { value: text, type: "" };
    }
    return { value: match[2], type: match[1].toUpperCase() };
  };

  const typeFromAddress = (address: unknown): string => {
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
  };

  const createNode = (
    title: string,
    level: number,
    content: HTMLElement,
    open = true
  ): HTMLElement => {
    const details = document.createElement("details");
    details.className = `tree-node level-${level}`;
    details.open = open;
    const summary = document.createElement("summary");
    summary.textContent = title;
    details.appendChild(summary);
    details.appendChild(content);
    return details;
  };

  const renderRows = (
    entries: IoEntry[],
    options: {
      allowActions?: boolean;
      showAddress?: boolean;
      allowWrite?: boolean;
      allowForce?: boolean;
      allowRelease?: boolean;
    } = {}
  ): HTMLElement => {
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
      empty.textContent =
        entries && entries.length > 0 && currentFilter.trim()
          ? "No matches"
          : "No entries";
      wrapper.appendChild(empty);
      return wrapper;
    }

    if (allowActions) {
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

    for (const entry of filtered) {
      const row = document.createElement("div");
      row.className = "row";
      const operationPending = entry.operationStatus === "pending";
      const operationError = entry.operationStatus === "error";
      row.classList.toggle("pending", operationPending);
      row.classList.toggle("error", operationError);
      if (operationError && entry.operationError) {
        row.title = entry.operationError;
      }

      const nameCell = document.createElement("div");
      nameCell.className = "name";
      const nameLabel = document.createElement("div");
      nameLabel.textContent = entry.name || "";
      nameCell.appendChild(nameLabel);

      const display = splitDisplayValue(entry.value || "");
      const explicitType = String((entry.valueType || (entry as { value_type?: string }).value_type || (entry as { type?: string }).type) || "").trim();
      const displayType = explicitType.toUpperCase() || display.type || typeFromAddress(entry.address);
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
      valueCell.textContent = display.value || "";

      const typeCell = document.createElement("div");
      typeCell.className = "type-cell";
      typeCell.textContent = displayType || "—";

      const stateCell = document.createElement("div");
      stateCell.className = "state-cell";
      const stateBadge = document.createElement("span");
      const isForced = Boolean(entry.forced);
      if (operationPending) {
        stateBadge.className = "state-badge pending";
        stateBadge.textContent = "pending";
      } else if (operationError) {
        stateBadge.className = "state-badge error";
        stateBadge.textContent = "error";
      } else if (isForced) {
        row.classList.add("forced");
        stateBadge.className = "state-badge forced";
        stateBadge.textContent = "FORCED";
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
        const canWrite = allowWrite && !isForced;
        const canForce = allowForce && !isForced;
        const canRelease = allowRelease && isForced;

        const key = `${entry.name || ""}|${entry.address || ""}|${
          entry.writeTarget || ""
        }`;
        const defaultValue = editCache.has(key)
          ? editCache.get(key)!
          : defaultWriteValue(entry, display);
        const createTextInput = (): HTMLInputElement => {
          const input = document.createElement("input");
          input.className = "value-input";
          input.type = "text";
          input.dataset.key = key;
          input.value = defaultValue;
          input.placeholder = entry.value || "";
          input.disabled = !(canWrite || canForce) || operationPending;
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
        // BOOL rows get a TRUE/FALSE chooser in the write-box slot so the operator explicitly
        // picks the value to write or force (starts at the current value).
        const createBoolToggle = (): HTMLButtonElement => {
          const toggle = document.createElement("button");
          toggle.type = "button";
          toggle.className = "value-input bool-toggle";
          const initial = boolCurrentValue ? "TRUE" : "FALSE";
          toggle.value = initial;
          toggle.textContent = initial;
          toggle.dataset.key = key;
          toggle.setAttribute("aria-pressed", boolCurrentValue ? "true" : "false");
          toggle.title = "Choose the value to write or force (click to toggle TRUE / FALSE)";
          toggle.setAttribute("aria-label", "Value to write or force");
          toggle.disabled = !(canWrite || canForce) || operationPending;
          toggle.addEventListener("click", () => {
            const next = toggle.value === "TRUE" ? "FALSE" : "TRUE";
            toggle.value = next;
            toggle.textContent = next;
            toggle.setAttribute("aria-pressed", next === "TRUE" ? "true" : "false");
          });
          return toggle;
        };
        const valueControl: HTMLInputElement | HTMLButtonElement | null = isForced
          ? null
          : displayType === "BOOL" ? createBoolToggle() : createTextInput();

        const actionTarget = String(
          entry.writeTarget && entry.writeTarget.trim().length > 0
            ? entry.writeTarget
            : entry.address
        ).trim();

        const sendValue = (action: "write" | "force" | "release") => {
          if (!actionTarget) {
            setStatusText("Missing runtime target.");
            return;
          }
          if (action !== "release") {
            const raw = String((valueControl && valueControl.value) || "").trim();
            if (!raw) {
              setStatusText("Enter a value.");
              return;
            }
            editCache.delete(key);
            vscode.postMessage({
              type: action === "force" ? "forceInput" : "writeInput",
              address: actionTarget,
              value: raw,
            });
            return;
          }
          editCache.delete(key);
          vscode.postMessage({
            type: "releaseInput",
            address: actionTarget,
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
        writeButton.disabled = !canWrite || operationPending;
        if (!canWrite && isForced) {
          writeButton.title = "Release force before writing this value.";
        }
        writeButton.addEventListener("click", () => sendValue("write"));

        const forceButton = document.createElement("button");
        forceButton.className = "mini-btn force-slot";
        forceButton.classList.toggle("active", isForced);
        forceButton.setAttribute("aria-pressed", isForced ? "true" : "false");
        forceButton.textContent = "Force";
        forceButton.title = isForced
          ? "Force continuously (active)"
          : displayType === "BOOL"
            ? "Force the chosen value continuously"
            : "Force continuously";
        forceButton.setAttribute("aria-label", "Force value continuously");
        forceButton.disabled = !canForce || operationPending;
        forceButton.addEventListener("click", () => sendValue("force"));

        const releaseButton = document.createElement("button");
        releaseButton.className = "mini-btn force-slot release";
        releaseButton.textContent = "Release";
        releaseButton.title = "Release force";
        releaseButton.setAttribute("aria-label", "Release forced value");
        releaseButton.disabled = !canRelease || operationPending;
        releaseButton.addEventListener("click", () => sendValue("release"));

        if (valueControl) {
          actions.appendChild(valueControl);
        }
        actions.appendChild(writeButton);
        if (isForced) {
          actions.appendChild(releaseButton);
        } else {
          actions.appendChild(forceButton);
        }
        row.appendChild(actions);
      }

      wrapper.appendChild(row);
    }

    return wrapper;
  };

  const captureActiveInput = () => {
    const active = document.activeElement;
    if (
      active instanceof HTMLInputElement &&
      active.dataset &&
      active.dataset.key
    ) {
      return {
        key: active.dataset.key,
        value: active.value,
        start:
          typeof active.selectionStart === "number" ? active.selectionStart : null,
        end: typeof active.selectionEnd === "number" ? active.selectionEnd : null,
      };
    }
    return null;
  };

  const restoreActiveInput = (
    state:
      | {
          key: string;
          value: string;
          start: number | null;
          end: number | null;
        }
      | null
  ) => {
    if (!state?.key) {
      return;
    }
    const input = root.querySelector<HTMLInputElement>(
      `input[data-key="${state.key}"]`
    );
    if (!input) {
      return;
    }
    input.value = state.value == null ? input.value : state.value;
    input.focus();
    if (typeof state.start === "number" && typeof state.end === "number") {
      input.setSelectionRange(state.start, state.end);
    }
  };

  const render = (ioState: IoState) => {
    if (!sections) {
      return;
    }

    const activeInput = captureActiveInput();
    sections.innerHTML = "";

    const ioContent = document.createElement("div");
    ioContent.appendChild(
      createNode(
        "Inputs",
        2,
        renderRows(ioState.inputs, {
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
        renderRows(ioState.outputs, {
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
        renderRows(ioState.memory, {
          allowActions: true,
          showAddress: true,
          allowWrite: false,
        }),
        true
      )
    );

    sections.appendChild(createNode("I/O", 0, ioContent, true));
    restoreActiveInput(activeInput);
  };

  addListener(window, "error", (event) => {
    const errorEvent = event as ErrorEvent;
    const message =
      errorEvent && typeof errorEvent.message === "string"
        ? errorEvent.message
        : "Unknown error";
    const stack = errorEvent.error?.stack || "";
    reportWebviewError(message, stack);
  });

  addListener(window, "unhandledrejection", (event) => {
    const rejection = event as PromiseRejectionEvent;
    const reason = rejection.reason;
    const message =
      reason && typeof reason.message === "string"
        ? reason.message
        : String(reason ?? "Unknown error");
    const stack = reason?.stack || "";
    reportWebviewError(message, stack);
  });

  addListener(runtimeStart, "click", () => {
    // Backend will toggle between start and stop based on execution state
    vscode.postMessage({ type: "runtimeStart" });
  });
  addListener(modeSimulate, "click", () => {
    vscode.postMessage({ type: "runtimeSetMode", mode: "simulate" });
  });
  addListener(modeOnline, "click", () => {
    vscode.postMessage({ type: "runtimeSetMode", mode: "online" });
  });
  if (options.enableSettingsButtonToggle !== false) {
    addListener(settingsButton, "click", () => {
      setSettingsOpen(!settingsOpen);
    });
  }
  addListener(settingsSave, "click", () => {
    const payload = collectSettingsPayload();
    if (!payload) {
      return;
    }
    vscode.postMessage({ type: "saveSettings", payload });
  });
  addListener(settingsCancel, "click", () => {
    setSettingsOpen(false);
  });
  addListener(filterInput, "input", () => {
    currentFilter = filterInput?.value || "";
    render(currentState);
  });

  const onMessage = (event: Event) => {
    const message = (event as MessageEvent).data;
    if (!message || typeof message.type !== "string") {
      return;
    }

    if (message.type === "ioState") {
      if (isTransientStatusText(status?.textContent || "")) {
        setStatusText("");
      }
      currentState = message.payload || { inputs: [], outputs: [], memory: [] };
      render(currentState);
      return;
    }
    if (message.type === "status") {
      setStatusText(String(message.payload || ""));
      return;
    }
    if (message.type === "compileResult") {
      compileState = message.payload || null;
      renderDiagnostics();
      return;
    }
    if (message.type === "settings") {
      applySettingsPayload((message.payload || {}) as SettingsPayload);
      return;
    }
    if (message.type === "runtimeStatus") {
      applyRuntimeStatus((message.payload || {}) as RuntimeStatusPayload);
      return;
    }
    if (message.type === "openSettings") {
      setSettingsOpen(true);
    }
  };

  addListener(window, "message", onMessage);

  setSettingsOpen(options.initialSettingsOpen === true);
  setStatusText("Live Values ready.");
  vscode.postMessage({ type: "webviewReady" });

  return () => {
    clearStatusClearTimer();
    for (const remove of removeListeners) {
      remove();
    }
  };
}
