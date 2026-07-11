#!/usr/bin/env node

// Launches the real truST extension in an Extension Development Host. The
// companion fixture only supplies deterministic ADS wire outcomes; all clicks,
// rendering, state handling, themes, and screenshots come from the real UI.

const crypto = require("crypto");
const cp = require("child_process");
const fs = require("fs");
const path = require("path");
const { runTests } = require(path.join(
  process.env.TRUST_REPO || "/home/johannes/projects/trust-platform-ads-windows-fix",
  "editors/vscode/node_modules/@vscode/test-electron"
));

const repo = path.resolve(
  process.env.TRUST_REPO || "/home/johannes/projects/trust-platform-ads-windows-fix"
);
const ext = path.join(repo, "editors/vscode");
const evidenceRoot = path.resolve(__dirname, "..");
const runnerOutput = path.join(evidenceRoot, "runner-output");
const userDataDir = path.join(runnerOutput, "user-data");
const extensionsDir = path.join(runnerOutput, "extensions");
const project = path.join(runnerOutput, "project");
const stateFile = path.join(evidenceRoot, "fixture-state.json");
const transcript = path.join(evidenceRoot, "logs", "fixture-transcript.jsonl");
const visualDiagnostics = path.join(
  evidenceRoot,
  "json",
  "ads-windows-visual-diagnostics.json"
);
const visualRunMetadata = path.join(evidenceRoot, "json", "run-metadata.json");
const runtimeFixture = path.join(
  evidenceRoot,
  "fixtures",
  "trust-runtime-ads-ui-fixture.py"
);
const realRuntime = requiredCandidateBinary("TRUST_REAL_RUNTIME");
const lspBin = requiredCandidateBinary("ST_LSP_TEST_SERVER");
const debugBin = requiredCandidateBinary("ST_DEBUG_TEST_BIN");
const bundle = path.join(ext, "media", "networkCanvasWebview.js");
const hostExtension = path.join(ext, "out", "extension.js");
const hostNetworkCanvasPanel = path.join(
  ext,
  "out",
  "networkCanvas",
  "networkCanvasPanel.js"
);
const hostDiscoveryOriginContext = path.join(
  ext,
  "out",
  "networkCanvas",
  "discoveryOriginContext.js"
);
const adsServiceProbeControllerSource = path.join(
  ext,
  "src",
  "networkCanvas",
  "adsServiceProbeController.ts"
);
const adsServiceProbeControllerOut = path.join(
  ext,
  "out",
  "networkCanvas",
  "adsServiceProbeController.js"
);
const cdpPort = Number(process.env.TRUST_ADS_VISUAL_CDP_PORT || 19971);

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function requiredCandidateBinary(name) {
  const configured = process.env[name];
  if (!configured) {
    throw new Error(
      `${name} is required and must point to the final candidate binary; the visual gate never falls back to the original checkout.`
    );
  }
  const resolved = path.resolve(configured);
  const oldCheckout = "/home/johannes/projects/trust-platform";
  if (resolved === oldCheckout || resolved.startsWith(`${oldCheckout}${path.sep}`)) {
    throw new Error(
      `${name} points into the original checkout (${resolved}); use a binary built from the final candidate isolation.`
    );
  }
  return resolved;
}

function binaryVersion(file) {
  const result = cp.spawnSync(file, ["--version"], {
    encoding: "utf8",
    timeout: 10000,
    maxBuffer: 1024 * 1024,
  });
  return {
    status: result.status,
    signal: result.signal,
    stdout: String(result.stdout || "").trim(),
    stderr: String(result.stderr || "").trim(),
    error: result.error ? String(result.error.message || result.error) : undefined,
  };
}

function jsTreeFingerprint(root) {
  const files = [];
  const pending = [root];
  while (pending.length > 0) {
    const directory = pending.pop();
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        pending.push(file);
      } else if (entry.isFile() && entry.name.endsWith(".js")) {
        files.push(file);
      }
    }
  }
  files.sort();
  const digest = crypto.createHash("sha256");
  let bytes = 0;
  for (const file of files) {
    const relative = path.relative(root, file);
    const content = fs.readFileSync(file);
    bytes += content.length;
    digest.update(relative);
    digest.update("\0");
    digest.update(crypto.createHash("sha256").update(content).digest("hex"));
    digest.update("\n");
  }
  return { sha256: digest.digest("hex"), file_count: files.length, bytes };
}

function gitText(args) {
  const result = cp.spawnSync("git", args, {
    cwd: repo,
    encoding: "utf8",
    timeout: 10000,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.status !== 0) {
    return `ERROR(${result.status}): ${String(result.stderr || result.stdout).trim()}`;
  }
  return String(result.stdout || "").trim();
}

function requireFile(file, label) {
  if (!fs.existsSync(file)) {
    throw new Error(`${label} does not exist: ${file}`);
  }
}

function findCodeBin() {
  if (process.env.TRUST_VSCODE_TEST_EXECUTABLE) {
    return path.resolve(process.env.TRUST_VSCODE_TEST_EXECUTABLE);
  }
  const roots = [
    path.join(ext, ".vscode-test"),
    "/home/johannes/projects/trust-platform/editors/vscode/.vscode-test",
  ];
  for (const root of roots) {
    if (!fs.existsSync(root)) continue;
    const found = fs
      .readdirSync(root)
      .filter((entry) => entry.startsWith("vscode-linux-"))
      .sort()
      .pop();
    if (found) return path.join(root, found, "code");
  }
  throw new Error("No cached vscode-linux-* Extension Development Host was found.");
}

async function main() {
  for (const [file, label] of [
    [bundle, "compiled network canvas bundle"],
    [hostExtension, "compiled extension host entrypoint"],
    [hostNetworkCanvasPanel, "compiled Network Canvas host panel"],
    [hostDiscoveryOriginContext, "compiled discovery-origin host context"],
    [adsServiceProbeControllerSource, "ADS service probe controller source"],
    [adsServiceProbeControllerOut, "compiled ADS service probe controller"],
    [runtimeFixture, "ADS visual fixture"],
    [realRuntime, "real trust-runtime"],
    [lspBin, "trust-lsp"],
    [debugBin, "trust-debug"],
  ]) {
    requireFile(file, label);
  }

  fs.rmSync(runnerOutput, { recursive: true, force: true });
  fs.rmSync(path.join(evidenceRoot, "screenshots"), {
    recursive: true,
    force: true,
  });
  // Visual recapture owns only these two JSON files. Real Windows/TwinCAT
  // device-in-loop artifacts share this evidence directory and must survive.
  fs.rmSync(visualDiagnostics, { force: true });
  fs.rmSync(visualRunMetadata, { force: true });
  fs.mkdirSync(path.join(userDataDir, "User"), { recursive: true });
  fs.mkdirSync(extensionsDir, { recursive: true });
  fs.mkdirSync(path.join(evidenceRoot, "screenshots"), { recursive: true });
  fs.mkdirSync(path.join(evidenceRoot, "json"), { recursive: true });
  fs.mkdirSync(path.join(evidenceRoot, "logs"), { recursive: true });
  fs.writeFileSync(transcript, "");
  fs.writeFileSync(stateFile, JSON.stringify({ state: "sole_runtime" }, null, 2));

  const sourceProject = path.join(repo, "examples", "network_canvas_demo");
  requireFile(path.join(sourceProject, "runtime.toml"), "network canvas demo");
  fs.cpSync(sourceProject, project, { recursive: true });

  fs.writeFileSync(
    path.join(userDataDir, "User", "settings.json"),
    JSON.stringify(
      {
        "window.titleBarStyle": "native",
        "window.commandCenter": false,
        "chat.commandCenter.enabled": false,
        "workbench.layoutControl.enabled": false,
        "window.menuBarVisibility": "hidden",
        "workbench.startupEditor": "none",
        "workbench.tips.enabled": false,
        "telemetry.telemetryLevel": "off",
        "update.mode": "none",
        "extensions.ignoreRecommendations": true,
        "git.enabled": false,
        "git.openRepositoryInParentFolders": "never",
        "workbench.colorTheme": "Default Dark Modern",
        "trust-lsp.runtime.cli.path": runtimeFixture,
        "trust-lsp.server.path": lspBin,
        "trust-lsp.debug.adapter.path": debugBin,
      },
      null,
      2
    ) + "\n"
  );

  const codeBin = findCodeBin();
  requireFile(codeBin, "VS Code test executable");
  const runMetadata = {
    generated_at: new Date().toISOString(),
    fixture_kind: "deterministic ADS UI fixture; not hardware proof",
    repo,
    extension_version: require(path.join(ext, "package.json")).version,
    network_canvas_bundle: path.relative(repo, bundle),
    network_canvas_bundle_sha256: sha256(bundle),
    network_canvas_bundle_mtime: fs.statSync(bundle).mtime.toISOString(),
    extension_host_entrypoint: path.relative(repo, hostExtension),
    extension_host_entrypoint_sha256: sha256(hostExtension),
    extension_host_entrypoint_mtime: fs.statSync(hostExtension).mtime.toISOString(),
    extension_host_out_js_tree: jsTreeFingerprint(path.join(ext, "out")),
    network_canvas_host_out_js_tree: jsTreeFingerprint(
      path.join(ext, "out", "networkCanvas")
    ),
    network_canvas_host_panel_sha256: sha256(hostNetworkCanvasPanel),
    network_canvas_host_panel_mtime: fs
      .statSync(hostNetworkCanvasPanel)
      .mtime.toISOString(),
    discovery_origin_context_sha256: sha256(hostDiscoveryOriginContext),
    discovery_origin_context_mtime: fs
      .statSync(hostDiscoveryOriginContext)
      .mtime.toISOString(),
    ads_service_probe_controller_source: path.relative(
      repo,
      adsServiceProbeControllerSource
    ),
    ads_service_probe_controller_source_sha256: sha256(
      adsServiceProbeControllerSource
    ),
    ads_service_probe_controller_out: path.relative(
      repo,
      adsServiceProbeControllerOut
    ),
    ads_service_probe_controller_out_sha256: sha256(
      adsServiceProbeControllerOut
    ),
    candidate_git_head: gitText(["rev-parse", "HEAD"]),
    candidate_git_status: gitText(["status", "--short"]),
    vscode_executable: codeBin,
    vscode_executable_sha256: sha256(codeBin),
    runtime_fixture: path.relative(evidenceRoot, runtimeFixture),
    runtime_fixture_sha256: sha256(runtimeFixture),
    real_runtime: realRuntime,
    real_runtime_sha256: sha256(realRuntime),
    real_runtime_version: binaryVersion(realRuntime),
    trust_lsp: lspBin,
    trust_lsp_sha256: sha256(lspBin),
    trust_lsp_version: binaryVersion(lspBin),
    trust_debug: debugBin,
    trust_debug_sha256: sha256(debugBin),
    trust_debug_version: binaryVersion(debugBin),
  };
  fs.writeFileSync(
    visualRunMetadata,
    JSON.stringify(runMetadata, null, 2) + "\n"
  );

  try {
    await runTests({
      vscodeExecutablePath: codeBin,
      extensionDevelopmentPath: ext,
      extensionTestsPath: path.join(__dirname, "ads-windows-extension-test.js"),
      launchArgs: [
        project,
        `--remote-debugging-port=${cdpPort}`,
        "--window-size=1920,1080",
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
        TRUST_REPO: repo,
        ST_LSP_TEST_SERVER: lspBin,
        ST_DEBUG_TEST_BIN: debugBin,
        ST_RUNTIME_TEST_BIN: runtimeFixture,
        TRUST_ADS_UI_FIXTURE_STATE: stateFile,
        TRUST_ADS_UI_FIXTURE_TRANSCRIPT: transcript,
        TRUST_REAL_RUNTIME: realRuntime,
        TRUST_ADS_VISUAL_EVIDENCE_ROOT: evidenceRoot,
        TRUST_ADS_VISUAL_CDP_PORT: String(cdpPort),
        TRUST_ADS_VISUAL_STRICT: process.env.TRUST_ADS_VISUAL_STRICT || "1",
        TRUST_PNG_HYGIENE:
          process.env.TRUST_PNG_HYGIENE ||
          "/home/johannes/projects/trust-platform/docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/png-hygiene.js",
      },
    });
  } finally {
    // The curated evidence is screenshots/JSON/logs. Never retain the disposable
    // VS Code profile, extensions directory, or copied demo workspace.
    fs.rmSync(runnerOutput, { recursive: true, force: true });
  }
  process.stdout.write(
    `ADS_WINDOWS_VISUAL_ACCEPTANCE_DONE ${path.join(
      evidenceRoot,
      "json",
      "ads-windows-visual-diagnostics.json"
    )}\n`
  );
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
