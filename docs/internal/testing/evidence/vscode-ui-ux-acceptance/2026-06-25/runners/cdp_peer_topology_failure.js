const path = require("path");
const fs = require("fs");
const cp = require("child_process");

const repo = process.env.TRUST_REPO || cp.execFileSync("git", ["rev-parse", "--show-toplevel"], {
  encoding: "utf8",
}).trim();
const ext = path.join(repo, "editors/vscode");
const { runTests } = require(path.join(ext, "node_modules/@vscode/test-electron"));
const WebSocketPath = path.join(ext, "node_modules/ws");
const port = 9381;
const base = process.env.TRUST_CAPTURE_TMP || "/tmp/trust-peer-topology-capture";
const outDir = path.join(base, "out");
const testsDir = path.join(base, "tests");
const screenshot = process.env.TRUST_CAPTURE_SCREENSHOT || path.join(base, "peer-topology-failure.png");
const sourceRevision = cp.execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: repo,
  encoding: "utf8",
}).trim();
const runnerSha256 = require("crypto").createHash("sha256").update(fs.readFileSync(__filename)).digest("hex");
const extensionVersion = JSON.parse(fs.readFileSync(path.join(ext, "package.json"), "utf8")).version;
const lspBinary = process.env.ST_LSP_TEST_SERVER || path.join(repo, "target/debug/trust-lsp");
if (!fs.existsSync(lspBinary)) {
  throw new Error(`ST_LSP_TEST_SERVER does not exist: ${lspBinary}`);
}
const lspBinarySha256 = require("crypto").createHash("sha256").update(fs.readFileSync(lspBinary)).digest("hex");
const themes = {
  dark: "Default Dark Modern",
  light: "Default Light Modern",
  high_contrast: "Default High Contrast",
};
const themeId = process.env.TRUST_CAPTURE_THEME || "dark";
const colorTheme = themes[themeId];
if (!colorTheme) {
  throw new Error(`TRUST_CAPTURE_THEME must be one of ${Object.keys(themes).join(", ")}`);
}

fs.rmSync(base, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });
fs.mkdirSync(testsDir, { recursive: true });
fs.mkdirSync(path.dirname(screenshot), { recursive: true });
fs.mkdirSync(path.join(outDir, "ud", "User"), { recursive: true });
fs.writeFileSync(
  path.join(outDir, "ud", "User", "settings.json"),
  JSON.stringify({
    "window.titleBarStyle": "native",
    "window.commandCenter": false,
    "workbench.layoutControl.enabled": false,
    "workbench.startupEditor": "none",
    "telemetry.telemetryLevel": "off",
    "update.mode": "none",
    "git.enabled": false,
    "workbench.colorTheme": colorTheme,
  })
);

const vscodeRoot = path.join(ext, ".vscode-test");
const codeDir = fs
  .readdirSync(vscodeRoot)
  .filter((entry) => entry.startsWith("vscode-linux-"))
  .sort()
  .pop();
if (!codeDir) {
  throw new Error("no cached VS Code test installation found");
}
const codeBin = path.join(vscodeRoot, codeDir, "code");

fs.writeFileSync(
  path.join(testsDir, "index.js"),
  `
const fs = require("fs");
const http = require("http");
const cp = require("child_process");
const vscode = require("vscode");
const WebSocket = require(${JSON.stringify(WebSocketPath)});
const PORT = ${port};
const screenshot = ${JSON.stringify(screenshot)};
const themeId = ${JSON.stringify(themeId)};
const provenance = ${JSON.stringify({ sourceRevision, runnerSha256, extensionVersion, lspBinary, lspBinarySha256 })};
function sleep(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }
function httpJson(urlPath) {
  return new Promise((resolve, reject) => {
    const request = http.get("http://localhost:" + PORT + urlPath, (response) => {
      let body = "";
      response.on("data", (chunk) => { body += chunk; });
      response.on("end", () => {
        try { resolve(JSON.parse(body)); } catch (error) { reject(error); }
      });
    });
    request.on("error", reject);
  });
}
function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    let nextId = 0;
    const pending = new Map();
    ws.on("message", (data) => {
      const message = JSON.parse(data.toString());
      if (message.id && pending.has(message.id)) {
        pending.get(message.id)(message);
        pending.delete(message.id);
      }
    });
    ws.on("error", reject);
    ws.on("open", () => resolve({
      send(method, params, sessionId) {
        return new Promise((finish) => {
          const id = ++nextId;
          pending.set(id, finish);
          ws.send(JSON.stringify({ id, method, params: params || {}, sessionId }));
        });
      },
      close() { ws.close(); },
    }));
  });
}
suite("peer-topology-visible-failure", function () {
  this.timeout(120000);
  test("keeps the local canvas and renders malformed peer status visibly", async function () {
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    if (extension) await extension.activate();
    await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
    await sleep(10000);
    const version = await httpJson("/json/version");
    const targets = await httpJson("/json");
    const page = targets.find((target) => target.type === "page");
    const webview = targets.find((target) => target.type === "iframe" && /index\\.html/.test(target.url || ""));
    if (!page || !webview) throw new Error("Devices & Connections webview target not found");
    const connection = await connect(version.webSocketDebuggerUrl);
    const windowInfo = await connection.send("Browser.getWindowForTarget", { targetId: page.id });
    if (windowInfo.result && windowInfo.result.windowId) {
      await connection.send("Browser.setWindowBounds", {
        windowId: windowInfo.result.windowId,
        bounds: { left: 0, top: 0, width: 1920, height: 1080, windowState: "normal" },
      });
    }
    const attached = await connection.send("Target.attachToTarget", { targetId: webview.id, flatten: true });
    const sessionId = attached.result.sessionId;
    await connection.send("Runtime.enable", {}, sessionId);
    const graph = {
      kind: "graph",
      title: "Devices & Connections",
      summary: "1 host · 1 runtime · Simulator stopped",
      banner: {
        kind: "error",
        text: "Peer topology degraded: peer-a connector status: unknown connector confidence: certainly_healthy",
        actions: [],
      },
      hosts: [{
        id: "host:this-computer",
        hostname: "This computer",
        label: "Simulator",
        health: "connected",
        containers: [],
        runtimes: [{
          id: "runtime:local",
          name: "Simulator",
          mode: "simulate",
          health: "stopped",
          detail: "Stopped — start the simulator to run it.",
          endpoints: [],
        }],
      }],
      links: [],
      external: [],
      faults: [],
    };
    if (process.env.TRUST_CAPTURE_HOST_FIXTURE !== "1") {
      const expression =
        "(function(){var f=document.querySelector('iframe');" +
        "if(!f||!f.contentWindow)return 'NO_INNER_FRAME';" +
        "var m=" + JSON.stringify(JSON.stringify({ type: "graph", graph })) + ";" +
        "clearInterval(window.__trustPeerTopologyFixture);" +
        "window.__trustPeerTopologyFixture=setInterval(function(){" +
        "f.contentWindow.postMessage(JSON.parse(m), '*');},50);" +
        "f.contentWindow.postMessage(JSON.parse(m), '*');return 'INJECTED';})()";
      const injected = await connection.send("Runtime.evaluate", { expression, returnByValue: true }, sessionId);
      const injectedValue = injected.result && injected.result.result && injected.result.result.value;
      if (injectedValue !== "INJECTED") throw new Error("graph injection failed: " + JSON.stringify(injected));
      await sleep(600);
    }
    const inspected = await connection.send("Runtime.evaluate", {
      expression: "(function(){var f=document.querySelector('iframe');var d=f&&f.contentDocument;return d&&d.body?d.body.innerText:'';})()",
      returnByValue: true,
    }, sessionId);
    const text = String(inspected.result && inspected.result.result && inspected.result.result.value || "");
    const required = ["Peer topology degraded", "unknown connector confidence", "Simulator stopped"];
    const missing = required.filter((fragment) => !text.includes(fragment));
    if (missing.length) throw new Error("rendered webview omitted required text: " + missing.join(", ") + "\\n" + text);
    await connection.send("Runtime.evaluate", {
      expression: "clearInterval(window.__trustPeerTopologyFixture);delete window.__trustPeerTopologyFixture;",
    }, sessionId);
    cp.execFileSync("/usr/bin/import", ["-window", "root", screenshot]);
    fs.writeFileSync(screenshot + ".json", JSON.stringify({ ...provenance, theme: themeId, required, rendered_text: text }, null, 2));
    connection.close();
  });
});
`
);
fs.writeFileSync(
  path.join(testsDir, "run.js"),
  `const Mocha=require(${JSON.stringify(path.join(ext, "node_modules/mocha"))});` +
    `const path=require("path");exports.run=function(){const m=new Mocha({ui:"tdd",timeout:120000});` +
    `m.addFile(path.join(__dirname,"index.js"));return new Promise((resolve,reject)=>m.run((failures)=>failures?reject(new Error(failures+" failure(s)")):resolve()));};`
);

runTests({
  vscodeExecutablePath: codeBin,
  extensionDevelopmentPath: ext,
  extensionTestsPath: path.join(testsDir, "run.js"),
  launchArgs: [
    path.join(repo, "examples/network_canvas_demo"),
    `--remote-debugging-port=${port}`,
    "--ozone-platform=x11",
    "--disable-gpu",
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--in-process-gpu",
    "--no-sandbox",
    `--user-data-dir=${path.join(outDir, "ud")}`,
    `--extensions-dir=${path.join(outDir, "ed")}`,
    "--disable-workspace-trust",
    "--skip-welcome",
  ],
  extensionTestsEnv: {
    ST_LSP_TEST_SERVER:
      process.env.ST_LSP_TEST_SERVER || path.join(repo, "target/debug/trust-lsp"),
    TRUST_NETWORK_CANVAS_CAPTURE_PEER_FAILURE:
      process.env.TRUST_NETWORK_CANVAS_CAPTURE_PEER_FAILURE || "",
    TRUST_CAPTURE_HOST_FIXTURE: process.env.TRUST_CAPTURE_HOST_FIXTURE || "",
  },
})
  .then(() => console.log(`PEER_TOPOLOGY_CAPTURE=${screenshot}`))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
