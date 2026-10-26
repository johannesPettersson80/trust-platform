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

function buildRustPackages(repoRoot: string, packages: string[]): void {
  execFileSync(cargoExecutable(repoRoot), ["build", "--locked", ...packages.flatMap(name => ["-p", name])], {
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
  // Control Studio's engine keeps its operation journals in truST's state folder, $XDG_STATE_HOME/trust on Linux (specification
  // 35, A8.3), where the command line finds them too. Each launch gets its own, so that an operation one launch leaves journaled
  // (for example a journey that closes the panel while its experiment runs) never refuses another launch's run, even while shards
  // run side by side.
  const stateDir = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-lsp-vscode-state-")
  );

  const defaultServerName =
    process.platform === "win32" ? "trust-lsp.exe" : "trust-lsp";
  const defaultServerPath = cargoDebugBinary(repoRoot, defaultServerName);
  const configured = process.env.ST_LSP_TEST_SERVER?.trim();
  const serverPath =
    configured || defaultServerPath;
  const runtimeName =
    process.platform === "win32" ? "trust-runtime.exe" : "trust-runtime";
  const runtimePath = cargoDebugBinary(repoRoot, runtimeName);
  const devName = process.platform === "win32" ? "trust-dev.exe" : "trust-dev";
  const devPath = cargoDebugBinary(repoRoot, devName);

  const debugPath = cargoDebugBinary(repoRoot, process.platform === "win32" ? "trust-debug.exe" : "trust-debug");
  const enginePath = cargoDebugBinary(repoRoot, process.platform === "win32" ? "trust-control-studio.exe" : "trust-control-studio");
  if (process.env.TRUST_VSCODE_REUSE_NATIVE_BINARIES === "1") {
    // Explicit caller-owned reuse after a frozen native build. Never silently fall back to a
    // rebuild, or proceed with a missing binary from a different validation copy.
    for (const binary of [serverPath, runtimePath, devPath, debugPath, enginePath]) {
      if (!fs.existsSync(binary) || !fs.statSync(binary).isFile()) throw new Error(`Prebuilt test binary not found: ${binary}`);
    }
  } else {
    if (configured && !fs.existsSync(serverPath)) throw new Error(`ST_LSP_TEST_SERVER not found at ${serverPath}`);
    buildRustPackages(repoRoot, [...(!configured ? ["trust-lsp"] : []), "trust-runtime", "trust-dev", "trust-debug", "trust-control-studio"]);
  }

  const extensionTestsEnv: NodeJS.ProcessEnv = {
    XDG_STATE_HOME: stateDir,
    ST_LSP_TEST_SERVER: serverPath,
    ST_RUNTIME_TEST_BIN: runtimePath,
    ST_DEV_TEST_BIN: devPath,
    ST_CONTROL_STUDIO_TEST_BIN: enginePath,
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

  // A saved workspace lets integration suites add/remove isolated fixture roots without
  // triggering the interactive conversion from a single-folder window to a workspace.
  const workspaceFile = path.join(userDataDir, "suite.code-workspace");
  fs.mkdirSync(userDataDir, { recursive: true });
  fs.writeFileSync(workspaceFile, JSON.stringify({ folders: [{ path: workspacePath }] }));
  // Generated fixtures live on /tmp, where Linux trash may be unavailable. Keep Undo
  // deterministic in this disposable test profile without changing the user's settings.
  fs.mkdirSync(path.join(userDataDir, "User"), { recursive: true });
  fs.writeFileSync(path.join(userDataDir, "User", "settings.json"), JSON.stringify({ "files.enableTrash": false }));
  try {
    await runTests({
      version: process.env.TRUST_VSCODE_TEST_VERSION,
      extensionDevelopmentPath,
      extensionTestsPath,
      launchArgs: [
        workspaceFile,
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
    removeTestTempDirectory(stateDir);
  }

  const fixture = (name: string) => path.join(extensionDevelopmentPath, "src", "test", "fixtures", name);
  // Journeys that wait for their own switch and need their own project: the test file names the switch it waits for.
  const gatedJourneys: Record<string, { fixture?: string; env: Record<string, string> }> = {
    "HMI agent proposal rendered explores existing navigation inferred faceplates and alarm equipment without runtime access":
      { fixture: path.join(repoRoot, "examples", "hmi_equipment"), env: { TRUST_HMI_EXPLORATION_RENDERED: "1" } },
    "HMI agent proposal rendered reviews HMI themes and high contrast and applies through the visible builder action":
      { fixture: path.join(repoRoot, "examples", "hmi_tank_level"), env: { TRUST_HMI_PROPOSAL_RENDERED: "1" } },
    "Control Studio automatic requirements UI prefills an older saved model that has no tuning task": { env: { TRUST_AUTOMATIC_REQUIREMENTS_LEGACY: "1" } },
    "Control Studio automatic requirements UI identifies, tunes without a form, retains editable defaults and validates their frozen criteria":
      { env: { TRUST_AUTOMATIC_REQUIREMENTS: "1" } },
    "Control Studio connected PID engineer path exposes all typed PID gains through the actual ST runtime":
      { fixture: fixture("control-studio-pid"), env: { TRUST_CONTROL_STUDIO_ACTUAL_PID: "1" } },
    "Control Studio connected live identification continues plotting current runtime samples after Reset": { env: { TRUST_IDENTIFY_LIVE: "1" } },
    "Control Studio connected live identification shows the connected plant evolving during Identify instead of returning a completed isolated trial": { env: { TRUST_IDENTIFY_LIVE: "1" } },
    "Control Studio connected live identification closing a live experiment leaves it running in the runtime and reopening follows it until the plant is restored": { env: { TRUST_IDENTIFY_LIVE: "1" } },
    // The process rests mid-range, so each method's excitation stays inside the output range.
    "Control Studio identification continuity runs each selected method in the same study and reacquires after reopening":
      { fixture: fixture("control-studio-continuity"), env: { TRUST_IDENTIFY_CONTINUOUS: "1" } },
    "Control Studio single identification experiment one Identify produces a provisional model and preserves its measured experiment": { env: { TRUST_IDENTIFY_SINGLE: "1" } },
    "Control Studio single identification experiment Validate collects independent model evidence and checks the tuned controller after an explicit click":
      { env: { TRUST_IDENTIFY_SINGLE_VALIDATE: "1" } },
    "Control Studio PID group setup rendered sets up two interacting Control.PID instances as one group and opens it in Control Studio":
      { fixture: fixture("control-studio-two-zone-group"), env: {} },
    "Control Studio ON/OFF setup rendered sets up the program's own switching logic, opens it in Control Studio and tunes its switching band and minimum times":
      { fixture: path.join(repoRoot, "examples", "control-onoff-commissioning"), env: {} },
    "Control Studio state-feedback setup rendered sets up the program's own state-feedback law and opens it in Control Studio":
      { fixture: path.join(repoRoot, "examples", "control-state-feedback-commissioning"), env: {} },
    "Control Studio own P or PI law setup rendered sets up the program's own PI and opens it in Control Studio":
      { fixture: fixture("control-studio-own-pi"), env: {} },
    "Control Studio three-page workflow executes the project-owned sequence and retains its restored native recording":
      { fixture: path.join(repoRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "01-cold-storage-loop-anatomy"),
        env: { TRUST_CONTROL_STUDIO_PROJECT_NATIVE: "1" } },
    // Lesson 69's LQR project: its loop, program and supplied model are the fixture, and the journey follows the lesson's README steps 2 to 4.
    "Control Studio three-page workflow renders state-design policy and preserves finite design drafts across reopen":
      { fixture: path.join(extensionDevelopmentPath, "media", "examples", "control", "11-state-space-estimation-and-predictive-control", "69-lqr"),
        env: {} },
    // Lesson 61's project test, then its measured MIMO paths: the lesson's own project is the fixture (README steps 4 to 7).
    "Control Studio course projects lesson 61 measures the coupled vessels gain matrix with measured MIMO paths after the project test":
      { fixture: path.join(extensionDevelopmentPath, "media", "examples", "control", "10-multivariable-control", "61-mimo-paths"), env: {} },
    // Lesson 89's finite search over the notch: the supplied model, Automatic Tune and Validate (README steps 4 to 6).
    "Control Studio course projects lesson 89 finds the notch of the flexible axis by a finite search and validates the controller":
      { fixture: path.join(extensionDevelopmentPath, "media", "examples", "control", "07-classical-stability-and-frequency-design", "89-mechanical-resonance"), env: {} },
    // Lesson 1's project declares the door indication and its own project experiment; README step 2 selects the indication in Signals.
    "Control Studio three-page workflow offers the declared project experiment and plots selected recorded signals in their own units":
      { fixture: path.join(repoRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "01-cold-storage-loop-anatomy"),
        env: {} },
    // The project tests of the heater lessons run in real time on the lesson's own project (README, Experiment in Control Studio):
    // lesson 13's 30.1 s held input (and its short display scenario), lesson 15's energy balance, and lessons 4 and 5's 120.1 s warming
    // and load experiment under P and PI control.
    "Control Studio process dynamics runs, keeps and reopens the published thirty-second held-input observation":
      { fixture: path.join(repoRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "13-process-response-classes"),
        env: { TRUST_PROCESS_DYNAMICS: "1" } },
    "Control Studio process dynamics keeps the retained recording of a short project test on the chart while live values keep arriving":
      { fixture: path.join(repoRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "13-process-response-classes"),
        env: { TRUST_PROCESS_DYNAMICS: "1", TRUST_PROCESS_DYNAMICS_UI_ONLY: "1" } },
    "Control Studio heater model actual rendered workflow control-water-heater-energy-balance executes its held heater case and retains independent evidence":
      { fixture: path.join(repoRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "15-water-heater-energy-balance"),
        env: { TRUST_CONTROL_STUDIO_HEATER_MODEL: "1" } },
    "Control Studio actual PID heater lesson records the published proportional warming and load experiment through ordinary I/O":
      { fixture: path.join(repoRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "04-water-heater-p"),
        env: { TRUST_CONTROL_STUDIO_HEATER_PID_LESSON: "1" } },
    "Control Studio actual PID heater lesson records the published proportional-integral warming and load experiment through ordinary I/O":
      { fixture: path.join(repoRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "05-water-heater-pi"),
        env: { TRUST_CONTROL_STUDIO_HEATER_PID_LESSON: "1" } },
    // The room heating acceptance takes hours, so it is listed only when the runner itself is asked for it.
    ...(process.env.TRUST_CONTROL_STUDIO_ROOM_HEATING === "1"
      ? {
          "Control Studio room heating acceptance identifies tunes validates applies restores and keeps TIC101 on the attached room heating runtime":
            { fixture: path.join(repoRoot, "examples", "room_heating"), env: { TRUST_CONTROL_STUDIO_ROOM_HEATING: "1" } },
        }
      : {}),
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
    "Control Studio three-page workflow starts identification from its visible button with the actual Control.PID plant",
    "Control Studio three-page workflow edits manual settings in the existing result area and preserves full values and pins when changing method",
    "Control Studio three-page recovery rendered projection shows the engine's block on the pages and Restore previous settings in the Apply position, while page navigation stays usable",
    "Control Studio three-page recovery rendered projection the failure's Restore previous settings asks for the Restore review on Validate, and a refused review leaves Restore in the position",
    "Control Studio three-page recovery rendered projection the confirmed Restore's stages end the block, and a normal Apply review adds no recovery control",
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
    "Control Studio three-page workflow opens a supplied plant model through the selected method and retains it for tuning after reopening",
    "Control Studio three-page workflow plots measured and predicted identification responses with their units",
    "Control Studio three-page workflow renders the identified plant equation from scoped host state",
    "Control Studio three-page workflow runs the declared root locus from Tune without a candidate or runtime write",
    "Control Studio three-page workflow shows Identify Tune Validate with page-specific controls and no runtime chrome",
    "Control Studio three-page workflow shows retained checks and an inconclusive row for every unfinished mandatory scenario",
    "Control Studio three-page workflow shows tuned settings and every mandatory validation requirement on their pages",
    "Control Studio three-page workflow starts guarded application and reports rejection without claiming adoption",
    "Rendered cascaded gravity tank editor offers coupled tank dynamics with two level states and feedback ports",
    ...Object.keys(gatedJourneys),
  ];
  // Normally the first failing journey ends the run. TRUST_VSCODE_RENDERED_CONTINUE=1 runs every journey and names each failure
  // at the end, so one failure does not hide the next.
  const continueAfterFailure = process.env.TRUST_VSCODE_RENDERED_CONTINUE === "1";
  const renderedFailures: string[] = [];
  for (const renderedTestFullTitle of renderedTestFullTitles) {
    if (configuredTestGrep && !new RegExp(configuredTestGrep).test(renderedTestFullTitle)) continue;
    const renderedRunDir = fs.mkdtempSync(
      path.join(os.tmpdir(), "trust-lsp-vscode-plant-rendered-")
    );
    const renderedWorkspacePath = path.join(renderedRunDir, "workspace");
    const renderedUserDataDir = path.join(renderedRunDir, "user-data");
    const renderedExtensionsDir = path.join(renderedRunDir, "extensions");
    // The journey's own state folder for the engine's operation journals (see `stateDir` above).
    const renderedStateDir = path.join(renderedRunDir, "state");
    try {
      const studioCase = renderedTestFullTitle.startsWith("Control Studio ");
      const gated = gatedJourneys[renderedTestFullTitle];
      const renderedFixture = gated?.fixture ? gated.fixture : studioCase
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
      fs.mkdirSync(path.join(renderedUserDataDir, "User"), { recursive: true });
      // VS Code opens its Chat side bar by default. On the CI screen (xvfb-run's 1280x1024) that leaves the editor narrower
      // than the desktop layout these journeys check, so the test profile keeps the side bar closed.
      fs.writeFileSync(path.join(renderedUserDataDir, "User", "settings.json"),
        JSON.stringify({ "files.enableTrash": false, "workbench.secondarySideBar.defaultVisibility": "hidden" }));
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
        version: process.env.TRUST_VSCODE_TEST_VERSION,
        extensionDevelopmentPath,
        extensionTestsPath,
        launchArgs: [
          renderedWorkspacePath,
          ...((studioCase || renderedTestFullTitle.startsWith("HMI agent")) ? ["--window-size=1600,1000"] : []),
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
          XDG_STATE_HOME: renderedStateDir,
          TRUST_PLANT_RENDERED_TEST: "1",
          ...(studioCase ? { TRUST_CONTROL_STUDIO_THREE_PAGE: "1" } : {}),
          ...(renderedTestFullTitle.startsWith("Control Studio three-page native journey") ? { TRUST_CONTROL_STUDIO_THREE_PAGE_NATIVE: "1" } : {}),
          ...(gated?.env ?? {}),
          TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT: renderedScreenshot,
          ...(process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME ? { TRUST_CONTROL_STUDIO_RENDERED_THEME: process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME } : {}),
          ...(process.env.TRUST_CONTROL_STUDIO_RENDERED_SPLIT ? { TRUST_CONTROL_STUDIO_RENDERED_SPLIT: process.env.TRUST_CONTROL_STUDIO_RENDERED_SPLIT } : {}),
          TRUST_PLANT_RENDERED_USER_DATA_DIR: renderedUserDataDir,
          TRUST_VSCODE_TEST_GREP: `^${renderedTestFullTitle}$`,
        },
      });
    } catch (error) {
      if (!continueAfterFailure) throw error;
      renderedFailures.push(renderedTestFullTitle);
      console.error(`Rendered journey failed: ${renderedTestFullTitle}`);
    } finally {
      removeTestTempDirectory(renderedRunDir);
    }
  }
  if (renderedFailures.length) {
    throw new Error(`${renderedFailures.length} rendered journey(s) failed:\n${renderedFailures.join("\n")}`);
  }
}

main().catch((error) => {
  console.error("Failed to run VS Code extension tests");
  console.error(error);
  process.exit(1);
});
