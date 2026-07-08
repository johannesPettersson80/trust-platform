#!/usr/bin/env node

// J-17 / ADSC-02..07 (local proof): open the ADS client example, inspect the
// configured ADS endpoint, browse tags until the honest missing-route recovery
// appears, and prove the imported ADS symbols compile in ST.
//
// Live TwinCAT discovery/browse/value proof remains lab-gated and is recorded in
// the journey report rather than faked by this runner.
const fs = require("fs");
const http = require("http");
const path = require("path");

const repo = process.env.TRUST_REPO || process.env.TRUST_PLATFORM_REPO_ROOT || "/home/johannes/projects/trust-platform";
const ext = path.join(repo, "editors/vscode");
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
const runtimeBin = process.env.ST_RUNTIME_TEST_BIN || path.join(repo, "target/debug/trust-runtime");
const lspBin = process.env.ST_LSP_TEST_SERVER || path.join(repo, "target/debug/trust-lsp");
const cdpPort = Number(process.env.TRUST_ADS_CLIENT_CDP_PORT || 19947);

fs.rmSync(outRoot, { recursive: true, force: true });
fs.mkdirSync(testsDir, { recursive: true });
fs.mkdirSync(path.join(userDataDir, "User"), { recursive: true });
fs.mkdirSync(screenshotsDir, { recursive: true });
fs.mkdirSync(jsonDir, { recursive: true });
fs.cpSync(path.join(repo, "examples/communication/ads_line1"), project, { recursive: true });

const runtimeToml = path.join(project, "runtime.toml");
fs.writeFileSync(
  runtimeToml,
  fs.readFileSync(runtimeToml, "utf8").replace(/name = "CommAdsRes"/, 'name = "ADS line 1"')
);

fs.writeFileSync(path.join(userDataDir, "User", "settings.json"), `${JSON.stringify({
  "workbench.colorTheme": "Default Dark Modern",
  "git.openRepositoryInParentFolders": "never",
  "git.autoRepositoryDetection": false,
  "chat.commandCenter.enabled": false,
  "window.commandCenter": false,
  "workbench.layoutControl.enabled": false,
  "workbench.startupEditor": "none",
  "telemetry.telemetryLevel": "off",
  "update.mode": "none",
  "trust-lsp.runtime.cli.path": runtimeBin,
}, null, 2)}\n`);

fs.writeFileSync(path.join(testsDir, "index.js"), `
const assert = require("assert");
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
const cdpPort = ${cdpPort};
const proof = {
  journey: "J-17",
  workflow: "Browse ADS tags and use them in a program",
  project,
  rows_proven: ["ADSC-02", "ADSC-03", "ADSC-05", "ADSC-06", "ADSC-07", "IOMAP-02", "ST-02", "RUN-01"],
  rows_not_fully_proven: ["ADSC-01", "ADSC-04", "RUN-04", "LV-02", "ERR-12"],
  steps: [],
};

function sleep(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }
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
      const browseText = await waitForText(conn, webSid, "No ADS route|StaticRoutes|Manual TwinCAT route steps|Download PowerShell", "ADS route recovery", 65000);
      proof.routeRecoveryText = browseText.slice(0, 2500);
      await screenshot(conn, pageSid, "ADSC-02-route-missing-recovery");
      const createRoute = await clickButton(conn, webSid, "Create route");
      proof.steps.push({ step: "click Create route", result: createRoute });
      assert.ok(createRoute.clicked, "could not click Create route: " + JSON.stringify(createRoute));
      const createRouteText = await waitForText(conn, webSid, "Automatic route creation|Administrator|TwinCAT computer", "ADS create route result");
      proof.createRouteText = createRouteText.slice(0, 2500);
      await screenshot(conn, pageSid, "ADSC-03-create-route-admin-needed");

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
    ST_RUNTIME_TEST_BIN: runtimeBin,
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
