#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const repo = path.resolve(
  process.env.TRUST_REPO ||
    "/home/johannes/projects/trust-platform-ads-windows-fix"
);
const ext = path.join(repo, "editors/vscode");
const evidenceRoot = path.resolve(__dirname, "..");
const evidencePath = path.join(
  evidenceRoot,
  "json",
  "real-twincat-service-probe-safety.json"
);
const runnerOutput = path.join(evidenceRoot, "real-safety-runner-output");
const { runTests } = require(path.join(
  ext,
  "node_modules/@vscode/test-electron"
));

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
  throw new Error("No cached VS Code Extension Development Host was found.");
}

async function main() {
  const controlSocket =
    process.env.TRUST_REAL_ADS_CONTROL_SOCKET ||
    "/tmp/trust-runtime-ads-safety-reader.sock";
  if (!fs.existsSync(controlSocket)) {
    throw new Error(`The live ADS reader control socket is missing: ${controlSocket}`);
  }
  const codeBin = findCodeBin();
  const candidateRuntime = path.resolve(
    process.env.TRUST_REAL_RUNTIME ||
      path.join(repo, "target/ads-visual-candidate/trust-runtime")
  );
  if (!fs.existsSync(candidateRuntime)) {
    throw new Error(`The final candidate runtime is missing: ${candidateRuntime}`);
  }
  const workspace = path.join(runnerOutput, "workspace");
  const userData = path.join(runnerOutput, "user-data");
  const extensions = path.join(runnerOutput, "extensions");
  fs.rmSync(runnerOutput, { recursive: true, force: true });
  fs.rmSync(evidencePath, { force: true });
  fs.mkdirSync(workspace, { recursive: true });
  fs.mkdirSync(userData, { recursive: true });
  fs.mkdirSync(extensions, { recursive: true });
  try {
    await runTests({
      vscodeExecutablePath: codeBin,
      extensionDevelopmentPath: ext,
      extensionTestsPath: path.join(
        __dirname,
        "real-twincat-safety-extension-test.js"
      ),
      launchArgs: [
        workspace,
        "--disable-gpu",
        "--no-sandbox",
        "--disable-workspace-trust",
        "--skip-welcome",
        "--user-data-dir",
        userData,
        "--extensions-dir",
        extensions,
      ],
      extensionTestsEnv: {
        TRUST_REPO: repo,
        TRUST_REAL_ADS_CONTROL_SOCKET: controlSocket,
        TRUST_REAL_ADS_SAFETY_EVIDENCE: evidencePath,
        TRUST_REAL_RUNTIME: candidateRuntime,
      },
    });
  } finally {
    fs.rmSync(runnerOutput, { recursive: true, force: true });
  }
  const evidence = JSON.parse(fs.readFileSync(evidencePath, "utf8"));
  if (evidence.status !== "passed") {
    throw new Error("Real TwinCAT safety evidence did not pass.");
  }
  process.stdout.write(`REAL_TWINCAT_SAFETY_ACCEPTANCE_DONE ${evidencePath}\n`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
