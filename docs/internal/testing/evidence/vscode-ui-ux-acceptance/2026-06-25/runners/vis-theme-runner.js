// VIS theme proof: open visual editors under one VS Code theme, capture PNGs,
// and record computed product-chrome styles.
//
// Usage:
//   VIS_THEME=dark VIS_WIDTH=960 VIS_HEIGHT=760 xvfb-run -a node .../runners/vis-theme-runner.js
//   VIS_THEME=light xvfb-run -a node .../runners/vis-theme-runner.js
//   VIS_THEME=hc xvfb-run -a node .../runners/vis-theme-runner.js
const path = require("path");
const fs = require("fs");
const http = require("http");
const cp = require("child_process");
const repo = process.env.TRUST_REPO || "/home/johannes/projects/trust-platform";
const ext = path.join(repo, "editors/vscode");
const pngHygienePath = path.join(__dirname, "png-hygiene.js");
const plantScenariosPath = path.join(__dirname, "plant-simulation-scenarios.js");
const { runTests } = require(path.join(ext, "node_modules/@vscode/test-electron"));
const evidenceRoot = process.env.TRUST_UX_EVIDENCE_ROOT
  ? path.resolve(process.env.TRUST_UX_EVIDENCE_ROOT)
  : path.join(repo, "docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25");
const screenshotsDir = process.env.TRUST_UX_SCREENSHOTS_DIR
  ? path.resolve(process.env.TRUST_UX_SCREENSHOTS_DIR)
  : path.join(evidenceRoot, "screenshots-raw");
const jsonDir = process.env.TRUST_UX_JSON_DIR
  ? path.resolve(process.env.TRUST_UX_JSON_DIR)
  : path.join(evidenceRoot, "json");
const themeKey = process.env.VIS_THEME || "light";
const requestedWidth = Number(process.env.VIS_WIDTH || "0");
const requestedHeight = Number(process.env.VIS_HEIGHT || "0");
const requestedZoomLevel = Number(process.env.VIS_ZOOM_LEVEL || "0");
const themes = {
  dark: { names: ["Dark Modern", "Default Dark Modern"], suffix: "dark", expectedKinds: ["Dark"] },
  light: { names: ["Light Modern", "Default Light Modern"], suffix: "light", expectedKinds: ["Light"] },
  hc: {
    names: [
      "Default High Contrast",
      "Dark High Contrast",
      "hc-dark",
      "High Contrast",
      "hc-black"
    ],
    suffix: "high-contrast",
    expectedKinds: ["HighContrast"]
  },
};
const theme = themes[themeKey];
if (!theme) {
  throw new Error(`Unknown VIS_THEME ${themeKey}; expected ${Object.keys(themes).join(", ")}`);
}

const runRoot = path.join(evidenceRoot, "runner-output", `vis-theme-${theme.suffix}`);
const testsDir = path.join(runRoot, "tests");
const outDir = path.join(runRoot, "out");
const ws = path.join(runRoot, "ws");
const PORT = themeKey === "hc" ? 9368 : 9367;
const captureSuffix =
  requestedWidth > 0 && requestedHeight > 0
    ? `${theme.suffix}-w${requestedWidth}`
    : theme.suffix;

for (const dir of [testsDir, outDir, ws]) {
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
}
for (const dir of [screenshotsDir, jsonDir]) {
  fs.mkdirSync(dir, { recursive: true });
}

function copyDirContents(src, dest) {
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name);
    const to = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      fs.mkdirSync(to, { recursive: true });
      copyDirContents(from, to);
    } else {
      fs.copyFileSync(from, to);
    }
  }
}

// Devices & Connections is the visual baseline for visual-editor product chrome.
// Keep the baseline in the same clean VS Code run/theme as the visual-editor shots.
copyDirContents(path.join(repo, "examples/network_canvas_demo"), ws);
const expectPlant = process.env.VIS_PLANT_FIXTURE === "1";
const plantActivation = process.env.VIS_PLANT_ACTIVATION || "";
const plantDrag = process.env.VIS_PLANT_DRAG || "";
const plantCancel = process.env.VIS_PLANT_CANCEL === "1";
const plantEditor = process.env.VIS_PLANT_EDITOR || "";
const plantScenario = process.env.VIS_PLANT_SCENARIO || "";
if (["signal-picker", "manual-address", "signal-conflicts", "save", "save-conflict", "state-space-save", "control-studio-handoff", "engineering-settings", "dynamics-editor", "imperfection-editor"].includes(plantScenario)) {
  fs.copyFileSync(
    path.join(repo, "editors/vscode/src/test/fixtures/control-engineering/offline-project/src/main.st"),
    path.join(ws, "src/main.st")
  );
  const oldConfig = path.join(ws, "src/config.st");
  if (fs.existsSync(oldConfig)) fs.unlinkSync(oldConfig);
}
if (plantScenario === "signal-conflicts") {
  fs.writeFileSync(
    path.join(ws, "simulation.toml"),
    `[simulation]\nenabled = true\nplant_step_ms = 10\n\n[[plants]]\nid = "existing_plant"\nname = "Existing plant"\nmodel = "first_order_dead_time"\ncommand_sources = ["%QL0"]\nfeedback_targets = ["%IL0"]\n\n[plants.parameters]\ngain = 1.0\ntime_constant_s = 5.0\ndead_time_s = 0.5\n`
  );
  fs.writeFileSync(
    path.join(ws, "io.toml"),
    `[io]\nsafe_state = []\n\n[[io.drivers]]\nname = "gpio"\nenabled = true\nparams = { backend = "sysfs", sysfs_base = "/sys/class/gpio", inputs = [{ address = "%IX0.0", line = 17 }], outputs = [{ address = "%QX0.0", line = 27 }] }\n`
  );
}
if (["engineering-settings", "dynamics-editor", "imperfection-editor", "scenario-events"].includes(plantScenario)) {
  fs.writeFileSync(
    path.join(ws, "simulation.toml"),
    `[simulation]\nenabled = true\nplant_step_ms = 10\n`
  );
}
if (plantScenario === "control-studio-handoff") {
  const projectConfig = path.join(ws, "trust-lsp.toml");
  fs.writeFileSync(
    projectConfig,
    fs.readFileSync(projectConfig, "utf8").replace(
      "unix:///tmp/trust-debug.sock",
      "unix:///tmp/trust-runtime-line-a.sock"
    )
  );
  const runtimeConfig = path.join(ws, "runtime.toml");
  fs.writeFileSync(
    runtimeConfig,
    fs.readFileSync(runtimeConfig, "utf8").replace(
      'name = "Simulator"',
      'name = "Res"'
    )
  );
}
if (expectPlant) {
  fs.writeFileSync(
    path.join(ws, "simulation.toml"),
    plantScenario === "remove"
      ? `# retained heading\n[simulation]\nenabled = true\nplant_step_ms = 10\n\n[[plants]]\nid = "tank_level"\nname = "Tank level"\ndescription = "A slow training process"\nmodel = "first_order_dead_time"\ncommand_sources = ["%QL0"]\nfeedback_targets = ["%IL0"]\ninitial_state = [0.0]\n\n[plants.parameters]\ngain = 1.0\ntime_constant_s = 5.0\ndead_time_s = 0.8\n\n# keep this plant and event\n[[plants]]\nid = "oven"\nname = "Oven"\nmodel = "first_order"\ncommand_sources = ["%QL8"]\nfeedback_targets = ["%IL8"]\ninitial_state = [0.0]\n\n[plants.parameters]\ngain = 2.0\ntime_constant_s = 20.0\n\n[[plant_events]]\nat_ms = 100\nplant = "tank_level"\nkind = "set_state"\nchannel = 0\nvalue = 3.0\n\n[[plant_events]]\nat_ms = 250\nplant = "oven"\nkind = "reset_plant"\n`
      : `[simulation]\nenabled = true\nplant_step_ms = 10\n\n[[plants]]\nid = "tank_level"\nname = "Tank level"\ndescription = "A slow training process"\nmodel = "first_order_dead_time"\ncommand_sources = ["%QL0"]\nfeedback_targets = ["%IL0"]\n\n[plants.parameters]\ngain = 1.0\ntime_constant_s = 5.0\ndead_time_s = 0.8\n`
  );
}

function findOne(patterns) {
  const files = [];
  (function walk(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (!/node_modules|\.git/.test(p)) {
          walk(p);
        }
      } else {
        files.push(p);
      }
    }
  })(path.join(repo, "examples"));
  for (const pattern of patterns) {
    const found = files.find((file) => pattern.test(file));
    if (found) {
      return found;
    }
  }
  return null;
}

const fixtures = {
  sfc: findOne([/ethercat-snake-advanced\.sfc\.json$/, /\.sfc\.json$/, /\.sfc$/]),
  statechart: findOne([/traffic-light\.statechart\.json$/, /\.statechart\.json$/]),
  ladder: findOne([/ethercat-snake\.ladder\.json$/, /\.ladder\.json$/]),
  blockly: findOne([/snake-simple-v2\.blockly\.json$/, /\.blockly\.json$/]),
};
const copied = {};
for (const [key, source] of Object.entries(fixtures)) {
  if (source) {
    const dest = path.join(ws, path.basename(source));
    fs.copyFileSync(source, dest);
    copied[key] = dest;
  }
}
const invalidPath = path.join(ws, "broken.statechart.json");
fs.writeFileSync(invalidPath, '{ "this is": not valid json,, ');
copied.invalid = invalidPath;

fs.mkdirSync(path.join(outDir, "ud", "User"), { recursive: true });
fs.writeFileSync(
  path.join(outDir, "ud", "User", "settings.json"),
  JSON.stringify({
    "window.titleBarStyle": "native",
    "window.commandCenter": false,
    "chat.commandCenter.enabled": false,
    "workbench.layoutControl.enabled": false,
    "workbench.startupEditor": "none",
    "workbench.tips.enabled": false,
    "window.zoomLevel": requestedZoomLevel,
    "window.autoDetectHighContrast": themeKey === "hc",
    "workbench.preferredHighContrastColorTheme": "Default High Contrast",
    "telemetry.telemetryLevel": "off",
    "update.mode": "none",
    "git.enabled": false,
    "git.openRepositoryInParentFolders": "never",
    "workbench.colorTheme": theme.names[0],
    ...(plantScenario === "control-studio-handoff"
      ? {
          "trust.runtime.mode": "online",
          "trust.runtime.controlEndpoint": "unix:///tmp/trust-runtime-line-a.sock",
          "trust.runtime.useControlEndpoint": true,
        }
      : {}),
  })
);

const codeDir = fs
  .readdirSync(path.join(ext, ".vscode-test"))
  .filter(
    (dir) =>
      dir.startsWith("vscode-linux-arm64-") || dir.startsWith("vscode-linux-x64-")
  )
  .sort()
  .pop();
if (!codeDir) {
  throw new Error("No Linux VS Code test build found under editors/vscode/.vscode-test");
}
const codeBin = path.join(ext, ".vscode-test", codeDir, "code");

fs.writeFileSync(
  path.join(testsDir, "index.js"),
  `
const path = require("path");
const fs = require("fs");
const http = require("http");
const cp = require("child_process");
const vscode = require("vscode");
const WebSocket = require(${JSON.stringify(path.join(ext, "node_modules/ws"))});
const pngHygiene = require(${JSON.stringify(pngHygienePath)});
const { runPlantSimulationScenario } = require(${JSON.stringify(plantScenariosPath)});

const outDir = ${JSON.stringify(outDir)};
const screenshotsDir = ${JSON.stringify(screenshotsDir)};
const jsonDir = ${JSON.stringify(jsonDir)};
const copied = ${JSON.stringify(copied)};
const PORT = ${PORT};
const suffix = ${JSON.stringify(captureSuffix)};
const requestedWindow = ${JSON.stringify(
    requestedWidth > 0 && requestedHeight > 0
      ? { width: requestedWidth, height: requestedHeight }
      : undefined
  )};
const expectPlant = ${JSON.stringify(expectPlant)};
const plantActivation = ${JSON.stringify(plantActivation)};
const plantDrag = ${JSON.stringify(plantDrag)};
const plantCancel = ${JSON.stringify(plantCancel)};
const plantEditor = ${JSON.stringify(plantEditor)};
const plantScenario = ${JSON.stringify(plantScenario)};
const VIEW = {
  sfc: "trust-lsp.sfc.editor",
  statechart: "trust-lsp.statechartEditor",
  ladder: "trust-lsp.ladder.editor",
  blockly: "trust-lsp.blockly.editor",
  invalid: "trust-lsp.statechartEditor",
};
const SHOT = {
  devicesSummary: "VIS-00-devices-summary-" + suffix,
  devicesAdd: "VIS-00-devices-add-" + suffix,
  liveValues: "VIS-00-live-values-disconnected-" + suffix,
  sfc: "VIS-01-sfc-" + suffix,
  statechart: "VIS-02-statechart-" + suffix,
  ladder: "VIS-03-ladder-" + suffix,
  blockly: "VIS-04-blockly-" + suffix,
  invalid: "VIS-06-invalid-model-" + suffix,
  focus: {
    sfc: "VIS-01-sfc-focus-" + suffix,
    statechart: "VIS-02-statechart-focus-" + suffix,
    ladder: "VIS-03-ladder-focus-" + suffix,
    blockly: "VIS-04-blockly-focus-" + suffix,
  },
};
const ATTACH_TEXT = {
  sfc: "SFC editor",
  statechart: "Statechart editor",
  ladder: "Ladder editor",
  blockly: "Blockly editor",
  invalid: "Could not open this statechart",
};

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function httpJson(endpoint) {
  return new Promise((resolve, reject) => {
    const req = http.get("http://localhost:" + PORT + endpoint, (res) => {
      let body = "";
      res.on("data", (chunk) => (body += chunk));
      res.on("end", () => {
        try {
          resolve(JSON.parse(body));
        } catch (error) {
          reject(error);
        }
      });
    });
    req.on("error", reject);
    req.setTimeout(5000, () => req.destroy(new Error("http timeout")));
  });
}

function screenshot(name) {
  const raw = path.join(outDir, name + ".raw.png");
  const dest = path.join(screenshotsDir, name + ".png");
  const env = Object.assign({}, process.env, {
    PATH: "/usr/bin:/bin:" + (process.env.PATH || ""),
  });
  cp.execFileSync("/usr/bin/import", ["-window", "root", raw], {
    stdio: "ignore",
    env,
  });
  pngHygiene.stripPngFile(raw);
  try {
    cp.execFileSync(
      "/usr/bin/convert",
      [raw, "-strip", "-bordercolor", "black", "-border", "1", "-trim", "+repage", dest],
      { stdio: "ignore", env }
    );
  } catch (_) {
    fs.copyFileSync(raw, dest);
  }
  pngHygiene.stripPngFile(dest);
}

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    let id = 0;
    const pending = new Map();
    ws.on("message", (data) => {
      const msg = JSON.parse(data.toString());
      if (msg.id && pending.has(msg.id)) {
        pending.get(msg.id)(msg);
        pending.delete(msg.id);
      }
    });
    ws.on("error", reject);
    ws.on("open", () =>
      resolve({
        send: (method, params, sessionId) =>
          new Promise((done) => {
            const callId = ++id;
            let settled = false;
            pending.set(callId, (value) => {
              if (!settled) {
                settled = true;
                done(value);
              }
            });
            setTimeout(() => {
              if (!settled) {
                settled = true;
                done({ timeout: method });
              }
            }, 8000);
            ws.send(JSON.stringify({ id: callId, method, params: params || {}, sessionId }));
          }),
        close: () => ws.close(),
      })
    );
  });
}

const STYLE_FIELDS = {
  header: ["color", "backgroundColor", "borderBottomColor", "fontWeight", "fontSize", "textTransform", "padding"],
  title: ["color", "backgroundColor", "fontWeight", "fontSize", "textTransform", "padding"],
  sectionTitle: ["color", "backgroundColor", "fontWeight", "fontSize", "textTransform", "padding"],
  inspector: ["color", "backgroundColor", "borderLeftColor", "fontWeight", "fontSize", "textTransform", "padding"],
  button: ["color", "backgroundColor", "borderColor", "fontWeight", "fontSize", "textTransform", "padding", "borderLeftColor", "borderBottomColor"],
  controls: ["color", "backgroundColor", "borderColor", "fontWeight", "fontSize", "textTransform", "padding"],
  controlsButton: ["color", "backgroundColor", "borderColor", "fontWeight", "fontSize", "textTransform", "padding", "width", "height"],
};

function compareStyle(role, actual, expected) {
  const fields = STYLE_FIELDS[role] || [];
  const mismatches = [];
  if (!actual || !expected) {
    return { ok: false, mismatches: [{ field: "element", actual: Boolean(actual), expected: Boolean(expected) }] };
  }
  for (const field of fields) {
    if (actual[field] !== expected[field]) {
      mismatches.push({ field, actual: actual[field], expected: expected[field] });
    }
  }
  return { ok: mismatches.length === 0, mismatches };
}

async function attachByText(conn, wantedText) {
  const targets = await httpJson("/json");
  let fallbackSessionId;
  for (const target of targets.filter((candidate) => candidate.type === "iframe")) {
    const attached = await conn.send("Target.attachToTarget", {
      targetId: target.id,
      flatten: true,
    });
    const sessionId = attached.result && attached.result.sessionId;
    if (!sessionId) {
      continue;
    }
    await conn.send("Runtime.enable", {}, sessionId);
    const probe = await conn.send(
      "Runtime.evaluate",
      {
        expression:
          "(function(){try{var f=document.querySelector('iframe');var d=(f&&f.contentDocument)||document;var text=(d.body&&d.body.innerText||'').slice(0,5000);return {text:text,visibility:d.visibilityState||document.visibilityState,hidden:Boolean(d.hidden),url:String(d.location&&d.location.href||document.location.href)};}catch(e){return {text:'ERR:'+e.message,visibility:'error',hidden:true};}})()",
        returnByValue: true,
      },
      sessionId
    );
    const value = probe && probe.result && probe.result.result && probe.result.result.value;
    const text = value && value.text;
    if (typeof text === "string" && text.toLowerCase().includes(wantedText.toLowerCase())) {
      if (value.visibility === "visible" && !value.hidden) {
        return sessionId;
      }
      fallbackSessionId = fallbackSessionId || sessionId;
    }
  }
  if (fallbackSessionId) {
    return fallbackSessionId;
  }
  throw new Error("No webview target containing " + wantedText);
}

async function attachVisibleByText(conn, wantedText) {
  const sessionId = await attachByText(conn, wantedText);
  const state = await evalIn(
    conn,
    sessionId,
    "return JSON.stringify({visibility:d.visibilityState||document.visibilityState,hidden:Boolean(d.hidden),text:(d.body&&d.body.innerText||'').slice(0,240)});"
  );
  const parsed = JSON.parse(state);
  if (parsed.visibility !== "visible" || parsed.hidden) {
    throw new Error("Matched webview is not visible for " + wantedText + ": " + state);
  }
  return sessionId;
}

async function evalIn(conn, sessionId, body) {
  const expr =
    "(function(){try{var f=document.querySelector('iframe');var d=(f&&f.contentDocument)||document;var w=(f&&f.contentWindow)||window;" +
    body +
    "}catch(e){return 'ERR:'+e.message;}})()";
  const result = await conn.send(
    "Runtime.evaluate",
    { expression: expr, returnByValue: true },
    sessionId
  );
  if (result && result.result && result.result.exceptionDetails) {
    throw new Error("CDP evaluation failed: " + JSON.stringify(result.result.exceptionDetails));
  }
  return result && result.result && result.result.result && result.result.result.value;
}

async function openEditor(kind) {
  await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  await sleep(500);
  await vscode.commands.executeCommand("vscode.openWith", vscode.Uri.file(copied[kind]), VIEW[kind]);
  await sleep(5000);
}

async function openDevicesAndConnections() {
  await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  await sleep(500);
  await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
  await sleep(6500);
}

async function focusFirstVisualControl(conn, sessionId, kind) {
  const raw = await evalIn(
    conn,
    sessionId,
    "var wantedKind=" + JSON.stringify(kind) + ";function visible(el){if(!el)return false;var s=w.getComputedStyle(el);var r=el.getBoundingClientRect();return r.width>0&&r.height>0&&s.visibility!=='hidden'&&s.display!=='none'&&!el.disabled&&el.getAttribute('aria-disabled')!=='true';}function label(el){return (el.getAttribute('aria-label')||el.getAttribute('title')||el.textContent||el.value||el.tagName||'').replace(/\\s+/g,' ').trim();}function css(el){if(!el)return null;var s=w.getComputedStyle(el);return {outlineStyle:s.outlineStyle,outlineWidth:s.outlineWidth,outlineColor:s.outlineColor,outlineOffset:s.outlineOffset,boxShadow:s.boxShadow,borderColor:s.borderColor};}var selectors='button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex=\\\"-1\\\"]),a[href],[role=\\\"button\\\"]';var candidates=[...d.querySelectorAll(selectors)].filter(visible);var inspectorCandidates=candidates.filter(function(el){return Boolean(el.closest('.trust-inspector,.trust-product-header,.react-flow__controls,.blocklyToolboxDiv'));});var target=(inspectorCandidates[0]||candidates[0]||null);if(wantedKind==='blockly'){target=candidates.find(function(el){return el.tagName==='BUTTON'&&Boolean(el.closest('.trust-inspector'));})||target;}if(target){target.scrollIntoView({block:'center',inline:'nearest'});try{target.focus({preventScroll:false,focusVisible:true});}catch(_){target.focus();}}var active=d.activeElement;var style=css(active);var outlineWidth=style?parseFloat(style.outlineWidth||'0'):0;var hasVisibleFocus=Boolean(active&&active!==d.body&&((style&&style.outlineStyle&&style.outlineStyle!=='none'&&outlineWidth>0)||(style&&style.boxShadow&&style.boxShadow!=='none')));return JSON.stringify({kind:wantedKind,count:candidates.length,order:candidates.slice(0,12).map(function(el){return {tag:el.tagName.toLowerCase(),label:label(el),className:String(el.className||'').slice(0,120)};}),active:active&&active!==d.body?{tag:active.tagName.toLowerCase(),label:label(active),className:String(active.className||'').slice(0,120),focused:active===d.activeElement,focusVisible:active.matches&&active.matches(':focus-visible'),style:style,hasVisibleFocus:hasVisibleFocus}:null});"
  );
  const proof = JSON.parse(raw);
  if (!proof.active || !proof.active.hasVisibleFocus) {
    throw new Error("VIS focus-visible proof failed for " + kind + ": " + raw);
  }
  return proof;
}

async function blurVisualFocus(conn, sessionId) {
  await evalIn(
    conn,
    sessionId,
    "var active=d.activeElement;if(active&&active.blur){active.blur();}if(d.body){d.body.setAttribute('tabindex','-1');try{d.body.focus({preventScroll:true});}catch(_){d.body.focus();}}return true;"
  );
}

suite("vis-theme", function () {
  this.timeout(240000);

  test("captures visual editor theme parity", async function () {
	    const log = { theme: ${JSON.stringify(theme)}, requestedZoomLevel: ${JSON.stringify(requestedZoomLevel)}, baselineClick: undefined, baseline: undefined, plant: undefined, devicesInteractions: undefined, liveValues: undefined, styles: [], comparisons: [], focus: [], knownBaselineDefects: [] };
	    const ext = vscode.extensions.getExtension("trust-platform.trust-lsp");
	    if (ext) {
	      await ext.activate();
	    }
	    const requestedThemeNames = ${JSON.stringify(theme.names)};
	    const expectedThemeKinds = ${JSON.stringify(theme.expectedKinds)};
	    function themeKindName(kind) {
	      for (const [name, value] of Object.entries(vscode.ColorThemeKind)) {
	        if (value === kind) {
	          return name;
	        }
	      }
	      return String(kind);
	    }
	    function contributedThemes() {
	      return vscode.extensions.all.flatMap((extension) => {
	        const themes = extension.packageJSON && extension.packageJSON.contributes && extension.packageJSON.contributes.themes;
	        if (!Array.isArray(themes)) {
	          return [];
	        }
	        return themes.map((entry) => ({
	          extension: extension.id,
	          id: entry.id,
	          label: entry.label,
	          uiTheme: entry.uiTheme,
	          path: entry.path
	        }));
	      });
	    }
	    async function applyRequestedTheme() {
	      const attempts = [];
	      const finals = [];
	      for (const name of requestedThemeNames) {
	        await vscode.workspace
	          .getConfiguration("workbench")
	          .update("colorTheme", name, vscode.ConfigurationTarget.Global);
	        for (let i = 0; i < 8; i += 1) {
	          await sleep(250);
	          const actualKind = themeKindName(vscode.window.activeColorTheme.kind);
	          const configured = vscode.workspace.getConfiguration("workbench").get("colorTheme");
	          attempts.push({ name, configured, actualKind });
	          if (expectedThemeKinds.includes(actualKind)) {
	            return { name, configured, actualKind, attempts };
	          }
	        }
	        finals.push(attempts[attempts.length - 1]);
	      }
	      throw new Error("VIS theme runner failed to activate requested theme kind: " + JSON.stringify({ expectedThemeKinds, requestedThemeNames, finals, availableThemes: contributedThemes().filter((entry) => /contrast|hc/i.test([entry.id, entry.label, entry.uiTheme].join(" "))) }));
	    }
	    log.appliedTheme = await applyRequestedTheme();
	    await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
    await vscode.commands.executeCommand("workbench.action.closePanel");

    const version = await httpJson("/json/version");
    const conn = await connect(version.webSocketDebuggerUrl);
    if (requestedWindow) {
      const targets = await httpJson("/json");
      const page = targets.find((target) => target.type === "page");
      if (page) {
        const win = await conn.send("Browser.getWindowForTarget", { targetId: page.id });
        if (win.result && win.result.windowId) {
          await conn.send("Browser.setWindowBounds", {
            windowId: win.result.windowId,
            bounds: {
              left: 0,
              top: 0,
              width: requestedWindow.width,
              height: requestedWindow.height,
              windowState: "normal",
            },
          });
          await sleep(1200);
        }
      }
    }

    await openDevicesAndConnections();
    let sid = await attachVisibleByText(conn, "This computer");
    if (expectPlant) {
      for (let attempt = 0; attempt < 12; attempt += 1) {
        const present = await evalIn(
          conn,
          sid,
          "return [...d.querySelectorAll('.react-flow__node-endpoint')].some(function(x){return (x.textContent||'').indexOf('SIMULATED PLANT')>=0;});"
        );
        if (present) break;
        await sleep(250);
      }
      const plantRaw = await evalIn(
        conn,
        sid,
        "var nodes=[...d.querySelectorAll('.react-flow__node-endpoint')];var node=nodes.find(function(x){return (x.textContent||'').indexOf('SIMULATED PLANT')>=0;});var text=(node&&node.textContent||'').replace(/\\s+/g,' ').trim();var rect=node&&node.getBoundingClientRect();return JSON.stringify({present:Boolean(node),sharedBrick:Boolean(node&&node.classList.contains('react-flow__node-endpoint')),text:text,identity:text.indexOf('Tank level')>=0,roleBand:text.indexOf('SIMULATED PLANT')>=0,hasInventedLive:/\\bSP\\b|\\bPV\\b|sparkline|live sample/i.test(text),rect:rect?{width:rect.width,height:rect.height,left:rect.left,top:rect.top}:null,endpointWidths:nodes.map(function(x){return x.getBoundingClientRect().width;}),nodeClasses:[...d.querySelectorAll('.react-flow__node')].map(function(x){return x.className;}),bodyText:(d.body&&d.body.innerText||'').replace(/\\s+/g,' ').slice(0,2000)});"
      );
      log.plant = JSON.parse(plantRaw);
      screenshot("PSUI-P3-plant-node-" + suffix);
      if (!log.plant.present || !log.plant.sharedBrick || !log.plant.identity || !log.plant.roleBand || log.plant.hasInventedLive || !log.plant.endpointWidths.every(function(width){return width === log.plant.rect.width;})) {
        throw new Error("Plant node semantic proof failed: " + plantRaw);
      }
      if (plantScenario === "remove") {
        const result = await runPlantSimulationScenario({ scenario: plantScenario, workspaceDir: ${JSON.stringify(ws)}, evalIn, attachVisibleByText, executeCommand: (...args) => vscode.commands.executeCommand(...args), refreshRuntimeStatus: () => vscode.workspace.getConfiguration("trust", vscode.workspace.workspaceFolders?.[0]?.uri).update("runtime.mode", "online", vscode.ConfigurationTarget.WorkspaceFolder), conn, sid, sleep, log, suffix, screenshot });
        screenshot(result.screenshot);
        conn.close();
        fs.writeFileSync(path.join(jsonDir, result.jsonFile), JSON.stringify(log, null, 2));
        return;
      }
    }
    const baselineStyle = await evalIn(
      conn,
      sid,
      "var node=d.querySelector('.react-flow__node');if(node){var r=node.getBoundingClientRect();node.dispatchEvent(new MouseEvent('click',{bubbles:true,clientX:r.left+r.width/2,clientY:r.top+r.height/2,view:w}));}return 'clicked';"
    );
    log.baselineClick = baselineStyle;
    await sleep(1200);
    log.preAddSelection = await evalIn(
      conn,
      sid,
      "var node=d.querySelector('.react-flow__node.selected');var plants=[...d.querySelectorAll('.react-flow__node-endpoint')].filter(function(x){return /SIMULATED PLANT/.test(x.textContent||'')&&!/^draft:plant:/.test(x.getAttribute('data-id')||'');});return JSON.stringify({id:node&&node.getAttribute('data-id'),plantCount:plants.length});"
    ).then(JSON.parse);
    const summaryBaseline = await evalIn(
      conn,
      sid,
      "function css(el){if(!el)return null;var s=w.getComputedStyle(el);return {color:s.color,backgroundColor:s.backgroundColor,borderColor:s.borderColor,fontWeight:s.fontWeight,fontSize:s.fontSize,textTransform:s.textTransform,padding:s.padding,borderLeftColor:s.borderLeftColor,borderBottomColor:s.borderBottomColor};}function panelTitle(panel){if(!panel)return null;return panel.querySelector('strong')||panel.querySelector('.trust-inspector__title')||(panel.children[0]&&panel.children[0].querySelector('div'));}var panel=[...d.querySelectorAll('aside')].find(function(x){return /Node summary|Node settings/.test(x.getAttribute('aria-label')||'');});var header=panel&&panel.querySelector('.trust-inspector__header');var title=panelTitle(panel);var sectionTitle=panel&&panel.querySelector('.trust-section__title');var button=panel&&panel.querySelector('footer button');var root=w.getComputedStyle(d.documentElement);return JSON.stringify({kind:'devices-connections-summary',header:css(header),titleText:title?(title.textContent||'').trim():null,title:css(title),sectionTitle:css(sectionTitle),inspector:css(panel),button:css(button),tokens:{trustText:root.getPropertyValue('--trust-text').trim(),trustOverlay:root.getPropertyValue('--trust-overlay').trim(),trustBorder:root.getPropertyValue('--trust-border').trim()}});"
    );
    log.baseline = { summary: JSON.parse(summaryBaseline), add: undefined };
    screenshot(SHOT.devicesSummary);

    const editResult = await evalIn(
      conn,
      sid,
      "var edit=[...d.querySelectorAll('button')].find(function(x){return (x.textContent||'').trim()==='Edit';});if(edit)edit.click();return JSON.stringify({clicked:Boolean(edit),pressed:edit&&edit.getAttribute('aria-pressed')});"
    );
    await sleep(900);
    const addResult = await evalIn(
      conn,
      sid,
      "var w=d.defaultView||window;var slot=[...d.querySelectorAll('button')].find(function(x){return (x.getAttribute('title')||'').indexOf('Add ')===0 && (x.textContent||'').indexOf('Add')>=0;});if(!slot){slot=[...d.querySelectorAll('.react-flow__node')].find(function(x){return /^\\\\+\\\\s*Add\\\\s*connection/i.test((x.textContent||'').trim());});}if(slot){var r=slot.getBoundingClientRect();var cx=r.left+r.width/2;var cy=r.top+r.height/2;try{slot.dispatchEvent(new w.PointerEvent('pointerdown',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,pointerId:1,button:0,isPrimary:true}));slot.dispatchEvent(new w.PointerEvent('pointerup',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,pointerId:1,button:0,isPrimary:true}));}catch(_){}slot.dispatchEvent(new w.MouseEvent('click',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,button:0}));}return slot?'clicked-add-slot':'missing-add-slot';"
    );
    await sleep(1200);
    const addBaseline = await evalIn(
      conn,
      sid,
      "function css(el){if(!el)return null;var s=w.getComputedStyle(el);return {color:s.color,backgroundColor:s.backgroundColor,borderColor:s.borderColor,fontWeight:s.fontWeight,fontSize:s.fontSize,textTransform:s.textTransform,padding:s.padding,borderLeftColor:s.borderLeftColor,borderBottomColor:s.borderBottomColor,width:s.width,height:s.height};}function text(el){return el?(el.textContent||'').trim():null;}function panelTitle(panel){if(!panel)return null;return panel.querySelector('strong')||panel.querySelector('.trust-inspector__title')||(panel.children[0]&&panel.children[0].querySelector('div'));}var panel=[...d.querySelectorAll('aside')].find(function(x){var hay=[x.getAttribute('aria-label')||'',x.textContent||''].join(' ').toLowerCase();return hay.indexOf('add to runtime')>=0||hay.indexOf('add to trust runtime')>=0||hay.indexOf('devices and i/o')>=0;});var header=panel&&panel.querySelector('.trust-inspector__header');var title=panelTitle(panel);var sectionTitle=panel&&panel.querySelector('.trust-section__title');var button=panel&&panel.querySelector('button:not([aria-label])');var controls=d.querySelector('.react-flow__controls');var controlsButton=controls&&controls.querySelector('button');var root=w.getComputedStyle(d.documentElement);return JSON.stringify({kind:'devices-connections-add',roleText:{title:text(title),sectionTitle:text(sectionTitle),button:text(button),panelText:text(panel)},header:css(header),titleText:text(title),title:css(title),sectionTitle:css(sectionTitle),inspector:css(panel),button:css(button),controls:css(controls),controlsButton:css(controlsButton),tokens:{trustText:root.getPropertyValue('--trust-text').trim(),trustOverlay:root.getPropertyValue('--trust-overlay').trim(),trustBorder:root.getPropertyValue('--trust-border').trim()}});"
    );
    log.baseline.addRaw = typeof addBaseline === "string" ? addBaseline : String(addBaseline);
    log.baseline.add = typeof addBaseline === "string" && addBaseline.trim().startsWith("{") ? JSON.parse(addBaseline) : null;
    screenshot(SHOT.devicesAdd);

    if (plantDrag) {
      const dragStartRaw = await evalIn(
        conn,
        sid,
        "var button=[...d.querySelectorAll('button')].find(function(x){var text=(x.textContent||'').replace(/\\s+/g,' ').trim();return text.indexOf('PLANT')>=0&&text.indexOf('normal PLC I/O')>=0;});var panel=button&&button.closest('aside');var dt=new w.DataTransfer();if(button){button.focus();button.dispatchEvent(new w.DragEvent('dragstart',{bubbles:true,cancelable:true,dataTransfer:dt}));}w.__trustPlantDragDataTransfer=dt;return JSON.stringify({button:Boolean(button),focused:Boolean(button&&d.activeElement===button),targetId:panel&&panel.getAttribute('data-target-runtime-id'),disabled:Boolean(button&&button.disabled),title:button&&button.title,mime:dt.types.includes('application/vnd.trust.simulated-plant+json'),payload:dt.getData('application/vnd.trust.simulated-plant+json')});"
      );
      const dragStart = JSON.parse(dragStartRaw);
      if (plantDrag === "malformed") {
        await evalIn(conn, sid, "var dt=w.__trustPlantDragDataTransfer;dt.clearData();dt.setData('application/vnd.trust.simulated-plant+json','not-json');return 'malformed';");
      }
      const hoverRaw = await evalIn(
        conn,
        sid,
        "var runtime=d.querySelector('.react-flow__node-runtime');var flow=d.querySelector('.react-flow');var target=" + JSON.stringify(plantDrag) + "==='outside'?flow:runtime;var r=target&&target.getBoundingClientRect();var x=" + JSON.stringify(plantDrag) + "==='outside'?(r&&r.right-8):(r&&r.left+r.width/2);var y=" + JSON.stringify(plantDrag) + "==='outside'?(r&&r.bottom-8):(r&&r.top+r.height/2);if(target){target.dispatchEvent(new w.DragEvent('dragover',{bubbles:true,cancelable:true,clientX:x,clientY:y,dataTransfer:w.__trustPlantDragDataTransfer}));}return JSON.stringify({runtime:Boolean(runtime),target:Boolean(target),x:x,y:y});"
      );
      await sleep(400);
      const targetRaw = await evalIn(
        conn,
        sid,
        "var runtime=d.querySelector('.react-flow__node-runtime');var body=runtime&&runtime.firstElementChild;var cue=body&&(body.querySelector('[data-plant-drop-cue]')||body.lastElementChild);var cr=cue&&cue.getBoundingClientRect();var plants=[...d.querySelectorAll('.react-flow__node-endpoint')].filter(function(x){return /SIMULATED PLANT/.test(x.textContent||'');}).map(function(x){return x.getBoundingClientRect();});var overlaps=Boolean(cr&&plants.some(function(r){return cr.left<r.right&&cr.right>r.left&&cr.top<r.bottom&&cr.bottom>r.top;}));return JSON.stringify({state:body&&body.getAttribute('data-plant-drop-state'),label:body&&body.getAttribute('data-plant-drop-label'),text:(body&&body.textContent||'').replace(/\\s+/g,' ').trim(),cueOverlapsPlant:overlaps,cueRect:cr?{left:cr.left,top:cr.top,right:cr.right,bottom:cr.bottom}:null});"
      );
      if (plantDrag === "valid") {
        screenshot("PSUI-P4-plant-drag-target-valid-" + suffix);
      }
      if (plantDrag === "escape") {
        await conn.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 }, sid);
        await conn.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 }, sid);
        await sleep(400);
        const cancelRaw = await evalIn(
          conn,
          sid,
          "var runtime=d.querySelector('.react-flow__node-runtime');var body=runtime&&runtime.firstElementChild;var drafts=[...d.querySelectorAll('.react-flow__node-endpoint')].filter(function(x){return /^draft:plant:/.test(x.getAttribute('data-id')||'');});return JSON.stringify({state:body&&body.getAttribute('data-plant-drop-state'),drafts:drafts.length,message:Boolean(d.querySelector('[data-plant-drop-message]'))});"
        );
        log.plantDrag = { dragStart, hover: JSON.parse(hoverRaw), target: JSON.parse(targetRaw), cancel: JSON.parse(cancelRaw) };
        screenshot("PSUI-P4-plant-drag-escape-" + suffix);
        if (!dragStart.button || !dragStart.mime || log.plantDrag.target.state !== "current" || log.plantDrag.cancel.state !== null || log.plantDrag.cancel.drafts !== 0 || log.plantDrag.cancel.message) {
          throw new Error("Plant drag Escape proof failed: " + JSON.stringify(log.plantDrag));
        }
        conn.close();
        fs.writeFileSync(path.join(jsonDir, "VIS-plant-drag-escape-" + suffix + ".json"), JSON.stringify(log, null, 2));
        return;
      }
      const dropRaw = await evalIn(
        conn,
        sid,
        "var runtime=d.querySelector('.react-flow__node-runtime');var flow=d.querySelector('.react-flow');var target=" + JSON.stringify(plantDrag) + "==='outside'?flow:runtime;var r=target&&target.getBoundingClientRect();var x=" + JSON.stringify(plantDrag) + "==='outside'?(r&&r.right-8):(r&&r.left+r.width/2);var y=" + JSON.stringify(plantDrag) + "==='outside'?(r&&r.bottom-8):(r&&r.top+r.height/2);if(target){target.dispatchEvent(new w.DragEvent('drop',{bubbles:true,cancelable:true,clientX:x,clientY:y,dataTransfer:w.__trustPlantDragDataTransfer}));}return JSON.stringify({dropped:Boolean(target)});"
      );
      await sleep(700);
      const resultRaw = await evalIn(
        conn,
        sid,
        "var drafts=[...d.querySelectorAll('.react-flow__node-endpoint')].filter(function(x){return /^draft:plant:/.test(x.getAttribute('data-id')||'');});var node=drafts[0];var panel=[...d.querySelectorAll('aside')].find(function(x){return /Plant properties/i.test(x.getAttribute('aria-label')||'');});var section=panel&&panel.querySelector('[data-plant-section]');var status=d.querySelector('[data-plant-drop-message]');return JSON.stringify({drafts:drafts.length,selected:Boolean(node&&node.classList.contains('selected')),panel:Boolean(panel),plantSection:Boolean(section),focused:Boolean(section&&d.activeElement&&section.contains(d.activeElement)),message:(status&&status.textContent||'').trim()});"
      );
      log.plantDrag = { dragStart, hover: JSON.parse(hoverRaw), target: JSON.parse(targetRaw), drop: JSON.parse(dropRaw), result: JSON.parse(resultRaw) };
      screenshot("PSUI-P4-plant-drag-" + plantDrag + "-" + suffix);
      const valid = plantDrag === "valid";
      if (!dragStart.button || !dragStart.mime || (valid && (log.plantDrag.target.state !== "current" || log.plantDrag.target.label !== "Add simulated plant" || log.plantDrag.target.cueOverlapsPlant || log.plantDrag.result.drafts !== 1 || !log.plantDrag.result.selected || !log.plantDrag.result.panel || !log.plantDrag.result.plantSection || !log.plantDrag.result.focused)) || (!valid && (log.plantDrag.result.drafts !== 0 || !log.plantDrag.result.message))) {
        throw new Error("Plant drag proof failed: " + JSON.stringify(log.plantDrag));
      }
      conn.close();
      fs.writeFileSync(path.join(jsonDir, "VIS-plant-drag-" + plantDrag + "-" + suffix + ".json"), JSON.stringify(log, null, 2));
      return;
    }

    if (plantActivation) {
      const activationRaw = await evalIn(
        conn,
        sid,
        "var button=[...d.querySelectorAll('button')].find(function(x){var text=(x.textContent||'').replace(/\\s+/g,' ').trim();return text.indexOf('PLANT')>=0&&text.indexOf('normal PLC I/O')>=0;});if(button){button.focus();if(" + JSON.stringify(plantActivation) + "==='click'){button.click();}}return JSON.stringify({button:Boolean(button),focused:Boolean(button&&d.activeElement===button),method:" + JSON.stringify(plantActivation) + "});"
      );
      if (plantActivation === "enter") {
        await conn.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 }, sid);
        await conn.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 }, sid);
      } else if (plantActivation === "space") {
        await conn.send("Input.dispatchKeyEvent", { type: "keyDown", key: " ", code: "Space", windowsVirtualKeyCode: 32, nativeVirtualKeyCode: 32 }, sid);
        await conn.send("Input.dispatchKeyEvent", { type: "keyUp", key: " ", code: "Space", windowsVirtualKeyCode: 32, nativeVirtualKeyCode: 32 }, sid);
      }
      await sleep(900);
      const draftRaw = await evalIn(
        conn,
        sid,
        "var plants=[...d.querySelectorAll('.react-flow__node-endpoint')].filter(function(x){return /SIMULATED PLANT/.test(x.textContent||'');});var drafts=plants.filter(function(x){return /^draft:plant:/.test(x.getAttribute('data-id')||'');});var node=drafts[0];var panel=[...d.querySelectorAll('aside')].find(function(x){return /Plant properties/i.test(x.getAttribute('aria-label')||'');});var plantSection=panel&&panel.querySelector('[data-plant-section]');var active=d.activeElement;var nodeText=(node&&node.textContent||'').replace(/\\s+/g,' ').trim();return JSON.stringify({node:Boolean(node),draft:Boolean(node&&/^draft:plant:/.test(node.getAttribute('data-id')||'')),singleDraft:drafts.length===1,plantCount:plants.length,panel:Boolean(panel),plantSection:Boolean(plantSection),focused:Boolean(plantSection&&active&&plantSection.contains(active)),nodeText:nodeText,panelText:(panel&&panel.textContent||'').replace(/\\s+/g,' ').trim()});"
      );
      log.plantActivation = {
        activation: JSON.parse(activationRaw),
        draft: JSON.parse(draftRaw),
      };
      if (plantScenario) {
        const result = await runPlantSimulationScenario({ scenario: plantScenario, workspaceDir: ${JSON.stringify(ws)}, evalIn, attachVisibleByText, executeCommand: (...args) => vscode.commands.executeCommand(...args), refreshRuntimeStatus: () => vscode.workspace.getConfiguration("trust", vscode.workspace.workspaceFolders?.[0]?.uri).update("runtime.mode", "online", vscode.ConfigurationTarget.WorkspaceFolder), conn, sid, sleep, log, suffix, screenshot });
        screenshot(result.screenshot);
        conn.close();
        fs.writeFileSync(path.join(jsonDir, result.jsonFile), JSON.stringify(log, null, 2));
        return;
      }
      if (plantEditor === "initial") {
        const editorRaw = await evalIn(
          conn,
          sid,
          "var panel=[...d.querySelectorAll('aside')].find(function(x){return /Plant properties/i.test(x.getAttribute('aria-label')||'');});var titles=panel?[...panel.querySelectorAll('.trust-section__title')].map(function(x){return (x.textContent||'').trim();}):[];var buttons=panel?[...panel.querySelectorAll('button')]:[];var save=buttons.find(function(x){return (x.textContent||'').trim()==='Save';});var cancel=buttons.find(function(x){return (x.textContent||'').trim()==='Cancel';});var disclosure=buttons.find(function(x){return /engineering details/i.test(x.textContent||'');});var summary=panel&&panel.querySelector('[data-plant-validation-summary]');return JSON.stringify({panel:Boolean(panel),dirty:panel&&panel.getAttribute('data-plant-dirty'),titles:titles,save:Boolean(save),saveDisabled:Boolean(save&&save.disabled),cancel:Boolean(cancel),summary:(summary&&summary.textContent||'').replace(/\\s+/g,' ').trim(),disclosure:Boolean(disclosure),expanded:disclosure&&disclosure.getAttribute('aria-expanded'),advancedVisible:titles.some(function(x){return ['Actuator','Imperfections','Scenario events'].includes(x);})});"
        );
        log.plantEditor = JSON.parse(editorRaw);
        screenshot("PSUI-P5-editor-initial-" + suffix);
        if (!log.plantEditor.panel || log.plantEditor.dirty !== "true" || JSON.stringify(log.plantEditor.titles)!==JSON.stringify(["Plant","PLC signals","Dynamics"]) || !log.plantEditor.save || !log.plantEditor.saveDisabled || !log.plantEditor.cancel || !log.plantEditor.summary || !log.plantEditor.disclosure || log.plantEditor.expanded !== "false" || log.plantEditor.advancedVisible) {
          throw new Error("Plant initial editor proof failed: " + JSON.stringify(log.plantEditor));
        }
        conn.close();
        fs.writeFileSync(path.join(jsonDir, "VIS-plant-editor-initial-" + suffix + ".json"), JSON.stringify(log, null, 2));
        return;
      }
      if (plantEditor === "identity") {
        const identityRaw = await evalIn(
          conn,
          sid,
          "var panel=[...d.querySelectorAll('aside')].find(function(x){return /Plant properties/i.test(x.getAttribute('aria-label')||'');});var id=panel&&panel.querySelector('input[name=plant-id]');var name=panel&&panel.querySelector('input[name=plant-display-name]');var description=panel&&panel.querySelector('textarea[name=plant-description]');var runtime=panel&&panel.querySelector('[data-plant-runtime-placement]');var enabled=panel&&panel.querySelector('input[name=plant-enabled]');var initialId=id&&id.value;if(id){var setter=Object.getOwnPropertyDescriptor(w.HTMLInputElement.prototype,'value').set;setter.call(id,'1 tank');id.dispatchEvent(new w.Event('input',{bubbles:true}));id.dispatchEvent(new w.Event('change',{bubbles:true}));}return JSON.stringify({id:Boolean(id),initialId:initialId,name:Boolean(name),nameValue:name&&name.value,description:Boolean(description),runtime:(runtime&&runtime.textContent||'').trim(),enabled:Boolean(enabled&&enabled.checked)});"
        );
        await sleep(300);
        const errorRaw = await evalIn(
          conn,
          sid,
          "var id=d.querySelector('input[name=plant-id]');var error=id&&d.querySelector('[data-field-error=plant-id]');return JSON.stringify({value:id&&id.value,invalid:id&&id.getAttribute('aria-invalid'),error:(error&&error.textContent||'').trim()});"
        );
        log.plantEditorIdentity = { fields: JSON.parse(identityRaw), validation: JSON.parse(errorRaw) };
        screenshot("PSUI-P5-editor-identity-error-" + suffix);
        if (!log.plantEditorIdentity.fields.id || log.plantEditorIdentity.fields.initialId !== "new_plant" || !log.plantEditorIdentity.fields.name || log.plantEditorIdentity.fields.nameValue !== "New simulated plant" || !log.plantEditorIdentity.fields.description || log.plantEditorIdentity.fields.runtime !== "Simulator" || !log.plantEditorIdentity.fields.enabled || log.plantEditorIdentity.validation.value !== "1 tank" || log.plantEditorIdentity.validation.invalid !== "true" || !/Start with a letter/.test(log.plantEditorIdentity.validation.error)) {
          throw new Error("Plant identity editor proof failed: " + JSON.stringify(log.plantEditorIdentity));
        }
        conn.close();
        fs.writeFileSync(path.join(jsonDir, "VIS-plant-editor-identity-" + suffix + ".json"), JSON.stringify(log, null, 2));
        return;
      }
      if (plantCancel) {
        const cancelRaw = await evalIn(
          conn,
          sid,
          "var panel=[...d.querySelectorAll('aside')].find(function(x){return /Plant properties/i.test(x.getAttribute('aria-label')||'');});var cancel=panel&&[...panel.querySelectorAll('button')].find(function(x){return (x.textContent||'').trim()==='Cancel';});if(cancel)cancel.click();return JSON.stringify({button:Boolean(cancel)});"
        );
        await sleep(700);
        const restoredRaw = await evalIn(
          conn,
          sid,
          "var plants=[...d.querySelectorAll('.react-flow__node-endpoint')].filter(function(x){return /SIMULATED PLANT/.test(x.textContent||'');});var drafts=plants.filter(function(x){return /^draft:plant:/.test(x.getAttribute('data-id')||'');});var selected=d.querySelector('.react-flow__node.selected');var focused=d.activeElement&&d.activeElement.closest&&d.activeElement.closest('.react-flow__node');return JSON.stringify({drafts:drafts.length,plantCount:plants.length,selectedId:selected&&selected.getAttribute('data-id'),focusedId:focused&&focused.getAttribute('data-id'),drawer:Boolean([...d.querySelectorAll('aside')].find(function(x){return /Plant properties/i.test(x.getAttribute('aria-label')||'');}))});"
        );
        log.plantCancel = { action: JSON.parse(cancelRaw), restored: JSON.parse(restoredRaw) };
        screenshot("PSUI-P4-plant-cancel-" + suffix);
        if (!log.plantCancel.action.button || log.plantCancel.restored.drafts !== 0 || log.plantCancel.restored.plantCount !== log.preAddSelection.plantCount || log.plantCancel.restored.drawer || !log.preAddSelection.id || log.plantCancel.restored.selectedId !== log.preAddSelection.id || log.plantCancel.restored.focusedId !== log.preAddSelection.id) {
          throw new Error("Plant cancel restoration proof failed: " + JSON.stringify({preAddSelection:log.preAddSelection,plantCancel:log.plantCancel}));
        }
        conn.close();
        fs.writeFileSync(path.join(jsonDir, "VIS-plant-cancel-" + suffix + ".json"), JSON.stringify(log, null, 2));
        return;
      }
      screenshot("PSUI-P4-add-" + plantActivation + "-" + suffix);
      if (!log.plantActivation.activation.button || !log.plantActivation.activation.focused || !log.plantActivation.draft.node || !log.plantActivation.draft.draft || !log.plantActivation.draft.singleDraft || !log.plantActivation.draft.panel || !log.plantActivation.draft.plantSection || !log.plantActivation.draft.focused) {
        throw new Error("Plant add activation proof failed: " + JSON.stringify(log.plantActivation));
      }
      conn.close();
      fs.writeFileSync(
        path.join(jsonDir, "VIS-plant-add-" + plantActivation + "-" + suffix + ".json"),
        JSON.stringify(log, null, 2)
      );
      return;
    }

    const cancelResult = await evalIn(
      conn,
      sid,
      "var panel=[...d.querySelectorAll('aside')].find(function(x){var text=((x.getAttribute('aria-label')||'')+' '+(x.textContent||'')).toLowerCase();return text.indexOf('add device or connection')>=0||text.indexOf('devices and i/o')>=0;});var close=panel&&[...panel.querySelectorAll('button')].find(function(x){return (x.getAttribute('aria-label')||'').toLowerCase()==='close';});return JSON.stringify({panel:Boolean(panel),close:Boolean(close)});"
    );
    await evalIn(conn, sid, "var panel=[...d.querySelectorAll('aside')].find(function(x){var text=((x.getAttribute('aria-label')||'')+' '+(x.textContent||'')).toLowerCase();return text.indexOf('add device or connection')>=0||text.indexOf('devices and i/o')>=0;});var close=panel&&[...panel.querySelectorAll('button')].find(function(x){return (x.getAttribute('aria-label')||'').toLowerCase()==='close';});if(close)close.click();return 'close-dispatched';");
    await sleep(700);
    const afterCancel = await evalIn(
      conn,
      sid,
      "var add=[...d.querySelectorAll('aside')].some(function(x){var text=((x.getAttribute('aria-label')||'')+' '+(x.textContent||'')).toLowerCase();return text.indexOf('add device or connection')>=0||text.indexOf('devices and i/o')>=0;});var edit=[...d.querySelectorAll('button')].find(function(x){return (x.textContent||'').trim()==='Edit';});if(edit){try{edit.focus({focusVisible:true});}catch(_){edit.focus();}}return JSON.stringify({addOpen:add,editPresent:Boolean(edit),focused:Boolean(edit&&d.activeElement===edit)});"
    );
    const filterResult = await evalIn(
      conn,
      sid,
      "var filter=[...d.querySelectorAll('button')].find(function(x){return (x.textContent||'').trim()==='Filter';});return JSON.stringify({clicked:Boolean(filter)});"
    );
    await evalIn(conn, sid, "var filter=[...d.querySelectorAll('button')].find(function(x){return (x.textContent||'').trim()==='Filter';});if(filter)filter.click();return 'filter-dispatched';");
    await sleep(700);
    const filterState = await evalIn(
      conn,
      sid,
      "var text=(d.body&&d.body.innerText||'').replace(/\\s+/g,' ').trim();return JSON.stringify({visible:/Filter devices|Status|Show/i.test(text)});"
    );
    log.devicesInteractions = {
      edit: JSON.parse(editResult),
      add: addResult,
      cancel: JSON.parse(cancelResult),
      afterCancel: JSON.parse(afterCancel),
      filter: JSON.parse(filterResult),
      filterState: JSON.parse(filterState)
    };

    await vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
    await sleep(1600);
    sid = await attachVisibleByText(conn, "Live Values");
    const liveValuesRaw = await evalIn(
      conn,
      sid,
      "var text=(d.body&&d.body.innerText||'').replace(/\\s+/g,' ').trim();var target=d.querySelector('[aria-label=\\\"Active Live Values target\\\"]');var status=d.querySelector('#status');var root=w.getComputedStyle(d.documentElement);var controls=[...d.querySelectorAll('button,input,select,[role=button]')].filter(function(el){var s=w.getComputedStyle(el),r=el.getBoundingClientRect();return r.width>0&&r.height>0&&s.display!=='none'&&s.visibility!=='hidden';});var first=controls[0]||null;if(first){try{first.focus({focusVisible:true});}catch(_){first.focus();}}var fs=first?w.getComputedStyle(first):null;var focusVisible=Boolean(first&&first===d.activeElement&&((fs.outlineStyle&&fs.outlineStyle!=='none'&&parseFloat(fs.outlineWidth||'0')>0)||(fs.boxShadow&&fs.boxShadow!=='none')));return JSON.stringify({title:d.title,text:text.slice(0,1800),target:Boolean(target),status:status?(status.textContent||'').trim():null,controlCount:controls.length,focusVisible:focusVisible,bodyClass:d.body&&d.body.className,tokens:{canvas:root.getPropertyValue('--trust-canvas').trim(),text:root.getPropertyValue('--trust-text').trim(),border:root.getPropertyValue('--trust-border').trim()}});"
    );
    log.liveValues = JSON.parse(liveValuesRaw);
    screenshot(SHOT.liveValues);

    for (const kind of ["sfc", "statechart", "ladder", "blockly"]) {
      await openEditor(kind);
      sid = await attachVisibleByText(conn, ATTACH_TEXT[kind]);
      const style = await evalIn(
        conn,
        sid,
        "var title=d.querySelector('.trust-inspector__title');var header=d.querySelector('.trust-inspector__header');var sectionTitle=d.querySelector('.trust-section__title');var inspector=d.querySelector('.trust-inspector');var button=d.querySelector('.trust-button');var controls=d.querySelector('.react-flow__controls');var controlsButton=controls&&controls.querySelector('button');var root=w.getComputedStyle(d.documentElement);function css(el){if(!el)return null;var s=w.getComputedStyle(el);return {color:s.color,backgroundColor:s.backgroundColor,borderColor:s.borderColor,fontWeight:s.fontWeight,fontSize:s.fontSize,textTransform:s.textTransform,padding:s.padding,borderLeftColor:s.borderLeftColor,borderBottomColor:s.borderBottomColor,width:s.width,height:s.height};}function text(el){return el?(el.textContent||'').trim():null;}var privateSelectors=['.ladder-tools-panel__button','.ladder-tools-panel__section-title','.blockly-tools-panel','.blockly-tools-panel__button'];var privateChromeCount=privateSelectors.reduce(function(total,sel){return total+d.querySelectorAll(sel).length;},0);var allText=(d.body&&d.body.innerText||'').replace(/\\s+/g,' ').trim();var allTitles=[...d.querySelectorAll('.trust-inspector__title,strong,h1,h2,h3')].map(text).filter(Boolean);var themeProbe={htmlClass:d.documentElement.className,bodyClass:d.body&&d.body.className,htmlAttrs:[...d.documentElement.attributes].map(function(a){return [a.name,a.value];}),bodyAttrs:d.body?[...d.body.attributes].map(function(a){return [a.name,a.value];}):[]};return JSON.stringify({kind:" + JSON.stringify(kind) + ",themeProbe:themeProbe,roleText:{title:text(title),sectionTitle:text(sectionTitle),button:text(button),panelText:text(inspector),allTitles:allTitles,forbiddenEditorTools:/\\bEditor tools\\b/i.test(allText)},header:css(header),titleText:text(title),title:css(title),sectionTitle:css(sectionTitle),inspector:css(inspector),button:css(button),controls:css(controls),controlsButton:css(controlsButton),privateChromeCount:privateChromeCount,tokens:{trustText:root.getPropertyValue('--trust-text').trim(),trustOverlay:root.getPropertyValue('--trust-overlay').trim(),trustBorder:root.getPropertyValue('--trust-border').trim()}});"
      );
      const parsed = JSON.parse(style);
      if (kind === "blockly") {
        const toolbox = await evalIn(
          conn,
          sid,
          "function parseRgb(value){var text=String(value||'');var open=text.indexOf('(');var close=text.indexOf(')');if(open<0||close<=open)return null;var raw=text.slice(open+1,close).split(',').join(' ').split('/').join(' ').trim().split(' ').filter(Boolean);var p=raw.map(function(x){return Number(x);});if(p.length<3||p.some(function(x){return Number.isNaN(x);}))return null;if(p.length>3&&p[3]===0)return null;return p.slice(0,3);}function channel(v){v=v/255;return v<=0.03928?v/12.92:Math.pow((v+0.055)/1.055,2.4);}function luminance(rgb){return 0.2126*channel(rgb[0])+0.7152*channel(rgb[1])+0.0722*channel(rgb[2]);}function contrast(fg,bg){if(!fg||!bg)return 0;var a=luminance(fg)+0.05;var b=luminance(bg)+0.05;return Math.max(a,b)/Math.min(a,b);}function resolveColor(property,value){var probe=d.createElement('div');probe.style[property]=value;probe.style.position='absolute';probe.style.left='-9999px';probe.style.width='1px';probe.style.height='1px';d.body.appendChild(probe);var c=w.getComputedStyle(probe)[property];probe.remove();return c;}function backgroundFor(el,fallback){var cur=el;while(cur&&cur!==d.documentElement){var c=w.getComputedStyle(cur).backgroundColor;if(parseRgb(c))return c;cur=cur.parentElement;}return fallback;}var toolbox=d.querySelector('.blocklyToolboxDiv');var resolvedOverlay=resolveColor('backgroundColor','var(--trust-overlay)');var toolboxBg=parseRgb(toolbox&&w.getComputedStyle(toolbox).backgroundColor)?w.getComputedStyle(toolbox).backgroundColor:resolvedOverlay;var bodyBg=parseRgb(w.getComputedStyle(d.body).backgroundColor)?w.getComputedStyle(d.body).backgroundColor:resolvedOverlay;var labels=[...d.querySelectorAll('.blocklyTreeLabel')].map(function(el){var s=w.getComputedStyle(el);var bg=backgroundFor(el.parentElement||el,toolboxBg||bodyBg);var ratio=contrast(parseRgb(s.color),parseRgb(bg));return {text:(el.textContent||'').trim(),color:s.color,backgroundColor:bg,contrast:Math.round(ratio*100)/100};}).filter(function(x){return x.text;});var failures=labels.filter(function(x){return !(x.contrast>=4.5);});return JSON.stringify({ok:labels.length>0&&failures.length===0,labels:labels,failures:failures});"
        );
        parsed.toolbox = JSON.parse(toolbox);
      }
      const expectsCanvasControls = kind === "sfc" || kind === "statechart";
      const headerChrome = compareStyle("header", parsed.header, log.baseline.add && log.baseline.add.header);
      const titleChrome = compareStyle("title", parsed.title, log.baseline.add && log.baseline.add.title);
      const sectionChrome = log.baseline.add && log.baseline.add.sectionTitle
        ? compareStyle("sectionTitle", parsed.sectionTitle, log.baseline.add.sectionTitle)
        : { ok: true, mismatches: [] };
      const inspectorChrome = compareStyle("inspector", parsed.inspector, log.baseline.add && log.baseline.add.inspector);
      const buttonChrome = compareStyle("button", parsed.button, log.baseline.summary && log.baseline.summary.button);
      const controlsChrome = expectsCanvasControls && log.baseline.add && log.baseline.add.controls
        ? compareStyle("controls", parsed.controls, log.baseline.add && log.baseline.add.controls)
        : { ok: true, mismatches: [] };
      const controlsButtonChrome = expectsCanvasControls && log.baseline.add && log.baseline.add.controlsButton
        ? compareStyle("controlsButton", parsed.controlsButton, log.baseline.add && log.baseline.add.controlsButton)
        : { ok: true, mismatches: [] };
      log.styles.push(parsed);
      log.comparisons.push({
        kind,
        headerChrome,
        titleChrome,
        sectionChrome,
        inspectorChrome,
        buttonChrome,
        controlsChrome,
        controlsButtonChrome,
        titleColorMatchesBaseline: Boolean(log.baseline.add && parsed.title && log.baseline.add.title && parsed.title.color === log.baseline.add.title.color),
        titleWeightMatchesBaseline: Boolean(log.baseline.add && parsed.title && log.baseline.add.title && parsed.title.fontWeight === log.baseline.add.title.fontWeight),
        titleSizeMatchesBaseline: Boolean(log.baseline.add && parsed.title && log.baseline.add.title && parsed.title.fontSize === log.baseline.add.title.fontSize),
        headerBackgroundMatchesBaseline: Boolean(log.baseline.add && parsed.header && log.baseline.add.header && parsed.header.backgroundColor === log.baseline.add.header.backgroundColor),
        headerBorderMatchesBaseline: Boolean(log.baseline.add && parsed.header && log.baseline.add.header && parsed.header.borderBottomColor === log.baseline.add.header.borderBottomColor),
        sectionTitleMatchesBaseline: !log.baseline.add?.sectionTitle || Boolean(log.baseline.add && parsed.sectionTitle && log.baseline.add.sectionTitle && parsed.sectionTitle.color === log.baseline.add.sectionTitle.color && parsed.sectionTitle.fontSize === log.baseline.add.sectionTitle.fontSize && parsed.sectionTitle.fontWeight === log.baseline.add.sectionTitle.fontWeight && parsed.sectionTitle.textTransform === log.baseline.add.sectionTitle.textTransform),
        inspectorColorMatchesBaseline: Boolean(log.baseline.add && parsed.inspector && log.baseline.add.inspector && parsed.inspector.color === log.baseline.add.inspector.color),
        inspectorBackgroundMatchesBaseline: Boolean(log.baseline.add && parsed.inspector && log.baseline.add.inspector && parsed.inspector.backgroundColor === log.baseline.add.inspector.backgroundColor),
        usesSurfaceTitle: Boolean(parsed.titleText && parsed.titleText !== "Editor tools" && parsed.titleText.toLowerCase().includes(kind === "sfc" ? "sfc" : kind === "statechart" ? "statechart" : kind === "ladder" ? "ladder" : "blockly")),
        hasNoForbiddenEditorToolsText: !(parsed.roleText && parsed.roleText.forbiddenEditorTools),
        hasNoPrivateChromeSelectors: parsed.privateChromeCount === 0,
        toolboxLabelsReadable: kind !== "blockly" || Boolean(parsed.toolbox && parsed.toolbox.ok),
        // AddPane protocol rows are list items, not generic action buttons. Compare visual-editor
        // generic tool buttons to the Devices & Connections summary/footer action button.
        buttonColorMatchesBaseline: !parsed.button || !log.baseline.summary?.button || parsed.button.color === log.baseline.summary.button.color,
        canvasControlsPresentWhenExpected: !expectsCanvasControls || Boolean(parsed.controls && parsed.controlsButton),
        canvasControlsMatchBaseline:
          !expectsCanvasControls ||
          !log.baseline.add?.controls ||
          Boolean(log.baseline.add?.controls && log.baseline.add?.controlsButton && parsed.controls && parsed.controlsButton &&
            parsed.controls.backgroundColor === log.baseline.add.controls.backgroundColor &&
            parsed.controls.borderColor === log.baseline.add.controls.borderColor &&
            parsed.controlsButton.backgroundColor === log.baseline.add.controlsButton.backgroundColor &&
            parsed.controlsButton.color === log.baseline.add.controlsButton.color &&
            parsed.controlsButton.width === log.baseline.add.controlsButton.width &&
            parsed.controlsButton.height === log.baseline.add.controlsButton.height),
      });
      await blurVisualFocus(conn, sid);
      screenshot(SHOT[kind]);
      const focusProof = await focusFirstVisualControl(conn, sid, kind);
      log.focus.push(focusProof);
      await sleep(250);
      screenshot(SHOT.focus[kind]);
    }

    await openEditor("invalid");
    sid = await attachVisibleByText(conn, ATTACH_TEXT.invalid);
    const invalid = await evalIn(
      conn,
      sid,
      "function css(el){if(!el)return null;var s=w.getComputedStyle(el);return {color:s.color,backgroundColor:s.backgroundColor,borderColor:s.borderColor,borderTopColor:s.borderTopColor,borderRightColor:s.borderRightColor,borderBottomColor:s.borderBottomColor,borderLeftColor:s.borderLeftColor,fontWeight:s.fontWeight,fontSize:s.fontSize,textTransform:s.textTransform,padding:s.padding,boxShadow:s.boxShadow};}function resolveCss(property,value){var probe=d.createElement('span');probe.style[property]=value;probe.style.position='absolute';probe.style.left='-9999px';probe.textContent='x';d.body.appendChild(probe);var resolved=w.getComputedStyle(probe)[property];probe.remove();return resolved;}function token(name,property){var raw=root.getPropertyValue(name).trim();return resolveCss(property,raw)||raw;}function eq(a,b){return String(a||'').trim()===String(b||'').trim();}var root=w.getComputedStyle(d.body||d.documentElement);var text=(d.body&&d.body.innerText||'').slice(0,1000);var header=d.querySelector('.trust-product-header');var brand=d.querySelector('.trust-product-brand');var surface=d.querySelector('.trust-product-brand__surface');var meta=d.querySelector('.trust-product-header__meta');var shell=d.querySelector('.trust-product-shell');var workspace=d.querySelector('.trust-product-workspace');var alert=d.querySelector('[role=alert]');var card=alert&&alert.firstElementChild;var title=card&&card.querySelector('h2');var bodyText=card&&card.querySelector('p');var detail=card&&card.querySelector('code');var h=css(header),sh=css(shell),ws=css(workspace),a=css(alert),c=css(card),ti=css(title),bt=css(bodyText),de=css(detail);var tokens={trustCanvas:token('--trust-canvas','backgroundColor'),trustSurface:token('--trust-surface','backgroundColor'),trustText:token('--trust-text','color'),trustTextMuted:token('--trust-text-muted','color'),trustBorder:token('--trust-border','borderTopColor'),trustDanger:token('--trust-danger','color')};var mismatches=[];function requireRole(flag,actual,expected){if(!flag)mismatches.push({actual:actual,expected:expected});}requireRole(Boolean(header),'missing product header','present');requireRole(Boolean(brand&&surface),'missing product brand/surface','present');requireRole((surface&&surface.textContent||'').trim()==='Statechart editor',(surface&&surface.textContent||'').trim(),'Statechart editor');requireRole((meta&&meta.textContent||'').trim()==='State machine diagram',(meta&&meta.textContent||'').trim(),'State machine diagram');requireRole(Boolean(alert),'missing alert region','present');requireRole(Boolean(card),'missing error card','present');requireRole(eq(sh&&sh.backgroundColor,tokens.trustCanvas),sh&&sh.backgroundColor,tokens.trustCanvas);requireRole(eq(sh&&sh.color,tokens.trustText),sh&&sh.color,tokens.trustText);requireRole(eq(h&&h.backgroundColor,tokens.trustSurface),h&&h.backgroundColor,tokens.trustSurface);requireRole(eq(h&&h.borderBottomColor,tokens.trustBorder),h&&h.borderBottomColor,tokens.trustBorder);requireRole(eq(a&&a.backgroundColor,tokens.trustCanvas),a&&a.backgroundColor,tokens.trustCanvas);requireRole(eq(a&&a.color,tokens.trustText),a&&a.color,tokens.trustText);requireRole(eq(c&&c.backgroundColor,tokens.trustSurface),c&&c.backgroundColor,tokens.trustSurface);requireRole(eq(c&&c.borderTopColor,tokens.trustDanger)&&eq(c&&c.borderRightColor,tokens.trustDanger)&&eq(c&&c.borderBottomColor,tokens.trustDanger)&&eq(c&&c.borderLeftColor,tokens.trustDanger),c&&[c.borderTopColor,c.borderRightColor,c.borderBottomColor,c.borderLeftColor].join('/'),tokens.trustDanger);requireRole(eq(ti&&ti.color,tokens.trustDanger),ti&&ti.color,tokens.trustDanger);requireRole(eq(bt&&bt.color,tokens.trustText),bt&&bt.color,tokens.trustText);requireRole(eq(de&&de.color,tokens.trustTextMuted),de&&de.color,tokens.trustTextMuted);return JSON.stringify({kind:'invalid',messageShown:text.indexOf('Could not open this statechart')>=0,recoveryShown:text.indexOf('Fix the JSON')>=0,rawDumpShown:/TypeError|SyntaxError|stack trace|Editor Error/.test(text),headerBrandPresent:Boolean(header&&brand&&surface),surfaceTitlePresent:(surface&&surface.textContent||'').trim()==='Statechart editor',headerMetaPresent:(meta&&meta.textContent||'').trim()==='State machine diagram',shellUsesTrustCanvas:eq(sh&&sh.backgroundColor,tokens.trustCanvas)&&eq(sh&&sh.color,tokens.trustText),headerUsesTrustSurface:eq(h&&h.backgroundColor,tokens.trustSurface)&&eq(h&&h.borderBottomColor,tokens.trustBorder),alertUsesTrustCanvas:eq(a&&a.backgroundColor,tokens.trustCanvas)&&eq(a&&a.color,tokens.trustText),cardUsesTrustSurface:eq(c&&c.backgroundColor,tokens.trustSurface),cardUsesTrustDangerBorder:eq(c&&c.borderTopColor,tokens.trustDanger)&&eq(c&&c.borderRightColor,tokens.trustDanger)&&eq(c&&c.borderBottomColor,tokens.trustDanger)&&eq(c&&c.borderLeftColor,tokens.trustDanger),titleUsesTrustDanger:eq(ti&&ti.color,tokens.trustDanger),bodyUsesTrustText:eq(bt&&bt.color,tokens.trustText),detailUsesTrustTextMuted:eq(de&&de.color,tokens.trustTextMuted),roleOk:mismatches.length===0,mismatches:mismatches,styles:{header:h,shell:sh,workspace:ws,alert:a,card:c,title:ti,bodyText:bt,detail:de},tokens:tokens});"
    );
    log.invalid = JSON.parse(invalid);
    const invalidOk =
      log.invalid.messageShown &&
      log.invalid.recoveryShown &&
      !log.invalid.rawDumpShown &&
      log.invalid.roleOk;
    log.comparisons.push({
      kind: "invalid",
      messageShown: log.invalid.messageShown,
      recoveryShown: log.invalid.recoveryShown,
      noRawDump: !log.invalid.rawDumpShown,
      headerBrandPresent: log.invalid.headerBrandPresent,
      surfaceTitlePresent: log.invalid.surfaceTitlePresent,
      headerMetaPresent: log.invalid.headerMetaPresent,
      shellUsesTrustCanvas: log.invalid.shellUsesTrustCanvas,
      headerUsesTrustSurface: log.invalid.headerUsesTrustSurface,
      alertUsesTrustCanvas: log.invalid.alertUsesTrustCanvas,
      cardUsesTrustSurface: log.invalid.cardUsesTrustSurface,
      cardUsesTrustDangerBorder: log.invalid.cardUsesTrustDangerBorder,
      titleUsesTrustDanger: log.invalid.titleUsesTrustDanger,
      bodyUsesTrustText: log.invalid.bodyUsesTrustText,
      detailUsesTrustTextMuted: log.invalid.detailUsesTrustTextMuted,
      headerChrome: { ok: log.invalid.headerBrandPresent && log.invalid.headerUsesTrustSurface, mismatches: [] },
      alertChrome: { ok: log.invalid.alertUsesTrustCanvas, mismatches: [] },
      cardChrome: {
        ok: log.invalid.cardUsesTrustSurface && log.invalid.cardUsesTrustDangerBorder,
        mismatches: log.invalid.mismatches || [],
      },
      errorTitleChrome: { ok: log.invalid.titleUsesTrustDanger, mismatches: [] },
      bodyTextChrome: { ok: log.invalid.bodyUsesTrustText, mismatches: [] },
      detailTextChrome: { ok: log.invalid.detailUsesTrustTextMuted, mismatches: [] },
      ok: invalidOk,
      mismatches: log.invalid.mismatches || [],
    });
    screenshot(SHOT.invalid);

    conn.close();
    fs.writeFileSync(
      path.join(jsonDir, "VIS-theme-" + suffix + ".json"),
      JSON.stringify(log, null, 2)
    );

    const failures = log.styles.filter((entry) => {
      return !entry.title || !entry.inspector || entry.title.color !== entry.inspector.color;
    });
    function isKnownSectionTransformBaseline(entry) {
      const mismatches = entry && entry.sectionChrome && entry.sectionChrome.mismatches;
      return Array.isArray(mismatches) && mismatches.length === 1 && mismatches[0].field === "textTransform" && mismatches[0].actual === "uppercase" && mismatches[0].expected === "none";
    }
    log.knownBaselineDefects = log.comparisons.filter(isKnownSectionTransformBaseline).map((entry) => ({ kind: entry.kind, defect: "visual editor section heading is uppercase while Devices & Connections add drawer is not" }));
    fs.writeFileSync(
      path.join(jsonDir, "VIS-theme-" + suffix + ".json"),
      JSON.stringify(log, null, 2)
    );
    const baselineFailures = log.comparisons.filter((entry) => {
      if (entry.kind === "invalid") {
        return !entry.ok;
      }
      return !entry.titleChrome.ok || !entry.headerChrome.ok || (!entry.sectionChrome.ok && !isKnownSectionTransformBaseline(entry)) || !entry.inspectorChrome.ok || !entry.buttonChrome.ok || !entry.controlsChrome.ok || !entry.controlsButtonChrome.ok || !entry.canvasControlsPresentWhenExpected || !entry.usesSurfaceTitle || !entry.hasNoForbiddenEditorToolsText || !entry.hasNoPrivateChromeSelectors || !entry.toolboxLabelsReadable;
    });
    if (failures.length) {
      throw new Error("VIS theme title/inspector color mismatch: " + JSON.stringify(failures));
    }
    if (baselineFailures.length) {
      throw new Error("VIS vs Devices & Connections theme mismatch: " + JSON.stringify(baselineFailures));
    }
    if (!log.invalid.messageShown || !log.invalid.recoveryShown || log.invalid.rawDumpShown || !log.invalid.roleOk) {
      throw new Error("VIS invalid-model state failed: " + JSON.stringify(log.invalid));
    }
    if (!log.liveValues || log.liveValues.title !== "Live Values" || !log.liveValues.target || !log.liveValues.status || !log.liveValues.focusVisible) {
      throw new Error("VIS Live Values baseline failed: " + JSON.stringify(log.liveValues));
    }
    if (!log.devicesInteractions || !log.devicesInteractions.edit.clicked || log.devicesInteractions.add !== "clicked-add-slot" || !log.baseline.add || !log.baseline.add.inspector || !log.devicesInteractions.cancel.panel || !log.devicesInteractions.cancel.close || log.devicesInteractions.afterCancel.addOpen || !log.devicesInteractions.afterCancel.editPresent || !log.devicesInteractions.afterCancel.focused || !log.devicesInteractions.filter.clicked || !log.devicesInteractions.filterState.visible) {
      throw new Error("VIS Devices & Connections interaction lock failed: " + JSON.stringify(log.devicesInteractions));
    }
  });
});
`
);

fs.writeFileSync(
  path.join(testsDir, "run.js"),
  `const Mocha=require(${JSON.stringify(path.join(ext, "node_modules/mocha"))});const path=require("path");exports.run=function(){const m=new Mocha({ui:"tdd",timeout:240000});m.addFile(path.join(__dirname,"index.js"));return new Promise((res,rej)=>m.run(f=>f?rej(new Error(f+" fail")):res()));};`
);

async function main() {
  await runTests({
    vscodeExecutablePath: codeBin,
    extensionDevelopmentPath: ext,
    extensionTestsPath: path.join(testsDir, "run.js"),
    launchArgs: [
      ws,
      "--remote-debugging-port=" + PORT,
      "--ozone-platform=x11",
      "--disable-gpu",
      "--use-gl=angle",
	      "--use-angle=swiftshader",
	      "--in-process-gpu",
	      ...(themeKey === "hc" ? ["--force-high-contrast"] : []),
	      "--no-sandbox",
      "--user-data-dir=" + path.join(outDir, "ud"),
      "--extensions-dir=" + path.join(outDir, "ed"),
      "--disable-workspace-trust",
      "--skip-welcome",
    ],
    extensionTestsEnv: {
      CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || "",
      ST_LSP_TEST_SERVER: process.env.ST_LSP_TEST_SERVER || path.join(repo, "target/debug/trust-lsp"),
      ST_RUNTIME_TEST_BIN:
        process.env.ST_RUNTIME_TEST_BIN || path.join(repo, "target/debug/trust-runtime"),
    },
  });
  console.log(`VIS_THEME_DONE ${theme.suffix}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
