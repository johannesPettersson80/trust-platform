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

function cargoTargetDir(repoRoot: string): string {
  const configured = process.env.CARGO_TARGET_DIR?.trim();
  return configured ? path.resolve(repoRoot, configured) : path.join(repoRoot, "target");
}

function cargoDebugBinary(repoRoot: string, binaryName: string): string {
  return path.join(cargoTargetDir(repoRoot), "debug", binaryName);
}

function removeTestTempDirectory(directory: string): void {
  const tempRoot = `${path.resolve(os.tmpdir())}${path.sep}`;
  const resolved = path.resolve(directory);
  if (!resolved.startsWith(tempRoot)) {
    throw new Error(`Refusing to remove non-temporary test directory: ${resolved}`);
  }
  fs.rmSync(resolved, { recursive: true, force: true });
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
  const defaultServerPath = cargoDebugBinary(repoRoot, defaultServerName);
  const configured = process.env.ST_LSP_TEST_SERVER?.trim();
  const serverPath =
    configured && fs.existsSync(configured) ? configured : defaultServerPath;
  const runtimeName =
    process.platform === "win32" ? "trust-runtime.exe" : "trust-runtime";
  const runtimePath = cargoDebugBinary(repoRoot, runtimeName);
  const devName = process.platform === "win32" ? "trust-dev.exe" : "trust-dev";
  const devPath = cargoDebugBinary(repoRoot, devName);

  if (!configured) {
    buildRustPackage(repoRoot, "trust-lsp");
  } else if (!fs.existsSync(serverPath)) {
    throw new Error(`ST_LSP_TEST_SERVER not found at ${serverPath}`);
  }

  buildRustPackage(repoRoot, "trust-runtime");
  buildRustPackage(repoRoot, "trust-dev");
  buildRustPackage(repoRoot, "trust-debug");

  const extensionTestsEnv: NodeJS.ProcessEnv = {
    ST_LSP_TEST_SERVER: serverPath,
    ST_RUNTIME_TEST_BIN: runtimePath,
    ST_DEV_TEST_BIN: devPath,
  };
  const configuredTestGrep = process.env.TRUST_VSCODE_TEST_GREP?.trim();
  if (configuredTestGrep) {
    extensionTestsEnv.TRUST_VSCODE_TEST_GREP = configuredTestGrep;
  }
  for (const key of [
    "TRUST_VSCODE_TEST_EVIDENCE",
    "TRUST_VSCODE_TEST_SOURCE_COMMIT",
    "TRUST_PLANT_RENDERED_THEME",
    "TRUST_PLANT_RENDERED_SPLIT",
    "TRUST_PLANT_RENDERED_SCREENSHOT",
    "TRUST_PLANT_RENDERED_CAPTURE_SECTION",
    "TRUST_PLANT_RENDERED_CAPTURE_MODEL",
  ]) {
    const value = process.env[key]?.trim();
    if (value) {
      extensionTestsEnv[key] = value;
    }
  }

  try {
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
      extensionTestsEnv,
    });
  } finally {
    removeTestTempDirectory(workspacePath);
    removeTestTempDirectory(userDataDir);
    removeTestTempDirectory(extensionsDir);
  }

  const fixture = (name: string) => path.join(extensionDevelopmentPath, "src", "test", "fixtures", name);
  // Journeys that wait for their own switch and need their own project: the test file names the switch it waits for.
  const gatedJourneys: Record<string, { fixture?: string; env: Record<string, string> }> = {
    "Control Studio isolated anti-windup actual-ST workflow runs matched untracked and tracked saturation-return records against the same ST source":
      { fixture: fixture("control-studio-anti-windup"), env: { TRUST_CONTROL_STUDIO_ACTUAL_ANTI_WINDUP: "1" } },
    "Control Studio automatic requirements UI prefills an older saved model that has no tuning task": { env: { TRUST_AUTOMATIC_REQUIREMENTS_LEGACY: "1" } },
    "Control Studio automatic requirements UI identifies, tunes without a form, retains editable defaults and validates their frozen criteria":
      { env: { TRUST_AUTOMATIC_REQUIREMENTS: "1" } },
    "Control Studio connected PID engineer path exposes all typed PID gains through the actual ST runtime":
      { fixture: fixture("control-studio-pid"), env: { TRUST_CONTROL_STUDIO_ACTUAL_PID: "1" } },
    "Control Studio connected live identification continues plotting current runtime samples after Reset": { env: { TRUST_IDENTIFY_LIVE: "1" } },
    "Control Studio connected live identification shows the connected plant evolving during Identify instead of returning a completed isolated trial": { env: { TRUST_IDENTIFY_LIVE: "1" } },
    "Control Studio connected live identification closing a live experiment restores the plant before reopening": { env: { TRUST_IDENTIFY_LIVE: "1" } },
    // The process rests mid-range, so each method's excitation stays inside the output range.
    "Control Studio identification continuity runs each selected method in the same study and reacquires after reopening":
      { fixture: fixture("control-studio-continuity"), env: { TRUST_IDENTIFY_CONTINUOUS: "1" } },
    "Control Studio single identification experiment one Identify produces a provisional model and preserves its measured experiment": { env: { TRUST_IDENTIFY_SINGLE: "1" } },
    "Control Studio single identification experiment Validate collects independent model evidence and checks the tuned controller after an explicit click":
      { env: { TRUST_IDENTIFY_SINGLE_VALIDATE: "1" } },
    "Control Studio PID group setup rendered sets up two interacting Control.PID instances as one group and opens it in Control Studio":
      { fixture: fixture("control-studio-two-zone-group"), env: {} },
    "Control Studio ON/OFF setup rendered sets up the program's own switching logic and opens it in Control Studio":
      { fixture: path.join(repoRoot, "examples", "control-onoff-commissioning"), env: {} },
    "Control Studio state-feedback setup rendered sets up the program's own state-feedback law and opens it in Control Studio":
      { fixture: path.join(repoRoot, "examples", "control-state-feedback-commissioning"), env: {} },
    "Control Studio own P or PI law setup rendered sets up the program's own PI and opens it in Control Studio":
      { fixture: fixture("control-studio-own-pi"), env: {} },
    "Control Studio three-page workflow executes the project-owned sequence and retains its restored native recording":
      { fixture: path.join(repoRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "01-cold-storage-loop-anatomy"),
        env: { TRUST_CONTROL_STUDIO_PROJECT_NATIVE: "1" } },
  };
  const renderedTestFullTitles = [
    "Rendered example open recovery CPEX-009 retries the same copied destination from the real notification",
    "Rendered retained lesson projects CPEX-012 selected lesson detail offers Open existing project",
    "Rendered plant editor CPFIX-003 rendered full candidate preservation",
    "Rendered plant editor CPFIX-006 rendered robot optional defaults preserve blank parameters",
    "Rendered plant editor CPFIX-006 rendered omitted initial state remains editable and omission-preserving",
    "Control Studio three-page workflow opens recorded data from its visible button and stops for model review without starting a runtime",
    "Rendered example filters CPEX-005 compact filters combine with search and reset empty results",
    "Rendered plant editor CPUI-017/019 rendered optional task destinations",
    "Rendered plant editor CPUI-004 all model routes",
    "Rendered plant editor CPUI-019 scenario seconds preserve exact backend milliseconds",
    "Rendered plant editor CPUI-019 scenario events use named state and sensor quantities",
    "Rendered plant editor CPUI-017/018 focused effect cards preserve order and applicable controls",
    "Rendered starter execution badges CPEX-005 shows simulator execution independently from optional hardware",
    "Rendered example focus CPEX-005 preserves focus through search, filters, and reset",
    "Rendered unavailable examples CPEX-013 keeps valid starters usable and disables damaged lesson creation",
    "Rendered examples gallery CPEX-007 full selected lesson detail",
    "Rendered plant editor CPFIX-005 rendered plant invalid input guard",
    "Rendered plant editor CPUI-005 rendered FOPDT temperature example",
    "Rendered plant editor CPFIX-008 rendered actuator blank defaults",
    "Rendered plant editor CPFIX-004 rendered robot joint multiport bindings",
    "Rendered plant editor CPFIX-004 rendered state-space multiport preservation",
    "Rendered plant editor CPUI-006 rendered model starting conditions",
    "Control Studio three-page native journey identifies tunes validates applies and restores the real library PID from the visible controls",
    "Control Studio three-page PID group rendered review shows group members in controller settings and channel mapping in the real editor",
    "Control Studio native PID group tunes validates applies and restores two actual library PIDs without changing an unselected PID",
    "Control Studio three-page workflow starts identification from its visible button with the actual Control.PID plant",
    "Control Studio three-page workflow edits manual settings in the existing result area and preserves full values and pins when changing method",
    "Control Studio three-page recovery rendered projection shows Identify cancellation recovery while keeping page navigation usable",
    "Control Studio three-page recovery rendered projection restores the same retry action after a generic recovery failure",
    "Control Studio three-page recovery rendered projection clears recovery locks on terminal resolution and does not add a second button for normal Apply review",
    "Control Studio three-page binding selection edits per-output requirements and per-channel limits in the rendered MIMO workspace",
    "Control Studio loop setup form rendered sets up the project's Control.PID from compiled facts and opens it in Control Studio",
    "Control Studio Identify initial readings shows actual selected plant signals before identification and marks stopped readings unavailable",
    "Control Studio Tune channel mapping renders Channels… only for MIMO Tune targets and exposes exact declared signals",
    "Control Studio approved three-page theme review captures every approved page and compact editor at all release themes",
    "Control Studio conditional setpoint identification shows the reference selector only when Setpoint step needs a configured signal",
    "Control Studio course tools in three pages calculates and restores exact Routh regions from the visible parameter field",
    "Control Studio course tools in three pages calculates channel frequency and declared loop plots using the selected model",
    "Control Studio course tools in three pages calculates multichannel state structure and RGA directly without a frequency detour",
    "Control Studio identification method applicability renders a specific Relay prerequisite failure without starting runtime",
    "Control Studio periodic analysis imports a retained recording, calculates host-owned FRF, and reopens it",
    "Control Studio shared signal multi-select rendered renders immediate signal dropdowns and remains usable at narrow width",
    "Control Studio three-page binding selection lists the actual compiled PID instances and opens an explicitly selected target with a fresh scope",
    "Control Studio three-page workflow asks for the missing experiment duration before live identification and leaves the field blank",
    "Control Studio three-page workflow calculates response, pole views and lead-lag designs through the visible model tools",
    "Control Studio three-page workflow edits experiment limits in one compact dialog and cancels without changing saved values",
    "Control Studio three-page workflow edits missing control requirements without inventing targets",
    "Control Studio three-page workflow fits a manually selected model and checks it against an independent recording in Identify",
    "Control Studio three-page workflow keeps the validation response and Apply visible with many required checks and exposes failures first",
    "Control Studio three-page workflow offers actual archived output channels to the opened-recording selector without starting a runtime",
    "Control Studio three-page workflow offers the declared project experiment and plots selected recorded signals in their own units",
    "Control Studio three-page workflow opens a supplied plant model through the selected method and retains it for tuning after reopening",
    "Control Studio three-page workflow plots measured and predicted identification responses with their units",
    "Control Studio three-page workflow renders state-design policy and preserves finite design drafts across reopen",
    "Control Studio three-page workflow renders the identified plant equation from scoped host state",
    "Control Studio three-page workflow runs the declared root locus from Tune without a candidate or runtime write",
    "Control Studio three-page workflow shows Identify Tune Validate with page-specific controls and no runtime chrome",
    "Control Studio three-page workflow shows retained checks and an inconclusive row for every unfinished mandatory scenario",
    "Control Studio three-page workflow shows tuned settings and every mandatory validation requirement on their pages",
    "Control Studio three-page workflow starts guarded application and reports rejection without claiming adoption",
    "Rendered cascaded gravity tank editor offers coupled tank dynamics with two level states and feedback ports",
    ...Object.keys(gatedJourneys),
  ];
  for (const renderedTestFullTitle of renderedTestFullTitles) {
    if (configuredTestGrep && !new RegExp(configuredTestGrep).test(renderedTestFullTitle)) continue;
    const renderedRunDir = fs.mkdtempSync(
      path.join(os.tmpdir(), "trust-lsp-vscode-plant-rendered-")
    );
    const renderedWorkspacePath = path.join(renderedRunDir, "workspace");
    const renderedUserDataDir = path.join(renderedRunDir, "user-data");
    const renderedExtensionsDir = path.join(renderedRunDir, "extensions");
    try {
      const nativePidGroup = renderedTestFullTitle.startsWith("Control Studio native PID group");
      const studioCase = nativePidGroup || renderedTestFullTitle.startsWith("Control Studio ");
      const gated = gatedJourneys[renderedTestFullTitle];
      const renderedFixture = gated?.fixture ? gated.fixture : nativePidGroup
        ? path.join(extensionDevelopmentPath, "src", "test", "fixtures", "control-studio-pid-group")
        : studioCase
        ? path.join(repoRoot, "examples", "control-pid-commissioning")
        : path.join(extensionDevelopmentPath, "src", "test", "fixtures",
          renderedTestFullTitle.includes("CPFIX-003 rendered full candidate") ? "plant-rendered-full-preservation"
          : renderedTestFullTitle.includes("CPFIX-006 rendered omitted") ? "plant-rendered-omitted-initial-state"
          : renderedTestFullTitle.startsWith("Rendered Control Studio ") ? "control-studio-recording-gap"
          : "plant-rendered-invalid-input");
      fs.cpSync(
        renderedFixture,
        renderedWorkspacePath,
        { recursive: true }
      );
      if (studioCase) {
        const settingsFile = path.join(renderedWorkspacePath, ".vscode", "settings.json");
        fs.mkdirSync(path.dirname(settingsFile), { recursive: true });
        const settings = fs.existsSync(settingsFile) ? JSON.parse(fs.readFileSync(settingsFile, "utf8")) : {};
        fs.writeFileSync(settingsFile, JSON.stringify({ ...settings, "trust.runtime.mode": "simulate",
          "trust.runtime.executablePath": runtimePath, "trust.languageServer.executablePath": serverPath,
          "trust.debugAdapter.executablePath": cargoDebugBinary(repoRoot, process.platform === "win32" ? "trust-debug.exe" : "trust-debug"),
        }));
      }
      const renderedEvidenceRoot = extensionTestsEnv.TRUST_VSCODE_TEST_EVIDENCE
        ? path.join(path.dirname(extensionTestsEnv.TRUST_VSCODE_TEST_EVIDENCE), "rendered",
          renderedTestFullTitle.replace(/[^a-z0-9]+/gi, "-").replace(/^-|-$/g, "").toLowerCase())
        : renderedRunDir;
      fs.mkdirSync(renderedEvidenceRoot, { recursive: true });
      const renderedScreenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT?.trim()
        || path.join(renderedEvidenceRoot, `${renderedTestFullTitle.replace(/[^a-z0-9]+/gi, "-").replace(/^-|-$/g, "").toLowerCase()}.png`);

      await runTests({
        extensionDevelopmentPath,
        extensionTestsPath,
        launchArgs: [
          renderedWorkspacePath,
          ...(studioCase ? ["--window-size=1600,1000"] : []),
          "--user-data-dir",
          renderedUserDataDir,
          "--extensions-dir",
          renderedExtensionsDir,
          "--remote-debugging-port=0",
          "--disable-gpu",
          "--no-sandbox",
          "--disable-workspace-trust",
          "--skip-welcome",
        ],
        extensionTestsEnv: {
          ...extensionTestsEnv,
          ...(extensionTestsEnv.TRUST_VSCODE_TEST_EVIDENCE
            ? {
                TRUST_VSCODE_TEST_EVIDENCE: `${extensionTestsEnv.TRUST_VSCODE_TEST_EVIDENCE}.plant-rendered.${renderedTestFullTitle
                  .replace(/[^a-z0-9]+/gi, "-")
                  .replace(/^-|-$/g, "")
                  .toLowerCase()}.json`,
              }
            : {}),
          TRUST_PLANT_RENDERED_TEST: "1",
          ...(studioCase ? { TRUST_CONTROL_STUDIO_THREE_PAGE: "1" } : {}),
          ...(nativePidGroup ? { TRUST_CONTROL_STUDIO_PID_GROUP_NATIVE: "1" } : {}),
          ...(renderedTestFullTitle.startsWith("Control Studio three-page native journey") ? { TRUST_CONTROL_STUDIO_THREE_PAGE_NATIVE: "1" } : {}),
          ...(gated?.env ?? {}),
          TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT: renderedScreenshot,
          ...(process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME ? { TRUST_CONTROL_STUDIO_RENDERED_THEME: process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME } : {}),
          ...(process.env.TRUST_CONTROL_STUDIO_RENDERED_SPLIT ? { TRUST_CONTROL_STUDIO_RENDERED_SPLIT: process.env.TRUST_CONTROL_STUDIO_RENDERED_SPLIT } : {}),
          TRUST_PLANT_RENDERED_USER_DATA_DIR: renderedUserDataDir,
          TRUST_VSCODE_TEST_GREP: `^${renderedTestFullTitle}$`,
        },
      });
    } finally {
      removeTestTempDirectory(renderedRunDir);
    }
  }
}

main().catch((error) => {
  console.error("Failed to run VS Code extension tests");
  console.error(error);
  process.exit(1);
});
