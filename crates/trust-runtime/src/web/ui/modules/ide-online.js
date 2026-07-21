// ide-online.js — Connection dialog, discovery scan, deploy flow, and sync
// status. Implements US-5.1, US-5.2, US-5.3 from user stories.

// ── Constants ──────────────────────────────────────────

const ONLINE_RECENT_KEY = "trust.ide.recentConnections";
const ONLINE_MAX_RECENT = 10;
const ONLINE_DISCOVERY_INTERVAL_MS = 5000;
const ONLINE_STATUS_POLL_INTERVAL_MS = 4000;
const ONLINE_CONNECTION_LOST_MS = 30000;

// ── Online State ───────────────────────────────────────

const onlineState = {
  connected: false,
  connectionState: "offline",
  address: null,
  target: null,
  targetIsRemote: false,
  authRequired: false,
  authUsername: "",
  authPassword: "",
  runtimeName: null,
  runtimeStatus: null,
  cycleMs: null,
  latencyMs: null,
  reconnectStartedMs: 0,
  reconnectError: "",
  discoveredRuntimes: [],
  discoveryTimer: null,
  statusTimer: null,
  scanning: false,
  deployedHash: null,
  builtHash: null,
  localHash: null,
  deployedFileHashes: {},
  changedFiles: [],
  syncStatus: "not-deployed",
  syncPopoverOpen: false,
  dialogError: "",
};

function onlineNormalizeTarget(address) {
  const trimmed = String(address || "").trim();
  if (!trimmed) return null;
  const withScheme = trimmed.startsWith("http://") || trimmed.startsWith("https://")
    ? trimmed
    : `http://${trimmed}`;
  const normalized = withScheme.replace(/\/+$/, "");
  if (normalized === "http:" || normalized === "https:" || normalized.endsWith("://")) {
    return null;
  }
  return normalized;
}

function onlineTargetDisplay(target) {
  return String(target || "").replace(/^https?:\/\//, "");
}

function onlineDefaultConnectAddress() {
  const host = String(window.location.hostname || "").trim();
  return host || "127.0.0.1";
}

function onlineDefaultConnectPort() {
  const pagePort = String(window.location.port || "").trim();
  if (pagePort) return pagePort;
  return window.location.protocol === "https:" ? "443" : "80";
}

function onlineBuildConnectTarget(address, port) {
  const host = String(address || "").trim();
  if (!host) return null;
  if (host.includes("://")) return host;

  const portText = String(port || "").trim();
  const isBareIpv6Literal = host.includes(":") && host.split(":").length > 2
    && !host.startsWith("[") && !host.includes("]");
  if (isBareIpv6Literal) {
    return portText ? `[${host}]:${portText}` : `[${host}]`;
  }
  if (host.includes(":")) return host;
  return portText ? `${host}:${portText}` : host;
}

function onlineSeedConnectionDefaults() {
  if (!el.connectAddress || !el.connectPort) return;

  const currentAddress = String(el.connectAddress.value || "").trim().toLowerCase();
  const currentPort = String(el.connectPort.value || "").trim();

  if (!currentAddress || currentAddress === "127.0.0.1" || currentAddress === "localhost") {
    el.connectAddress.value = onlineDefaultConnectAddress();
  }
  if (!currentPort || currentPort === "18080") {
    el.connectPort.value = onlineDefaultConnectPort();
  }
}

function onlineIsRemoteTarget(target) {
  try {
    const parsed = new URL(String(target || ""));
    return parsed.origin !== window.location.origin;
  } catch {
    return true;
  }
}

function onlineSleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function onlineCycleMsFromStatus(status) {
  const cycle = Number(status?.metrics?.cycle_ms?.last);
  return Number.isFinite(cycle) && cycle >= 0 ? Math.round(cycle) : null;
}

function onlineReadAuthCredentialsFromDialog() {
  const username = String(el.connectUsername?.value || "").trim();
  const password = String(el.connectPassword?.value || "");
  if (!username) return null;
  return { username, password };
}

function onlineConnectionCredentials() {
  if (!onlineState.targetIsRemote) return null;
  const username = String(onlineState.authUsername || "").trim();
  if (!username) return null;
  return {
    username,
    password: String(onlineState.authPassword || ""),
  };
}

function onlineNotifyDisconnected(transition) {
  if (transition !== "online") return;
  if (typeof hwStopLivePolling === "function") hwStopLivePolling();
  if (typeof debugDeactivate === "function") debugDeactivate();
  document.dispatchEvent(new CustomEvent("ide-runtime-disconnected"));
}

function onlineNotifyConnected(transition) {
  if (transition === "online") return;
  document.dispatchEvent(new CustomEvent("ide-runtime-connected", {
    detail: {
      target: onlineState.target,
      remote: onlineState.targetIsRemote,
    },
  }));
}

// ── Runtime API Bridge ─────────────────────────────────

async function runtimeControlRequest(controlRequest, options = {}) {
  const timeoutMs = Number(options.timeoutMs) || 5000;
  const hasTargetOverride = Object.prototype.hasOwnProperty.call(options, "target");
  const target = hasTargetOverride
    ? onlineNormalizeTarget(options.target)
    : onlineNormalizeTarget(onlineState.target);
  const remote = target ? onlineIsRemoteTarget(target) : false;
  const credentials = options.credentials || onlineConnectionCredentials();

  if (remote) {
    return await apiJson("/api/control/proxy", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify({
        target,
        control_request: controlRequest,
        auth_basic: credentials || undefined,
      }),
      timeoutMs,
    });
  }

  return await apiJson("/api/control", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify(controlRequest),
    timeoutMs,
  });
}

async function runtimeDeployRequest(deployRequest, options = {}) {
  const timeoutMs = Number(options.timeoutMs) || 30000;
  const hasTargetOverride = Object.prototype.hasOwnProperty.call(options, "target");
  const target = hasTargetOverride
    ? onlineNormalizeTarget(options.target)
    : onlineNormalizeTarget(onlineState.target);
  const remote = target ? onlineIsRemoteTarget(target) : false;
  const credentials = options.credentials || onlineConnectionCredentials();

  if (remote) {
    return await apiJson("/api/deploy/proxy", {
      method: "POST",
      headers: apiHeaders(),
      body: JSON.stringify({
        target,
        deploy_request: deployRequest,
        auth_basic: credentials || undefined,
      }),
      timeoutMs,
    });
  }

  return await apiJson("/api/deploy", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify(deployRequest),
    timeoutMs,
  });
}

// ── Connection Dialog ──────────────────────────────────

function openConnectionDialog() {
  const dialog = el.connectionDialog;
  if (!dialog) return;
  dialog.classList.add("open");
  if (el.connectUsername) el.connectUsername.value = onlineState.authUsername || "";
  if (el.connectPassword) el.connectPassword.value = onlineState.authPassword || "";
  onlineStartDiscovery();
  onlineRenderRecent();
  onlineRenderDialogState();
  if (el.connectAddress) el.connectAddress.focus();
}

function closeConnectionDialog() {
  const dialog = el.connectionDialog;
  if (!dialog) return;
  dialog.classList.remove("open");
  onlineStopDiscovery();
}

function onlineRenderDialogState() {
  if (!el.connectStatus) return;

  if (onlineState.connected) {
    if (el.connectAuthFields) {
      el.connectAuthFields.hidden = true;
    }
    const target = onlineTargetDisplay(onlineState.target || onlineState.address || "");
    el.connectStatus.innerHTML = `<div class="online-connected-info">
      <span class="online-connected-dot"></span>
      Connected to <strong>${escapeHtml(onlineState.runtimeName || target)}</strong>
      <span class="muted" style="margin-left:6px">${escapeHtml(target)}</span>
      <button type="button" class="btn ghost" id="disconnectBtn" style="margin-left:8px;color:var(--danger)">Disconnect</button>
    </div>`;
    const dcBtn = document.getElementById("disconnectBtn");
    if (dcBtn) dcBtn.addEventListener("click", onlineDisconnect);
    return;
  }

  const showCredentials = !!onlineState.authRequired;
  if (el.connectAuthFields) {
    el.connectAuthFields.hidden = !showCredentials;
  }

  if (!onlineState.dialogError) {
    el.connectStatus.innerHTML = "";
    return;
  }

  const level = showCredentials ? "warn" : "error";
  el.connectStatus.innerHTML = `<div class="status ${level}" style="margin-bottom:8px">${escapeHtml(onlineState.dialogError)}</div>`;
}

// ── Discovery Scan ─────────────────────────────────────

function onlineStartDiscovery() {
  onlineState.scanning = true;
  onlineRenderDiscovered();
  onlineScanOnce();
  onlineState.discoveryTimer = setInterval(onlineScanOnce, ONLINE_DISCOVERY_INTERVAL_MS);
}

function onlineStopDiscovery() {
  onlineState.scanning = false;
  if (onlineState.discoveryTimer) {
    clearInterval(onlineState.discoveryTimer);
    onlineState.discoveryTimer = null;
  }
}

async function onlineScanOnce() {
  try {
    const result = await apiJson("/api/discovery", {
      method: "GET",
      timeoutMs: 3000,
    });
    const items = Array.isArray(result.items) ? result.items : [];
    onlineState.discoveredRuntimes = items.map((item) => ({
      id: item.id,
      name: item.name || item.id,
      address: Array.isArray(item.addresses) && item.addresses.length > 0
        ? `${item.addresses[0]}:${item.web_port || 18080}`
        : null,
      webPort: item.web_port || 18080,
      state: item.last_seen_ns > 0 ? "online" : "offline",
      lastSeen: item.last_seen_ns,
    }));
    onlineState.scanning = false;
    onlineRenderDiscovered();
  } catch {
    onlineState.scanning = false;
    onlineRenderDiscovered();
  }
}

function onlineRenderDiscovered() {
  const container = el.discoveredRuntimes;
  if (!container) return;

  if (onlineState.scanning && onlineState.discoveredRuntimes.length === 0) {
    container.innerHTML = '<div class="muted" style="padding:8px;font-size:12px">Scanning for runtimes...</div>';
    return;
  }

  if (onlineState.discoveredRuntimes.length === 0) {
    container.innerHTML = `<div style="padding:8px;font-size:12px">
      <p class="muted">No runtimes found. Check that the runtime is running and on the same network.</p>
      <button type="button" class="btn ghost" onclick="onlineScanOnce()">Scan Again</button>
    </div>`;
    return;
  }

  let html = "";
  for (const rt of onlineState.discoveredRuntimes) {
    const stateClass = rt.state === "online" ? "online-rt-online" : "online-rt-offline";
    const stateIcon = rt.state === "online" ? "\u25CF" : "\u25CB";
    html += `<div class="online-rt-entry ${stateClass}">
      <div class="online-rt-info">
        <span class="online-rt-icon">${stateIcon}</span>
        <div>
          <strong>${escapeHtml(rt.name)}</strong>
          <span class="muted" style="font-size:11px">${escapeHtml(rt.address || "unknown")}</span>
        </div>
      </div>
      <button type="button" class="btn ghost" data-connect-addr="${escapeAttr(rt.address || "")}" data-connect-name="${escapeAttr(rt.name)}">Connect</button>
    </div>`;
  }
  container.innerHTML = html;

  container.querySelectorAll("[data-connect-addr]").forEach((btn) => {
    btn.addEventListener("click", () => {
      onlineConnect(btn.dataset.connectAddr, btn.dataset.connectName);
    });
  });
}

function onlineStartStatusPolling() {
  onlineStopStatusPolling();
  onlineState.statusTimer = setInterval(() => {
    void onlinePollStatus();
  }, ONLINE_STATUS_POLL_INTERVAL_MS);
}

function onlineStopStatusPolling() {
  if (onlineState.statusTimer) {
    clearInterval(onlineState.statusTimer);
    onlineState.statusTimer = null;
  }
}

async function onlinePollStatus() {
  const target = onlineNormalizeTarget(onlineState.target);
  if (!target) return;
  const previous = onlineState.connectionState;
  const startedAt = performance.now();
  try {
    const status = await runtimeControlRequest(
      { id: 1, type: "status" },
      {
        target: onlineState.targetIsRemote ? target : null,
        timeoutMs: 2500,
        credentials: onlineConnectionCredentials(),
      },
    );
    onlineState.connected = true;
    onlineState.connectionState = "online";
    onlineState.reconnectStartedMs = 0;
    onlineState.reconnectError = "";
    onlineState.runtimeStatus = status?.state || "online";
    onlineState.cycleMs = onlineCycleMsFromStatus(status);
    onlineState.latencyMs = Math.max(0, Math.round(performance.now() - startedAt));
    onlineUpdateStatusBar();
    onlineNotifyConnected(previous);
  } catch (err) {
    const now = Date.now();
    if (!onlineState.reconnectStartedMs) {
      onlineState.reconnectStartedMs = now;
    }
    const elapsed = now - onlineState.reconnectStartedMs;
    onlineState.connected = false;
    onlineState.reconnectError = String(err?.message || err || "connection failed");
    if (elapsed >= ONLINE_CONNECTION_LOST_MS) {
      onlineState.connectionState = "lost";
      onlineStopStatusPolling();
    } else {
      onlineState.connectionState = "reconnecting";
    }
    onlineUpdateStatusBar();
    onlineNotifyDisconnected(previous);
  }
}

// ── Recent Connections ─────────────────────────────────

function onlineGetRecent() {
  try {
    const raw = localStorage.getItem(ONLINE_RECENT_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function onlineSaveRecent(address, name) {
  const recent = onlineGetRecent().filter((r) => r.address !== address);
  recent.unshift({ address, name, ts: Date.now() });
  if (recent.length > ONLINE_MAX_RECENT) recent.length = ONLINE_MAX_RECENT;
  try {
    localStorage.setItem(ONLINE_RECENT_KEY, JSON.stringify(recent));
  } catch {
    // Ignore storage errors
  }
}

function onlineRenderRecent() {
  const container = el.recentConnections;
  if (!container) return;
  const recent = onlineGetRecent();
  if (recent.length === 0) {
    container.innerHTML = '<div class="muted" style="padding:4px;font-size:11px">No recent connections.</div>';
    return;
  }
  let html = "";
  for (const entry of recent) {
    const ago = onlineTimeAgo(entry.ts);
    html += `<button type="button" class="online-recent-entry" data-connect-addr="${escapeAttr(entry.address)}" data-connect-name="${escapeAttr(entry.name || "")}">
      <span>${escapeHtml(entry.address)}</span>
      <span class="muted" style="font-size:10px">${entry.name ? `(${escapeHtml(entry.name)}) ` : ""}${ago}</span>
    </button>`;
  }
  container.innerHTML = html;
  container.querySelectorAll("[data-connect-addr]").forEach((btn) => {
    btn.addEventListener("click", () => {
      onlineConnect(btn.dataset.connectAddr, btn.dataset.connectName);
    });
  });
}

function onlineTimeAgo(ts) {
  const diff = Date.now() - ts;
  if (diff < 60000) return "just now";
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
  return `${Math.floor(diff / 86400000)}d ago`;
}

// ── Connect / Disconnect ───────────────────────────────

async function onlineProbeTarget(target, credentials) {
  let probeUrl = `/api/probe?url=${encodeURIComponent(target)}`;
  const username = String(credentials?.username || "").trim();
  if (username) {
    probeUrl += `&username=${encodeURIComponent(username)}`;
    probeUrl += `&password=${encodeURIComponent(String(credentials?.password || ""))}`;
  }
  return await apiJson(probeUrl, {
    method: "GET",
    timeoutMs: 2500,
  });
}

async function onlineConnect(address, name, options = {}) {
  const silent = !!options.silent;
  const target = onlineNormalizeTarget(address);
  if (!target) {
    if (!silent && typeof showIdeToast === "function") {
      showIdeToast("Invalid runtime address.", "error");
    }
    return false;
  }

  onlineState.dialogError = "";
  if (!silent) {
    onlineRenderDialogState();
  }

  if (!silent && el.connectBtn) {
    el.connectBtn.disabled = true;
    el.connectBtn.textContent = "Connecting...";
  }

  try {
    const targetIsRemote = onlineIsRemoteTarget(target);
    const credentials = onlineReadAuthCredentialsFromDialog();
    const probe = targetIsRemote
      ? await onlineProbeTarget(target, credentials)
      : { ok: true };
    const status = await runtimeControlRequest(
      { id: 1, type: "status" },
      { target: targetIsRemote ? target : null, timeoutMs: 5000, credentials },
    );

    const resolvedName = name
      || status?.plc_name
      || status?.resource
      || probe?.name
      || onlineTargetDisplay(target);

    const previous = onlineState.connectionState;
    onlineState.connected = true;
    onlineState.connectionState = "online";
    onlineState.address = onlineTargetDisplay(target);
    onlineState.target = target;
    onlineState.targetIsRemote = targetIsRemote;
    onlineState.authRequired = false;
    onlineState.authUsername = String(credentials?.username || "");
    onlineState.authPassword = String(credentials?.password || "");
    onlineState.runtimeName = resolvedName;
    onlineState.runtimeStatus = status?.state || probe?.state || "online";
    onlineState.cycleMs = onlineCycleMsFromStatus(status);
    onlineState.latencyMs = null;
    onlineState.reconnectStartedMs = 0;
    onlineState.reconnectError = "";
    onlineState.dialogError = "";

    onlineSaveRecent(onlineState.address, resolvedName);
    onlineStartStatusPolling();
    onlineUpdateStatusBar();
    if (!silent) {
      onlineRenderDialogState();
      closeConnectionDialog();
    }

    if (!silent && typeof showIdeToast === "function") {
      showIdeToast(`Connected to ${resolvedName}`, "success");
    }
    setStatus(`Connected to ${resolvedName}`);

    onlineUpdateSyncStatus();
    onlineNotifyConnected(previous);
    return true;
  } catch (err) {
    if (silent) {
      return false;
    }
    const rawMessage = String(err?.message || err || "connection failed");
    const authRequired = rawMessage === "auth_required"
      || rawMessage.toLowerCase().includes("unauthorized")
      || rawMessage.includes("401");
    const message = authRequired
      ? "Auth required. Enter username/password and retry."
      : rawMessage;
    onlineState.authRequired = authRequired;
    onlineState.dialogError = message;
    onlineRenderDialogState();
    if (authRequired && el.connectUsername) {
      el.connectUsername.focus();
    }
    if (typeof showIdeToast === "function") {
      showIdeToast(`Connection failed: ${message}`, "error");
    }
    return false;
  } finally {
    if (!silent && el.connectBtn) {
      el.connectBtn.disabled = false;
      el.connectBtn.textContent = "Connect";
    }
  }
}

function onlineDisconnect() {
  const previous = onlineState.connectionState;
  onlineState.connected = false;
  onlineState.connectionState = "offline";
  onlineState.address = null;
  onlineState.target = null;
  onlineState.targetIsRemote = false;
  onlineState.authRequired = false;
  onlineState.authUsername = "";
  onlineState.authPassword = "";
  onlineState.runtimeName = null;
  onlineState.runtimeStatus = null;
  onlineState.cycleMs = null;
  onlineState.latencyMs = null;
  onlineState.reconnectStartedMs = 0;
  onlineState.reconnectError = "";
  onlineState.dialogError = "";
  onlineState.syncStatus = "not-deployed";
  onlineState.deployedHash = null;
  onlineState.builtHash = null;
  onlineState.deployedFileHashes = {};
  onlineState.changedFiles = [];
  onlineState.syncPopoverOpen = false;
  onlineStopStatusPolling();

  onlineNotifyDisconnected(previous);

  onlineUpdateStatusBar();
  onlineRenderDialogState();

  if (typeof showIdeToast === "function") {
    showIdeToast("Disconnected.", "warn");
  }
  setStatus("Disconnected from runtime.");
}

function onlineUpdateStatusBar() {
  const stateLabel = String(onlineState.connectionState || "offline");
  const displayTarget = onlineState.address || onlineTargetDisplay(onlineState.target || "");
  if (el.connectionPill) {
    el.connectionPill.dataset.state = stateLabel;
  }
  if (el.connectionPillText) {
    if (stateLabel === "online") {
      const suffix = displayTarget ? ` ${displayTarget}` : "";
      el.connectionPillText.textContent = `ONLINE${suffix}`;
    } else if (stateLabel === "reconnecting") {
      el.connectionPillText.textContent = "RECONNECTING...";
    } else if (stateLabel === "lost") {
      el.connectionPillText.textContent = "CONNECTION LOST";
    } else {
      el.connectionPillText.textContent = "OFFLINE";
    }
  }
  if (el.runtimeState) {
    if (stateLabel === "online") {
      el.runtimeState.textContent = String(onlineState.runtimeStatus || "running").toUpperCase();
    } else {
      el.runtimeState.textContent = "";
    }
  }
  if (el.statusText) {
    if (stateLabel === "online") {
      const cycleText = onlineState.cycleMs != null ? ` | Cycle: ${onlineState.cycleMs}ms` : "";
      el.statusText.textContent = `${onlineState.runtimeStatus || "running"}${cycleText}`;
    } else if (stateLabel === "reconnecting") {
      el.statusText.textContent = `Connection lost to ${displayTarget || "runtime"}`;
    } else if (stateLabel === "lost") {
      el.statusText.textContent = `Connection lost to ${displayTarget || "runtime"}`;
    } else {
      el.statusText.textContent = "No runtime connected";
    }
  }
  if (el.statusLatency) {
    if (stateLabel === "online" && Number.isFinite(onlineState.latencyMs)) {
      el.statusLatency.textContent = `lat ${onlineState.latencyMs}ms`;
      el.statusLatency.className = "ide-badge ok";
    } else {
      el.statusLatency.textContent = "latency --";
      el.statusLatency.className = "ide-badge";
    }
  }
  if (el.connectionRetryBtn) {
    const showRetry = stateLabel === "lost";
    el.connectionRetryBtn.hidden = !showRetry;
  }
  onlineUpdateDeployButtonState();
}

function onlineUpdateDeployButtonState() {
  if (!el.deployBtn) return;
  const canDeploy = onlineState.connectionState === "online";
  el.deployBtn.disabled = !canDeploy;
  el.deployBtn.title = canDeploy
    ? "Deploy to runtime"
    : "Connect to a runtime first";
}

async function onlineRetryConnection() {
  if (onlineState.connectionState !== "lost" && onlineState.connectionState !== "reconnecting") {
    return;
  }
  if (!onlineState.target) {
    openConnectionDialog();
    return;
  }
  onlineState.connectionState = "reconnecting";
  onlineState.reconnectStartedMs = Date.now();
  onlineUpdateStatusBar();
  await onlinePollStatus();
  if (onlineState.connectionState === "online") {
    onlineStartStatusPolling();
  }
  if (onlineState.connectionState === "online" && typeof showIdeToast === "function") {
    showIdeToast("Reconnected.", "success");
  }
}

// ── Deploy Flow (US-5.2) ──────────────────────────────

function onlineProjectSummary() {
  const projectName = (typeof state !== "undefined" && state.activeProject)
    ? state.activeProject
    : "project";
  const pouCount = Array.isArray(state?.files)
    ? state.files.filter((path) => String(path).toLowerCase().endsWith(".st")).length
    : 0;
  return { projectName, pouCount };
}

function onlineParseBundleSizeText(output) {
  const text = String(output || "");
  const match = text.match(/(\d+(?:\.\d+)?)\s*(?:KB|kB|bytes?|B)\b/i);
  return match ? match[0] : "unknown";
}

async function onlineRunBuildTaskForDeploy() {
  const task = await apiJson("/api/ide/build", {
    method: "POST",
    headers: apiHeaders(),
    body: "{}",
    timeoutMs: 3000,
  });
  let current = task;
  const startedAt = Date.now();
  while (current && current.status !== "completed") {
    if (Date.now() - startedAt > 120000) {
      throw new Error("Build timed out");
    }
    await onlineSleep(400);
    current = await apiJson(`/api/ide/task?id=${task.job_id}`, {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 4000,
    });
  }
  if (!current?.success) {
    const output = String(current?.output || "Build failed");
    throw new Error(output.split("\n").slice(-3).join(" ").trim() || "Build failed");
  }
  const localHash = onlineComputeLocalHash();
  onlineState.builtHash = localHash;
  return {
    output: String(current.output || ""),
    sizeText: onlineParseBundleSizeText(current.output || ""),
    localHash,
  };
}

async function onlineDeployFlow() {
  if (onlineState.connectionState !== "online") {
    if (typeof showIdeToast === "function") showIdeToast("Connect to a runtime first.", "warn");
    return;
  }

  const errorCount = (state?.diagnostics || []).filter((d) =>
    String(d.severity || "").toLowerCase().includes("error")
  ).length;
  if (errorCount > 0) {
    if (typeof showIdeToast === "function") showIdeToast(`Fix ${errorCount} error(s) before deploying.`, "error");
    return;
  }

  const summary = onlineProjectSummary();
  const projectName = summary.projectName;
  const targetName = onlineState.runtimeName || onlineState.address;
  const currentHash = onlineComputeLocalHash();
  let buildInfo = null;

  if (onlineState.builtHash !== currentHash) {
    setStatus("Build required before deploy...");
    try {
      buildInfo = await onlineRunBuildTaskForDeploy();
    } catch (err) {
      const msg = `Build failed: ${err.message || err}`;
      setStatus(msg);
      if (typeof showIdeToast === "function") showIdeToast(msg, "error");
      return;
    }
  } else {
    buildInfo = {
      output: "",
      sizeText: "unknown",
      localHash: currentHash,
    };
  }

  const proceed = await ideConfirm(
    "Deploy Project",
    `Deploy ${projectName} to ${targetName}?\nBundle: ${buildInfo.sizeText}, ${summary.pouCount} POUs\nRuntime is currently ${onlineState.runtimeStatus || "running"}. Program will restart.`
  );
  if (!proceed) return;

  setStatus("Uploading bundle...");
  if (typeof showIdeToast === "function") showIdeToast("Deploying project...", "success");

  try {
    const sources = [];
    if (Array.isArray(state?.files)) {
      for (const filePath of state.files) {
        const normalizedPath = String(filePath || "").trim();
        if (!normalizedPath || !normalizedPath.toLowerCase().endsWith(".st")) {
          continue;
        }
        try {
          const content = await apiJson(`/api/ide/file?path=${encodeURIComponent(normalizedPath)}`, {
            method: "GET",
            headers: apiHeaders(),
            timeoutMs: 3000,
          });
          if (content && content.content != null) {
            sources.push({ path: normalizedPath, content: content.content });
          }
        } catch {
          // Skip unreadable files
        }
      }
    }

    const deployResult = await runtimeDeployRequest({
      sources,
      restart: "cold",
    }, {
      credentials: onlineConnectionCredentials(),
    });

    if (deployResult && deployResult.ok) {
      onlineState.deployedFileHashes = onlineHashesFromSources(sources);
      onlineState.deployedHash = onlineHashFileMap(onlineState.deployedFileHashes);
      onlineState.localHash = onlineComputeLocalHash();
      onlineUpdateSyncStatus();
      if (typeof showIdeToast === "function") showIdeToast("Deployed successfully. Program running.", "success");
      setStatus("Deployed successfully.");
      return;
    }

    const err = deployResult?.error || "Unknown deploy error";
    if (typeof showIdeToast === "function") showIdeToast(`Deploy failed: ${err}`, "error");
    setStatus(`Deploy failed: ${err}`);
  } catch (err) {
    if (typeof showIdeToast === "function") showIdeToast(`Deploy failed: ${err.message || err}`, "error");
    setStatus(`Deploy error: ${err.message || err}`);
  }
}

// ── Sync Status (US-5.3) ──────────────────────────────

function onlineHashText(input) {
  const text = String(input || "");
  let hash = 0;
  for (let i = 0; i < text.length; i++) {
    hash = ((hash << 5) - hash + text.charCodeAt(i)) | 0;
  }
  return String(hash >>> 0);
}

function onlineHashFileMap(fileHashes) {
  const map = fileHashes && typeof fileHashes === "object" ? fileHashes : {};
  const keys = Object.keys(map).sort((a, b) => a.localeCompare(b));
  let acc = "";
  for (const key of keys) {
    acc += `${key}:${map[key]};`;
  }
  return onlineHashText(acc);
}

function onlineCurrentOpenFileHashes() {
  const hashes = {};
  if (typeof state !== "object" || !state || !(state.openTabs instanceof Map)) {
    return hashes;
  }
  for (const [path, tab] of state.openTabs.entries()) {
    if (!path || !tab) continue;
    let content = tab.content;
    if (state.activePath === path && state.editorView && typeof state.editorView.getValue === "function") {
      content = state.editorView.getValue();
    }
    hashes[path] = onlineHashText(content);
  }
  return hashes;
}

function onlineHashesFromSources(sources) {
  const hashes = {};
  if (!Array.isArray(sources)) return hashes;
  for (const source of sources) {
    const path = String(source?.path || "").trim();
    if (!path) continue;
    hashes[path] = onlineHashText(source?.content ?? "");
  }
  return hashes;
}

function onlineComputeChangedFiles(currentHashes) {
  const current = currentHashes && typeof currentHashes === "object" ? currentHashes : {};
  const deployed = onlineState.deployedFileHashes && typeof onlineState.deployedFileHashes === "object"
    ? onlineState.deployedFileHashes
    : {};
  const changed = new Set();
  for (const [path, hash] of Object.entries(current)) {
    if (deployed[path] !== hash) changed.add(path);
  }
  if (typeof state === "object" && state && state.openTabs instanceof Map) {
    for (const [path, tab] of state.openTabs.entries()) {
      if (tab?.dirty) changed.add(path);
    }
  }
  return Array.from(changed).sort((a, b) => a.localeCompare(b));
}

function onlineComputeLocalHash() {
  return onlineHashFileMap(onlineCurrentOpenFileHashes());
}

function onlineUpdateSyncStatus() {
  if (!onlineState.connected) {
    onlineState.syncStatus = "not-deployed";
    onlineState.changedFiles = [];
    onlineUpdateSyncBadge();
    return;
  }
  const currentHashes = onlineCurrentOpenFileHashes();
  const currentHash = onlineHashFileMap(currentHashes);
  onlineState.localHash = currentHash;
  if (!onlineState.deployedHash) {
    onlineState.syncStatus = "not-deployed";
    onlineState.changedFiles = [];
  } else {
    const changedFiles = onlineComputeChangedFiles(currentHashes);
    onlineState.changedFiles = changedFiles;
    onlineState.syncStatus = changedFiles.length > 0 ? "modified" : "in-sync";
  }
  onlineUpdateSyncBadge();
}

function onlineSyncLabel() {
  if (onlineState.syncStatus === "in-sync") return "In sync";
  if (onlineState.syncStatus === "modified") return "Modified";
  return "Not deployed";
}

function onlineSyncClass() {
  if (onlineState.syncStatus === "in-sync") return "sync-ok";
  if (onlineState.syncStatus === "modified") return "sync-modified";
  return "sync-none";
}

function onlineCloseSyncPopover() {
  onlineState.syncPopoverOpen = false;
  if (el.headerSyncPopover) el.headerSyncPopover.hidden = true;
}

function onlineRenderSyncPopover() {
  const popover = el.headerSyncPopover;
  if (!popover) return;
  if (!onlineState.connected || !onlineState.syncPopoverOpen) {
    popover.hidden = true;
    return;
  }
  const syncLabel = onlineSyncLabel();
  const className = onlineSyncClass();
  let html = `<div class="ide-sync-popover-head"><span class="online-sync-badge ${className}">${escapeHtml(syncLabel)}</span></div>`;
  if (onlineState.syncStatus === "in-sync") {
    html += '<p class="muted">Runtime matches local project.</p>';
  } else if (onlineState.syncStatus === "modified") {
    const changed = onlineState.changedFiles.slice(0, 12);
    html += `<p class="muted">${changed.length} changed file(s) since last deploy.</p>`;
    html += '<ul class="ide-sync-popover-list">';
    for (const filePath of changed) {
      html += `<li><button type="button" data-sync-open-file="${escapeAttr(filePath)}">${escapeHtml(filePath)}</button></li>`;
    }
    html += "</ul>";
  } else {
    html += '<p class="muted">Project has not been deployed to this runtime.</p>';
  }
  html += `<div class="ide-sync-popover-actions"><button type="button" class="btn secondary" id="syncPopoverDeployBtn">Deploy Now</button></div>`;
  popover.innerHTML = html;
  popover.hidden = false;

  const deployBtn = document.getElementById("syncPopoverDeployBtn");
  if (deployBtn) {
    deployBtn.disabled = onlineState.connectionState !== "online";
    deployBtn.addEventListener("click", () => {
      onlineCloseSyncPopover();
      void onlineDeployFlow();
    });
  }
  popover.querySelectorAll("[data-sync-open-file]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const path = String(btn.dataset.syncOpenFile || "").trim();
      if (!path) return;
      if (typeof switchIdeTab === "function") {
        switchIdeTab("code");
      }
      if (typeof openFile === "function") {
        void openFile(path);
      }
      onlineCloseSyncPopover();
    });
  });
}

function onlineToggleSyncPopover() {
  if (!onlineState.connected) return;
  onlineState.syncPopoverOpen = !onlineState.syncPopoverOpen;
  onlineRenderSyncPopover();
}

function onlineUpdateSyncBadge() {
  const statusBadge = el.syncBadge;
  const headerBadge = el.headerSyncBadge;
  const syncLabel = onlineSyncLabel();
  const className = onlineSyncClass();

  if (!onlineState.connected) {
    if (statusBadge) statusBadge.style.display = "none";
    if (headerBadge) headerBadge.hidden = true;
    onlineCloseSyncPopover();
    return;
  }

  if (statusBadge) {
    statusBadge.style.display = "";
    statusBadge.textContent = syncLabel;
    statusBadge.className = `online-sync-badge ${className}`;
  }
  if (headerBadge) {
    const changedCount = onlineState.syncStatus === "modified" ? onlineState.changedFiles.length : 0;
    headerBadge.hidden = false;
    headerBadge.textContent = changedCount > 0 ? `${syncLabel} (${changedCount})` : syncLabel;
    headerBadge.className = `online-sync-badge ide-header-sync-badge ${className}`;
  }

  if (onlineState.syncPopoverOpen) {
    onlineRenderSyncPopover();
  }
}

// ── Init ───────────────────────────────────────────────

function onlineInit() {
  onlineSeedConnectionDefaults();

  if (el.connectionPill) {
    el.connectionPill.addEventListener("click", openConnectionDialog);
  }
  if (el.connectBtn) {
    el.connectBtn.addEventListener("click", () => {
      const addr = (el.connectAddress && el.connectAddress.value.trim()) || "";
      const port = (el.connectPort && el.connectPort.value.trim()) || onlineDefaultConnectPort();
      if (addr) {
        const withPort = onlineBuildConnectTarget(addr, port);
        if (withPort) {
          void onlineConnect(withPort);
        }
      }
    });
  }
  if (el.connectAddress) {
    el.connectAddress.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        if (el.connectBtn) el.connectBtn.click();
      }
    });
  }
  if (el.connectUsername) {
    el.connectUsername.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        if (el.connectBtn) el.connectBtn.click();
      }
    });
  }
  if (el.connectPassword) {
    el.connectPassword.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        if (el.connectBtn) el.connectBtn.click();
      }
    });
  }
  if (el.connectionDialogClose) {
    el.connectionDialogClose.addEventListener("click", closeConnectionDialog);
  }
  if (el.connectionRetryBtn) {
    el.connectionRetryBtn.addEventListener("click", () => {
      void onlineRetryConnection();
    });
  }
  if (el.deployBtn) {
    el.deployBtn.addEventListener("click", onlineDeployFlow);
  }
  if (el.headerSyncBadge) {
    el.headerSyncBadge.addEventListener("click", () => {
      onlineToggleSyncPopover();
    });
  }

  document.addEventListener("click", (event) => {
    if (!onlineState.syncPopoverOpen) return;
    const target = event?.target;
    if (!target) return;
    if (el.headerSyncBadge && el.headerSyncBadge.contains(target)) return;
    if (el.headerSyncPopover && el.headerSyncPopover.contains(target)) return;
    onlineCloseSyncPopover();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && onlineState.syncPopoverOpen) {
      onlineCloseSyncPopover();
    }
  });

  document.addEventListener("ide-file-saved", () => {
    if (onlineState.connected) onlineUpdateSyncStatus();
  });
  document.addEventListener("ide-tab-dirty-change", () => {
    if (onlineState.connected) onlineUpdateSyncStatus();
  });
  document.addEventListener("ide-active-path-change", () => {
    if (onlineState.connected) onlineUpdateSyncStatus();
  });
  document.addEventListener("ide-project-changed", () => {
    onlineState.syncPopoverOpen = false;
    onlineState.deployedHash = null;
    onlineState.builtHash = null;
    onlineState.deployedFileHashes = {};
    onlineState.changedFiles = [];
    onlineState.localHash = null;
    onlineState.syncStatus = "not-deployed";
    onlineUpdateSyncStatus();
  });
  document.addEventListener("ide-task-complete", (event) => {
    const detail = event?.detail || {};
    if (detail.kind !== "build" || !detail.success) return;
    onlineState.builtHash = onlineComputeLocalHash();
    onlineUpdateSyncStatus();
    onlineUpdateDeployButtonState();
  });

  onlineUpdateStatusBar();
  onlineUpdateSyncBadge();
  onlineRenderDialogState();

  // In standard runtime mode, IDE and control endpoints share origin.
  // Auto-connecting here removes manual connect friction and unlocks Deploy.
  const addr = (el.connectAddress && el.connectAddress.value.trim()) || onlineDefaultConnectAddress();
  const port = (el.connectPort && el.connectPort.value.trim()) || onlineDefaultConnectPort();
  const withPort = onlineBuildConnectTarget(addr, port);
  if (withPort) {
    void onlineConnect(withPort, null, { silent: true });
  }
}

document.addEventListener("DOMContentLoaded", () => {
  // Defer init until the main IDE script has set up el.
  setTimeout(onlineInit, 0);
});
