// The acceptance journeys on the packaged extension (specification 22, "Acceptance journeys on the packaged extension"). It
// installs a VSIX into a fresh extensions folder of a VS Code that has never run truST, copies each journey's project from
// the checkout's examples, and runs each journey in its own window at 1×, with an empty truST data folder and no Rust
// toolchain on PATH. F23 first starts the project's runtime from the command line, as the example's README says.
//
// Usage, under a display (`xvfb-run -a -s "-screen 0 1920x1080x24"` where there is none):
//   node out/acceptance/runAcceptance.js --vsix <file> --out <folder> [--journey <session>]...
// E2E_VSCODE names the VS Code executable to use; otherwise the stable VS Code (TRUST_VSCODE_TEST_VERSION, when set) is
// downloaded. The journal and screenshots of each session go to <folder>/<session>, and <folder>/summary.json lists each
// session's result. Every session runs, and the run fails when one of them failed.
import * as child from "child_process";
import * as crypto from "crypto";
import * as fs from "fs";
import * as net from "net";
import * as os from "os";
import * as path from "path";
import { downloadAndUnzipVSCode, resolveCliPathFromVSCodeExecutablePath, runTests } from "@vscode/test-electron";

const REPO = path.resolve(__dirname, "..", "..", "..", "..");
const PID_EXAMPLE = "control-pid-commissioning";
const TANK_EXAMPLE = "hmi_tank_level";

/** One window of the run: its journey, the example its project copies, an edit of the copy that the journey needs, and for
 * F23 how the project's runtime listens. */
type Session = {
  name: string;
  journey: string;
  example: string;
  setup?: (project: string) => void;
  runtime?: "default endpoint" | "TCP endpoint with a token";
};

const SESSIONS: Session[] = [
  { name: "pid-commissioning", journey: "pid-commissioning", example: PID_EXAMPLE },
  { name: "pid-try", journey: "pid-try", example: PID_EXAMPLE },
  { name: "pid-slow", journey: "pid-slow", example: PID_EXAMPLE, setup: slowerHeater },
  { name: "identify-auto-rest", journey: "identify-auto-rest", example: PID_EXAMPLE, setup: startInAuto },
  { name: "f23-local", journey: "f23-project-runtime", example: TANK_EXAMPLE, runtime: "default endpoint" },
  { name: "f23-tcp", journey: "f23-project-runtime", example: TANK_EXAMPLE, setup: tcpEndpointWithToken, runtime: "TCP endpoint with a token" },
];

/** Replaces the one occurrence of `old` in the file, or fails: an example that changed must not let a journey run on the
 * unmodified project. */
function replaceOnce(file: string, old: string, replacement: string): void {
  const text = fs.readFileSync(file, "utf8");
  const count = text.split(old).length - 1;
  if (count !== 1) throw new Error(`${file}: expected one "${old}", found ${count}`);
  fs.writeFileSync(file, text.replace(old, replacement));
}

/** Identify from Auto (specification 35, B3.4 step 5): the copy starts in Auto with its setpoint at 30 °C, so the controller
 * still drives the heater when Identify plant is pressed. */
function startInAuto(project: string): void {
  const main = path.join(project, "src", "Main.st");
  replaceOnce(main, "AutoMode : BOOL := FALSE;", "AutoMode : BOOL := TRUE;");
  replaceOnce(main, "Setpoint : LREAL := 20.0;", "Setpoint : LREAL := 30.0;");
}

/** The README's slower heater: the simulated heater's time constant doubles from 4 s to 8 s. */
function slowerHeater(project: string): void {
  replaceOnce(path.join(project, "simulation.toml"), "time_constant_s = 4.0", "time_constant_s = 8.0");
}

/** F23 over TCP: the runtime's control endpoint on 127.0.0.1:9123 with a control token that is generated here and never
 * printed; the file that holds it is readable by its owner only. */
function tcpEndpointWithToken(project: string): void {
  const file = path.join(project, "runtime.toml");
  const token = crypto.randomBytes(24).toString("hex");
  replaceOnce(file, "[runtime.control]\n", `[runtime.control]\nendpoint = "tcp://127.0.0.1:9123"\nauth_token = "${token}"\n`);
  fs.chmodSync(file, 0o600);
}

function options(argv: string[]): { vsix: string; out: string; only: string[] } {
  let vsix = "";
  let out = "";
  const only: string[] = [];
  for (let index = 0; index < argv.length; index += 2) {
    const [flag, value] = [argv[index], argv[index + 1]];
    if (!value) throw new Error(`${flag} needs a value`);
    if (flag === "--vsix") vsix = path.resolve(value);
    else if (flag === "--out") out = path.resolve(value);
    else if (flag === "--journey") only.push(value);
    else throw new Error(`unknown option ${flag}`);
  }
  if (!vsix || !out) throw new Error("usage: runAcceptance.js --vsix <file> --out <folder> [--journey <session>]...");
  const unknown = only.filter((name) => !SESSIONS.some((session) => session.name === name));
  if (unknown.length) throw new Error(`unknown sessions ${unknown.join(", ")}; the sessions are ${SESSIONS.map((s) => s.name).join(", ")}`);
  return { vsix, out, only };
}

/** The environment of a machine without Rust: no folder on PATH that holds rustc, cargo or rustup (a link to them counts),
 * and no toolchain variables. Fails when a Rust compiler is still found, since the journeys would then not prove the
 * packaged toolchain. */
function withoutRust(): void {
  const holdsRust = (folder: string) =>
    ["rustc", "cargo", "rustup"].some((tool) => fs.existsSync(path.join(folder, tool)) || fs.existsSync(path.join(folder, `${tool}.exe`)));
  const entries = (process.env.PATH ?? "").split(path.delimiter).filter((entry) => entry && !holdsRust(entry));
  process.env.PATH = entries.join(path.delimiter);
  for (const name of ["TRUST_TOOLCHAIN", "CARGO_HOME", "RUSTUP_HOME", "CARGO_TARGET_DIR", "RUSTC_WRAPPER"]) delete process.env[name];
  // VS Code would otherwise read the login shell's PATH, which may name Cargo again.
  process.env.VSCODE_SKIP_RESOLVING_SHELL_ENV = "1";
  const found = child.spawnSync("sh", ["-c", "command -v rustc cargo"], { encoding: "utf8" }).stdout.trim();
  if (found) throw new Error(`a Rust toolchain is still on PATH: ${found}`);
}

/** Installs the VSIX into the empty extensions folder `extensions`; gives the folder of the installed extension. */
function install(vscodeExecutable: string, vsix: string, extensions: string, profile: string): string {
  const cli = resolveCliPathFromVSCodeExecutablePath(vscodeExecutable);
  const result = child.spawnSync(cli, ["--extensions-dir", extensions, "--user-data-dir", profile, "--install-extension", vsix, "--force"],
    { encoding: "utf8" });
  process.stdout.write(result.stdout ?? "");
  if (result.status !== 0) throw new Error(`installing ${vsix} failed (${result.status}): ${result.stderr || result.error?.message}`);
  const installed = fs.readdirSync(extensions).filter((name) => name.startsWith("trust-platform.trust-lsp-"));
  if (installed.length !== 1) throw new Error(`expected one installed truST extension in ${extensions}, found ${installed.join(", ") || "none"}`);
  return path.join(extensions, installed[0]);
}

/** The settings of a quiet window: no welcome page, tips, update checks, telemetry, chat or workspace-trust prompts. */
const SETTINGS = {
  "workbench.colorTheme": "Default Dark Modern",
  "workbench.startupEditor": "none",
  "workbench.tips.enabled": false,
  "workbench.enableExperiments": false,
  "telemetry.telemetryLevel": "off",
  "update.mode": "none",
  "extensions.autoUpdate": false,
  "extensions.autoCheckUpdates": false,
  "security.workspace.trust.enabled": false,
  "chat.commandCenter.enabled": false,
  "chat.disableAIFeatures": true,
  "workbench.secondarySideBar.defaultVisibility": "hidden",
  "window.titleBarStyle": "custom",
  "git.enabled": false,
  "editor.accessibilitySupport": "off",
  "workbench.editor.enablePreview": false,
  "files.simpleDialog.enable": true,
};

/** The host extension of the journeys. It adds nothing: the extension under test is the installed VSIX. */
function writeHost(folder: string): void {
  fs.mkdirSync(folder, { recursive: true });
  fs.writeFileSync(path.join(folder, "package.json"), JSON.stringify({
    name: "trust-acceptance-host", version: "0.0.1", publisher: "acceptance", engines: { vscode: "^1.100.0" }, main: "./extension.js",
  }, null, 2));
  fs.writeFileSync(path.join(folder, "extension.js"), "exports.activate = () => {};\nexports.deactivate = () => {};\n");
}

function wait(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function until(label: string, holds: () => boolean | Promise<boolean>, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await holds()) return;
    await wait(1_000);
  }
  throw new Error(`timed out waiting for ${label}`);
}

function answers(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.connect(port, "127.0.0.1", () => { socket.destroy(); resolve(true); });
    socket.on("error", () => resolve(false));
  });
}

/** Builds the project with the installed extension's trust-runtime and starts it as the README says; gives its process once
 * it reports its control endpoint and its web server answers. */
async function startRuntime(bin: string, project: string, log: string): Promise<child.ChildProcess> {
  const runtime = path.join(bin, process.platform === "win32" ? "trust-runtime.exe" : "trust-runtime");
  const build = child.spawnSync(runtime, ["build", "--project", "."], { cwd: project, encoding: "utf8" });
  fs.writeFileSync(`${log}.build`, `${build.stdout ?? ""}${build.stderr ?? ""}`);
  if (build.status !== 0) throw new Error(`trust-runtime build failed (${build.status}); see ${log}.build`);
  const output = fs.openSync(log, "w");
  const process_ = child.spawn(runtime, ["run", "--project", "."], { cwd: project, stdio: ["ignore", output, output] });
  await until("the runtime's control endpoint", () => fs.existsSync(path.join(project, ".trust-runtime", "control.json")), 60_000);
  await until("the runtime's web server on port 8080", () => answers(8080), 60_000);
  return process_;
}

async function stopRuntime(runtime: child.ChildProcess): Promise<void> {
  if (runtime.exitCode !== null || runtime.signalCode !== null) return;
  const ended = new Promise((resolve) => runtime.once("exit", resolve));
  runtime.kill("SIGTERM");
  await Promise.race([ended, wait(15_000)]);
  if (runtime.exitCode === null && runtime.signalCode === null) runtime.kill("SIGKILL");
}

type Result = { session: string; passed: boolean; error?: string; seconds: number };

async function main(): Promise<number> {
  const { vsix, out, only } = options(process.argv.slice(2));
  withoutRust();
  const vscodeExecutable = process.env.E2E_VSCODE?.trim() || await downloadAndUnzipVSCode(process.env.TRUST_VSCODE_TEST_VERSION?.trim() || "stable");
  // Short paths: a project's default control endpoint and the window's sockets live under them.
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "trust-acc-"));
  const extensions = path.join(scratch, "ext");
  const installed = install(vscodeExecutable, vsix, extensions, path.join(scratch, "install-profile"));
  const bin = path.join(installed, "bin");
  const host = path.join(scratch, "host");
  writeHost(host);
  // truST's data folder (the installed toolchain, specification 44 §4.6): one for every session, empty at the start, as on a
  // machine that never ran truST.
  const data = path.join(scratch, "data");
  fs.mkdirSync(data);
  fs.rmSync(out, { recursive: true, force: true });
  fs.mkdirSync(out, { recursive: true });
  console.log(`acceptance: ${path.basename(installed)} in ${vscodeExecutable}; rustc: none`);

  const results: Result[] = [];
  for (const session of SESSIONS.filter((entry) => !only.length || only.includes(entry.name))) {
    const started = Date.now();
    const project = path.join(scratch, session.name);
    const profile = path.join(scratch, `ud-${session.name}`);
    const shots = path.join(out, session.name);
    let runtime: child.ChildProcess | undefined;
    console.log(`== ${session.name} (${new Date().toISOString()})`);
    try {
      fs.cpSync(path.join(REPO, "examples", session.example), project, { recursive: true });
      session.setup?.(project);
      fs.mkdirSync(path.join(profile, "User"), { recursive: true });
      fs.writeFileSync(path.join(profile, "User", "settings.json"), JSON.stringify(SETTINGS, null, 2));
      fs.mkdirSync(shots, { recursive: true });
      if (session.runtime) {
        runtime = await startRuntime(bin, project, path.join(out, `${session.name}-runtime.log`));
        console.log(`   runtime on the ${session.runtime}, process ${runtime.pid}`);
      }
      await runTests({
        vscodeExecutablePath: vscodeExecutable,
        extensionDevelopmentPath: host,
        extensionTestsPath: path.join(__dirname, "suite", "index"),
        launchArgs: [
          project,
          "--extensions-dir", extensions,
          "--user-data-dir", profile,
          "--remote-debugging-port=0",
          "--disable-workspace-trust", "--skip-welcome", "--skip-release-notes",
          ...(process.platform === "linux" ? ["--ozone-platform=x11"] : []),
          "--disable-gpu", "--use-gl=angle", "--use-angle=swiftshader", "--in-process-gpu", "--no-sandbox",
        ],
        extensionTestsEnv: {
          E2E_SESSION: session.name,
          E2E_JOURNEY: session.journey,
          E2E_SHOTS: shots,
          E2E_UD: profile,
          E2E_TIMEOUT_MS: process.env.E2E_TIMEOUT_MS || "1800000",
          // Control Studio journals its operations in $XDG_STATE_HOME/trust (specification 35, A8.3): each session has its own.
          XDG_STATE_HOME: path.join(scratch, `state-${session.name}`),
          XDG_DATA_HOME: data,
          ...(runtime?.pid ? { E2E_RUNTIME_PID: String(runtime.pid) } : {}),
        },
      });
      results.push({ session: session.name, passed: true, seconds: Math.round((Date.now() - started) / 1_000) });
    } catch (error) {
      results.push({ session: session.name, passed: false, error: (error as Error).message, seconds: Math.round((Date.now() - started) / 1_000) });
    } finally {
      if (runtime) await stopRuntime(runtime);
    }
    const last = results[results.length - 1];
    console.log(`   ${last.passed ? "passed" : `FAILED: ${last.error}`} (${last.seconds} s)`);
  }
  fs.writeFileSync(path.join(out, "summary.json"), JSON.stringify(results, null, 2));
  fs.rmSync(scratch, { recursive: true, force: true });
  const failed = results.filter((result) => !result.passed);
  console.log(`acceptance: ${results.length - failed.length} of ${results.length} sessions passed`);
  return failed.length ? 1 : 0;
}

main().then((code) => process.exit(code), (error: Error) => {
  console.error(`acceptance: ${error.message}`);
  process.exit(2);
});
