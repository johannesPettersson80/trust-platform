#!/usr/bin/env node

// J-17 / ADSC-02..07 (local proof): open the ADS client example, inspect the
// configured ADS endpoint, prove that a natively entered ADS port reaches the
// runtime command boundary, browse tags until the honest missing-route recovery
// appears, and prove the imported ADS symbols compile in ST.
//
// Port 301 receives a deterministic unavailable response at the runtime-wrapper
// boundary. Live TwinCAT discovery/browse/value proof remains lab-gated and is
// recorded in the journey report rather than faked by this runner.
const fs = require("fs");
const crypto = require("crypto");
const http = require("http");
const path = require("path");

const repo = process.env.TRUST_REPO || process.env.TRUST_PLATFORM_REPO_ROOT || "/home/johannes/projects/trust-platform";
const ext = path.join(repo, "editors/vscode");
const networkCanvasBundle = path.join(ext, "media", "networkCanvasWebview.js");
const bundleSha256 = crypto
  .createHash("sha256")
  .update(fs.readFileSync(networkCanvasBundle))
  .digest("hex");
const bundleBuiltAt = fs.statSync(networkCanvasBundle).mtime.toISOString();
const pngHygienePath = path.join(__dirname, "png-hygiene.js");
const { runTests } = require(path.join(ext, "node_modules/@vscode/test-electron"));

const evidenceRoot = process.env.TRUST_UX_EVIDENCE_ROOT
  ? path.resolve(process.env.TRUST_UX_EVIDENCE_ROOT)
  : path.resolve(__dirname, "..");
const screenshotsDir = process.env.TRUST_UX_SCREENSHOTS_DIR
  ? path.resolve(process.env.TRUST_UX_SCREENSHOTS_DIR)
  : path.join(evidenceRoot, "screenshots-raw");
const jsonDir = process.env.TRUST_UX_JSON_DIR
  ? path.resolve(process.env.TRUST_UX_JSON_DIR)
  : path.join(evidenceRoot, "json");
const outRoot = path.join(evidenceRoot, "runner-output", "ads-client-program");
const testsDir = path.join(outRoot, "tests");
const project = path.join(outRoot, "project");
const userDataDir = path.join(outRoot, "ud");
const extensionsDir = path.join(outRoot, "ext");
const realRuntimeBin = process.env.ST_RUNTIME_TEST_BIN || path.join(repo, "target/debug/trust-runtime");
const lspBin = process.env.ST_LSP_TEST_SERVER || path.join(repo, "target/debug/trust-lsp");
const cdpPort = Number(process.env.TRUST_ADS_CLIENT_CDP_PORT || 19947);
const colorTheme = process.env.TRUST_UX_THEME || "Default Dark Modern";

fs.rmSync(outRoot, { recursive: true, force: true });
fs.mkdirSync(testsDir, { recursive: true });
fs.mkdirSync(path.join(userDataDir, "User"), { recursive: true });
fs.mkdirSync(screenshotsDir, { recursive: true });
fs.mkdirSync(jsonDir, { recursive: true });
fs.cpSync(path.join(repo, "examples/communication/ads_line1"), project, { recursive: true });

// Keep the real runtime for every command except the deterministic port-301 probe. The wrapper logs
// the actual argv received from the extension host, which proves the selected port crossed the
// webview/host boundary without requiring a TwinCAT server on the acceptance machine.
const runtimeProbeLog = path.join(outRoot, "runtime-probe.jsonl");
const runtimeProbeBin = path.join(outRoot, "runtime-probe.js");
fs.writeFileSync(
  runtimeProbeBin,
  `#!/usr/bin/env node
const cp = require("child_process");
const fs = require("fs");
const realRuntime = ${JSON.stringify(realRuntimeBin)};
const probeLog = ${JSON.stringify(runtimeProbeLog)};
const args = process.argv.slice(2);
const targetIndex = args.indexOf("--target");
let target;
if (targetIndex >= 0 && targetIndex + 1 < args.length) {
  try { target = JSON.parse(args[targetIndex + 1]); } catch { target = undefined; }
}
fs.appendFileSync(probeLog, JSON.stringify({ args, target }) + "\\n");
const isPort301Browse =
  args[0] === "comm" &&
  args[1] === "browse-symbols" &&
  target &&
  target.ams_port === 301;
if (isPort301Browse) {
  process.stdout.write(JSON.stringify({
    schema_version: 1,
    protocol: "ads",
    tree: [],
    error: {
      code: "ads_port_unavailable",
      message: "Acceptance boundary: ADS port 301 is unavailable; no hardware request was made.",
    },
  }) + "\\n");
  process.exit(0);
}
const isPort851Browse =
  args[0] === "comm" &&
  args[1] === "browse-symbols" &&
  target &&
  target.ams_port === 851;
const port851BrowseCount = fs
  .readFileSync(probeLog, "utf8")
  .trim()
  .split(String.fromCharCode(10))
  .filter(Boolean)
  .map((line) => JSON.parse(line))
  .filter((entry) =>
    entry.args &&
    entry.args[0] === "comm" &&
    entry.args[1] === "browse-symbols" &&
    entry.target &&
    entry.target.ams_port === 851
  ).length;
if (isPort851Browse && port851BrowseCount >= 2) {
  const children = Array.from({ length: 60 }, (_, index) => {
    const suffix = String(index).padStart(2, "0");
    return {
      id: "test-symbol-" + suffix,
      name: "TestSymbol" + suffix,
      path: "Test group.TestSymbol" + suffix,
      data_type: "DINT",
      writable: true,
    };
  });
  process.stdout.write(JSON.stringify({
    schema_version: 1,
    protocol: "ads",
    tree: [{
      id: "test-group",
      name: "Test group",
      path: "Test group",
      children,
    }],
  }) + "\\n");
  process.exit(0);
}
const result = cp.spawnSync(realRuntime, args, {
  env: process.env,
  encoding: "utf8",
  maxBuffer: 64 * 1024 * 1024,
});
if (result.stdout) process.stdout.write(result.stdout);
if (result.stderr) process.stderr.write(result.stderr);
if (result.error) {
  process.stderr.write(String(result.error.message || result.error) + "\\n");
  process.exit(1);
}
process.exit(typeof result.status === "number" ? result.status : 1);
`
);
fs.chmodSync(runtimeProbeBin, 0o755);

const runtimeToml = path.join(project, "runtime.toml");
fs.writeFileSync(
  runtimeToml,
  fs.readFileSync(runtimeToml, "utf8").replace(/name = "CommAdsRes"/, 'name = "ADS line 1"')
);

fs.writeFileSync(path.join(userDataDir, "User", "settings.json"), `${JSON.stringify({
  "workbench.colorTheme": colorTheme,
  "git.openRepositoryInParentFolders": "never",
  "git.autoRepositoryDetection": false,
  "chat.commandCenter.enabled": false,
  "window.commandCenter": false,
  "workbench.layoutControl.enabled": false,
  "workbench.startupEditor": "none",
  "telemetry.telemetryLevel": "off",
  "update.mode": "none",
  "trust-lsp.runtime.cli.path": runtimeProbeBin,
}, null, 2)}\n`);

fs.writeFileSync(path.join(testsDir, "index.js"), `
const assert = require("assert");
const cp = require("child_process");
const fs = require("fs");
const http = require("http");
const path = require("path");
const vscode = require("vscode");
const WebSocket = require(${JSON.stringify(path.join(ext, "node_modules/ws"))});
const pngHygiene = require(${JSON.stringify(pngHygienePath)});

const evidenceRoot = ${JSON.stringify(evidenceRoot)};
const screenshotsDir = ${JSON.stringify(screenshotsDir)};
const jsonDir = ${JSON.stringify(jsonDir)};
const project = ${JSON.stringify(project)};
const runtimeProbeLog = ${JSON.stringify(runtimeProbeLog)};
const colorTheme = ${JSON.stringify(colorTheme)};
const cdpPort = ${cdpPort};
const proof = {
  journey: "J-17",
  workflow: "Browse ADS tags and use them in a program",
  bundle: {
    path: "editors/vscode/media/networkCanvasWebview.js",
    sha256: ${JSON.stringify(bundleSha256)},
    builtAt: ${JSON.stringify(bundleBuiltAt)},
  },
  theme: colorTheme,
  project,
  rows_proven: ["ADSC-02", "ADSC-03", "ADSC-05", "ADSC-06", "ADSC-07", "IOMAP-02", "ST-02", "RUN-01"],
  rows_not_fully_proven: ["ADSC-01", "ADSC-04", "RUN-04", "LV-02", "ERR-12"],
  steps: [],
};

function sleep(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }
function xtestClick(x, y, holdMs) {
  const script = [
    "import ctypes,sys,time",
    "x11=ctypes.CDLL('libX11.so.6')",
    "xtst=ctypes.CDLL('libXtst.so.6')",
    "x11.XOpenDisplay.argtypes=[ctypes.c_char_p]",
    "x11.XOpenDisplay.restype=ctypes.c_void_p",
    "x11.XFlush.argtypes=[ctypes.c_void_p]",
    "xtst.XTestFakeMotionEvent.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_ulong]",
    "xtst.XTestFakeButtonEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]",
    "display=x11.XOpenDisplay(None)",
    "assert display, 'Could not open X display'",
    "x=int(float(sys.argv[1]));y=int(float(sys.argv[2]));hold=float(sys.argv[3])/1000.0",
    "xtst.XTestFakeMotionEvent(display,-1,x,y,0);x11.XFlush(display);time.sleep(0.10)",
    "xtst.XTestFakeButtonEvent(display,1,1,0);x11.XFlush(display);time.sleep(hold)",
    "xtst.XTestFakeButtonEvent(display,1,0,0);x11.XFlush(display);time.sleep(0.10)",
  ].join("\\n");
  cp.execFileSync("/usr/bin/python3", ["-c", script, String(x), String(y), String(holdMs)], { env: process.env });
}
function xtestReplaceText(x, y, value) {
  const script = [
    "import ctypes,sys,time",
    "x11=ctypes.CDLL('libX11.so.6')",
    "xtst=ctypes.CDLL('libXtst.so.6')",
    "x11.XOpenDisplay.argtypes=[ctypes.c_char_p]",
    "x11.XOpenDisplay.restype=ctypes.c_void_p",
    "x11.XKeysymToKeycode.argtypes=[ctypes.c_void_p,ctypes.c_ulong]",
    "x11.XKeysymToKeycode.restype=ctypes.c_uint",
    "x11.XFlush.argtypes=[ctypes.c_void_p]",
    "xtst.XTestFakeMotionEvent.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_ulong]",
    "xtst.XTestFakeButtonEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]",
    "xtst.XTestFakeKeyEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]",
    "display=x11.XOpenDisplay(None)",
    "assert display, 'Could not open X display'",
    "def key(keysym,down):",
    "  keycode=x11.XKeysymToKeycode(display,keysym)",
    "  assert keycode, 'Could not resolve keysym'",
    "  xtst.XTestFakeKeyEvent(display,keycode,down,0);x11.XFlush(display);time.sleep(0.03)",
    "x=int(float(sys.argv[1]));y=int(float(sys.argv[2]));value=sys.argv[3]",
    "xtst.XTestFakeMotionEvent(display,-1,x,y,0);x11.XFlush(display);time.sleep(0.10)",
    "xtst.XTestFakeButtonEvent(display,1,1,0);x11.XFlush(display);time.sleep(0.04)",
    "xtst.XTestFakeButtonEvent(display,1,0,0);x11.XFlush(display);time.sleep(0.08)",
    "key(0xffe3,1);key(0x61,1);key(0x61,0);key(0xffe3,0)",
    "time.sleep(0.12)",
    "for _ in range(8):",
    "  key(0xff08,1);key(0xff08,0)",
    "for character in value:",
    "  key(ord(character),1);key(ord(character),0)",
    "time.sleep(0.10)",
  ].join("\\n");
  cp.execFileSync("/usr/bin/python3", ["-c", script, String(x), String(y), String(value)], { env: process.env });
}
function xtestSpace() {
  const script = [
    "import ctypes,time",
    "x11=ctypes.CDLL('libX11.so.6')",
    "xtst=ctypes.CDLL('libXtst.so.6')",
    "x11.XOpenDisplay.argtypes=[ctypes.c_char_p]",
    "x11.XOpenDisplay.restype=ctypes.c_void_p",
    "x11.XKeysymToKeycode.argtypes=[ctypes.c_void_p,ctypes.c_ulong]",
    "x11.XKeysymToKeycode.restype=ctypes.c_uint",
    "x11.XFlush.argtypes=[ctypes.c_void_p]",
    "xtst.XTestFakeKeyEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]",
    "display=x11.XOpenDisplay(None)",
    "assert display, 'Could not open X display'",
    "keycode=x11.XKeysymToKeycode(display,0x20)",
    "xtst.XTestFakeKeyEvent(display,keycode,1,0);x11.XFlush(display);time.sleep(0.07)",
    "xtst.XTestFakeKeyEvent(display,keycode,0,0);x11.XFlush(display);time.sleep(0.10)",
  ].join("\\n");
  cp.execFileSync("/usr/bin/python3", ["-c", script], { env: process.env });
}
function httpJson(pathname) {
  return new Promise((resolve, reject) => {
    const req = http.get("http://127.0.0.1:" + cdpPort + pathname, (res) => {
      let body = "";
      res.on("data", (chunk) => { body += chunk; });
      res.on("end", () => {
        try { resolve(JSON.parse(body)); } catch (error) { reject(error); }
      });
    });
    req.on("error", reject);
    req.setTimeout(5000, () => req.destroy(new Error("http timeout " + pathname)));
  });
}
async function waitForHttpJson(pathname) {
  const start = Date.now();
  let last;
  while (Date.now() - start < 20000) {
    try { return await httpJson(pathname); }
    catch (error) { last = error; await sleep(250); }
  }
  throw last || new Error("timed out waiting for " + pathname);
}
function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    let id = 0;
    const pending = new Map();
    ws.on("message", (data) => {
      const message = JSON.parse(data.toString());
      if (message.id && pending.has(message.id)) {
        pending.get(message.id)(message);
        pending.delete(message.id);
      }
    });
    ws.on("error", reject);
    ws.on("open", () => {
      resolve({
        send(method, params, sessionId) {
          return new Promise((done) => {
            const messageId = ++id;
            let settled = false;
            const finish = (value) => {
              if (!settled) {
                settled = true;
                done(value);
              }
            };
            pending.set(messageId, finish);
            setTimeout(() => finish({ __timeout: method }), 10000);
            ws.send(JSON.stringify({ id: messageId, method, params: params || {}, sessionId }));
          });
        },
        close() { ws.close(); },
      });
    });
  });
}
async function attach(conn, target, page = false) {
  const attached = await conn.send("Target.attachToTarget", { targetId: target.id, flatten: true });
  const sessionId = attached.result && attached.result.sessionId;
  assert.ok(sessionId, "expected CDP session id");
  await conn.send(page ? "Page.enable" : "Runtime.enable", {}, sessionId);
  return sessionId;
}
async function evalInner(conn, sid, body) {
  const expression = "(function(){try{var f=document.querySelector('iframe');var d=f&&f.contentDocument;if(!d)return {error:'NO_INNER_DOC'};var w=f.contentWindow;" + body + "}catch(e){return {error:e.message, stack:e.stack};}})()";
  const result = await conn.send("Runtime.evaluate", { expression, returnByValue: true }, sid);
  return result && result.result && result.result.result && result.result.result.value;
}
async function screenshot(conn, pageSid, name) {
  await sleep(350);
  const captured = await conn.send("Page.captureScreenshot", { format: "png", fromSurface: true }, pageSid);
  const data = captured && captured.result && captured.result.data;
  assert.ok(data, "expected screenshot data for " + name);
  const dest = path.join(screenshotsDir, name + ".png");
  pngHygiene.writePngBase64(dest, data);
  const bytes = fs.statSync(dest).size;
  assert.ok(bytes > 20000, name + " screenshot too small: " + bytes);
  proof.steps.push({ screenshot: path.relative(evidenceRoot, dest), bytes });
  return { path: path.relative(evidenceRoot, dest), bytes };
}
async function innerText(conn, sid) {
  return await evalInner(conn, sid, "return (d.body&&d.body.innerText||'').replace(/\\\\s+/g,' ').trim();");
}
async function adsDraftState(conn, sid) {
  return await evalInner(
    conn,
    sid,
    "var panel=d.querySelector('aside[aria-label~=Browse]');if(!panel)return {error:'NO_BROWSE_PANEL'};var visible=function(element){if(!element)return false;var rect=element.getBoundingClientRect();var style=w.getComputedStyle(element);return rect.width>0&&rect.height>0&&style.visibility!=='hidden'&&style.display!=='none';};var text=panel.innerText||'';var input=panel.querySelector('input[data-role=ads-browse-port]');var allow=panel.querySelector('input[data-role=allow-writes]');var add=[...panel.querySelectorAll('button')].find(function(button){return (button.textContent||'').trim().indexOf('Add tags')===0;});var stale=[...panel.querySelectorAll('p')].find(function(element){return (element.textContent||'').indexOf('displayed ADS port has not been browsed yet')>=0;});var oldError=[...panel.querySelectorAll('span')].find(function(element){return (element.textContent||'').indexOf('ADS port unavailable')>=0;});var route=[...panel.querySelectorAll('button')].find(function(button){return (button.textContent||'').trim()==='Create route';});return {value:input&&input.value,staleVisible:visible(stale),errorVisible:visible(oldError),routeVisible:visible(route),treeVisible:text.indexOf('Test group')>=0,symbolSelectionCount:[...panel.querySelectorAll('input[data-role=symbol-selection]')].filter(visible).length,allowDisabled:Boolean(allow&&allow.disabled),addDisabled:Boolean(add&&add.disabled),addText:add&&(add.textContent||'').trim(),text:text.slice(0,1600)};"
  );
}
async function waitForText(conn, sid, pattern, label, timeoutMs = 45000) {
  const re = new RegExp(pattern, "i");
  let text = "";
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    text = String((await innerText(conn, sid)) || "");
    if (re.test(text)) {
      return text;
    }
    await sleep(500);
  }
  throw new Error("Timed out waiting for " + label + ": " + text.slice(0, 1800));
}
async function pageText(conn, sid) {
  const result = await conn.send(
    "Runtime.evaluate",
    {
      expression: "(document.body&&document.body.innerText||'').replace(/\\\\s+/g,' ').trim()",
      returnByValue: true,
    },
    sid
  );
  return String(result && result.result && result.result.result && result.result.result.value || "");
}
async function waitForPageText(conn, sid, pattern, label, timeoutMs = 20000) {
  const re = new RegExp(pattern, "i");
  let text = "";
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    text = await pageText(conn, sid);
    if (re.test(text)) {
      return text;
    }
    await sleep(250);
  }
  throw new Error("Timed out waiting for " + label + ": " + text.slice(0, 1800));
}
async function clickNode(conn, sid, pattern) {
  return await evalInner(conn, sid, "var re=new RegExp(" + JSON.stringify(pattern) + ",'i');var nodes=[...d.querySelectorAll('.react-flow__node')];var node=nodes.find(function(n){return re.test((n.textContent||'').replace(/\\\\s+/g,' '))||re.test(n.getAttribute('data-id')||'');});if(!node)return {clicked:false,pattern:" + JSON.stringify(pattern) + ",nodes:nodes.map(function(n){return {id:n.getAttribute('data-id'), text:(n.textContent||'').replace(/\\\\s+/g,' ').trim().slice(0,120)};}).slice(0,80)};var r=node.getBoundingClientRect();var cx=r.left+r.width/2;var cy=r.top+r.height/2;['pointerdown','pointerup'].forEach(function(type){try{node.dispatchEvent(new w.PointerEvent(type,{bubbles:true,cancelable:true,clientX:cx,clientY:cy,pointerId:1,button:0,isPrimary:true}));}catch(e){}});node.dispatchEvent(new w.MouseEvent('click',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,button:0}));return {clicked:true,id:node.getAttribute('data-id'),text:(node.textContent||'').replace(/\\\\s+/g,' ').trim().slice(0,160)};");
}
async function clickButton(conn, sid, pattern) {
  return await evalInner(conn, sid, "var re=new RegExp(" + JSON.stringify(pattern) + ",'i');var buttons=[...d.querySelectorAll('button,[role=button]')];var button=buttons.find(function(b){return re.test((b.textContent||b.getAttribute('aria-label')||'').replace(/\\\\s+/g,' ').trim());});if(!button)return {clicked:false,pattern:" + JSON.stringify(pattern) + ",buttons:buttons.map(function(b){return (b.textContent||b.getAttribute('aria-label')||'').replace(/\\\\s+/g,' ').trim();}).filter(Boolean).slice(0,80)};button.scrollIntoView({block:'center',inline:'nearest'});button.click();return {clicked:true,text:(button.textContent||button.getAttribute('aria-label')||'').replace(/\\\\s+/g,' ').trim().slice(0,160)};");
}

suite("ads-client-program", function() {
  this.timeout(220000);

  test("reviews the local ADS client path and honest route recovery", async function() {
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    if (extension) {
      await extension.activate();
    }
    await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
    await vscode.commands.executeCommand("workbench.action.closePanel");
    await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
    await sleep(6500);

    const version = await waitForHttpJson("/json/version");
    const targets = await waitForHttpJson("/json");
    const page = targets.find((target) => target.type === "page");
    const webview = targets.find((target) => target.type === "iframe" && /index\\.html/.test(target.url || ""));
    assert.ok(page, "expected VS Code page target");
    assert.ok(webview, "expected Devices & Connections webview target");
    const conn = await connect(version.webSocketDebuggerUrl);
    try {
      const win = await conn.send("Browser.getWindowForTarget", { targetId: page.id });
      if (win.result && win.result.windowId) {
        await conn.send("Browser.setWindowBounds", {
          windowId: win.result.windowId,
          bounds: { left: 0, top: 0, width: 1280, height: 900, windowState: "normal" },
        });
      }
      await sleep(1000);
      const pageSid = await attach(conn, page, true);
      const webSid = await attach(conn, webview, false);
      await waitForText(conn, webSid, "ADS line 1|ADS client", "ADS topology");
      proof.steps.push({ step: "open Devices & Connections ADS topology" });
      await screenshot(conn, pageSid, "ADSC-05-configured-ads-client");

      const clicked = await clickNode(conn, webSid, "endpoint:.*:ads");
      proof.steps.push({ step: "click ADS endpoint", result: clicked });
      assert.ok(clicked.clicked, "could not click ADS endpoint: " + JSON.stringify(clicked));
      await waitForText(conn, webSid, "ADS client|Connections|line1", "ADS inspector");
      await screenshot(conn, pageSid, "ADSC-05-selected-tags-inspector");

      const browse = await clickButton(conn, webSid, "Browse tags");
      proof.steps.push({ step: "click Browse tags", result: browse });
      assert.ok(browse.clicked, "could not click Browse tags: " + JSON.stringify(browse));
      await waitForText(conn, webSid, "ADS port|Browse symbols", "ADS browse target controls");

      const adsControls = await evalInner(
        conn,
        webSid,
        "var panel=d.querySelector('aside[aria-label~=Browse]');" +
          "var port=panel&&panel.querySelector('input[data-role=ads-browse-port]');" +
          "var browseButton=panel&&panel.querySelector('button[data-role=browse-ads-symbols]');" +
          "if(!port||!browseButton)return {error:'NO_ADS_PORT_CONTROLS',text:panel&&(panel.textContent||'').slice(0,1000)};" +
          "function point(element){var rect=element.getBoundingClientRect();var frame=f.getBoundingClientRect();return {x:frame.left+rect.left+rect.width/2,y:frame.top+rect.top+rect.height/2,frame:{width:frame.width,height:frame.height}};}" +
          "return {port:point(port),browse:point(browseButton),value:port.value,netId:(panel.querySelector('code')&&panel.querySelector('code').textContent||'').trim()};"
      );
      assert.ok(adsControls && !adsControls.error, "could not locate ADS browse controls: " + JSON.stringify(adsControls));
      assert.strictEqual(adsControls.value, "851", "ADS browse must start on the configured PLC port");

      const adsGeometryResponse = await conn.send(
        "Runtime.evaluate",
        {
          expression: "({screenX:window.screenX,screenY:window.screenY,iframes:[...document.querySelectorAll('iframe')].map(function(frame){var rect=frame.getBoundingClientRect();return {left:rect.left,top:rect.top,width:rect.width,height:rect.height};})})",
          returnByValue: true,
        },
        pageSid
      );
      const adsPageGeometry = adsGeometryResponse && adsGeometryResponse.result && adsGeometryResponse.result.result && adsGeometryResponse.result.result.value;
      assert.ok(adsPageGeometry && adsPageGeometry.iframes && adsPageGeometry.iframes.length, "expected VS Code iframe geometry for ADS controls");
      const adsOuterFrame =
        adsPageGeometry.iframes.find(
          (frame) => Math.abs(frame.width - adsControls.port.frame.width) < 1 && Math.abs(frame.height - adsControls.port.frame.height) < 1
        ) || adsPageGeometry.iframes[0];
      const adsRootPoint = (point) => ({
        x: adsPageGeometry.screenX + adsOuterFrame.left + point.x,
        y: adsPageGeometry.screenY + adsOuterFrame.top + point.y,
      });
      const portRootPoint = adsRootPoint(adsControls.port);
      const browseRootPoint = adsRootPoint(adsControls.browse);

      // Enter the non-PLC I/O server port with trusted XTest mouse/key events, then observe the
      // request at the runtime wrapper. The deterministic unavailable response is boundary proof,
      // not a claim that a physical TwinCAT I/O server was contacted.
      xtestReplaceText(portRootPoint.x, portRootPoint.y, "301");
      await sleep(350);
      const port301State = await evalInner(
        conn,
        webSid,
        "var panel=d.querySelector('aside[aria-label~=Browse]');var input=panel&&panel.querySelector('input[data-role=ads-browse-port]');return {value:input&&input.value,active:d.activeElement===input,netId:(panel&&panel.querySelector('code')&&panel.querySelector('code').textContent||'').trim()};"
      );
      assert.strictEqual(port301State.value, "301", "native ADS port entry must update the controlled input");
      assert.ok(port301State.active, "native ADS port entry must focus the port input");
      assert.ok(port301State.netId, "AMS Net ID must remain visible beside the selected port");
      const port301ThemeState = await evalInner(
        conn,
        webSid,
        "var panel=d.querySelector('aside[aria-label~=Browse]');var input=panel&&panel.querySelector('input[data-role=ads-browse-port]');var button=panel&&panel.querySelector('button[data-role=browse-ads-symbols]');var netId=panel&&panel.querySelector('code');function snapshot(element){if(!element)return null;var rect=element.getBoundingClientRect();var style=w.getComputedStyle(element);return {tag:element.tagName.toLowerCase(),type:element.type||'',text:(element.textContent||element.value||'').trim(),visible:rect.width>0&&rect.height>0&&style.visibility!=='hidden'&&style.display!=='none',color:style.color,backgroundColor:style.backgroundColor,borderColor:style.borderColor};}return {panelLabel:panel&&panel.getAttribute('aria-label'),input:snapshot(input),button:snapshot(button),netId:snapshot(netId)};"
      );
      assert.strictEqual(port301ThemeState.panelLabel, "Browse tags", "ADS controls need a labelled region");
      assert.ok(port301ThemeState.input && port301ThemeState.input.tag === "input" && port301ThemeState.input.type === "number" && port301ThemeState.input.visible, "ADS port must remain a visible native number/spinbutton input");
      assert.ok(port301ThemeState.button && port301ThemeState.button.tag === "button" && port301ThemeState.button.visible, "Browse symbols must remain a visible native button");
      assert.ok(port301ThemeState.netId && port301ThemeState.netId.visible && port301ThemeState.netId.text === port301State.netId, "AMS Net ID must remain visibly readable");
      const port301Screenshot = await screenshot(conn, pageSid, "ADSC-05-ads-port-301-selected");

      xtestClick(browseRootPoint.x, browseRootPoint.y, 60);
      const port301ResponseText = await waitForText(
        conn,
        webSid,
        "ADS port unavailable|Symbol Upload unsupported|No ADS route",
        "structured ADS port 301 response"
      );
      assert.match(port301ResponseText, /ADS port unavailable/i, "port 301 probe must surface the structured unavailable response");
      assert.ok(!/empty symbol table|No compatible symbols/i.test(port301ResponseText), "structured ADS errors must not also render contradictory empty-result copy");
      const port301ErrorThemeState = await evalInner(
        conn,
        webSid,
        "var panel=d.querySelector('aside[aria-label~=Browse]');var warning=panel&&[...panel.querySelectorAll('span')].find(function(element){return /ADS port unavailable/i.test(element.textContent||'');});if(!warning)return {found:false};var rect=warning.getBoundingClientRect();var style=w.getComputedStyle(warning);return {found:true,text:(warning.textContent||'').replace(/\\\\s+/g,' ').trim().slice(0,600),visible:rect.width>0&&rect.height>0&&style.visibility!=='hidden'&&style.display!=='none',color:style.color,backgroundColor:style.backgroundColor,borderColor:style.borderColor};"
      );
      assert.ok(port301ErrorThemeState.found && port301ErrorThemeState.visible, "ADS port error copy must remain visibly readable");
      const port301ErrorScreenshot = await screenshot(conn, pageSid, "ADSC-05-ads-port-301-unavailable");

      xtestReplaceText(portRootPoint.x, portRootPoint.y, "501");
      await sleep(350);
      const staleAfterError = await adsDraftState(conn, webSid);
      assert.strictEqual(staleAfterError.value, "501", "unbrowsed Motion port draft must remain visible");
      assert.ok(staleAfterError.staleVisible, "edited ADS port must explain that it has not been browsed");
      assert.ok(!staleAfterError.errorVisible, "edited ADS port must hide the prior port error");
      assert.ok(!staleAfterError.routeVisible && !staleAfterError.treeVisible && staleAfterError.symbolSelectionCount === 0, "edited ADS port must not expose stale route or symbol state");
      assert.ok(staleAfterError.allowDisabled && staleAfterError.addDisabled, "edited ADS port must disable write and Add tags controls");
      const staleErrorScreenshot = await screenshot(conn, pageSid, "ADSC-05-port-501-draft-invalidates-error");
      xtestReplaceText(portRootPoint.x, portRootPoint.y, "301");
      await sleep(350);
      const restored301 = await adsDraftState(conn, webSid);
      assert.ok(restored301.errorVisible && !restored301.staleVisible, "restoring browsed port 301 must restore its scoped error state");

      const probeRequests = fs
        .readFileSync(runtimeProbeLog, "utf8")
        .trim()
        .split(String.fromCharCode(10))
        .filter(Boolean)
        .map((line) => JSON.parse(line));
      const port301Request = probeRequests.find(
        (request) =>
          request.args &&
          request.args[0] === "comm" &&
          request.args[1] === "browse-symbols" &&
          request.target &&
          request.target.ams_port === 301
      );
      assert.ok(port301Request, "extension host must emit a browse-symbols request with ams_port=301");
      assert.ok(port301Request.args.includes("--target"), "ADS port 301 must be carried in the serialized target request");

      xtestReplaceText(portRootPoint.x, portRootPoint.y, "851");
      await sleep(350);
      const port851State = await evalInner(
        conn,
        webSid,
        "var input=d.querySelector('input[data-role=ads-browse-port]');return {value:input&&input.value,active:d.activeElement===input};"
      );
      assert.strictEqual(port851State.value, "851", "native input must return the browse target to PLC port 851");
      xtestClick(browseRootPoint.x, browseRootPoint.y, 60);
      const browseText = await waitForText(conn, webSid, "No ADS route|StaticRoutes|Manual TwinCAT route steps|Download PowerShell", "ADS route recovery", 65000);
      proof.routeRecoveryText = browseText.slice(0, 2500);
      const post851Requests = fs
        .readFileSync(runtimeProbeLog, "utf8")
        .trim()
        .split(String.fromCharCode(10))
        .filter(Boolean)
        .map((line) => JSON.parse(line));
      const port851Request = post851Requests.find(
        (request) =>
          request.args &&
          request.args[0] === "comm" &&
          request.args[1] === "browse-symbols" &&
          request.target &&
          request.target.ams_port === 851
      );
      assert.ok(port851Request, "extension host must emit the follow-up browse-symbols request with ams_port=851");
      xtestReplaceText(portRootPoint.x, portRootPoint.y, "501");
      await sleep(350);
      const staleAfterRoute = await adsDraftState(conn, webSid);
      assert.strictEqual(staleAfterRoute.value, "501", "unbrowsed Motion port draft must remain visible over route recovery");
      assert.ok(staleAfterRoute.staleVisible && !staleAfterRoute.routeVisible, "edited ADS port must hide prior route recovery state");
      assert.ok(!staleAfterRoute.errorVisible && !staleAfterRoute.treeVisible && staleAfterRoute.symbolSelectionCount === 0, "edited ADS port must expose no stale error or symbol state");
      assert.ok(staleAfterRoute.allowDisabled && staleAfterRoute.addDisabled, "edited ADS port must keep write and Add tags disabled over route recovery");
      const staleRouteScreenshot = await screenshot(conn, pageSid, "ADSC-05-port-501-draft-invalidates-route");
      xtestReplaceText(portRootPoint.x, portRootPoint.y, "851");
      await sleep(350);
      const restored851Route = await adsDraftState(conn, webSid);
      assert.ok(restored851Route.routeVisible && !restored851Route.staleVisible, "restoring browsed port 851 must restore its scoped route state");
      proof.adsPortSelection = {
        validation: "deterministic runtime-wrapper boundary; no ADS hardware validation claimed",
        netId: port301State.netId,
        selectedPort: 301,
        nativeInputFocused: port301State.active,
        screenshot: port301Screenshot.path,
        errorScreenshot: port301ErrorScreenshot.path,
        theme: colorTheme,
        themeSemantics: port301ThemeState,
        errorReadability: port301ErrorThemeState,
        emittedRequest: port301Request,
        structuredResponse: "ads_port_unavailable",
        draftInvalidation: {
          draftPort: 501,
          error: staleAfterError,
          errorScreenshot: staleErrorScreenshot.path,
          route: staleAfterRoute,
          routeScreenshot: staleRouteScreenshot.path,
        },
        returnedToPort: 851,
        followUpRequest: port851Request,
      };
      proof.steps.push({ step: "select ADS port 301 natively, observe host request, then return to 851", result: proof.adsPortSelection });
      await screenshot(conn, pageSid, "ADSC-02-route-missing-recovery");
      const createRoute = await clickButton(conn, webSid, "Create route");
      proof.steps.push({ step: "click Create route", result: createRoute });
      assert.ok(createRoute.clicked, "could not click Create route: " + JSON.stringify(createRoute));
      const createRouteText = await waitForText(conn, webSid, "Automatic route creation|Administrator|TwinCAT computer", "ADS create route result");
      proof.createRouteText = createRouteText.slice(0, 2500);
      await screenshot(conn, pageSid, "ADSC-03-create-route-admin-needed");

      // Browse again through the real host boundary. The runtime wrapper returns a deterministic
      // tree on the second 851 request; the extension host echoes the current browse session/request
      // identity, so stale/unscoped symbolTree messages cannot satisfy this proof.
      xtestClick(browseRootPoint.x, browseRootPoint.y, 60);
      await waitForText(conn, webSid, "Test group", "correlated deterministic ADS symbol tree", 30000);
      const correlatedRequests = fs
        .readFileSync(runtimeProbeLog, "utf8")
        .trim()
        .split(String.fromCharCode(10))
        .filter(Boolean)
        .map((line) => JSON.parse(line))
        .filter(
          (request) =>
            request.args &&
            request.args[0] === "comm" &&
            request.args[1] === "browse-symbols" &&
            request.target &&
            request.target.ams_port === 851
        );
      assert.ok(correlatedRequests.length >= 2, "expected the correlated second 851 browse request");
      proof.correlatedSymbolTree = {
        transport: "webview request -> extension host -> runtime wrapper -> scoped symbolTree response",
        port: 851,
        requestCount: correlatedRequests.length,
        nodes: 60,
      };
      xtestReplaceText(portRootPoint.x, portRootPoint.y, "501");
      await sleep(350);
      const staleAfterTree = await adsDraftState(conn, webSid);
      assert.strictEqual(staleAfterTree.value, "501", "unbrowsed Motion port draft must remain visible over symbol results");
      assert.ok(staleAfterTree.staleVisible && !staleAfterTree.treeVisible && staleAfterTree.symbolSelectionCount === 0, "edited ADS port must hide the prior symbol tree");
      assert.ok(!staleAfterTree.errorVisible && !staleAfterTree.routeVisible, "edited ADS port must expose no stale error or route state over symbols");
      assert.ok(staleAfterTree.allowDisabled && staleAfterTree.addDisabled, "edited ADS port must keep write and Add tags disabled over symbol results");
      const staleTreeScreenshot = await screenshot(conn, pageSid, "ADSC-05-port-501-draft-invalidates-tree");
      xtestReplaceText(portRootPoint.x, portRootPoint.y, "851");
      await sleep(350);
      const restored851Tree = await adsDraftState(conn, webSid);
      assert.ok(restored851Tree.treeVisible && !restored851Tree.staleVisible, "restoring browsed port 851 must restore its scoped symbol tree");
      proof.adsPortSelection.draftInvalidation.tree = staleAfterTree;
      proof.adsPortSelection.draftInvalidation.treeScreenshot = staleTreeScreenshot.path;
      const expanded = await clickButton(conn, webSid, "Test group");
      assert.ok(expanded.clicked, "could not expand the deterministic ADS symbol group: " + JSON.stringify(expanded));
      await sleep(350);

      const probe = await evalInner(
        conn,
        webSid,
        "var panel=d.querySelector('aside[aria-label~=Browse]');" +
          "var inputs=panel&&[...panel.querySelectorAll('input[data-role=symbol-selection]')];" +
          "var cb=inputs&&inputs.find(function(input){return input.getAttribute('aria-label')==='Select Test group.TestSymbol30';});" +
          "if(!cb)return {error:'NO_SYMBOL_CHECKBOX',count:inputs&&inputs.length,text:panel&&(panel.textContent||'').slice(0,1000)};" +
          "cb.scrollIntoView({block:'center',inline:'nearest'});" +
          "w.__issue94Cb=cb;w.__issue94Events=[];w.__issue94Samples=[];w.__issue94GraphMessages=0;" +
          "w.__issue94GraphListener=function(event){if(event.data&&event.data.type==='graph')w.__issue94GraphMessages+=1;};" +
          "w.addEventListener('message',w.__issue94GraphListener);" +
          "['pointerdown','mousedown','focus','pointerup','mouseup','click','input','change','keydown','keypress','keyup'].forEach(function(type){d.addEventListener(type,function(event){if(event.target!==cb)return;w.__issue94Events.push({type:type,checked:Boolean(event.target.checked),defaultPrevented:event.defaultPrevented,key:event.key||'',trusted:event.isTrusted});},true);});" +
          "w.__issue94Timer=w.setInterval(function(){var current=[...panel.querySelectorAll('input[data-role=symbol-selection]')].find(function(input){return input.getAttribute('aria-label')==='Select Test group.TestSymbol30';});var rect=current&&current.getBoundingClientRect();w.__issue94Samples.push({same:current===w.__issue94Cb,connected:Boolean(current&&current.isConnected),checked:Boolean(current&&current.checked),left:rect&&Number(rect.left.toFixed(2)),top:rect&&Number(rect.top.toFixed(2)),width:rect&&Number(rect.width.toFixed(2)),height:rect&&Number(rect.height.toFixed(2))});},50);" +
          "var rect=cb.getBoundingClientRect();var frame=f.getBoundingClientRect();" +
          "return {checked:cb.checked,x:frame.left+rect.left+rect.width/2,y:frame.top+rect.top+rect.height/2,frame:{width:frame.width,height:frame.height},rect:{left:rect.left,top:rect.top,width:rect.width,height:rect.height}};"
      );
      assert.ok(probe && !probe.error, "could not install the native checkbox probe: " + JSON.stringify(probe));
      assert.strictEqual(probe.checked, false, "symbol checkbox should start unchecked");

      const geometryResponse = await conn.send(
        "Runtime.evaluate",
        {
          expression: "({screenX:window.screenX,screenY:window.screenY,iframes:[...document.querySelectorAll('iframe')].map(function(frame){var rect=frame.getBoundingClientRect();return {left:rect.left,top:rect.top,width:rect.width,height:rect.height};})})",
          returnByValue: true,
        },
        pageSid
      );
      const pageGeometry = geometryResponse && geometryResponse.result && geometryResponse.result.result && geometryResponse.result.result.value;
      assert.ok(pageGeometry && pageGeometry.iframes && pageGeometry.iframes.length, "expected VS Code iframe geometry");
      const outerFrame =
        pageGeometry.iframes.find(
          (frame) => Math.abs(frame.width - probe.frame.width) < 1 && Math.abs(frame.height - probe.frame.height) < 1
        ) || pageGeometry.iframes[0];
      const rootPoint = {
        x: pageGeometry.screenX + outerFrame.left + probe.x,
        y: pageGeometry.screenY + outerFrame.top + probe.y,
      };

      // The host refreshes graph data every 1.5 seconds. Keep sampling through at least two refreshes
      // and prove React preserves the exact checkbox node and hit rectangle across both of them.
      await sleep(5000);
      const lifecycle = await evalInner(
        conn,
        webSid,
        "var panel=d.querySelector('aside[aria-label~=Browse]');" +
          "var current=[...panel.querySelectorAll('input[data-role=symbol-selection]')].find(function(input){return input.getAttribute('aria-label')==='Select Test group.TestSymbol30';});" +
          "return {same:current===w.__issue94Cb,connected:Boolean(current&&current.isConnected),checked:Boolean(current&&current.checked),graphMessages:w.__issue94GraphMessages,samples:w.__issue94Samples.slice()};"
      );
      const rectSignatures = [...new Set(lifecycle.samples.map((sample) => [sample.left, sample.top, sample.width, sample.height].join(",")))];
      assert.ok(lifecycle.same && lifecycle.connected, "symbol checkbox must retain stable DOM identity");
      assert.strictEqual(lifecycle.checked, false, "polling must not change checkbox state");
      assert.ok(lifecycle.graphMessages >= 2, "expected at least two host graph refreshes, got " + lifecycle.graphMessages);
      assert.ok(lifecycle.samples.length >= 20, "expected checkbox lifecycle samples");
      assert.ok(lifecycle.samples.every((sample) => sample.same && sample.connected), "checkbox was replaced or disconnected during polling");
      assert.strictEqual(rectSignatures.length, 1, "checkbox hit rectangle moved during polling");

      xtestClick(rootPoint.x, rootPoint.y, 60);
      await sleep(250);
      const quickClick = await evalInner(
        conn,
        webSid,
        "var cb=w.__issue94Cb;return {checked:cb.checked,active:d.activeElement===cb,events:w.__issue94Events.splice(0)};"
      );
      const quickEventTypes = quickClick.events.map((event) => event.type);
      assert.ok(quickClick.checked && quickClick.active, "60 ms native click must check and focus the symbol checkbox");
      for (const type of ["pointerdown", "pointerup", "click", "input", "change"]) {
        assert.ok(quickEventTypes.includes(type), "60 ms native click did not emit " + type);
      }
      const quickNativeClick = quickClick.events.find((event) => event.type === "click");
      assert.ok(quickNativeClick && quickNativeClick.trusted, "symbol click must be a trusted native event");
      await screenshot(conn, pageSid, "ADSC-05-native-symbol-checkbox-quick-click");

      xtestSpace();
      await sleep(250);
      const keyboard = await evalInner(
        conn,
        webSid,
        "var cb=w.__issue94Cb;return {checked:cb.checked,active:d.activeElement===cb,events:w.__issue94Events.splice(0)};"
      );
      const keyboardEventTypes = keyboard.events.map((event) => event.type);
      assert.ok(!keyboard.checked && keyboard.active, "Space must toggle the focused native checkbox off");
      for (const type of ["keydown", "keyup", "click", "input", "change"]) {
        assert.ok(keyboardEventTypes.includes(type), "Space did not emit " + type);
      }
      const spaceKeydown = keyboard.events.find((event) => event.type === "keydown");
      const keyboardNativeClick = keyboard.events.find((event) => event.type === "click");
      assert.ok(spaceKeydown && !spaceKeydown.defaultPrevented, "Space keydown must retain native checkbox behavior");
      assert.ok(keyboardNativeClick && keyboardNativeClick.trusted, "Space must produce a trusted native checkbox click");

      await evalInner(
        conn,
        webSid,
        "w.clearInterval(w.__issue94Timer);w.removeEventListener('message',w.__issue94GraphListener);return {stopped:true};"
      );
      proof.nativeCheckboxInteraction = {
        input: "X11 XTest",
        target: "Test group.TestSymbol30",
        graphRefreshes: lifecycle.graphMessages,
        samples: lifecycle.samples.length,
        stableDomIdentity: lifecycle.same,
        stableRect: rectSignatures[0],
        quickClickHoldMs: 60,
        quickClickEventTypes: quickEventTypes,
        spaceEventTypes: keyboardEventTypes,
        rootPoint,
      };
      proof.steps.push({ step: "toggle ADS symbol checkbox with trusted 60 ms click and Space", result: proof.nativeCheckboxInteraction });

      if (process.env.TRUST_SKIP_WRITE_GUARD !== "1") {
      // #96: select a writable symbol and invoke the real Add tags path while the runtime is
      // stopped. The product must explain the write-ack requirement and offer Start runtime; this
      // runner dismisses the modal so no runtime lifecycle mutation enters the deterministic story.
      xtestClick(rootPoint.x, rootPoint.y, 60);
      await sleep(250);
      const writeSelection = await evalInner(
        conn,
        webSid,
        "var cb=w.__issue94Cb;return {checked:cb.checked,active:d.activeElement===cb};"
      );
      assert.ok(writeSelection.checked, "writable symbol must be selected before Add tags");
      const writeControls = await evalInner(
        conn,
        webSid,
        "var panel=d.querySelector('aside[aria-label~=Browse]');var allow=panel&&panel.querySelector('input[data-role=allow-writes]');var add=panel&&[...panel.querySelectorAll('button')].find(function(button){return /^Add tags/.test((button.textContent||'').trim());});if(!allow||!add)return {error:'NO_WRITE_CONTROLS',text:panel&&(panel.textContent||'').slice(0,1200)};function point(element){var rect=element.getBoundingClientRect();var frame=f.getBoundingClientRect();return {x:frame.left+rect.left+rect.width/2,y:frame.top+rect.top+rect.height/2};}return {allow:point(allow),add:point(add),allowChecked:allow.checked,allowDisabled:allow.disabled,addDisabled:add.disabled,addText:(add.textContent||'').trim()};"
      );
      assert.ok(writeControls && !writeControls.error, "write controls must be present: " + JSON.stringify(writeControls));
      assert.ok(!writeControls.allowDisabled && !writeControls.addDisabled, "write controls must be actionable with one selected symbol");
      const writeRootPoint = (point) => ({
        x: pageGeometry.screenX + outerFrame.left + point.x,
        y: pageGeometry.screenY + outerFrame.top + point.y,
      });
      const allowWritesRootPoint = writeRootPoint(writeControls.allow);
      const addTagsRootPoint = writeRootPoint(writeControls.add);
      xtestClick(allowWritesRootPoint.x, allowWritesRootPoint.y, 60);
      await sleep(250);
      const allowWritesState = await evalInner(
        conn,
        webSid,
        "var allow=d.querySelector('input[data-role=allow-writes]');var add=[...d.querySelectorAll('button')].find(function(button){return /^Add tags/.test((button.textContent||'').trim());});return {checked:allow&&allow.checked,addDisabled:add&&add.disabled,addText:add&&(add.textContent||'').trim()};"
      );
      assert.ok(allowWritesState.checked && !allowWritesState.addDisabled, "Allow writes must be checked before invoking Add tags");
      xtestClick(addTagsRootPoint.x, addTagsRootPoint.y, 60);
      const writeGuardPageText = await waitForPageText(
        conn,
        pageSid,
        "Write-enabled ADS imports need a running runtime.*explicit write acknowledgement.*Start runtime",
        "write-ack running-runtime modal"
      );
      assert.match(writeGuardPageText, /Write-enabled ADS imports need a running runtime/i);
      assert.match(writeGuardPageText, /truST must verify the explicit write acknowledgement before importing writable tags/i);
      assert.match(writeGuardPageText, /Start runtime/i);
      const writeGuardScreenshot = await screenshot(conn, pageSid, "ADSC-06-write-ack-runtime-required");
      await conn.send(
        "Input.dispatchKeyEvent",
        { type: "rawKeyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 },
        pageSid
      );
      await conn.send(
        "Input.dispatchKeyEvent",
        { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 },
        pageSid
      );
      let modalDismissed = false;
      let afterDismissText = "";
      for (let attempt = 0; attempt < 40; attempt += 1) {
        afterDismissText = await pageText(conn, pageSid);
        if (!/Write-enabled ADS imports need a running runtime/i.test(afterDismissText)) {
          modalDismissed = true;
          break;
        }
        await sleep(100);
      }
      assert.ok(modalDismissed, "Escape must cancel the write-ack modal");
      assert.match(afterDismissText, /Stopped|Simulator stopped/i, "canceling must leave the runtime stopped");
      proof.writeAckGuard = {
        input: "X11 XTest",
        symbolSelected: writeSelection.checked,
        allowWrites: allowWritesState.checked,
        addAction: allowWritesState.addText,
        reason: "truST must verify the explicit write acknowledgement before importing writable tags.",
        action: "Start runtime",
        canceled: modalDismissed,
        runtimeRemainedStopped: true,
        screenshot: writeGuardScreenshot.path,
      };
      proof.steps.push({ step: "show and cancel stopped-runtime writable ADS import guard", result: proof.writeAckGuard });
      }

      await vscode.commands.executeCommand("workbench.action.closeEditorsInOtherGroups").catch(() => undefined);
      const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(project, "src", "main.st")));
      await vscode.window.showTextDocument(doc, { preview: false });
      await sleep(1200);
      await screenshot(conn, pageSid, "ADSC-07-imported-ads-tags-in-st");

      const compile = await vscode.commands.executeCommand("trust-lsp.checkProgram");
      proof.compile = compile;
      await sleep(1200);
      await screenshot(conn, pageSid, "ADSC-07-compile-clean");
      assert.ok(compile && compile.ok, "ADS example must compile cleanly: " + JSON.stringify(compile));

      const adsToml = fs.readFileSync(path.join(project, "ads.toml"), "utf8");
      proof.adsToml = adsToml;
      assert.ok(/line1_temp/.test(adsToml) && /line1_setpoint/.test(adsToml), "ads.toml must contain imported tag variables");
      proof.verdict = "partial_provisional_lab_required";
      fs.writeFileSync(path.join(jsonDir, "ADSC-local-proof.json"), JSON.stringify(proof, null, 2) + "\\n");
    } finally {
      conn.close();
    }
  });
});
`);

fs.writeFileSync(path.join(testsDir, "run.js"), `const Mocha = require(${JSON.stringify(path.join(ext, "node_modules/mocha"))});\nconst path = require("path");\nexports.run = function () {\n  const mocha = new Mocha({ ui: "tdd", timeout: 220000 });\n  mocha.addFile(path.join(__dirname, "index.js"));\n  return new Promise((resolve, reject) => mocha.run((failures) => failures ? reject(new Error(failures + " test(s) failed")) : resolve()));\n};\n`);

function findCodeBin() {
  const configured = process.env.TRUST_VSCODE_TEST_EXECUTABLE;
  if (configured) {
    if (!fs.existsSync(configured)) {
      throw new Error(`Configured VS Code test executable does not exist: ${configured}`);
    }
    return configured;
  }
  const testRoot = path.join(ext, ".vscode-test");
  const codeDir = fs
    .readdirSync(testRoot)
    .filter((entry) => entry.startsWith("vscode-linux-"))
    .sort()
    .pop();
  if (!codeDir) {
    throw new Error(`No vscode-linux-* test build found under ${testRoot}`);
  }
  return path.join(testRoot, codeDir, "code");
}

runTests({
  vscodeExecutablePath: findCodeBin(),
  extensionDevelopmentPath: ext,
  extensionTestsPath: path.join(testsDir, "run.js"),
  launchArgs: [
    project,
    `--remote-debugging-port=${cdpPort}`,
    "--ozone-platform=x11",
    "--disable-gpu",
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--in-process-gpu",
    "--no-sandbox",
    "--user-data-dir",
    userDataDir,
    "--extensions-dir",
    extensionsDir,
    "--disable-workspace-trust",
    "--skip-welcome",
  ],
  extensionTestsEnv: {
    ST_LSP_TEST_SERVER: lspBin,
    ST_RUNTIME_TEST_BIN: runtimeProbeBin,
    TRUST_UX_JOURNEY: "J-17",
  },
}).then(
  () => {
    console.log("ADS_CLIENT_PROGRAM_DONE");
  },
  (error) => {
    console.error(error);
    process.exit(1);
  }
);
