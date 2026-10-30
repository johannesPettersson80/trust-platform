#!/usr/bin/env node
// Durable Phase 3 evidence runner for Control Studio (docs/internal/reviews/control-course-review-2026-09-03.md
// finding W-13: "Capture one real lesson (project-scripted) through Monitor -> Review test -> Confirm and
// start -> completion and keep the runner in the repo.").
//
// This is the outer launcher, structurally parallel to editors/vscode/src/test/runTest.ts: it builds the
// Rust binaries the extension needs, copies the reference lesson (Lesson 8, "08-water-heater-saturation",
// the one lesson in the accepted 1-14 band with a project-scripted experiment block) into a fresh temp
// workspace, and hands off to @vscode/test-electron's runTests() with suite.js (in this same directory) as
// extensionTestsPath.
//
// Must be run under Xvfb, e.g.:
//   xvfb-run -a -s "-screen 0 1920x1080x24" node run.js
//
// Required env:
//   TRUST_CONTROL_STUDIO_EVIDENCE_DIR - absolute output directory for screenshots-raw/, steps.json, etc.
// Optional env:
//   ST_LSP_TEST_SERVER - pre-built trust-lsp binary (skips building it)
//   TRUST_CONTROL_STUDIO_CDP_PORT - remote debugging port (default 9222)
//   TRUST_CONTROL_STUDIO_SKIP_COMPILE=1 - skip `npm run compile` (assumes out/ and media/*.js are current)

"use strict";

const path = require("path");
const fs = require("fs");
const os = require("os");
const { execFileSync } = require("child_process");
const { runTests } = require("@vscode/test-electron");

const EXTENSION_ROOT = path.resolve(__dirname, "..", "..");
const REPO_ROOT = path.resolve(EXTENSION_ROOT, "..", "..");
const LESSON_SOURCE = path.join(
  REPO_ROOT,
  "examples",
  "control",
  "01-feedback-foundations-and-temperature-control",
  "08-water-heater-saturation"
);

function cargoExecutable(repoRoot) {
  const fastLinkWrapper = path.join(repoRoot, "scripts", "cargo_test_fast_link.sh");
  if (process.platform === "linux" && fs.existsSync(fastLinkWrapper)) {
    return fastLinkWrapper;
  }
  return "cargo";
}

function buildRustPackage(repoRoot, packageName) {
  console.log(`[control-studio-evidence] cargo build -p ${packageName}`);
  execFileSync(cargoExecutable(repoRoot), ["build", "-p", packageName], {
    cwd: repoRoot,
    stdio: "inherit",
    timeout: 10 * 60 * 1000,
  });
}

function cargoTargetDir(repoRoot) {
  const configured = (process.env.CARGO_TARGET_DIR || "").trim();
  return configured ? path.resolve(repoRoot, configured) : path.join(repoRoot, "target");
}

function cargoDebugBinary(repoRoot, binaryName) {
  return path.join(cargoTargetDir(repoRoot), "debug", binaryName);
}

function gitSha(repoRoot) {
  try {
    return execFileSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8" }).trim();
  } catch (error) {
    return "";
  }
}

async function main() {
  const evidenceDir = (process.env.TRUST_CONTROL_STUDIO_EVIDENCE_DIR || "").trim();
  if (!evidenceDir) {
    throw new Error("TRUST_CONTROL_STUDIO_EVIDENCE_DIR must be set to an absolute output directory.");
  }
  const resolvedEvidenceDir = path.resolve(evidenceDir);
  fs.mkdirSync(resolvedEvidenceDir, { recursive: true });

  if (!fs.existsSync(LESSON_SOURCE)) {
    throw new Error(`Lesson source not found: ${LESSON_SOURCE}`);
  }

  if (process.env.TRUST_CONTROL_STUDIO_SKIP_COMPILE !== "1") {
    // Deliberately narrower than `npm run compile`: that script also rebuilds the Blockly, ladder,
    // SFC, statechart, network-canvas, and IO-panel webview bundles and the examples gallery media,
    // none of which this journey touches. Rebuild just the TypeScript extension host (out/) and the
    // Control Studio webview bundle (media/controlStudioChart.js, media/controlStudioWorkflow.js)
    // this suite depends on.
    // Mirrors the "clean:out" step of npm run compile so a stale out/ from a previous checkout
    // cannot leave a deleted .ts source's compiled .js behind.
    fs.rmSync(path.join(EXTENSION_ROOT, "out"), { recursive: true, force: true });
    console.log("[control-studio-evidence] tsc -p ./ (extension host)");
    execFileSync(path.join(EXTENSION_ROOT, "node_modules", ".bin", "tsc"), ["-p", "./"], {
      cwd: EXTENSION_ROOT,
      stdio: "inherit",
      timeout: 10 * 60 * 1000,
    });
    console.log("[control-studio-evidence] build:control-studio (webview bundle)");
    execFileSync("node", [path.join("scripts", "build-control-studio-webview.js")], {
      cwd: EXTENSION_ROOT,
      stdio: "inherit",
      timeout: 5 * 60 * 1000,
    });
  } else {
    console.log("[control-studio-evidence] skipping compile (TRUST_CONTROL_STUDIO_SKIP_COMPILE=1)");
  }

  const extensionDevelopmentPath = EXTENSION_ROOT;
  const extensionTestsPath = path.join(__dirname, "suite.js");

  const workspaceParent = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-control-studio-evidence-ws-")
  );
  const workspacePath = path.join(workspaceParent, "08-water-heater-saturation");
  fs.cpSync(LESSON_SOURCE, workspacePath, { recursive: true });
  console.log(`[control-studio-evidence] lesson workspace copied to ${workspacePath}`);

  const userDataDir = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-control-studio-evidence-user-data-")
  );
  const extensionsDir = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-control-studio-evidence-extensions-")
  );

  const defaultServerName = process.platform === "win32" ? "trust-lsp.exe" : "trust-lsp";
  const defaultServerPath = cargoDebugBinary(REPO_ROOT, defaultServerName);
  const configuredServer = (process.env.ST_LSP_TEST_SERVER || "").trim();
  const serverPath =
    configuredServer && fs.existsSync(configuredServer) ? configuredServer : defaultServerPath;
  const runtimeName = process.platform === "win32" ? "trust-runtime.exe" : "trust-runtime";
  const runtimePath = cargoDebugBinary(REPO_ROOT, runtimeName);
  const devName = process.platform === "win32" ? "trust-dev.exe" : "trust-dev";
  const devPath = cargoDebugBinary(REPO_ROOT, devName);

  if (!configuredServer) {
    buildRustPackage(REPO_ROOT, "trust-lsp");
  } else if (!fs.existsSync(serverPath)) {
    throw new Error(`ST_LSP_TEST_SERVER not found at ${serverPath}`);
  }
  buildRustPackage(REPO_ROOT, "trust-runtime");
  buildRustPackage(REPO_ROOT, "trust-dev");
  buildRustPackage(REPO_ROOT, "trust-debug");

  const cdpPort = (process.env.TRUST_CONTROL_STUDIO_CDP_PORT || "9222").trim();

  console.log(`[control-studio-evidence] evidence dir: ${resolvedEvidenceDir}`);
  console.log(`[control-studio-evidence] CDP port: ${cdpPort}`);

  await runTests({
    extensionDevelopmentPath,
    extensionTestsPath,
    launchArgs: [
      workspacePath,
      "--user-data-dir",
      userDataDir,
      "--extensions-dir",
      extensionsDir,
      "--ozone-platform=x11",
      "--disable-gpu",
      "--use-gl=angle",
      "--use-angle=swiftshader",
      "--in-process-gpu",
      "--no-sandbox",
      `--remote-debugging-port=${cdpPort}`,
    ],
    extensionTestsEnv: {
      ST_LSP_TEST_SERVER: serverPath,
      ST_RUNTIME_TEST_BIN: runtimePath,
      ST_DEV_TEST_BIN: devPath,
      TRUST_CONTROL_STUDIO_EVIDENCE_DIR: resolvedEvidenceDir,
      TRUST_CONTROL_STUDIO_SOURCE_COMMIT: gitSha(REPO_ROOT),
      TRUST_CONTROL_STUDIO_CDP_PORT: cdpPort,
    },
  });

  console.log("[control-studio-evidence] runTests() completed without a hard failure.");
}

main().catch((error) => {
  console.error("[control-studio-evidence] run.js failed");
  console.error(error);
  process.exit(1);
});
