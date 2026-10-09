// Mocha suite for the Control Studio Phase 3 rendered evidence (finding W-13). Loaded as
// extensionTestsPath by run.js in this directory, the same way editors/vscode/src/test/suite/index.ts is
// loaded by runTest.ts: it exports a run(): Promise<void> that @vscode/test-electron's runTests() calls
// once the Extension Development Host has activated.
//
// Journey driven (docs/internal/reviews/control-course-review-2026-09-03.md W-13's required improvement):
//   Monitor -> Response, Start recording, Test tab, Review test, Confirm and start, running (Abort visible),
//   completion, then a wide/split/narrow/200%-zoom/high-contrast layout matrix.
//
// Ground rule: never fabricate a screenshot or a step result. Every step is wrapped so that if it cannot
// be reached, whatever is on screen is still captured, the failure is recorded in steps.json, and the
// remaining steps still run.

"use strict";

const fs = require("fs");
const path = require("path");
const http = require("http");
const { execFile } = require("child_process");
const vscode = require("vscode");
const Mocha = require("mocha");

let WebSocket;
try {
  WebSocket = require("ws");
} catch (error) {
  WebSocket = null;
}

const EXTENSION_ROOT = path.resolve(__dirname, "..", "..");
const OUT_DIR = path.join(EXTENSION_ROOT, "out");

const EVIDENCE_DIR = (process.env.TRUST_CONTROL_STUDIO_EVIDENCE_DIR || "").trim();
const SOURCE_COMMIT = (process.env.TRUST_CONTROL_STUDIO_SOURCE_COMMIT || "").trim();
const CDP_PORT = Number(process.env.TRUST_CONTROL_STUDIO_CDP_PORT || 9222);

const SCREENSHOTS_DIR = EVIDENCE_DIR ? path.join(EVIDENCE_DIR, "screenshots-raw") : "";
const STEPS_JSON_PATH = EVIDENCE_DIR ? path.join(EVIDENCE_DIR, "steps.json") : "";

const WIDE_BOUNDS = { left: 0, top: 0, width: 1600, height: 1000, windowState: "normal" };
const NARROW_BOUNDS = { left: 0, top: 0, width: 900, height: 900, windowState: "normal" };
const CONSTRAINED_BOUNDS = { left: 0, top: 0, width: 1440, height: 900, windowState: "normal" };

// ---------------------------------------------------------------------------------------------
// steps.json (written incrementally so a hard crash still leaves partial, honest evidence)
// ---------------------------------------------------------------------------------------------

const steps = [];
let stepIndex = 0;

function persistSteps(extra) {
  if (!EVIDENCE_DIR) {
    return;
  }
  try {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const payload = Object.assign(
      {
        schema_version: 1,
        source_commit: SOURCE_COMMIT || null,
        vscode_version: vscode.version,
        generated_at: new Date().toISOString(),
        steps,
      },
      extra || {}
    );
    const tmp = `${STEPS_JSON_PATH}.tmp-${process.pid}`;
    fs.writeFileSync(tmp, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
    fs.renameSync(tmp, STEPS_JSON_PATH);
  } catch (error) {
    console.error("[control-studio-evidence] failed to persist steps.json:", error);
  }
}

function recordStep(name, status, detail) {
  stepIndex += 1;
  steps.push({
    index: stepIndex,
    name,
    status,
    at: new Date().toISOString(),
    detail: detail === undefined ? null : detail,
  });
  persistSteps();
}

function slugify(name) {
  return String(name)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60);
}

/** Runs fn(), recording a passed/failed step either way. On failure it still tries a screenshot of
 * whatever is on screen before recording, then rethrows so Mocha's own pass/fail tally agrees with
 * steps.json. Mocha does not bail on a failed test, so the remaining steps still run. */
async function step(name, fn) {
  const startedAt = Date.now();
  try {
    const detail = await fn();
    recordStep(name, "passed", Object.assign({ duration_ms: Date.now() - startedAt }, detail || {}));
  } catch (error) {
    let failureScreenshot = null;
    try {
      failureScreenshot = await screenshot(`failed-${String(stepIndex + 1).padStart(2, "0")}-${slugify(name)}`);
    } catch (screenshotError) {
      // Screenshot tooling itself may be unavailable; still record the original failure below.
    }
    recordStep(name, "failed", {
      duration_ms: Date.now() - startedAt,
      error: String((error && error.stack) || (error && error.message) || error),
      screenshot: failureScreenshot,
    });
    throw error;
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ---------------------------------------------------------------------------------------------
// screenshot helper: /usr/bin/import -window root, then /usr/bin/convert ... -trim +repage
// ---------------------------------------------------------------------------------------------

function runBin(cmd, args) {
  return new Promise((resolve, reject) => {
    execFile(
      cmd,
      args,
      {
        env: Object.assign({}, process.env, { PATH: "/usr/bin:/bin" }),
        timeout: 20000,
        maxBuffer: 16 * 1024 * 1024,
      },
      (error, stdout, stderr) => {
        if (error) {
          reject(new Error(`${cmd} ${args.join(" ")} failed: ${error.message}${stderr ? `\n${stderr}` : ""}`));
          return;
        }
        resolve({ stdout, stderr });
      }
    );
  });
}

async function screenshot(name) {
  if (!SCREENSHOTS_DIR) {
    throw new Error("TRUST_CONTROL_STUDIO_EVIDENCE_DIR is not set; cannot write screenshots.");
  }
  fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });
  const raw = path.join(SCREENSHOTS_DIR, `${name}.raw.png`);
  const out = path.join(SCREENSHOTS_DIR, `${name}.png`);
  await runBin("/usr/bin/import", ["-window", "root", raw]);
  await runBin("/usr/bin/convert", [raw, "-bordercolor", "black", "-border", "1", "-trim", "+repage", out]);
  return out;
}

// ---------------------------------------------------------------------------------------------
// minimal CDP client (proven pattern; see
// docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/cdp_managed_run_target.js)
// ---------------------------------------------------------------------------------------------

function httpJson(requestPath) {
  return new Promise((resolve, reject) => {
    const request = http.get(`http://127.0.0.1:${CDP_PORT}${requestPath}`, (response) => {
      let body = "";
      response.on("data", (chunk) => {
        body += chunk;
      });
      response.on("end", () => {
        try {
          resolve(JSON.parse(body));
        } catch (error) {
          reject(error);
        }
      });
    });
    request.on("error", reject);
    request.setTimeout(5000, () => request.destroy(new Error(`HTTP timeout: ${requestPath}`)));
  });
}

function connectCdp(wsUrl) {
  if (!WebSocket) {
    return Promise.reject(new Error("the bundled 'ws' module is not resolvable from node_modules"));
  }
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(wsUrl, { perMessageDeflate: false, maxPayload: 256 * 1024 * 1024 });
    let nextId = 0;
    const pending = new Map();
    socket.on("message", (data) => {
      let message;
      try {
        message = JSON.parse(data.toString());
      } catch (error) {
        return;
      }
      if (message.id !== undefined && pending.has(message.id)) {
        const entry = pending.get(message.id);
        clearTimeout(entry.timer);
        pending.delete(message.id);
        if (message.error) {
          entry.reject(new Error(JSON.stringify(message.error)));
        } else {
          entry.resolve(message.result);
        }
      }
    });
    socket.on("error", reject);
    socket.on("open", () => {
      resolve({
        send(method, params, sessionId) {
          return new Promise((resolveSend, rejectSend) => {
            const id = (nextId += 1);
            const timer = setTimeout(() => {
              pending.delete(id);
              rejectSend(new Error(`CDP timeout after 5s: ${method}`));
            }, 5000);
            pending.set(id, { resolve: resolveSend, reject: rejectSend, timer });
            const payload = { id, method, params: params || {} };
            if (sessionId) {
              payload.sessionId = sessionId;
            }
            socket.send(JSON.stringify(payload));
          });
        },
        close() {
          try {
            socket.close();
          } catch (error) {
            // ignore
          }
        },
      });
    });
  });
}

/** Evaluates `body` (a JS statement block, already inside a function that has `doc` bound to the inner
 * webview document, or returns {ok:false,error:'NO_INNER_DOCUMENT'} if there isn't one yet) in the given
 * CDP session. */
async function evalInWebview(conn, sessionId, body) {
  const expression = `(function(){
    try {
      var outer = document.querySelector('iframe');
      var doc = outer && outer.contentDocument;
      if (!doc) { return { ok:false, error:'NO_INNER_DOCUMENT' }; }
      ${body}
    } catch (error) {
      return { ok:false, error: String((error && error.message) || error) };
    }
  })()`;
  const evaluation = await conn.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: false }, sessionId);
  if (evaluation.exceptionDetails) {
    throw new Error(`Runtime.evaluate exception: ${JSON.stringify(evaluation.exceptionDetails)}`);
  }
  return evaluation.result ? evaluation.result.value : undefined;
}

/** Finds the Control Studio webview among possibly several open "index.html" iframe targets (the VS Code
 * home sidebar view uses the same host page) by attaching and probing for the [data-control-studio] root,
 * detaching from any candidate that does not carry it. */
async function attachControlStudioWebview(conn, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let attempts = 0;
  let lastTargets = [];
  while (Date.now() < deadline) {
    attempts += 1;
    lastTargets = await httpJson("/json");
    const candidates = lastTargets.filter(
      (candidate) => /index\.html/.test(candidate.url || "") && (candidate.type === "iframe" || candidate.type === "page")
    );
    for (const candidate of candidates) {
      let sessionId;
      try {
        const attach = await conn.send("Target.attachToTarget", { targetId: candidate.id, flatten: true });
        sessionId = attach.sessionId;
        await conn.send("Runtime.enable", {}, sessionId);
        const marker = await evalInWebview(conn, sessionId, "return { ok: !!doc.querySelector('[data-control-studio]') };");
        if (marker && marker.ok) {
          return { targetId: candidate.id, sessionId };
        }
        await conn.send("Target.detachFromTarget", { sessionId }).catch(() => {});
      } catch (error) {
        // try the next candidate / next poll
      }
    }
    await sleep(500);
  }
  throw new Error(
    `Control Studio webview not found after ${attempts} attempts. Targets seen: ${JSON.stringify(
      lastTargets.map((candidate) => ({ type: candidate.type, url: candidate.url }))
    )}`
  );
}

async function waitForCondition(conn, sessionId, body, timeoutMs, pollMs) {
  const deadline = Date.now() + timeoutMs;
  let last;
  do {
    last = await evalInWebview(conn, sessionId, body);
    if (last && last.ok) {
      return last;
    }
    await sleep(pollMs);
  } while (Date.now() < deadline);
  return last;
}

async function clickSelector(conn, sessionId, selector) {
  return evalInWebview(
    conn,
    sessionId,
    `
      var el = doc.querySelector(${JSON.stringify(selector)});
      if (!el) { return { ok:false, error:'NOT_FOUND', selector: ${JSON.stringify(selector)} }; }
      if (el.hidden) { return { ok:false, error:'HIDDEN', selector: ${JSON.stringify(selector)}, text:(el.textContent||'').trim() }; }
      if (el.disabled) { return { ok:false, error:'DISABLED', selector: ${JSON.stringify(selector)}, text:(el.textContent||'').trim() }; }
      el.click();
      return { ok:true, text:(el.textContent||'').trim(), disabled: !!el.disabled };
    `
  );
}

async function clickModeTab(conn, sessionId, stageName) {
  return evalInWebview(
    conn,
    sessionId,
    `
      var buttons = [].slice.call(doc.querySelectorAll('[data-control-studio-mode]'));
      var target = buttons.find(function (b) { return (b.textContent||'').trim() === ${JSON.stringify(stageName)}; });
      if (!target) {
        return { ok:false, error:'TAB_NOT_FOUND', stage: ${JSON.stringify(stageName)}, available: buttons.map(function(b){return (b.textContent||'').trim();}) };
      }
      if (target.disabled) {
        return { ok:false, error:'TAB_DISABLED', stage: ${JSON.stringify(stageName)}, status: target.getAttribute('data-status') };
      }
      target.click();
      return { ok:true, status: target.getAttribute('data-status'), ariaLabel: target.getAttribute('aria-label') };
    `
  );
}

async function clickPlotTab(conn, sessionId, plotId) {
  return evalInWebview(
    conn,
    sessionId,
    `
      var el = doc.querySelector('[data-plot-tab=${JSON.stringify(plotId)}]');
      if (!el) { return { ok:false, error:'PLOT_TAB_NOT_FOUND', plotId: ${JSON.stringify(plotId)} }; }
      if (el.disabled) { return { ok:false, error:'PLOT_TAB_DISABLED', plotId: ${JSON.stringify(plotId)} }; }
      el.click();
      return { ok:true };
    `
  );
}

// One comprehensive DOM snapshot, reused at every step so steps.json is checkable without eyes: header
// identity/connection/source, the SP/PV/MV value-strip tiles, the five workflow tabs (status + aria
// label, W-14/V-06), workflow pane visibility, the Test plan card (W-17's "affected signal"), and the
// Abort button's on-screen geometry (W-12).
const SNAPSHOT_BODY = `
  function txt(sel) { var e = doc.querySelector(sel); return e ? (e.textContent||'').trim() : null; }
  function attrOf(sel, name) { var e = doc.querySelector(sel); return e ? e.getAttribute(name) : null; }
  var panes = [].slice.call(doc.querySelectorAll('[data-workflow-pane]')).map(function (p) {
    return { pane: p.getAttribute('data-workflow-pane'), hidden: !!p.hidden };
  });
  var values = [].slice.call(doc.querySelectorAll('.values .value')).map(function (v) {
    var name = v.querySelector('.value-name');
    var reading = v.querySelector('.value-reading');
    return { name: name ? (name.textContent||'').trim() : null, reading: reading ? (reading.textContent||'').trim() : null };
  });
  var testAbort = doc.querySelector('[data-test-abort]');
  var abortRect = null;
  var abortVisibleInViewport = null;
  if (testAbort) {
    var r = testAbort.getBoundingClientRect();
    var viewportHeight = (doc.defaultView || window).innerHeight;
    var viewportWidth = (doc.defaultView || window).innerWidth;
    abortRect = { top: r.top, left: r.left, width: r.width, height: r.height };
    abortVisibleInViewport = r.width > 0 && r.height > 0 && r.top >= 0 && r.left >= 0 && r.bottom <= viewportHeight && r.right <= viewportWidth;
  }
  return {
    ok: true,
    header: {
      loop: txt('[data-control-studio-loop]'),
      source: txt('[data-control-studio-source]'),
      runtime: txt('[data-control-studio-runtime]'),
      connection: txt('[data-control-studio-connection]'),
      controllerInstance: txt('[data-control-studio-controller-instance]'),
      controllerMode: txt('[data-control-studio-controller-mode]'),
      controllerRevision: txt('[data-control-studio-controller-revision]'),
      candidateRevision: txt('[data-control-studio-candidate-revision]'),
      freshness: txt('[data-control-studio-freshness]'),
    },
    panes: panes,
    valueStrip: values,
    quality: txt('[data-control-studio-quality]'),
    recording: {
      startDisabled: (function(){ var e = doc.querySelector('[data-control-studio-record-start]'); return e ? !!e.disabled : null; })(),
      stopDisabled: (function(){ var e = doc.querySelector('[data-control-studio-record-stop]'); return e ? !!e.disabled : null; })(),
    },
    test: {
      state: txt('[data-test-state]'),
      signal: txt('[data-test-signal]'),
      bound: txt('[data-test-bound]'),
      timeout: txt('[data-test-timeout]'),
      restoration: txt('[data-test-restoration]'),
      stops: txt('[data-test-stops]'),
      reviewHidden: (function(){ var e = doc.querySelector('[data-test-review]'); return e ? !!e.hidden : null; })(),
      approveHidden: (function(){ var e = doc.querySelector('[data-test-approve]'); return e ? !!e.hidden : null; })(),
      abortDisabled: (function(){ var e = doc.querySelector('[data-test-abort]'); return e ? !!e.disabled : null; })(),
      abortRect: abortRect,
      abortVisibleInViewport: abortVisibleInViewport,
    },
    viewport: { width: (doc.defaultView||window).innerWidth, height: (doc.defaultView||window).innerHeight },
  };
`;

async function snapshot(conn, sessionId) {
  return evalInWebview(conn, sessionId, SNAPSHOT_BODY);
}

// Geometry check for the Test-stage action row (Review test / Confirm and start / Abort test)
// against the Amplitude and Maximum-duration form fields. A prior build pinned the action row so
// it floated over these fields at rest; this makes the non-overlap checkable from steps.json
// without opening a screenshot.
const TEST_FORM_OVERLAP_BODY = `
  function rectOf(sel) {
    var e = doc.querySelector(sel);
    if (!e) { return null; }
    var r = e.getBoundingClientRect();
    return { top: r.top, left: r.left, right: r.right, bottom: r.bottom, width: r.width, height: r.height };
  }
  function intersects(a, b) {
    if (!a || !b) { return null; }
    return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
  }
  var amplitudeRect = rectOf('#test-amplitude');
  var durationRect = rectOf('#test-duration');
  var actionRowRect = rectOf('[data-workflow-pane="Test"] .workspace-actions');
  return {
    ok: true,
    amplitudeRect: amplitudeRect,
    durationRect: durationRect,
    actionRowRect: actionRowRect,
    actionRowOverlapsAmplitude: intersects(actionRowRect, amplitudeRect),
    actionRowOverlapsDuration: intersects(actionRowRect, durationRect),
  };
`;

async function testFormOverlapCheck(conn, sessionId) {
  return evalInWebview(conn, sessionId, TEST_FORM_OVERLAP_BODY);
}

async function cmd(id) {
  try {
    return await vscode.commands.executeCommand(id);
  } catch (error) {
    return { __error: String((error && error.message) || error) };
  }
}

// ---------------------------------------------------------------------------------------------
// journey state shared across steps
// ---------------------------------------------------------------------------------------------

let conn = null;
let sessionId = null;
let windowId = null;
let debugSession = null;

async function setWindowBounds(bounds) {
  if (!windowId) {
    return false;
  }
  try {
    await conn.send("Browser.setWindowBounds", { windowId, bounds });
    await sleep(500);
    return true;
  } catch (error) {
    // Best-effort: the layout matrix steps still capture whatever the window's actual size is.
    return false;
  }
}

function registerTests() {
  suite("Control Studio Phase 3 rendered evidence (W-13)", function () {
    this.timeout(0);

    suiteSetup(async function () {
      this.timeout(20000);
      if (!EVIDENCE_DIR) {
        throw new Error("TRUST_CONTROL_STUDIO_EVIDENCE_DIR must be set.");
      }
      fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });
      persistSteps();
      await cmd("workbench.action.closePanel");
      await cmd("workbench.action.closeAuxiliaryBar");
      await cmd("workbench.action.closeSidebar");
    });

    test("start the Simulator run target (trust-lsp.debug.start)", async function () {
      await step("start-simulator-run-target", async () => {
        const folder = vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders[0];
        if (!folder) {
          throw new Error("No workspace folder is open.");
        }
        const alreadyActive = vscode.debug.activeDebugSession;
        const needsWait = !(alreadyActive && alreadyActive.type === "structured-text");
        const startedSessionPromise = needsWait
          ? new Promise((resolve, reject) => {
              const timer = setTimeout(() => {
                disposable.dispose();
                reject(new Error("Timed out waiting for a structured-text debug session to start."));
              }, 30000);
              const disposable = vscode.debug.onDidStartDebugSession((session) => {
                if (session.type !== "structured-text") {
                  return;
                }
                clearTimeout(timer);
                disposable.dispose();
                resolve(session);
              });
            })
          : Promise.resolve(alreadyActive);
        const startResult = await vscode.commands.executeCommand("trust-lsp.debug.start");
        if (startResult === false) {
          throw new Error("trust-lsp.debug.start returned false (configuration/diagnostics error).");
        }
        debugSession = await startedSessionPromise;
        return { workspace: folder.uri.fsPath, debugSessionName: debugSession.name };
      });
    });

    test("open Control Studio through trust-lsp.controlStudio.open with the workspace's real loop", async function () {
      await step("open-control-studio", async () => {
        const projectLoopPath = path.join(OUT_DIR, "controlStudio", "projectLoop.js");
        const selectedRuntimePath = path.join(OUT_DIR, "selectedRuntime.js");
        const projectLoopModule = require(projectLoopPath);
        let runtimeId = "simulator";
        try {
          const selectedRuntimeModule = require(selectedRuntimePath);
          runtimeId = selectedRuntimeModule.getSelectedRuntimeId();
        } catch (error) {
          // Falls back to the literal default; control-loop.json for this lesson uses "simulator" too.
        }
        const resolution = await projectLoopModule.controlStudioRequestForWorkspace(runtimeId);
        if (resolution.kind !== "accepted") {
          throw new Error(`controlStudioRequestForWorkspace: ${resolution.kind} - ${resolution.reason}`);
        }
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", resolution.request);
        await sleep(1500);

        const version = await httpJson("/json/version");
        conn = await connectCdp(version.webSocketDebuggerUrl);

        // Essential: attach to the Control Studio webview first. Everything else in this journey
        // depends on `sessionId` being set; window-bounds control (below) is comparatively a nice
        // to have for the layout matrix and must never be allowed to abort this step before the
        // attach happens.
        const attached = await attachControlStudioWebview(conn, 25000);
        sessionId = attached.sessionId;

        let pageTargetId = null;
        let windowBoundsAvailable = false;
        try {
          const targets = await httpJson("/json");
          const pageTarget = targets.find((candidate) => candidate.type === "page");
          if (pageTarget) {
            pageTargetId = pageTarget.id;
            const windowInfo = await conn.send("Browser.getWindowForTarget", { targetId: pageTarget.id });
            windowId = windowInfo && windowInfo.windowId;
            if (windowId) {
              windowBoundsAvailable = await setWindowBounds(WIDE_BOUNDS);
            }
          }
        } catch (error) {
          // Some Electron/CDP builds do not implement the Browser domain (observed: "'Browser.
          // getWindowForTarget' wasn't found"). The layout matrix still runs; wide/split/narrow
          // screenshots just reflect the default window size instead of a deliberately varied one.
          windowId = null;
          windowBoundsAvailable = false;
        }

        return {
          runtimeId,
          loopId: resolution.request.loopDefinition.id,
          pageTargetId,
          windowId,
          windowBoundsAvailable,
        };
      });
    });

    test("wait for header Simulation/Running, capture Monitor -> Response", async function () {
      await step("monitor-response", async () => {
        const condition = await waitForCondition(
          conn,
          sessionId,
          `
            var connectionText = (doc.querySelector('[data-control-studio-connection]')||{}).textContent || '';
            var sourceText = (doc.querySelector('[data-control-studio-source]')||{}).textContent || '';
            connectionText = connectionText.trim();
            sourceText = sourceText.trim();
            return { ok: connectionText === 'Running' && sourceText === 'Simulation', connectionText: connectionText, sourceText: sourceText };
          `,
          30000,
          500
        );
        if (!condition || !condition.ok) {
          throw new Error(`Timed out waiting for header Simulation/Running. Last seen: ${JSON.stringify(condition)}`);
        }
        try {
          await clickPlotTab(conn, sessionId, "response");
        } catch (error) {
          // Response is the default plot view; a failure here is not fatal to the journey.
        }
        await sleep(300);
        const dom = await snapshot(conn, sessionId);
        const shot = await screenshot("01-monitor-response");
        return { condition, dom, screenshot: shot };
      });
    });

    test("start recording", async function () {
      await step("start-recording", async () => {
        const click = await clickSelector(conn, sessionId, "[data-control-studio-record-start]");
        if (!click || !click.ok) {
          throw new Error(`Start recording control not clickable: ${JSON.stringify(click)}`);
        }
        await sleep(1000);
        const dom = await snapshot(conn, sessionId);
        const shot = await screenshot("02-recording");
        return { click, dom, screenshot: shot };
      });
    });

    test("open Test tab and review the bounded test plan", async function () {
      await step("test-review", async () => {
        const tabClick = await clickModeTab(conn, sessionId, "Test");
        if (!tabClick || !tabClick.ok) {
          throw new Error(`Test tab not reachable: ${JSON.stringify(tabClick)}`);
        }
        await sleep(300);
        const paneVisible = await evalInWebview(
          conn,
          sessionId,
          `
            var pane = doc.querySelector('[data-workflow-pane="Test"]');
            return { ok: !!pane && !pane.hidden, hidden: pane ? !!pane.hidden : null };
          `
        );
        if (!paneVisible || !paneVisible.ok) {
          throw new Error(`Test workflow pane did not become visible after the tab click: ${JSON.stringify(paneVisible)}`);
        }
        const reviewClick = await clickSelector(conn, sessionId, "[data-test-review]");
        if (!reviewClick || !reviewClick.ok) {
          throw new Error(`Review test control not clickable: ${JSON.stringify(reviewClick)}`);
        }
        await sleep(800);
        const dom = await snapshot(conn, sessionId);
        const formOverlapCheck = await testFormOverlapCheck(conn, sessionId);
        const shot = await screenshot("03-test-review");
        return { tabClick, reviewClick, dom, formOverlapCheck, screenshot: shot };
      });
    });

    test("confirm and start; verify Abort stays visible while running (W-12)", async function () {
      await step("confirm-and-start-running", async () => {
        const approveClick = await clickSelector(conn, sessionId, "[data-test-approve]");
        if (!approveClick || !approveClick.ok) {
          throw new Error(`Confirm and start control not clickable: ${JSON.stringify(approveClick)}`);
        }
        const running = await waitForCondition(
          conn,
          sessionId,
          `
            var abortEl = doc.querySelector('[data-test-abort]');
            var stateEl = doc.querySelector('[data-test-state]');
            return { ok: !!abortEl && !abortEl.disabled, abortDisabled: abortEl ? !!abortEl.disabled : null, state: stateEl ? (stateEl.textContent||'').trim() : null };
          `,
          20000,
          500
        );
        if (!running || !running.ok) {
          throw new Error(`Test did not enter a running state (Abort stayed disabled): ${JSON.stringify(running)}`);
        }
        const domWide = await snapshot(conn, sessionId);
        const shotWide = await screenshot("04-test-running");

        // W-12 check (docs/internal/reviews/control-course-review-2026-09-03.md): is Abort still on
        // screen without scrolling while a test runs? Attempt the 1440x900 reproduction window via
        // CDP (best-effort; some Electron/CDP builds do not implement Browser.setWindowBounds, see
        // open-control-studio's windowBoundsAvailable), but always take the screenshot and DOM/
        // geometry snapshot regardless, at whatever window size is actually in effect, so this
        // required piece of evidence is never silently skipped.
        const windowResizedForCheck = await setWindowBounds(CONSTRAINED_BOUNDS);
        const constrained = await snapshot(conn, sessionId);
        const constrainedShot = await screenshot("11-abort-visible-while-running");
        if (windowResizedForCheck) {
          await setWindowBounds(WIDE_BOUNDS);
        }

        return {
          approveClick,
          running,
          domWide,
          screenshotWide: shotWide,
          windowResizedForConstrainedCheck: windowResizedForCheck,
          constrainedCheck: constrained,
          screenshotConstrained: constrainedShot,
        };
      });
    });

    test("wait for the 95.1 s experiment to reach a terminal state", async function () {
      await step("test-complete", async () => {
        const completion = await waitForCondition(
          conn,
          sessionId,
          `
            var abortEl = doc.querySelector('[data-test-abort]');
            var stateEl = doc.querySelector('[data-test-state]');
            var stateText = stateEl ? (stateEl.textContent||'').trim() : '';
            var stillRunning = stateText.indexOf('Test running') === 0;
            var terminal = !!abortEl && !!abortEl.disabled && !stillRunning;
            return { ok: terminal, abortDisabled: abortEl ? !!abortEl.disabled : null, state: stateText };
          `,
          130000,
          1000
        );
        if (!completion || !completion.ok) {
          throw new Error(`Test did not reach a terminal state within budget: ${JSON.stringify(completion)}`);
        }
        const dom = await snapshot(conn, sessionId);
        const shot = await screenshot("05-test-complete");
        return { completion, dom, screenshot: shot };
      });
    });

    test("layout matrix: wide", async function () {
      await step("layout-wide", async () => {
        try {
          await clickModeTab(conn, sessionId, "Monitor");
        } catch (error) {
          // non-fatal
        }
        await setWindowBounds(WIDE_BOUNDS);
        const dom = await snapshot(conn, sessionId);
        const shot = await screenshot("06-wide");
        return { dom, screenshot: shot };
      });
    });

    test("layout matrix: split editor", async function () {
      await step("layout-split", async () => {
        await cmd("workbench.action.splitEditorRight");
        await sleep(700);
        const dom = await snapshot(conn, sessionId);
        const shot = await screenshot("07-split");
        await cmd("workbench.action.joinAllGroups");
        await sleep(400);
        return { dom, screenshot: shot };
      });
    });

    test("layout matrix: narrow window", async function () {
      await step("layout-narrow", async () => {
        // Browser.setWindowBounds is not implemented on every Electron/CDP build (see
        // open-control-studio's windowBoundsAvailable). Whether or not it worked, also force a
        // genuinely narrower Control Studio column via a 3-way editor split, which depends only on
        // ordinary VS Code commands and needs no CDP window control.
        const windowResized = await setWindowBounds(NARROW_BOUNDS);
        await cmd("workbench.action.splitEditorRight");
        await sleep(300);
        await cmd("workbench.action.splitEditorRight");
        await sleep(500);
        const dom = await snapshot(conn, sessionId);
        const shot = await screenshot("08-narrow");
        await cmd("workbench.action.joinAllGroups");
        await sleep(300);
        await setWindowBounds(WIDE_BOUNDS);
        return { dom, screenshot: shot, windowResized };
      });
    });

    test("layout matrix: 200% zoom (V-04)", async function () {
      await step("layout-zoom-200", async () => {
        for (let i = 0; i < 4; i += 1) {
          await cmd("workbench.action.zoomIn");
          await sleep(150);
        }
        await sleep(300);
        const dom = await snapshot(conn, sessionId);
        const shot = await screenshot("09-zoom200");
        for (let i = 0; i < 4; i += 1) {
          await cmd("workbench.action.zoomOut");
          await sleep(150);
        }
        await sleep(300);
        return { dom, screenshot: shot };
      });
    });

    test("layout matrix: high contrast (V-06/V-18)", async function () {
      await step("layout-high-contrast", async () => {
        await cmd("workbench.action.toggleHighContrast");
        await sleep(700);
        const dom = await snapshot(conn, sessionId);
        const shot = await screenshot("10-high-contrast");
        await cmd("workbench.action.toggleHighContrast");
        await sleep(300);
        return { dom, screenshot: shot };
      });
    });

    suiteTeardown(async function () {
      this.timeout(30000);
      try {
        if (conn) {
          conn.close();
        }
      } catch (error) {
        // ignore
      }
      try {
        if (debugSession) {
          await vscode.debug.stopDebugging(debugSession);
        }
      } catch (error) {
        // ignore
      }
      persistSteps({ completed_at: new Date().toISOString() });
    });
  });
}

exports.run = function run() {
  const mocha = new Mocha({ ui: "tdd", color: true, timeout: 0 });
  mocha.suite.emit("pre-require", global, __filename, mocha);
  registerTests();
  return new Promise((resolve, reject) => {
    try {
      mocha.run((failures) => {
        persistSteps({ mocha_failures: failures });
        if (failures > 0) {
          reject(new Error(`${failures} Control Studio evidence step(s) failed; see steps.json for detail.`));
        } else {
          resolve();
        }
      });
    } catch (error) {
      reject(error);
    }
  });
};
