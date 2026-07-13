import {
  assert,
  fs,
  path,
  workspaceRoot,
  readSrc,
  readIoPanelDocumentSource,
} from "./ux-shell-contract-fixtures";

suite("Phase 4 — Live Values (v5 shell)", () => {
  test("the values surface is named 'Live Values' (not 'Structured Text Runtime')", () => {
    const host = readSrc("ioPanel.ts");
    const html = readIoPanelDocumentSource();
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      /createWebviewPanel\(\s*"trust-io-panel",\s*"Live Values"/.test(host),
      "the panel title must be 'Live Values'"
    );
    assert.ok(html.includes("<title>Live Values</title>"), "the HTML title must be 'Live Values'");
    assert.ok(!html.includes("Structured Text Runtime"), "Live Values HTML must not reintroduce the old Runtime wording");
    for (const [file, text] of [
      ["ioPanel.ts", host],
      ["io-panel/html.ts", html],
      ["ioPanel.webview.js", web],
    ] as const) {
      assert.ok(
        !/Runtime panel/i.test(text),
        `${file} must not reintroduce old Runtime panel wording`
      );
    }
  });
  test("write / force / release are preserved (NOT read-only)", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(host.includes("trust-lsp.debug.io.write"), "write preserved");
    assert.ok(host.includes("trust-lsp.debug.io.force"), "force preserved");
    assert.ok(host.includes("trust-lsp.debug.io.release"), "release preserved");
  });
  test("Live Values does not force a stale split beside Devices & Connections", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      host.includes("function liveValuesViewColumn") &&
        host.includes('activeTab?.label === "Devices & Connections"') &&
        host.includes("return vscode.ViewColumn.Active") &&
        host.includes("return vscode.ViewColumn.Two") &&
        host.includes("liveValuesViewColumn()"),
      "opening Live Values from Devices & Connections must use the active editor group instead of forcing a blank side-by-side canvas"
    );
  });
  test("Release all forces exists end-to-end (button + message + host loop)", () => {
    const host = readSrc("ioPanel.ts");
    const html = readIoPanelDocumentSource();
    const web = readSrc("ioPanel.webview.js");
    assert.ok(html.includes('id="releaseAllForces"'), "toolbar has the Release all forces button");
    assert.ok(host.includes("async function releaseAllForces"), "host releases every force");
    assert.ok(host.includes('case "releaseAllForces"'), "host handles the releaseAllForces message");
    assert.ok(
      web.includes('type: "releaseAllForces"'),
      "the webview posts releaseAllForces with the forced addresses"
    );
  });
  test("row write force and release wait for the next runtime scan before refreshing rows", () => {
    const host = readSrc("ioPanel.ts");
    for (const [name, successText] of [
      ["writeInput", "I/O write queued for"],
      ["forceInput", "I/O force active at"],
      ["releaseInput", "I/O force released at"],
    ] as const) {
      const start = host.indexOf(`async function ${name}`);
      assert.ok(start >= 0, `${name} must exist`);
      const end = host.indexOf("\nasync function", start + 1);
      const body = host.slice(start, end >= 0 ? end : undefined);
      assert.ok(body.includes(successText), `${name} must post success feedback`);
      assert.ok(
        body.includes("const previousScan = await currentIoScan();") &&
          body.includes("void requestIoStateAfterScan(previousScan);"),
        `${name} must wait for a newer scan before refreshing visible rows`
      );
    }
  });
  test("Force/Unforce work on remote attach too — the old 'not available' gate is removed", () => {
    const host = readSrc("ioPanel.ts");
    const web = readSrc("ioPanel.webview.js");
    // The backend now forwards io.force/io.unforce via attach (bbe4dacf2), so the remote-only block is
    // gone — leaving it would be a FALSE limitation. Force/release flow on sim AND remote; the runtime
    // authorizes by role and the catch surfaces any error.
    assert.ok(
      !host.includes("REMOTE_FORCE_UNAVAILABLE") && !host.includes("isRemoteTarget"),
      "the remote-only force/release block must be removed"
    );
    assert.ok(
      !/not available for remote targets yet/i.test(host) &&
        !/not available for remote targets yet/i.test(web),
      "no stale 'not available for remote targets yet' copy remains"
    );
    assert.ok(
      !/allowForce:\s*!remote/.test(web) && !/allowRelease:\s*!remote/.test(web),
      "the webview must NOT disable force/release for remote targets"
    );
    // Still wired (sim + remote) and still surfaces backend errors honestly.
    assert.ok(
      host.includes("trust-lsp.debug.io.force") &&
        host.includes("trust-lsp.debug.io.release"),
      "force/release commands stay wired"
    );
  });
  test("viewer Live Values permissions disable Write/Force before a backend rejection", () => {
    const web = readSrc("ioPanel.webview.js");
    const status = readSrc("io-panel/status.ts");
    const runtime = fs.readFileSync(
      path.join(workspaceRoot(), "crates", "trust-runtime", "src", "control.rs"),
      "utf8"
    );

    assert.ok(
      runtime.includes('"access"') &&
        runtime.includes('"io"') &&
        runtime.includes('"write"') &&
        runtime.includes('"force"') &&
        runtime.includes('"release"') &&
        runtime.includes("connect with an engineer token"),
      "runtime status must expose role-derived I/O capabilities"
    );
    assert.ok(
      status.includes("normalizeRuntimeAccess") &&
        status.includes("controlAuthToken") &&
        status.includes("controlEndpoint") &&
        status.includes("access,"),
      "Live Values status payload must carry the active session's access capabilities"
    );
    assert.ok(
      web.includes("let currentAccess") &&
        web.includes("allowWrite: currentAccess.allowWrite") &&
        web.includes("allowForce: currentAccess.allowForce") &&
        web.includes("allowRelease: currentAccess.allowRelease"),
      "the webview must render row controls from runtime-reported capabilities"
    );
    assert.ok(
      web.includes("writeButton.disabled = !canWrite") &&
        web.includes("forceButton.disabled = !canForce") &&
        web.includes("releaseButton.disabled = !canRelease") &&
        web.includes("releaseAllForcesBtn.disabled"),
      "denied write/force/release controls must be disabled before the user clicks"
    );
    assert.ok(
      web.includes("writeDisabledReason || remoteReason || \"Write is not available for this value.\"") &&
        web.includes("\"Release force before writing this value.\"") &&
        web.includes("if (!canForce && remoteReason)") &&
        web.includes("forceButton.title = remoteReason") &&
        web.includes("setStatusText(currentAccess.reason)"),
      "denied controls must carry a visible reason, and forced rows must explain why Write is disabled"
    );
  });
  test("non-simulator force is explicitly armed before pinning a value", () => {
    const web = readSrc("ioPanel.webview.js");
    const html = readIoPanelDocumentSource();
    assert.ok(
      web.includes("function forceRequiresArming") &&
        web.includes('currentMode !== "simulate"') &&
        web.includes("currentRuntimeState === \"connected\""),
      "Live Values must distinguish simulator one-click force from managed/remote force arming"
    );
    assert.ok(
      web.includes("function armForceForTarget") &&
        web.includes("Force armed for this target. Click Force again to pin a value."),
      "the arming step must be visible in the sticky status banner"
    );
    assert.ok(
      web.includes("function updateForcePolicy") &&
        web.includes("simulator pins immediately") &&
        web.includes("managed/remote targets require Arm force first") &&
        web.includes("this target requires Arm force first"),
      "the simulator-vs-managed force ceremony difference must be explained in the rendered panel"
    );
    assert.ok(
      html.includes('id="forcePolicy"') &&
        html.includes(".force-policy") &&
        html.includes("Force policy: simulator pins immediately; managed/remote targets require Arm force first."),
      "the force policy explanation must exist in the real Live Values webview HTML/CSS"
    );
    assert.ok(
      web.includes("Force remains armed for this target."),
      "release feedback must explain when the target remains armed for the session"
    );
    assert.ok(
      web.includes('forceButton.textContent = needsForceArm ? "Arm force" : "Force"'),
      "non-simulator targets must expose an Arm force first click"
    );
    assert.ok(
      /action === "force"[\s\S]*forceRequiresArming\(\)[\s\S]*!forceArmed[\s\S]*armForceForTarget\(\)/.test(web),
      "the first non-simulator Force click must arm instead of posting io.force"
    );
    for (const [name, source] of [["io-panel/html.ts", html]] as const) {
      assert.ok(source.includes(".mini-btn.armed"), `${name} must style the armed force state`);
      assert.ok(
        source.includes("background: color-mix(in srgb, var(--trust-warn) 14%, var(--trust-surface))") &&
          source.includes("box-shadow: inset 2px 0 0 var(--trust-warn)"),
        `${name} must use a quiet amber treatment for force arming, not a solid action fill`
      );
    }
  });
  test("forced values are always visibly marked", () => {
    const html = readIoPanelDocumentSource();
    const web = readSrc("ioPanel.webview.js");
    assert.ok(html.includes(".state-badge.forced"), "CSS marks forced values in the State column");
    assert.ok(
      web.includes('"state-badge forced"') && web.includes('"FORCED"'),
      "the webview renders a FORCED state badge on forced rows"
    );
  });
  test("Live Values exposes a forced-values inventory filter", () => {
    const web = readSrc("ioPanel.webview.js");
    for (const [name, source] of [
      ["io-panel/html.ts", readIoPanelDocumentSource()],
    ] as const) {
      assert.ok(
        source.includes('id="forcedFilter"') &&
          source.includes("Forced") &&
          source.includes(".forced-filter") &&
          source.includes('aria-pressed="false"'),
        `${name} must render the Forced (N) filter chip in the Live Values header`
      );
      assert.ok(
        source.includes("var(--trust-warn)") &&
          source.includes(".forced-filter.active") &&
          source.includes("white-space: nowrap"),
        `${name} must style the active Forced filter with the shared force/warning role`
      );
    }
    assert.ok(
      web.includes("const forcedFilterBtn = document.getElementById(\"forcedFilter\")") &&
        web.includes("let forcedOnly = false") &&
        web.includes("function updateForcedFilter") &&
        web.includes("Forced (\" + count + \")") &&
        web.includes("forcedFilterBtn.setAttribute(\"aria-pressed\"") &&
        web.includes("forcedFilterBtn.addEventListener(\"click\"") &&
        web.includes("forcedOnly && !entry.forced") &&
        web.includes("function appendIoSection") &&
        web.includes("forcedOnly && !hasForcedEntry(entries)"),
      "the webview must count forced rows, toggle the chip, and filter to forced rows only without empty groups"
    );
  });
  test("Live Values uses explicit safety verbs for row actions", () => {
    const web = readSrc("ioPanel.webview.js");
    const visualRuntime = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["visual/runtime/webview/stRuntimePanelController.ts", visualRuntime],
    ] as const) {
      assert.match(
        source,
        /writeButton\.textContent\s*=\s*[^;]*["`]Write\b/s,
        `${name} must label write actions with the explicit Write verb`
      );
      assert.match(
        source,
        /forceButton\.textContent\s*=\s*[^;]*["`]Force\b/s,
        `${name} must label force actions`
      );
      assert.ok(source.includes('textContent = "Release"'), `${name} must label release actions`);
      assert.ok(!/textContent\s*=\s*"W"/.test(source), `${name} must not use W as a safety action label`);
      assert.ok(!/textContent\s*=\s*"R"/.test(source), `${name} must not use R as a safety action label`);
      assert.ok(!/\?\s*"F\*"\s*:\s*"F"/.test(source), `${name} must not use F/F* as a safety action label`);
    }
  });
  test("Live Values explains disabled program-driven writes", () => {
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      web.includes("Outputs and memory are program-driven") &&
        web.includes("use Force to override"),
      "Live Values must show a visible hint explaining why outputs/memory Write is disabled"
    );
    assert.ok(
      web.includes("writeDisabledReason") &&
        web.includes("Program-driven") &&
        web.includes("Write is not available for this value."),
      "disabled Write buttons must carry a concrete tooltip reason"
    );
  });
  test("Live Values renders visible data-type labels instead of hidden value inference", () => {
    const web = readSrc("ioPanel.webview.js");
    const visualRuntime = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual/runtime/webview/stRuntimePanelController.ts", visualRuntime],
    ] as const) {
      assert.ok(source.includes("typeFromAddress"), `${name} must derive BOOL/WORD-style types from I/O addresses`);
      assert.ok(
        source.includes("valueType") && source.includes("typeFromAddress(entry"),
        `${name} must prefer backend-provided I/O value types before address fallback`
      );
      assert.ok(
        source.includes('source.className = "source-subtitle"') &&
          source.includes("nameCell.appendChild(source)"),
        `${name} must render source as muted name-cell context instead of a width-consuming column`
      );
      assert.ok(source.includes('typeCell.className = "type-cell"'), `${name} must render type in its own column`);
      assert.ok(source.includes('typeCell.textContent = displayType || "—"'), `${name} must show a stable type-cell value`);
      assert.ok(source.includes('stateCell.className = "state-cell"'), `${name} must render state in its own column`);
      assert.ok(source.includes("state-badge"), `${name} must use explicit state badges`);
    }
  });
  test("Live Values keeps BOOL rows compact and contextual", () => {
    const web = readSrc("ioPanel.webview.js");
    const visualRuntime = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual/runtime/webview/stRuntimePanelController.ts", visualRuntime],
    ] as const) {
      assert.ok(source.includes('displayType === "BOOL"'), `${name} must branch from the visible data type`);
      // BOOL rows expose an explicit TRUE/FALSE chooser in the write slot (parity with the numeric
      // write-box), so the operator picks what to write/force instead of an implicit hidden value.
      assert.ok(
        source.includes("createBoolToggle") && source.includes('"value-input bool-toggle"'),
        `${name} must give BOOL rows a TRUE/FALSE chooser in the write slot`
      );
      assert.ok(
        source.includes('toggle.value === "TRUE" ? "FALSE" : "TRUE"'),
        `${name} BOOL chooser must toggle between TRUE and FALSE`
      );
      assert.match(
        source,
        /writeButton\.textContent\s*=\s*["`]Write["`]/,
        `${name} must keep the visible BOOL Write action compact`
      );
      assert.ok(
        source.includes("valueControl.value"),
        `${name} must write/force the value chosen in the row control (BOOL toggle or numeric input)`
      );
      assert.ok(
        source.includes("if (isForced)") &&
          source.includes("actions.appendChild(releaseButton)") &&
          source.includes("actions.appendChild(forceButton)"),
        `${name} must show Release only for forced rows and Force otherwise`
      );
      assert.ok(
        /const valueControl[\s\S]*(isForced|forced)[\s\S]*\?\s*null/.test(source) &&
          source.includes("Release force before writing this value."),
        `${name} must not crowd forced rows with an editable value control beside the FORCED badge`
      );
    }
    for (const [name, source] of [
      ["io-panel/html.ts", readIoPanelDocumentSource()],
      ["visual/runtime/webview/stRuntimePanel.css", readSrc("visual/runtime/webview/stRuntimePanel.css")],
    ] as const) {
      assert.ok(source.includes(".value-input"), `${name} must style the value editor`);
      assert.ok(source.includes("height: 24px"), `${name} must keep row controls aligned`);
    }
  });
  test("Live Values keeps operation feedback visible in the sticky header", () => {
    const web = readSrc("ioPanel.webview.js");
    const visualRuntime = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["io-panel/html.ts", readIoPanelDocumentSource()],
    ] as const) {
      assert.ok(
        /<header>[\s\S]*<div class="status" id="status">/.test(source),
        `${name} must render operation feedback inside the sticky header`
      );
      assert.ok(
        !/<\/header>[\s\S]*<div class="status" id="status">/.test(source),
        `${name} must not hide operation feedback below the value list`
      );
      assert.ok(source.includes(".status:not(:empty)"), `${name} must hide only empty status text`);
      assert.ok(source.includes(".status.status-error"), `${name} must style failed writes/forces visibly`);
      assert.ok(source.includes(".status.status-warn"), `${name} must style armed/active force feedback as warning`);
    }
    assert.ok(
      web.includes('if (message.type === "status")') &&
        web.includes("const payload = String(message.payload || \"\")") &&
        web.includes("setStatusText(payload"),
      "status messages must go through the styled status renderer"
    );
    assert.ok(web.includes("status-error"), "webview must mark failed operations as error status");
    assert.ok(
      web.includes("status-warn") &&
        web.includes("isPermissionGuidanceText") &&
        web.includes("force armed|force active|force remains armed") &&
        web.includes("!isWarning && /queued|released|cleared/i.test(text)") &&
        web.includes('status.classList.toggle("status-error", isError)'),
      "force armed/active feedback and permission guidance must be amber warning, not green success or alarm red"
    );
    assert.ok(
      web.includes("updateForceStatusFromState") &&
        web.includes("forcedAddresses(state)") &&
        web.includes('"I/O force active at " + addresses[0]') &&
        web.includes("updateForceStatusFromState(currentState)"),
      "active forces from runtime snapshots must render a standing amber warning, even without a fresh button click"
    );
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual/runtime/webview/stRuntimePanelController.ts", visualRuntime],
    ] as const) {
      assert.ok(
        source.includes("isTransientStatusText"),
        `${name} must clear only startup/unavailable guidance when live values arrive`
      );
      assert.ok(
        source.includes("Start (?:the Simulator|the selected runtime) to see live values") &&
          source.includes("Connect to the selected runtime to see live values"),
        `${name} must clear stale empty-state guidance after live values arrive`
      );
      assert.ok(
        source.includes("TRANSIENT_STATUS_CLEAR_MS = 5000") &&
          source.includes("isAutoExpiringStatusText") &&
          source.includes("force released at") &&
          source.includes("Released \\d+ forces?") &&
          source.includes("No forces to release") &&
          /status(?:\?\.|\.)textContent === (text|message)/.test(source),
        `${name} must auto-expire short success feedback without clearing newer status`
      );
      assert.ok(
        !source.includes("I/O force active at .+"),
        `${name} must not auto-expire force-active standing-state banners`
      );
      assert.ok(
        !/if \(message\.type === "ioState"\) \{\s*setStatusText\(""\);/.test(source),
        `${name} must not clear operation feedback on every value refresh`
      );
    }
  });
});
