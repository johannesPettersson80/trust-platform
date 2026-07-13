"use strict";
// This module is loaded by VS Code through --extensionTestsPath. It deliberately
// uses only Node built-ins and the VS Code API so the acceptance run exercises
// the exact extension extracted from the packaged win32-x64 VSIX.

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { connectCdp, waitForCdpJson } = require("./PackagedSimulatorCdp");
const {
  safeError,
  serializeWithoutCredential,
} = require("./AcceptanceRedaction");
const { runtimeControlToken } = require("./RuntimeControlToken");
const {
  provePackagedProductIdentity,
} = require("./PackagedBinaryIdentity");
const { runPackagedAdsUiAcceptance } = require("./PackagedAdsUiAcceptance");
const { parseExpectedCustomAdsPorts } = require("./PackagedAdsCustomPorts");
const {
  provePackagedAdsLiveValues,
} = require("./PackagedAdsLiveValuesAcceptance");
const { requestIoStateEvent } = require("./PackagedDapState");
const { sleep, waitFor } = require("./AcceptanceWait");
const { createScreenshotProof, createSimulatorObservations } = require("./PackagedSimulatorVisualProof");

const extensionRoot = requiredPath("TRUST_PACKAGED_EXTENSION_ROOT");
const projectRoot = requiredPath("TRUST_PACKAGED_SIMULATOR_PROJECT");
const evidencePath = requiredValue("TRUST_PACKAGED_SIMULATOR_EVIDENCE");
const screenshotDir = requiredPath("TRUST_PACKAGED_SIMULATOR_SCREENSHOT_DIR");
const expectedVersion = requiredValue("TRUST_PACKAGED_SIMULATOR_VERSION");
const cdpPort = Number(requiredValue("TRUST_PACKAGED_SIMULATOR_CDP_PORT"));
const adsUiRequired = process.env.TRUST_PACKAGED_ADS_UI_REQUIRED === "1";
const expectedAdsTargetNetId = adsUiRequired
  ? requiredValue("TRUST_PACKAGED_ADS_EXPECTED_TARGET_NET_ID")
  : null;
const expectedCustomAdsPorts = parseExpectedCustomAdsPorts(process.env.TRUST_PACKAGED_ADS_EXPECTED_CUSTOM_PORTS, adsUiRequired);
const proof = {
  schema_version: 1,
  gate: "windows_packaged_simulator_acceptance",
  generated_at_utc: new Date().toISOString(),
  status: "running",
  package: {
    expected_version: expectedVersion,
    extension_root: extensionRoot,
  },
  host: {
    platform: process.platform,
    architecture: process.arch,
    node: process.versions.node,
    vscode: vscode.version,
  },
  journey: {
    sequence:
      "fresh VS Code -> Start -> Devices -> Stop -> Start -> Stop -> Discover ADS -> Advanced custom-port rescan -> Browse 851 -> Add read-only variable -> Start -> Live Values ADS -> Stop",
    debug_sessions_started: 0,
    debug_sessions_terminated: 0,
    start_attempts: 0,
    starting_states_observed: 0,
    blocked_duplicate_start_clicks: 0,
    token_migrated: false,
    migrated_token_length: 0,
    session_control_auth_present: false,
    session_control_auth_length: 0,
    session_control_auth_stable: false,
    session_control_endpoint_loopback: false,
    dap_io_state_received: false,
    dap_io_round_trips: 0,
    live_values_ever_opened: false,
    live_values_ever_focused: false,
    tab_observations: [],
    first_devices_paint_ms: null,
    first_devices_loading_text_visible: null,
    auth_error_visible: null,
    initial_stopped: null,
    first_starting: null,
    first_running_before_devices: null,
    first_devices_running: null,
    stopped_with_devices_open: null,
    second_starting_with_devices_open: null,
    second_running_with_devices_open: null,
    pre_ads_stopped: null,
    final_stopped: null,
    ads_ui: {
      required: adsUiRequired,
      expected_target_ams_net_id: expectedAdsTargetNetId,
      status: adsUiRequired ? "pending" : "skip",
      phases_observed: [],
      default_surface: null,
      discovered_target: null,
      browse_851: null,
      custom_port_recovery: null,
      imported_variable: null,
      live_values: null,
    },
  },
  screenshots: [],
  assertions: [],
  error: null,
};
function requiredValue(name) {
  const value = String(process.env[name] || "").trim();
  if (!value) throw new Error(`Missing required environment value ${name}.`);
  return value;
}
function requiredPath(name) {
  const value = path.resolve(requiredValue(name));
  if (!fs.existsSync(value)) throw new Error(`${name} does not exist: ${value}`);
  return value;
}
function check(id, pass, detail) {
  const row = { id, pass: Boolean(pass), detail };
  proof.assertions.push(row);
  if (!row.pass) throw new Error(`Acceptance assertion failed: ${id}`);
}
function writeEvidence(credentials) {
  let output = serializeWithoutCredential(proof, credentials);
  if (output.credentialFound) {
    proof.status = "fail";
    proof.error =
      "Acceptance evidence contained the disposable runtime control credential and was rejected.";
    output = serializeWithoutCredential(proof, credentials);
  }
  fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
  fs.writeFileSync(evidencePath, output.serialized, "utf8");
}

const { activeTabLabel, allTabLabels, hasVisibleAuthError } =
  createSimulatorObservations(vscode);
exports.run = async function run() {
  let cdp;
  let activeSession;
  let tabPoll;
  let migratedControlToken = "";
  const sessionControlCredentials = new Set();
  const subscriptions = [];
  try {
    check("windows-host", process.platform === "win32", {
      platform: process.platform,
    });
    check("x64-host", process.arch === "x64", { architecture: process.arch });

    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    check("packaged-extension-present", Boolean(extension), null);
    const actualExtensionRoot = path.resolve(extension.extensionPath);
    check(
      "exact-installed-extension-loaded",
      actualExtensionRoot.toLowerCase() === extensionRoot.toLowerCase(),
      { actual_extension_root: actualExtensionRoot }
    );
    check(
      "packaged-extension-version",
      extension.packageJSON.version === expectedVersion,
      { actual_version: extension.packageJSON.version }
    );
    proof.package.actual_version = extension.packageJSON.version;
    await provePackagedProductIdentity({
      vscode,
      extension,
      extensionRoot,
      expectedVersion,
      packageProof: proof.package,
      check,
    });
    subscriptions.push(
      vscode.debug.onDidStartDebugSession((session) => {
        if (session.type === "structured-text") {
          proof.journey.debug_sessions_started += 1;
          activeSession = session;
        }
      }),
      vscode.debug.onDidTerminateDebugSession((session) => {
        if (session.type === "structured-text") {
          proof.journey.debug_sessions_terminated += 1;
        }
      })
    );

    const journeyStartedAt = Date.now();
    function observeTabs(source) {
      const labels = allTabLabels();
      const active = activeTabLabel();
      const hasLiveValues = labels.some((label) => /^Live Values$/i.test(label));
      const focusedLiveValues = /^Live Values$/i.test(active);
      proof.journey.live_values_ever_opened ||= hasLiveValues;
      proof.journey.live_values_ever_focused ||= focusedLiveValues;
      const observation = {
        elapsed_ms: Date.now() - journeyStartedAt,
        source,
        active,
        has_live_values: hasLiveValues,
      };
      const previous = proof.journey.tab_observations.at(-1);
      if (
        proof.journey.tab_observations.length < 100 &&
        (!previous ||
          previous.active !== observation.active ||
          previous.has_live_values !== observation.has_live_values ||
          source === "event")
      ) {
        proof.journey.tab_observations.push(observation);
      }
    }
    observeTabs("initial");
    subscriptions.push(
      vscode.window.tabGroups.onDidChangeTabs(() => observeTabs("event"))
    );
    tabPoll = setInterval(() => observeTabs("poll"), 25);

    await vscode.commands.executeCommand("workbench.action.closePanel");
    const configDocument = await vscode.workspace.openTextDocument(
      path.join(projectRoot, "src", "config.st")
    );
    await vscode.window.showTextDocument(configDocument, { preview: false });
    await vscode.commands.executeCommand("workbench.view.extension.trust");
    await vscode.commands.executeCommand("trust.home.focus");
    await sleep(500);

    cdp = await connectCdp(cdpPort);

    async function attachTarget(target) {
      const attached = await cdp.send("Target.attachToTarget", {
        targetId: target.id,
        flatten: true,
      });
      const sessionId = attached.result.sessionId;
      await cdp.send("Runtime.enable", {}, sessionId);
      return sessionId;
    }

    async function rawEval(sessionId, expression) {
      const response = await cdp.send(
        "Runtime.evaluate",
        { expression, returnByValue: true, awaitPromise: true },
        sessionId
      );
      return response.result?.result?.value;
    }

    async function deepHas(sessionId, selector) {
      return Boolean(
        await rawEval(
          sessionId,
          `(function(){function find(doc,depth){if(!doc||depth>5)return false;if(doc.querySelector(${JSON.stringify(
            selector
          )}))return true;for(var frame of Array.from(doc.querySelectorAll('iframe'))){try{if(find(frame.contentDocument,depth+1))return true;}catch(_){}}return false;}return find(document,0);})()`
        )
      );
    }

    async function attachByKind(kind, timeoutMs = 30_000) {
      const deadline = Date.now() + timeoutMs;
      while (Date.now() < deadline) {
        const targets = await waitForCdpJson(cdpPort, "/json");
        for (const target of targets.filter(
          (item) => item.type === "page" || item.type === "iframe"
        )) {
          const sessionId = await attachTarget(target);
          const matches =
            (kind === "page" && target.type === "page") ||
            (kind === "sidebar" && (await deepHas(sessionId, "#action"))) ||
            (kind === "canvas" && (await deepHas(sessionId, ".react-flow"))) ||
            (kind === "live-values" && (await deepHas(sessionId, "#sections")));
          if (matches) return sessionId;
          await cdp.send("Target.detachFromTarget", { sessionId });
        }
        await sleep(180);
      }
      throw new Error(`No ${kind} CDP target appeared.`);
    }

    const pageSession = await attachByKind("page");
    const sidebarSession = await attachByKind("sidebar");
    let canvasSession;
    const screenshots = await createScreenshotProof({
      cdp,
      pageSession,
      screenshotDir,
      records: proof.screenshots,
    });

    async function evalInDoc(sessionId, selector, body) {
      const value = await rawEval(
        sessionId,
        `(function(){try{function find(doc,depth){if(!doc||depth>5)return null;if(doc.querySelector(${JSON.stringify(
          selector
        )}))return doc;for(var frame of Array.from(doc.querySelectorAll('iframe'))){try{var found=find(frame.contentDocument,depth+1);if(found)return found;}catch(_){}}return null;}var d=find(document,0);if(!d)return {__missing:${JSON.stringify(
          selector
        )}};var w=d.defaultView;${body}}catch(error){return {__error:error.message};}})()`
      );
      if (value?.__error || value?.__missing) {
        throw new Error("Packaged VS Code surface evaluation failed.");
      }
      return value;
    }

    async function sidebarSnapshot() {
      return evalInDoc(
        sidebarSession,
        "#action",
        `var action=d.getElementById('action');return {action:{text:(action.innerText||'').trim(),state:action.dataset.state||'',disabled:Boolean(action.disabled),title:action.title||''},target:(d.getElementById('targetValue')?.innerText||'').trim(),homeVisible:Boolean(d.getElementById('project')&&!d.getElementById('project').classList.contains('hidden')),controls:[...d.querySelectorAll('.action-row button')].map(function(button){return {id:button.id,text:(button.innerText||'').trim(),disabled:Boolean(button.disabled)};}),bodyText:(d.body.innerText||'').replace(/\\s+/g,' ').trim()};`
      );
    }

    async function canvasSnapshot() {
      if (!canvasSession) throw new Error("Devices canvas is not open.");
      return evalInDoc(
        canvasSession,
        ".react-flow",
        `var sim=[...d.querySelectorAll('.react-flow__node')].find(function(node){return /Simulator/i.test(node.innerText||'');});var card=sim?.querySelector('[data-role="runtime-card"]');var status=sim?.querySelector('[data-role="status-pill"]');return {simulator:sim?{text:(sim.innerText||'').replace(/\\s+/g,' ').trim(),surfaceTone:card?.getAttribute('data-surface-tone')||'',statusText:(status?.innerText||'').trim()}:null,canvasText:(d.body.innerText||'').replace(/\\s+/g,' ').trim()};`
      );
    }

    async function pageText() {
      return String(
        (await rawEval(
          pageSession,
          `(function(){return (document.body?.innerText||'').replace(/\\s+/g,' ').trim();})()`
        )) || ""
      );
    }

    async function clickAction(expectedState) {
      return evalInDoc(
        sidebarSession,
        "#action",
        `var button=d.getElementById('action');if(${JSON.stringify(
          expectedState
        )}&&button.dataset.state!==${JSON.stringify(
          expectedState
        )})return {clicked:false,state:button.dataset.state||'',disabled:Boolean(button.disabled),text:(button.innerText||'').trim()};if(button.disabled)return {clicked:false,state:button.dataset.state||'',disabled:true,text:(button.innerText||'').trim()};button.click();return {clicked:true,state:button.dataset.state||'',disabled:false,text:(button.innerText||'').trim()};`
      );
    }

    async function beginStartAttempt(name, devicesOpen) {
      const activeBefore = activeTabLabel();
      activeSession = undefined;
      proof.journey.start_attempts += 1;
      const click = await clickAction("start");
      check(`${name}-click-accepted`, click.clicked, click);
      const sidebar = await waitFor(
        sidebarSnapshot,
        (value) => value.action.state === "busy" || /Starting/i.test(value.action.text),
        `${name} sidebar Starting`,
        15_000
      );
      const statusText = await waitFor(
        pageText,
        (value) => /truST:\s*Simulator starting/i.test(value),
        `${name} status bar Starting`,
        15_000
      );
      const duplicate = await clickAction("busy");
      const duplicateBlocked = !duplicate.clicked && duplicate.disabled === true;
      let canvas;
      if (devicesOpen) {
        canvas = await waitFor(
          canvasSnapshot,
          (value) => value.simulator?.statusText === "Starting",
          `${name} canvas Starting`,
          15_000
        );
      }
      proof.journey.starting_states_observed += 1;
      if (duplicateBlocked) proof.journey.blocked_duplicate_start_clicks += 1;
      const state = {
        sidebar_action: sidebar.action,
        canvas_simulator: canvas?.simulator,
        status_bar_starting: /truST:\s*Simulator starting/i.test(statusText),
        duplicate_click: duplicate,
        active_tab_before: activeBefore,
        active_tab_during: activeTabLabel(),
      };
      check(
        `${name}-starting-is-one-disabled-attempt`,
        sidebar.action.disabled &&
          state.status_bar_starting &&
          duplicateBlocked &&
          (!devicesOpen || canvas?.simulator?.statusText === "Starting"),
        state
      );
      return state;
    }

    async function waitForRunning(name, devicesOpen, expectedSessionCount) {
      const sidebar = await waitFor(
        sidebarSnapshot,
        (value) => value.action.state === "stop" && value.action.text === "Stop",
        `${name} sidebar Running`,
        60_000
      );
      const statusText = await waitFor(
        pageText,
        (value) => /truST:\s*Simulator running/i.test(value),
        `${name} status bar Running`,
        15_000
      );
      let canvas;
      if (devicesOpen) {
        canvas = await waitFor(
          canvasSnapshot,
          (value) => value.simulator?.statusText === "Running",
          `${name} canvas Running`,
          20_000
        );
      }
      const session = await waitFor(
        async () =>
          activeSession ||
          (vscode.debug.activeDebugSession?.type === "structured-text"
            ? vscode.debug.activeDebugSession
            : undefined),
        Boolean,
        `${name} Structured Text debug session`,
        15_000
      );
      check(
        `${name}-debug-session-count`,
        proof.journey.debug_sessions_started === expectedSessionCount,
        { debug_sessions_started: proof.journey.debug_sessions_started }
      );
      return {
        evidence: {
          sidebar_action: sidebar.action,
          sidebar_target: sidebar.target,
          canvas_simulator: canvas?.simulator,
          status_bar_running: /truST:\s*Simulator running/i.test(statusText),
        },
        session,
      };
    }

    async function stopAndWait(name, expectedTerminationCount) {
      const click = await clickAction("stop");
      check(`${name}-click-accepted`, click.clicked, click);
      const sidebar = await waitFor(
        sidebarSnapshot,
        (value) => value.action.state === "start" && value.action.text === "Start",
        `${name} sidebar Stopped`,
        35_000
      );
      const canvas = await waitFor(
        canvasSnapshot,
        (value) => value.simulator?.statusText === "Stopped",
        `${name} canvas Stopped`,
        20_000
      );
      const statusText = await waitFor(
        pageText,
        (value) => /truST:\s*Simulator stopped/i.test(value),
        `${name} status bar Stopped`,
        15_000
      );
      await waitFor(
        async () => proof.journey.debug_sessions_terminated,
        (count) => count === expectedTerminationCount,
        `${name} terminated Structured Text session`,
        15_000
      );
      return {
        sidebar_action: sidebar.action,
        sidebar_target: sidebar.target,
        sidebar_text: sidebar.bodyText,
        canvas_simulator: canvas.simulator,
        canvas_text: canvas.canvasText,
        status_bar_stopped: /truST:\s*Simulator stopped/i.test(statusText),
      };
    }

    const initialSidebar = await waitFor(
      sidebarSnapshot,
      (value) => value.action.state === "start" && !value.action.disabled,
      "fresh packaged Simulator Start"
    );
    const initialPageText = await waitFor(
      pageText,
      (value) => /truST:\s*Simulator stopped/i.test(value),
      "fresh status bar Stopped",
      15_000
    );
    proof.journey.initial_stopped = {
      sidebar_action: initialSidebar.action,
      sidebar_target: initialSidebar.target,
      status_bar_stopped: /truST:\s*Simulator stopped/i.test(initialPageText),
      active_tab: activeTabLabel(),
      tabs: allTabLabels(),
    };
    check(
      "fresh-reload-start-before-devices",
      initialSidebar.action.text === "Start" &&
        initialSidebar.homeVisible &&
        proof.journey.initial_stopped.status_bar_stopped &&
        !vscode.debug.activeDebugSession &&
        proof.journey.debug_sessions_started === 0 &&
        !proof.journey.initial_stopped.tabs.some((label) =>
          /Devices & Connections|Live Values/i.test(label)
        ),
      proof.journey.initial_stopped
    );
    check(
      "single-lifecycle-control",
      JSON.stringify(initialSidebar.controls.map((control) => control.id)) ===
        JSON.stringify(["compile", "action"]),
      { control_ids: initialSidebar.controls.map((control) => control.id) }
    );
    await screenshots.capture("01-initial-stopped");

    proof.journey.first_starting = await beginStartAttempt("first-start", false);
    const firstRunning = await waitForRunning(
      "first-start",
      false,
      1
    );
    proof.journey.first_running_before_devices = firstRunning.evidence;
    check(
      "first-start-keeps-editor-open",
      proof.journey.first_starting.active_tab_before === activeTabLabel() &&
        !proof.journey.live_values_ever_opened &&
        !proof.journey.live_values_ever_focused,
      {
        before: proof.journey.first_starting.active_tab_before,
        after: activeTabLabel(),
      }
    );
    await screenshots.capture("02-running-editor-preserved");

    const firstSession = firstRunning.session;
    const runtimeToml = fs.readFileSync(path.join(projectRoot, "runtime.toml"), "utf8");
    const token = runtimeControlToken(runtimeToml) || "";
    migratedControlToken = token;
    proof.journey.migrated_token_length = token.length;
    proof.journey.token_migrated = token.length >= 24;
    const sessionControlToken = firstSession.configuration.controlAuthToken;
    if (typeof sessionControlToken === "string" && sessionControlToken.length > 0) {
      sessionControlCredentials.add(sessionControlToken);
    }
    const sessionControlEndpoint = firstSession.configuration.controlEndpoint;
    proof.journey.session_control_auth_length =
      typeof sessionControlToken === "string" ? sessionControlToken.length : 0;
    proof.journey.session_control_auth_present =
      proof.journey.session_control_auth_length >= 24;
    proof.journey.session_control_endpoint_loopback =
      typeof sessionControlEndpoint === "string" &&
      /^tcp:\/\/(?:127(?:\.\d{1,3}){3}|localhost|\[::1\]):\d+$/i.test(
        sessionControlEndpoint
      );
    check(
      "tokenless-project-migrated-before-launch",
      proof.journey.token_migrated &&
        proof.journey.session_control_auth_present &&
        proof.journey.session_control_endpoint_loopback &&
        !runtimeToml.includes("some-secret-value"),
      {
        runtime_toml_token_length: proof.journey.migrated_token_length,
        session_control_auth_length: proof.journey.session_control_auth_length,
        session_control_endpoint_loopback:
          proof.journey.session_control_endpoint_loopback,
      }
    );
    const firstIoState = await requestIoStateEvent(vscode, firstSession);
    if (firstIoState !== null && typeof firstIoState === "object") {
      proof.journey.dap_io_round_trips += 1;
    }

    const devicesStartedAt = Date.now();
    await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
    canvasSession = await attachByKind("canvas", 8_000);
    const firstDevices = await waitFor(
      canvasSnapshot,
      (value) => Boolean(value.simulator),
      "bounded first Devices paint",
      8_000
    );
    proof.journey.first_devices_paint_ms = Date.now() - devicesStartedAt;
    proof.journey.first_devices_loading_text_visible = /Loading your devices/i.test(
      firstDevices.canvasText
    );
    const firstDevicesPageText = await pageText();
    const firstDevicesSidebar = await sidebarSnapshot();
    proof.journey.auth_error_visible = hasVisibleAuthError(
      `${firstDevices.canvasText} ${firstDevicesSidebar.bodyText} ${firstDevicesPageText}`
    );
    proof.journey.first_devices_running = {
      canvas_simulator: firstDevices.simulator,
      status_bar_running: /truST:\s*Simulator running/i.test(firstDevicesPageText),
      active_tab: activeTabLabel(),
    };
    check(
      "devices-first-paint-bounded",
      proof.journey.first_devices_paint_ms <= 8_000 &&
        !proof.journey.first_devices_loading_text_visible,
      {
        first_paint_ms: proof.journey.first_devices_paint_ms,
        loading_visible: proof.journey.first_devices_loading_text_visible,
      }
    );
    check(
      "running-surfaces-agree",
      firstDevices.simulator?.statusText === "Running" &&
        proof.journey.first_devices_running.status_bar_running &&
        proof.journey.first_running_before_devices.sidebar_action.text === "Stop",
      proof.journey.first_devices_running
    );
    check(
      "no-visible-auth-error-after-devices-open",
      !proof.journey.auth_error_visible,
      { auth_error_visible: proof.journey.auth_error_visible }
    );
    await screenshots.capture("03-devices-running-consistent");

    proof.journey.stopped_with_devices_open = await stopAndWait("first-stop", 1);
    check(
      "stopped-surfaces-agree-after-stop",
      proof.journey.stopped_with_devices_open.sidebar_action.text === "Start" &&
        proof.journey.stopped_with_devices_open.canvas_simulator.statusText === "Stopped" &&
        proof.journey.stopped_with_devices_open.status_bar_stopped,
      proof.journey.stopped_with_devices_open
    );
    check(
      "no-visible-auth-error-after-stop",
      !hasVisibleAuthError(
        `${proof.journey.stopped_with_devices_open.sidebar_text} ${proof.journey.stopped_with_devices_open.canvas_text} ${await pageText()}`
      ),
      null
    );
    await screenshots.capture("04-devices-stopped-consistent");

    const devicesTabBeforeSecondStart = activeTabLabel();
    proof.journey.second_starting_with_devices_open = await beginStartAttempt(
      "second-start",
      true
    );
    const secondRunning = await waitForRunning(
      "second-start",
      true,
      2
    );
    proof.journey.second_running_with_devices_open = secondRunning.evidence;
    check(
      "start-keeps-devices-open",
      devicesTabBeforeSecondStart === activeTabLabel() &&
        !proof.journey.live_values_ever_opened &&
        !proof.journey.live_values_ever_focused,
      {
        before: devicesTabBeforeSecondStart,
        after: activeTabLabel(),
        live_values_opened: proof.journey.live_values_ever_opened,
        live_values_focused: proof.journey.live_values_ever_focused,
      }
    );
    check(
      "second-start-after-stop-has-no-stale-session",
      proof.journey.debug_sessions_started === 2 &&
        proof.journey.debug_sessions_terminated === 1 &&
        proof.journey.second_running_with_devices_open.sidebar_action.text === "Stop" &&
        proof.journey.second_running_with_devices_open.canvas_simulator.statusText === "Running" &&
        proof.journey.second_running_with_devices_open.status_bar_running,
      {
        debug_sessions_started: proof.journey.debug_sessions_started,
        debug_sessions_terminated: proof.journey.debug_sessions_terminated,
      }
    );
    const secondRuntimeToml = fs.readFileSync(
      path.join(projectRoot, "runtime.toml"),
      "utf8"
    );
    const secondSessionControlToken =
      secondRunning.session.configuration.controlAuthToken;
    if (
      typeof secondSessionControlToken === "string" &&
      secondSessionControlToken.length > 0
    ) {
      sessionControlCredentials.add(secondSessionControlToken);
    }
    proof.journey.session_control_auth_stable =
      typeof sessionControlToken === "string" &&
      typeof secondSessionControlToken === "string" &&
      sessionControlToken.length >= 24 &&
      secondSessionControlToken.length === sessionControlToken.length &&
      crypto.timingSafeEqual(
        Buffer.from(sessionControlToken, "utf8"),
        Buffer.from(secondSessionControlToken, "utf8")
      );
    check(
      "second-start-keeps-migrated-token",
      runtimeControlToken(secondRuntimeToml) === token &&
        proof.journey.session_control_auth_stable,
      {
        runtime_toml_token_length: token.length,
        second_session_control_auth_length:
          typeof secondSessionControlToken === "string"
            ? secondSessionControlToken.length
            : 0,
        first_and_second_session_control_auth_equal:
          proof.journey.session_control_auth_stable,
      }
    );
    const secondIoState = await requestIoStateEvent(
      vscode,
      secondRunning.session
    );
    if (secondIoState !== null && typeof secondIoState === "object") {
      proof.journey.dap_io_round_trips += 1;
    }
    proof.journey.dap_io_state_received = proof.journey.dap_io_round_trips === 2;
    check(
      "dap-io-state-round-trip",
      proof.journey.dap_io_state_received,
      { round_trips: proof.journey.dap_io_round_trips }
    );
    check(
      "live-values-never-opened-or-focused",
      !proof.journey.live_values_ever_opened &&
        !proof.journey.live_values_ever_focused,
      {
        opened: proof.journey.live_values_ever_opened,
        focused: proof.journey.live_values_ever_focused,
      }
    );
    check(
      "no-visible-auth-error-after-second-start",
      !hasVisibleAuthError(
        `${(await sidebarSnapshot()).bodyText} ${(await canvasSnapshot()).canvasText} ${await pageText()}`
      ),
      null
    );
    await screenshots.capture("05-devices-restarted-consistent");

    proof.journey.pre_ads_stopped = await stopAndWait("pre-ads-stop", 2);
    check(
      "pre-ads-stopped-surfaces-agree",
      proof.journey.pre_ads_stopped.sidebar_action.text === "Start" &&
        proof.journey.pre_ads_stopped.canvas_simulator.statusText === "Stopped" &&
        proof.journey.pre_ads_stopped.status_bar_stopped &&
        proof.journey.debug_sessions_terminated === 2,
      proof.journey.pre_ads_stopped
    );

    if (adsUiRequired) {
      const importedVariable = await runPackagedAdsUiAcceptance({
        evaluate: (selector, body) =>
          evalInDoc(canvasSession, selector, body),
        waitFor,
        sleep,
        check,
        state: proof.journey.ads_ui,
        expectedTargetNetId: expectedAdsTargetNetId,
        expectedCustomPorts: expectedCustomAdsPorts,
        projectRoot,
      });
      await screenshots.capture("06-ads-discovered-and-imported");

      await provePackagedAdsLiveValues({
        vscode,
        proof,
        importedVariable,
        beginStartAttempt,
        waitForRunning,
        attachByKind,
        evalInDoc,
        waitFor,
        sleep,
        check,
      });
      await screenshots.capture("07-live-values-ads-good");
    }

    if (adsUiRequired) {
      proof.journey.final_stopped = await stopAndWait("final-stop", 3);
      check(
        "final-stopped-surfaces-agree",
        proof.journey.final_stopped.sidebar_action.text === "Start" &&
          proof.journey.final_stopped.canvas_simulator.statusText === "Stopped" &&
          proof.journey.final_stopped.status_bar_stopped &&
          proof.journey.debug_sessions_terminated === 3,
        proof.journey.final_stopped
      );
    } else {
      proof.journey.final_stopped = proof.journey.pre_ads_stopped;
    }

    screenshots.assertComplete(check, adsUiRequired ? 7 : 5);
    proof.status = "pass";
  } catch (error) {
    proof.status = "fail";
    proof.error = safeError(error);
    throw error;
  } finally {
    if (tabPoll) clearInterval(tabPoll);
    for (const subscription of subscriptions) subscription.dispose();
    if (vscode.debug.activeDebugSession?.type === "structured-text") {
      await vscode.debug.stopDebugging(vscode.debug.activeDebugSession).catch(() => undefined);
    }
    if (cdp) cdp.close();
    proof.completed_at_utc = new Date().toISOString();
    writeEvidence([migratedControlToken, ...sessionControlCredentials]);
  }
};
