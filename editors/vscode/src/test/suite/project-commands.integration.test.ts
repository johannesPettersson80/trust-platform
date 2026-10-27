import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import { compileGate } from "../../compileGate";
import type { CheckProgramResponse } from "../../checkProgramModel";
import { projectRequest } from "../../projectRequest";

// The project commands of the language server (specification 25 §6.2, §6.6, §6.7). Their rules are proved by the Rust tests
// beside them (crates/trust-lsp/src/handlers/commands/project/*_tests.rs). These tests prove the wiring: the language server
// advertises each command, the extension reaches it and reads its answer.

suite("Project commands of the language server", function () {
  this.timeout(30000);

  let folder: string;

  suiteSetup(async () => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-project-commands-"));
    // The first request waits a bounded time for the language server to start.
    await projectRequest("trust-lsp.manifestCheck", { text: "" });
  });

  suiteTeardown(() => {
    fs.rmSync(folder, { recursive: true, force: true });
  });

  test("the manifest check names the line the server cannot read", async () => {
    const broken = await projectRequest<{ issue: string | null }>("trust-lsp.manifestCheck", { text: "[project\n" });
    assert.ok(broken.ok, broken.ok ? "" : broken.error);
    assert.ok(broken.issue?.startsWith("Project settings on line 1 cannot be read"), String(broken.issue));
    const readable = await projectRequest<{ issue: string | null }>("trust-lsp.manifestCheck", {
      text: 'include_paths = ["src"]\n',
    });
    assert.ok(readable.ok && readable.issue === null);
  });

  test("the sidebar's gate reaches the server with the last Compile", async () => {
    const report = {
      version: 1,
      ok: false,
      status: "failed",
      errors: 3,
      warnings: 0,
      issues: [
        { severity: "error", message: "a", code: "compile" },
        { severity: "error", message: "b", code: "compile" },
        { severity: "error", message: "c", code: "sources" },
      ],
    } satisfies CheckProgramResponse;
    const gate = await compileGate({ kind: "failed", report }, "unreachable");
    assert.strictEqual(gate.compile?.errors, 3);
    assert.strictEqual(gate.debug.enabled, false);
    // Problems of the test workspace may add errors; the reason names a count either way.
    assert.match(gate.reasons.start ?? "", /^Fix (\d+ errors?|runtime\.toml) to start\.$/);
  });

  test("Compile's project is the nearest folder with trust-lsp.toml", async () => {
    fs.mkdirSync(path.join(folder, "plant", "src"), { recursive: true });
    fs.writeFileSync(path.join(folder, "plant", "trust-lsp.toml"), 'include_paths = ["src"]\n');
    const main = path.join(folder, "plant", "src", "Main.st");
    fs.writeFileSync(main, "PROGRAM Main\nEND_PROGRAM\n");
    const answer = await projectRequest<{ root: string | null }>("trust-lsp.projectRoot", { path: main, folders: [folder] });
    assert.ok(answer.ok && answer.root === path.join(folder, "plant"), JSON.stringify(answer));
  });

  test("a chosen simulation speed is saved and read back by the server", async () => {
    fs.writeFileSync(path.join(folder, "runtime.toml"), "");
    const saved = await projectRequest<{ live: unknown }>("trust-lsp.simulationSpeed", {
      action: "save",
      folder,
      speed: 20,
      target: { kind: "simulator", status: "stopped", endpoint: "" },
    });
    assert.ok(saved.ok && saved.live === null, JSON.stringify(saved));
    const read = await projectRequest<{ speed: number; speeds: number[] }>("trust-lsp.simulationSpeed", {
      action: "read",
      folder,
    });
    assert.ok(read.ok, JSON.stringify(read));
    assert.strictEqual(read.speed, 20);
    assert.deepStrictEqual(read.speeds, [1, 2, 5, 10, 20, 50, 100]);
    assert.strictEqual(fs.readFileSync(path.join(folder, "simulation.toml"), "utf8"), "[simulation]\ntime_scale = 20\n");
  });
});
