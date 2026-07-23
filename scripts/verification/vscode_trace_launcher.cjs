"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");

async function main() {
  if (process.argv.length !== 8) {
    throw new Error(
      "usage: node vscode_trace_launcher.cjs <repo-root> <suite> <selection> <attempts-out> <coverage-root> <target-dir>"
    );
  }
  const root = path.resolve(process.argv[2]);
  const extensionRoot = path.join(root, "editors", "vscode");
  const { runTests } = require(
    path.join(extensionRoot, "node_modules", "@vscode", "test-electron")
  );
  const suite = path.resolve(process.argv[3]);
  const selection = path.resolve(process.argv[4]);
  const attemptsOut = path.resolve(process.argv[5]);
  const coverageRoot = path.resolve(process.argv[6]);
  const targetDir = path.resolve(process.argv[7]);
  const workspace = fs.mkdtempSync(path.join(os.tmpdir(), "trust-catalog-workspace-"));
  const userData = fs.mkdtempSync(path.join(os.tmpdir(), "trust-catalog-user-data-"));
  const extensions = fs.mkdtempSync(path.join(os.tmpdir(), "trust-catalog-extensions-"));
  await runTests({
    extensionDevelopmentPath: extensionRoot,
    extensionTestsPath: suite,
    launchArgs: [
      workspace,
      "--user-data-dir",
      userData,
      "--extensions-dir",
      extensions,
    ],
    extensionTestsEnv: {
      ...process.env,
      CARGO_TARGET_DIR: targetDir,
      NODE_V8_COVERAGE: coverageRoot,
      ST_LSP_TEST_SERVER: binary(targetDir, "trust-lsp"),
      ST_RUNTIME_TEST_BIN: binary(targetDir, "trust-runtime"),
      ST_DEV_TEST_BIN: binary(targetDir, "trust-dev"),
      TRUST_CATALOG_REPO_ROOT: root,
      TRUST_CATALOG_SELECTION: selection,
      TRUST_CATALOG_ATTEMPTS_OUT: attemptsOut,
    },
  });
}

function binary(targetDir, name) {
  const suffix = process.platform === "win32" ? ".exe" : "";
  const candidate = path.join(targetDir, "debug", `${name}${suffix}`);
  if (!fs.existsSync(candidate)) {
    throw new Error(`required VS Code trace binary is missing: ${candidate}`);
  }
  return candidate;
}

main().catch((error) => {
  console.error(`VS Code trace launch failed: ${error.stack || error}`);
  process.exit(1);
});
