import * as vscode from "vscode";

export function trustHomeWebviewHtml(
  webview: vscode.Webview,
  extensionUri: vscode.Uri
): string {
    const nonce = makeNonce();
    const themeUri = webview.asWebviewUri(
      vscode.Uri.joinPath(extensionUri, "src", "webview", "theme.css")
    );
    const codiconsUri = webview.asWebviewUri(
      vscode.Uri.joinPath(
        extensionUri,
        "node_modules",
        "@vscode",
        "codicons",
        "dist",
        "codicon.css"
      )
    );
    const csp = `default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; font-src ${webview.cspSource}; script-src 'nonce-${nonce}';`;
    return `<!DOCTYPE html>
	<html lang="en">
	<head>
	<meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
	<meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <link rel="stylesheet" href="${themeUri}" />
  <link rel="stylesheet" href="${codiconsUri}" />
	<style>
	  * { box-sizing: border-box; }
	  body {
      margin: 0;
      padding: 10px 11px;
      font-family: var(--vscode-font-family);
      color: var(--trust-text);
      background: var(--vscode-sideBar-background, var(--trust-canvas));
    }
    .top {
      border-bottom: 1px solid var(--trust-border);
      padding-bottom: 10px;
      margin-bottom: 8px;
    }
    .project-name {
      color: var(--trust-text);
      font-size: 13px;
      font-weight: 700;
      line-height: 1.25;
      margin-bottom: 8px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .project-identity {
      align-items: center;
      display: flex;
      gap: 6px;
      margin-bottom: 8px;
      min-width: 0;
    }
    .project-identity__icon {
      color: var(--trust-text-muted);
      flex: none;
      font-size: 14px;
    }
    .project-identity .project-name {
      font-weight: 600;
      margin-bottom: 0;
      min-width: 0;
    }
    .hint {
      color: var(--trust-text-muted);
      font-size: 11.5px;
      line-height: 1.4;
      margin: 0 0 9px;
    }
    button {
      font-family: var(--vscode-font-family);
    }
    .primary-start,
    .secondary-start,
    .target-button,
    .action-button,
    .update-button,
    .nav-button {
      border-radius: var(--trust-radius);
      cursor: pointer;
      min-width: 0;
      transition: background var(--trust-ease), border-color var(--trust-ease), color var(--trust-ease);
    }
    .primary-start,
    .secondary-start {
      align-items: center;
      display: flex;
      justify-content: center;
      min-height: 31px;
      width: 100%;
      margin-top: 7px;
      padding: 7px 9px;
      font-size: 12px;
      font-weight: 650;
    }
    .primary-start {
      background: var(--trust-action-primary-bg);
      border: 1px solid var(--trust-action-primary-bg);
      color: var(--trust-action-primary-fg);
    }
    .secondary-start {
      background: transparent;
      border: 1px solid var(--trust-border);
      color: var(--trust-text);
    }
    .target-label {
      color: var(--trust-text-muted);
      font-size: 10px;
      font-weight: 750;
      letter-spacing: 0.5px;
      margin: 0 0 4px;
      text-transform: uppercase;
    }
    .target-button {
      align-items: center;
      background: var(--trust-surface);
      border: 1px solid var(--trust-border);
      color: var(--trust-text);
      display: flex;
      gap: 7px;
      justify-content: space-between;
      min-height: 30px;
      padding: 6px 8px;
      width: 100%;
    }
    .target-button .value {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .target-button:disabled {
      background: var(--trust-surface);
      border-color: var(--trust-border);
    }
    .action-row {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 5px;
      margin-top: 8px;
    }
    .action-button {
      align-items: center;
      background: transparent;
      border: 1px solid var(--trust-border);
      color: var(--trust-text);
      display: inline-flex;
      flex-direction: column;
      gap: 2px;
      justify-content: center;
      min-height: 42px;
      padding: 5px 3px;
    }
    .action-button .icon {
      font-size: 14px;
      line-height: 1;
    }
    .action-button .codicon {
      font-size: 15px;
    }
    .action-button .label {
      font-size: 10.5px;
      line-height: 1.1;
      max-width: 100%;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .action-button.label-clipped:not(#action) .label { display: none; }
    #action .label { display: inline; }
    .action-button[data-variant="filled"] {
      background: var(--trust-action-primary-bg);
      border-color: var(--trust-action-primary-bg);
      color: var(--trust-action-primary-fg);
      font-weight: 700;
    }
    .action-button[data-tone="success"] {
      border-color: color-mix(in srgb, var(--trust-ok) 55%, var(--trust-border));
      color: var(--trust-ok);
    }
    .action-button[data-tone="danger"] {
      border-color: color-mix(in srgb, var(--trust-danger) 58%, var(--trust-border));
      color: var(--trust-danger);
    }
    .action-button[data-tone="warning"] {
      border-color: color-mix(in srgb, var(--trust-warn) 58%, var(--trust-border));
      color: var(--trust-warn);
    }
    .action-button[data-tone="disabled"] {
      color: var(--trust-text-subtle);
    }
    button:hover:not(:disabled) {
      background: var(--trust-selected-bg);
      border-color: var(--trust-accent);
    }
    .action-button[data-variant="filled"]:hover:not(:disabled),
    .primary-start:hover:not(:disabled) {
      background: var(--trust-action-primary-hover-bg);
      border-color: var(--trust-action-primary-hover-bg);
      color: var(--trust-action-primary-fg);
    }
    button:disabled {
      color: var(--trust-text-subtle);
      cursor: not-allowed;
      opacity: 0.62;
    }
    .update-button {
      background: var(--trust-action-primary-bg);
      border: 1px solid var(--trust-action-primary-bg);
      color: var(--trust-action-primary-fg);
      display: none;
      font-size: 12px;
      font-weight: 650;
      margin-top: 7px;
      min-height: 30px;
      width: 100%;
    }
    .message {
      color: var(--trust-text-muted);
      display: none;
      font-size: 11px;
      line-height: 1.4;
      margin-top: 7px;
    }
    .message.success { color: var(--trust-ok); }
    .message.error { color: var(--trust-danger); }
    .recovery-button {
      background: transparent;
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      color: var(--trust-text);
      display: none;
      font-size: 11px;
      margin-top: 6px;
      min-height: 28px;
      padding: 5px 8px;
      width: 100%;
    }
    .hint-line {
      color: var(--trust-warn);
      display: none;
      font-size: 11px;
      line-height: 1.35;
      margin-top: 7px;
    }
    .nav {
      display: flex;
      flex-direction: column;
      gap: 3px;
    }
    .nav-button {
      align-items: center;
      background: transparent;
      border: 1px solid transparent;
      color: var(--trust-text);
      display: flex;
      gap: 8px;
      min-height: 31px;
      padding: 6px 7px;
      text-align: left;
      width: 100%;
    }
    .nav-button .nav-icon {
      color: var(--trust-text-muted);
      flex: 0 0 auto;
      text-align: center;
      width: 18px;
    }
    .nav-button:disabled {
      color: var(--trust-text-subtle);
      opacity: 0.72;
    }
    .nav-button:hover:not(:disabled) {
      background: var(--trust-selected-bg);
      border-color: var(--trust-border);
    }
    .disabled-reason {
      color: var(--trust-text-subtle);
      font-size: 10.5px;
      line-height: 1.25;
      margin: -1px 0 4px 26px;
    }
    @media (max-width: 245px) {
      .action-button .label { display: none; }
      #action .label { display: inline; }
      .action-button { min-height: 32px; }
    }
	  .hidden { display: none; }
	</style>
	</head>
	<body>
	  <!-- No project open: same sidebar shell, onboarding top region only. -->
	  <section id="welcome" class="hidden">
      <div class="top">
        <div class="project-name" id="welcomeTitle">No project</div>
        <p class="hint" id="welcomeText">Start with a runnable example, create a blank project, or open an existing folder.</p>
        <button id="startExample" class="primary-start">▦ Start from example</button>
        <button id="createProject" class="secondary-start">+ Create project</button>
        <button id="openProject" class="secondary-start">Open project</button>
      </div>
      <nav class="nav" aria-label="truST destinations disabled until a project is open">
        <button class="nav-button" disabled><span class="nav-icon">▤</span><span>Devices &amp; Connections</span></button>
        <div class="disabled-reason">Open a project to use this.</div>
        <button class="nav-button" disabled><span class="nav-icon">▦</span><span>Libraries</span></button>
        <div class="disabled-reason">Open a project to use this.</div>
        <button class="nav-button" disabled><span class="nav-icon">◉</span><span>Live Values</span></button>
        <div class="disabled-reason">Start a project to watch values.</div>
        <button class="nav-button" disabled><span class="nav-icon">▭</span><span>Create HMI</span></button>
        <div class="disabled-reason">Open a project to use this.</div>
      </nav>
	  </section>

	  <!-- Project open: compact action surface + visible truST destinations. -->
	  <section id="project" class="hidden">
      <div class="top">
        <div class="project-identity" title="Current truST project">
          <span class="codicon codicon-root-folder-opened project-identity__icon" aria-hidden="true"></span>
          <div class="project-name" id="projectName">Project</div>
        </div>
        <div class="target-label">Target</div>
        <button id="targetButton" class="target-button" type="button">
          <span class="value" id="targetValue">Simulator</span>
          <span aria-hidden="true">▾</span>
        </button>
        <div class="action-row" aria-label="Run controls">
          <button id="compile" class="action-button" type="button" title="Compile project">
            <span class="icon codicon codicon-tools" id="compileIcon" aria-hidden="true"></span><span class="label" id="compileLabel">Compile</span>
          </button>
          <button id="action" class="action-button" type="button" disabled title="Selected target action">
            <span class="icon codicon codicon-play" id="actionIcon" aria-hidden="true"></span><span class="label" id="actionLabel">Run</span>
          </button>
        </div>
        <button id="apply" class="update-button" type="button">Update running simulation</button>
        <div class="message" id="applyMessage"></div>
        <button class="recovery-button" id="recoveryAction" type="button"></button>
        <div class="hint-line" id="hint"></div>
      </div>

      <nav class="nav" aria-label="truST destinations">
        <button class="nav-button" id="navDevices"><span class="nav-icon">▤</span><span>Devices &amp; Connections</span></button>
        <button class="nav-button" id="navLibraries"><span class="nav-icon">▦</span><span>Libraries</span></button>
        <button class="nav-button" id="navLiveValues"><span class="nav-icon">◉</span><span>Live Values</span></button>
        <button class="nav-button" id="navHmi"><span class="nav-icon">▭</span><span id="navHmiLabel">HMI</span></button>
      </nav>
	  </section>
	<script nonce="${nonce}">
	  const vscode = acquireVsCodeApi();
	  const welcomeEl = document.getElementById("welcome");
	  const projectEl = document.getElementById("project");
	  const welcomeTitle = document.getElementById("welcomeTitle");
	  const welcomeText = document.getElementById("welcomeText");
	  const createProjectEl = document.getElementById("createProject");
    const projectNameEl = document.getElementById("projectName");
    const targetButton = document.getElementById("targetButton");
    const targetValue = document.getElementById("targetValue");
    const compileEl = document.getElementById("compile");
    const compileIcon = document.getElementById("compileIcon");
    const compileLabel = document.getElementById("compileLabel");
	  const actionEl = document.getElementById("action");
    const actionIcon = document.getElementById("actionIcon");
    const actionLabel = document.getElementById("actionLabel");
	  const applyEl = document.getElementById("apply");
	  const applyMessageEl = document.getElementById("applyMessage");
	  const recoveryActionEl = document.getElementById("recoveryAction");
	  const hintEl = document.getElementById("hint");
	  const navHmiLabel = document.getElementById("navHmiLabel");

	  function post(type) { return () => vscode.postMessage({ type }); }
    function setButton(button, icon, label, view) {
      button.disabled = !view.enabled;
      button.title = view.title;
      button.dataset.baseTitle = view.title || "";
      button.dataset.state = view.state;
      button.dataset.tone = view.tone;
      button.dataset.variant = view.variant;
      label.textContent = view.label;
      icon.className = "icon codicon " + view.icon;
    }
    // CROSS-09: collapse an action label to icon-only (with the full label in the tooltip) when it would
    // clip in its column, so "Disconnect"/"Connecting…" never render as "Disconn…".
    function measureLabelTextWidth(label) {
      const text = label.textContent || "";
      if (!text) { return 0; }
      const probe = document.createElement("span");
      const style = getComputedStyle(label);
      probe.textContent = text;
      probe.style.position = "absolute";
      probe.style.visibility = "hidden";
      probe.style.whiteSpace = "nowrap";
      probe.style.font = style.font;
      probe.style.letterSpacing = style.letterSpacing;
      document.body.appendChild(probe);
      const width = probe.getBoundingClientRect().width;
      probe.remove();
      return width;
    }
    function fitActionLabels() {
      document.querySelectorAll(".action-row .action-button").forEach((btn) => {
        const label = btn.querySelector(".label");
        if (!label) { return; }
        btn.classList.remove("label-clipped");
        const base = btn.dataset.baseTitle || "";
        // The ONE lifecycle control must always remain literal. An unexplained
        // play/stop/spinner icon recreates the exact ambiguity this surface removes.
        // Compile may collapse first when the sidebar is exceptionally narrow.
        if (btn.id === "action") {
          if (base) { btn.title = base; }
          return;
        }
        const buttonStyle = getComputedStyle(btn);
        const available = btn.clientWidth -
          parseFloat(buttonStyle.paddingLeft || "0") -
          parseFloat(buttonStyle.paddingRight || "0");
        const measuredText = measureLabelTextWidth(label);
        // Chromium can still apply text-overflow ellipsis when measured text is only
        // fractionally below the content box. Keep a small reserve so long transport
        // labels collapse before they visibly truncate in the four-column action row.
        if (measuredText > Math.max(0, available - 4)) {
          btn.classList.add("label-clipped");
          const text = label.textContent || "";
          btn.title = text ? (base && base !== text ? text + " — " + base : text) : base;
        } else if (base) {
          btn.title = base;
        }
      });
    }
    window.addEventListener("resize", fitActionLabels);
    targetButton.addEventListener("click", post("chooseTarget"));
    compileEl.addEventListener("click", post("compile"));
	  actionEl.addEventListener("click", () => { if (!actionEl.disabled) { vscode.postMessage({ type: "action" }); } });
	  applyEl.addEventListener("click", () => vscode.postMessage({ type: "applyChanges" }));
	  createProjectEl.addEventListener("click", post("createProject"));
	  document.getElementById("openProject").addEventListener("click", post("openProject"));
	  document.getElementById("startExample").addEventListener("click", post("startExample"));
	  document.getElementById("navDevices").addEventListener("click", post("navDevices"));
	  document.getElementById("navLibraries").addEventListener("click", post("navLibraries"));
	  document.getElementById("navLiveValues").addEventListener("click", post("navLiveValues"));
	  document.getElementById("navHmi").addEventListener("click", post("navHmi"));
	  recoveryActionEl.addEventListener("click", () => {
	    const action = recoveryActionEl.dataset.action;
	    if (action) vscode.postMessage({ type: action });
	  });

  window.addEventListener("message", (event) => {
    const msg = event.data;
    if (!msg || msg.type !== "state") { return; }
	    welcomeEl.classList.toggle("hidden", msg.projectOpen);
	    projectEl.classList.toggle("hidden", !msg.projectOpen);
	    if (!msg.projectOpen) {
	      if (msg.workspaceKind === "malformed") {
	        const name = msg.workspaceName ? "“" + msg.workspaceName + "”" : "This folder";
	        welcomeTitle.textContent = "Project needs repair";
	        welcomeText.textContent = name + " looks like a truST project, but its project settings file cannot be read. Fix the settings file, open another project, or start from an example.";
	        createProjectEl.textContent = "+ Create project";
	      } else if (msg.workspaceKind === "nonTrust") {
	        const name = msg.workspaceName ? "“" + msg.workspaceName + "”" : "This folder";
	        welcomeTitle.textContent = "No truST project";
	        welcomeText.textContent = name + " does not contain a truST project yet. Initialize it here, open an existing project, or start from an example.";
	        createProjectEl.textContent = "Initialize truST here";
	      } else {
	        welcomeTitle.textContent = "No project";
	        welcomeText.textContent = "Start with a runnable example, create a blank project, or open an existing folder.";
	        createProjectEl.textContent = "+ Create project";
	      }
	      return;
	    }

      projectNameEl.textContent = msg.workspaceName || "truST project";
      projectNameEl.title = msg.workspaceName || "truST project";
      targetValue.textContent = msg.selected.label + " · " + msg.selected.statusLabel;
      targetButton.disabled = !msg.targetEnabled;
      targetButton.title = msg.targetTitle || ("Target: " + msg.selected.label + " — " + msg.selected.statusLabel);
      setButton(compileEl, compileIcon, compileLabel, msg.buttons.compile);
      setButton(actionEl, actionIcon, actionLabel, msg.buttons.action);
      fitActionLabels();
	    applyEl.style.display = msg.canApply ? "block" : "none";
	    applyEl.disabled = !msg.applyEnabled;
	    applyEl.title = msg.applyTitle || "Update running simulation";
	    const applyMessage = msg.applyMessage || "";
	    applyMessageEl.textContent = applyMessage;
	    applyMessageEl.className = "message " + (msg.applyMessageKind || "");
	    applyMessageEl.style.display = applyMessage ? "block" : "none";
	    const recoveryAction = msg.recoveryAction;
	    recoveryActionEl.textContent = recoveryAction?.label || "";
	    recoveryActionEl.style.display = recoveryAction ? "block" : "none";
	    recoveryActionEl.dataset.action = recoveryAction?.action || "";
	    const hint = msg.actionHint || msg.selected.primary.hint || "";
	    hintEl.textContent = hint ? "⚠ " + hint : "";
	    hintEl.style.display = hint ? "block" : "none";
	    navHmiLabel.textContent = msg.hmiLabel || "HMI";
	  });

  vscode.postMessage({ type: "ready" });
</script>
</body>
</html>`;
}

function makeNonce(): string {
  const chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let nonce = "";
  for (let i = 0; i < 32; i += 1) {
    nonce += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return nonce;
}
