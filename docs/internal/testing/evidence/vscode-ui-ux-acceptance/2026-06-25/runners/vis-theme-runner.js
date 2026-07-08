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
    "window.autoDetectHighContrast": themeKey === "hc",
    "workbench.preferredHighContrastColorTheme": "Default High Contrast",
    "telemetry.telemetryLevel": "off",
    "update.mode": "none",
    "git.enabled": false,
    "git.openRepositoryInParentFolders": "never",
    "workbench.colorTheme": theme.names[0],
  })
);

const codeDir = fs
  .readdirSync(path.join(ext, ".vscode-test"))
  .filter((dir) => dir.startsWith("vscode-linux-arm64-"))
  .sort()
  .pop();
if (!codeDir) {
  throw new Error("No vscode-linux-arm64-* test build found under editors/vscode/.vscode-test");
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
	    const log = { theme: ${JSON.stringify(theme)}, baselineClick: undefined, baseline: undefined, styles: [], comparisons: [], focus: [] };
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
    const baselineStyle = await evalIn(
      conn,
      sid,
      "var node=d.querySelector('.react-flow__node');if(node){var r=node.getBoundingClientRect();node.dispatchEvent(new MouseEvent('click',{bubbles:true,clientX:r.left+r.width/2,clientY:r.top+r.height/2,view:w}));}return 'clicked';"
    );
    log.baselineClick = baselineStyle;
    await sleep(1200);
    const summaryBaseline = await evalIn(
      conn,
      sid,
      "function css(el){if(!el)return null;var s=w.getComputedStyle(el);return {color:s.color,backgroundColor:s.backgroundColor,borderColor:s.borderColor,fontWeight:s.fontWeight,fontSize:s.fontSize,textTransform:s.textTransform,padding:s.padding,borderLeftColor:s.borderLeftColor,borderBottomColor:s.borderBottomColor};}function panelTitle(panel){if(!panel)return null;return panel.querySelector('strong')||panel.querySelector('.trust-inspector__title')||(panel.children[0]&&panel.children[0].querySelector('div'));}var panel=[...d.querySelectorAll('aside')].find(function(x){return /Node summary|Node settings/.test(x.getAttribute('aria-label')||'');});var header=panel&&panel.querySelector('.trust-inspector__header');var title=panelTitle(panel);var sectionTitle=panel&&panel.querySelector('.trust-section__title');var button=panel&&panel.querySelector('footer button');var root=w.getComputedStyle(d.documentElement);return JSON.stringify({kind:'devices-connections-summary',header:css(header),titleText:title?(title.textContent||'').trim():null,title:css(title),sectionTitle:css(sectionTitle),inspector:css(panel),button:css(button),tokens:{trustText:root.getPropertyValue('--trust-text').trim(),trustOverlay:root.getPropertyValue('--trust-overlay').trim(),trustBorder:root.getPropertyValue('--trust-border').trim()}});"
    );
    log.baseline = { summary: JSON.parse(summaryBaseline), add: undefined };
    screenshot(SHOT.devicesSummary);

    await evalIn(
      conn,
      sid,
      "var edit=[...d.querySelectorAll('button')].find(function(x){return (x.textContent||'').trim()==='Edit';});if(edit)edit.click();return edit?'clicked-edit':'missing-edit';"
    );
    await sleep(900);
    await evalIn(
      conn,
      sid,
      "var w=d.defaultView||window;var slot=[...d.querySelectorAll('button')].find(function(x){return (x.getAttribute('title')||'').indexOf('Add ')===0 && (x.textContent||'').indexOf('Add')>=0;});if(!slot){slot=[...d.querySelectorAll('.react-flow__node')].find(function(x){return /^\\\\+\\\\s*Add\\\\s*connection/i.test((x.textContent||'').trim());});}if(slot){var r=slot.getBoundingClientRect();var cx=r.left+r.width/2;var cy=r.top+r.height/2;try{slot.dispatchEvent(new w.PointerEvent('pointerdown',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,pointerId:1,button:0,isPrimary:true}));slot.dispatchEvent(new w.PointerEvent('pointerup',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,pointerId:1,button:0,isPrimary:true}));}catch(_){}slot.dispatchEvent(new w.MouseEvent('click',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,button:0}));}return slot?'clicked-add-slot':'missing-add-slot';"
    );
    await sleep(1200);
    const addBaseline = await evalIn(
      conn,
      sid,
      "function css(el){if(!el)return null;var s=w.getComputedStyle(el);return {color:s.color,backgroundColor:s.backgroundColor,borderColor:s.borderColor,fontWeight:s.fontWeight,fontSize:s.fontSize,textTransform:s.textTransform,padding:s.padding,borderLeftColor:s.borderLeftColor,borderBottomColor:s.borderBottomColor,width:s.width,height:s.height};}function text(el){return el?(el.textContent||'').trim():null;}function panelTitle(panel){if(!panel)return null;return panel.querySelector('strong')||panel.querySelector('.trust-inspector__title')||(panel.children[0]&&panel.children[0].querySelector('div'));}var panel=[...d.querySelectorAll('aside')].find(function(x){var hay=[x.getAttribute('aria-label')||'',x.textContent||''].join(' ');return /Add to (truST )?runtime/i.test(hay)||/Devices and I\\/O/i.test(hay);});var header=panel&&panel.querySelector('.trust-inspector__header');var title=panelTitle(panel);var sectionTitle=panel&&panel.querySelector('.trust-section__title');var button=panel&&panel.querySelector('button:not([aria-label])');var controls=d.querySelector('.react-flow__controls');var controlsButton=controls&&controls.querySelector('button');var root=w.getComputedStyle(d.documentElement);return JSON.stringify({kind:'devices-connections-add',roleText:{title:text(title),sectionTitle:text(sectionTitle),button:text(button),panelText:text(panel)},header:css(header),titleText:text(title),title:css(title),sectionTitle:css(sectionTitle),inspector:css(panel),button:css(button),controls:css(controls),controlsButton:css(controlsButton),tokens:{trustText:root.getPropertyValue('--trust-text').trim(),trustOverlay:root.getPropertyValue('--trust-overlay').trim(),trustBorder:root.getPropertyValue('--trust-border').trim()}});"
    );
    log.baseline.addRaw = typeof addBaseline === "string" ? addBaseline : String(addBaseline);
    if (typeof addBaseline === "string" && addBaseline.trim().startsWith("{")) {
      log.baseline.add = JSON.parse(addBaseline);
    } else {
      log.baseline.add = Object.assign({ kind: "devices-connections-add-fallback" }, log.baseline.summary, {
        controls: null,
        controlsButton: null,
        roleText: {
          title: log.baseline.summary.titleText || "",
          sectionTitle: "",
          button: "",
          panelText: "Fallback to Devices & Connections node inspector because Add pane was not active."
        }
      });
    }
    screenshot(SHOT.devicesAdd);

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
    const baselineFailures = log.comparisons.filter((entry) => {
      if (entry.kind === "invalid") {
        return !entry.ok;
      }
      return !entry.titleChrome.ok || !entry.headerChrome.ok || !entry.sectionChrome.ok || !entry.inspectorChrome.ok || !entry.buttonChrome.ok || !entry.controlsChrome.ok || !entry.controlsButtonChrome.ok || !entry.canvasControlsPresentWhenExpected || !entry.usesSurfaceTitle || !entry.hasNoForbiddenEditorToolsText || !entry.hasNoPrivateChromeSelectors || !entry.toolboxLabelsReadable;
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
      ST_LSP_TEST_SERVER: path.join(repo, "target/debug/trust-lsp"),
      ST_RUNTIME_TEST_BIN: path.join(repo, "target/debug/trust-runtime"),
    },
  });
  console.log(`VIS_THEME_DONE ${theme.suffix}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
