import * as path from "path";
import * as fs from "fs";
import * as os from "os";
import { execFileSync } from "child_process";
import { runTests } from "@vscode/test-electron";

function cargoExecutable(repoRoot: string): string {
  const fastLinkWrapper = path.join(repoRoot, "scripts", "cargo_test_fast_link.sh");
  if (process.platform === "linux" && fs.existsSync(fastLinkWrapper)) {
    return fastLinkWrapper;
  }
  return "cargo";
}

function buildRustPackage(repoRoot: string, packageName: string): void {
  execFileSync(cargoExecutable(repoRoot), ["build", "-p", packageName], {
    cwd: repoRoot,
    stdio: "inherit",
  });
}

async function main(): Promise<void> {
  const extensionDevelopmentPath = path.resolve(__dirname, "../../");
  const extensionTestsPath = path.resolve(__dirname, "./suite/index");
  const repoRoot = path.resolve(extensionDevelopmentPath, "..", "..");
  const workspacePath = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-lsp-vscode-workspace-")
  );
  const userDataDir = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-lsp-vscode-user-data-")
  );
  const extensionsDir = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-lsp-vscode-extensions-")
  );

  const defaultServerName =
    process.platform === "win32" ? "trust-lsp.exe" : "trust-lsp";
  const defaultServerPath = path.join(
    repoRoot,
    "target",
    "debug",
    defaultServerName
  );
  const configured = process.env.ST_LSP_TEST_SERVER?.trim();
  const serverPath =
    configured && fs.existsSync(configured) ? configured : defaultServerPath;
  const runtimeName =
    process.platform === "win32" ? "trust-runtime.exe" : "trust-runtime";
  const runtimePath = path.join(repoRoot, "target", "debug", runtimeName);

  if (!configured) {
    buildRustPackage(repoRoot, "trust-lsp");
  } else if (!fs.existsSync(serverPath)) {
    throw new Error(`ST_LSP_TEST_SERVER not found at ${serverPath}`);
  }

  buildRustPackage(repoRoot, "trust-runtime");
  buildRustPackage(repoRoot, "trust-debug");

  await runTests({
    extensionDevelopmentPath,
    extensionTestsPath,
    launchArgs: [
      workspacePath,
      "--user-data-dir",
      userDataDir,
      "--extensions-dir",
      extensionsDir,
    ],
    extensionTestsEnv: {
      ST_LSP_TEST_SERVER: serverPath,
      ST_RUNTIME_TEST_BIN: runtimePath,
    },
  });
}

main().catch((error) => {
  console.error("Failed to run VS Code extension tests");
  console.error(error);
  process.exit(1);
});
