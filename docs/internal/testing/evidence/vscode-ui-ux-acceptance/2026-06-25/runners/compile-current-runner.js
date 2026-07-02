#!/usr/bin/env node
// Current-sidebar Compile proof for RUN-01/RUN-02.
const fs = require("fs");
const path = require("path");
const cp = require("child_process");
const pngHygienePath = path.join(__dirname, "png-hygiene.js");
const { runTests } = require(path.join(
  "/home/johannes/projects/trust-platform",
  "editors/vscode/node_modules/@vscode/test-electron"
));

const repo = process.env.TRUST_REPO || "/home/johannes/projects/trust-platform";
const ext = path.join(repo, "editors/vscode");
const evidenceRoot =
  process.env.TRUST_UX_EVIDENCE_ROOT ||
  path.join(repo, "docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-29/J-03-write-compile-fix-st");
const screenshotsDir =
  process.env.TRUST_UX_SCREENSHOTS_DIR || path.join(evidenceRoot, "screenshots-raw");
const jsonDir = process.env.TRUST_UX_JSON_DIR || path.join(evidenceRoot, "json");
const outRoot = path.join(evidenceRoot, "runner-output", "compile-current");
const testsDir = path.join(outRoot, "tests");
const project = path.join(outRoot, "project");
const userDataRoot = path.join(outRoot, "user-data");
const extensionsRoot = path.join(outRoot, "extensions");
const lspBin = process.env.ST_LSP_TEST_SERVER || path.join(repo, "target/debug/trust-lsp");
const runtimeBin = process.env.ST_RUNTIME_TEST_BIN || path.join(repo, "target/debug/trust-runtime");

fs.rmSync(outRoot, { recursive: true, force: true });
for (const dir of [screenshotsDir, jsonDir, testsDir, path.join(project, "src"), userDataRoot, extensionsRoot]) {
  fs.mkdirSync(dir, { recursive: true });
}

fs.writeFileSync(
  path.join(project, "trust-lsp.toml"),
  '[project]\nname = "compile-current"\nentry = "src/config.st"\ninclude_paths = ["src"]\n'
);
fs.writeFileSync(
  path.join(project, "runtime.toml"),
  [
    "[bundle]",
    "version = 1",
    "[resource]",
    'name = "compile-current"',
    "cycle_interval_ms = 100",
    "",
    "[runtime.control]",
    'endpoint = "unix:///tmp/trust-compile-current.sock"',
    'mode = "production"',
    "debug_enabled = false",
    "",
    "[runtime.web]",
    "enabled = false",
    'listen = "127.0.0.1:8080"',
    'auth = "local"',
    "tls = false",
    "",
    "[runtime.tls]",
    'mode = "disabled"',
    "require_remote = false",
    "",
    "[runtime.discovery]",
    "enabled = false",
    'service_name = "truST"',
    "advertise = false",
    "interfaces = []",
    "",
    "[runtime.mesh]",
    "enabled = false",
    'listen = "0.0.0.0:5200"',
    "tls = false",
    'auth_token = ""',
    "publish = []",
    "",
    "[runtime.observability]",
    "enabled = false",
    "sample_interval_ms = 1000",
    'mode = "all"',
    "include = []",
    'history_path = "history/h.jsonl"',
    "max_entries = 20000",
    "prometheus_enabled = false",
    'prometheus_path = "/metrics"',
    "",
    "[runtime.log]",
    'level = "info"',
    "",
    "[runtime.retain]",
    'mode = "none"',
    "save_interval_ms = 1000",
    "",
    "[runtime.watchdog]",
    "enabled = false",
    "timeout_ms = 1000",
    'action = "halt"',
    "",
    "[runtime.fault]",
    'policy = "halt"',
    "",
  ].join("\n")
);
fs.writeFileSync(
  path.join(project, "io.toml"),
  '[io]\ndriver = "simulated"\nparams = {}\n'
);
fs.writeFileSync(
  path.join(project, "src", "Main.st"),
  [
    "PROGRAM Main",
    "VAR",
    "    counter : INT := 0;",
    "END_VAR",
    "counter := counter + 1;",
    "END_PROGRAM",
    "",
  ].join("\n")
);
fs.writeFileSync(
  path.join(project, "src", "config.st"),
  [
    "CONFIGURATION Config",
    "RESOURCE Res ON PLC",
    "    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);",
    "    PROGRAM App WITH MainTask : Main;",
    "END_RESOURCE",
    "END_CONFIGURATION",
    "",
  ].join("\n")
);

function writeSettings(dir) {
  fs.mkdirSync(path.join(dir, "User"), { recursive: true });
  fs.writeFileSync(
    path.join(dir, "User", "settings.json"),
    JSON.stringify(
      {
        "window.commandCenter": false,
        "chat.commandCenter.enabled": false,
        "workbench.layoutControl.enabled": false,
        "window.menuBarVisibility": "hidden",
        "window.titleBarStyle": "native",
        "workbench.startupEditor": "none",
        "workbench.tips.enabled": false,
        "telemetry.telemetryLevel": "off",
        "update.mode": "none",
        "extensions.ignoreRecommendations": true,
        "git.enabled": false,
        "git.openRepositoryInParentFolders": "never",
        "workbench.colorTheme": "Default Dark Modern",
      },
      null,
      2
    )
  );
}

writeSettings(userDataRoot);

fs.writeFileSync(
  path.join(testsDir, "index.js"),
  `
const assert = require("assert");
const fs = require("fs");
const path = require("path");
const cp = require("child_process");
const vscode = require("vscode");
const pngHygiene = require(${JSON.stringify(pngHygienePath)});

const project = ${JSON.stringify(project)};
const screenshotsDir = ${JSON.stringify(screenshotsDir)};
const jsonDir = ${JSON.stringify(jsonDir)};
const proofPath = path.join(jsonDir, "compile-current-proof.json");
const proof = {
  journey: process.env.TRUST_UX_JOURNEY || "J-03",
  rows: ["RUN-01", "RUN-02"],
  project,
  steps: [],
};

function sleep(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }
function shot(name) {
  const raw = path.join(${JSON.stringify(outRoot)}, name + ".raw.png");
  const dest = path.join(screenshotsDir, name + ".png");
  const env = Object.assign({}, process.env, { PATH: "/usr/bin:/bin:" + (process.env.PATH || "") });
  cp.execFileSync("/usr/bin/import", ["-window", "root", raw], { stdio: "ignore", env });
  pngHygiene.stripPngFile(raw);
  try {
    cp.execFileSync("/usr/bin/convert", [raw, "-strip", "-bordercolor", "black", "-border", "1", "-trim", "+repage", dest], { stdio: "ignore", env });
  } catch (_) {
    fs.copyFileSync(raw, dest);
  }
  pngHygiene.stripPngFile(dest);
  proof.steps.push({ screenshot: path.relative(${JSON.stringify(evidenceRoot)}, dest) });
  fs.writeFileSync(proofPath, JSON.stringify(proof, null, 2));
}
async function activate() {
  const ext = vscode.extensions.getExtension("trust-platform.trust-lsp");
  assert.ok(ext, "trust-platform.trust-lsp extension is available");
  await ext.activate();
  await sleep(1500);
}
async function openHome() {
  await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar").catch(() => undefined);
  await vscode.commands.executeCommand("workbench.action.closePanel").catch(() => undefined);
  await vscode.commands.executeCommand("workbench.view.extension.trust");
  await vscode.commands.executeCommand("trust.home.focus").catch(() => undefined);
  await sleep(1800);
}
function diagnostics(uri) {
  return vscode.languages.getDiagnostics(uri).map((d) => ({
    severity: d.severity,
    message: d.message,
    line: d.range.start.line,
    character: d.range.start.character,
    source: d.source,
  }));
}
async function waitForDiagnostics(uri, predicate, label) {
  const start = Date.now();
  let value = [];
  while (Date.now() - start < 30000) {
    value = diagnostics(uri);
    if (predicate(value)) return value;
    await sleep(500);
  }
  throw new Error("Timed out waiting for diagnostics " + label + ": " + JSON.stringify(value));
}

suite("current compile runner", function () {
  this.timeout(180000);
  test("captures Compile pass and failure from current sidebar", async function () {
    await activate();
    const uri = vscode.Uri.file(path.join(project, "src", "Main.st"));
    const doc = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(doc, { preview: false });
    await openHome();

    await vscode.commands.executeCommand("trust-lsp.checkProgram");
    await sleep(1800);
    const clean = await waitForDiagnostics(uri, (items) => items.length === 0, "clean");
    proof.steps.push({ workflow: "RUN-01", diagnostics: clean });
    await vscode.commands.executeCommand("notifications.clearAll");
    await sleep(300);
    shot("RUN-01-compile-current-passed");

    const line = doc.getText().split("\\n").findIndex((text) => text.includes("counter := counter + 1;"));
    assert.ok(line >= 0, "expected counter assignment");
    await editor.edit((edit) => edit.replace(doc.lineAt(line).range, "counter := counter + ;"));
    await doc.save();
    await sleep(2200);
    await vscode.commands.executeCommand("trust-lsp.checkProgram");
    await sleep(1800);
    const failed = await waitForDiagnostics(uri, (items) => items.length > 0, "failed");
    proof.steps.push({ workflow: "RUN-02", diagnostics: failed });
    await vscode.commands.executeCommand("workbench.actions.view.problems");
    await sleep(1000);
    await vscode.commands.executeCommand("notifications.clearAll");
    await sleep(300);
    shot("RUN-02-compile-current-failed-problems");
    fs.writeFileSync(proofPath, JSON.stringify(proof, null, 2));
  });
});
`
);

fs.writeFileSync(
  path.join(testsDir, "run.js"),
  `const Mocha=require(${JSON.stringify(path.join(ext, "node_modules/mocha"))});const path=require("path");exports.run=function(){const m=new Mocha({ui:"tdd",timeout:180000});m.addFile(path.join(__dirname,"index.js"));return new Promise((res,rej)=>m.run(f=>f?rej(new Error(f+" fail")):res()));};`
);

async function main() {
  await runTests({
    version: "1.126.0",
    extensionDevelopmentPath: ext,
    extensionTestsPath: path.join(testsDir, "run.js"),
    launchArgs: [
      project,
      "--ozone-platform=x11",
      "--disable-gpu",
      "--use-gl=angle",
      "--use-angle=swiftshader",
      "--in-process-gpu",
      "--no-sandbox",
      "--user-data-dir=" + userDataRoot,
      "--extensions-dir=" + extensionsRoot,
      "--disable-workspace-trust",
      "--skip-welcome",
      "--new-window",
    ],
    extensionTestsEnv: {
      ST_LSP_TEST_SERVER: lspBin,
      ST_RUNTIME_TEST_BIN: runtimeBin,
    },
  });
  console.log("COMPILE_CURRENT_DONE");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
