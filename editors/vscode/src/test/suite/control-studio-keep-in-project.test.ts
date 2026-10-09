import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { decodeKeepCommand, keepableRecord, keepAppliedParameters, KeepInProjectCoordinator } from "../../controlStudio/keepInProject";
import { pidLoopDefinition } from "../../controlStudio/loopSetup/pidLoopSetup";
import { pidSymbols, scope, TASK } from "./control-studio-cascade-fixtures";

/** Specification 35 §17.17: Keep in project writes the values the running PLC has into the project, separately from Apply. */
const catalog = {
  symbols: [
    { symbol: "P1.TemperatureC", type: "LREAL", address: "%IL0" },
    { symbol: "P1.Output", type: "LREAL", address: "%QL0" },
    { symbol: "P1.Setpoint", type: "LREAL" },
    { symbol: "P1.ManualOutput", type: "LREAL" },
    { symbol: "P1.AutoMode", type: "BOOL" },
    ...pidSymbols("P1.Controller", "P1.Parameters"),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Controller", type: "Control.PID", tasks: TASK }],
  links: [{ instance: "P1.Controller", method: "Configure", argument: "Parameters", record: "P1.Parameters", output: null, fields: {} }],
  writes: ["P1.Output"],
};

function loop() {
  return pidLoopDefinition(catalog, { ...scope, name: "Oven", instance: "P1.Controller", action: "direct", parameterRecord: "P1.Parameters",
    processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 100 },
    manipulatedVariable: { symbol: "P1.Output", unit: "%", minimum: 0, maximum: 100 }, setpoint: { symbol: "P1.Setpoint" },
    mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualOutput", experimentStep: 10, experimentDurationSeconds: 60 } as any);
}

const messageScope = { projectId: scope.projectId, editorSessionId: "editor-1", loopId: "loop:oven", loopRevision: "sha256:loop" };
const keepMessage = (overrides: Record<string, unknown> = {}) => ({ schemaVersion: 1, kind: "control-studio.application.keep", messageId: "m-1",
  projectId: messageScope.projectId, editorSessionId: messageScope.editorSessionId, loopId: messageScope.loopId,
  expectedLoopRevision: messageScope.loopRevision, payload: {}, ...overrides });

/** Runs `body` with the language server's commands answered by `answer`; every other command runs as usual. */
async function withCommands(answer: (command: string, args: Record<string, unknown>) => unknown, body: () => Promise<void>): Promise<void> {
  const original = vscode.commands.executeCommand;
  vscode.commands.executeCommand = (async (command: string, ...args: unknown[]) => command.startsWith("trust-lsp.")
    ? answer(command, args[0] as Record<string, unknown>) : original(command, ...args)) as typeof vscode.commands.executeCommand;
  try { await body(); } finally { vscode.commands.executeCommand = original; }
}

suite("Control Studio Keep in project", () => {
  test("keeps the loop's parameter record, field by field", () => {
    const record = keepableRecord(loop());
    assert.strictEqual(record?.record, "P1.Parameters");
    assert.deepStrictEqual(record?.fields.slice(0, 3), ["ProportionalGain", "IntegralGain", "DerivativeGain"]);
    assert.strictEqual(record?.fields.length, 11, "every numeric field the loop binds");
  });

  test("answers only a keep request of the open study", () => {
    assert.strictEqual(decodeKeepCommand(keepMessage(), messageScope), "m-1");
    assert.strictEqual(decodeKeepCommand(keepMessage({ loopId: "loop:other" }), messageScope), undefined);
    assert.strictEqual(decodeKeepCommand(keepMessage({ payload: { values: {} } }), messageScope), undefined);
    assert.strictEqual(decodeKeepCommand(keepMessage({ kind: "control-studio.application.apply" }), messageScope), undefined);
  });

  test("posts what was kept, or why not, as the application's answer", async () => {
    const posts: Record<string, unknown>[] = [];
    const folder = vscode.Uri.file(os.tmpdir());
    let fail = false;
    const coordinator = new KeepInProjectCoordinator({
      runningValues: async () => ({ "P1.Parameters.ProportionalGain": 2.5 }),
      folder: () => folder,
      keep: async (_loop, running) => {
        if (fail) throw new Error("the project changed while preparing this edit");
        assert.deepStrictEqual(running, { "P1.Parameters.ProportionalGain": 2.5 }, "the values the running PLC has");
        return { kept: ["ProportionalGain"], unknown: ["TrackingTime"] };
      },
    });
    const post = async (payload: Record<string, unknown>) => { posts.push(payload); };
    assert.strictEqual(await coordinator.handle({ kind: "control-studio.application.apply" }, messageScope, loop(), post), false);
    assert.strictEqual(await coordinator.handle(keepMessage(), messageScope, loop(), post), true);
    assert.deepStrictEqual(posts[0], { status: "kept", applicationAction: "keep", kept: ["ProportionalGain"], unknown: ["TrackingTime"],
      reason: "Kept in the project: ProportionalGain of P1.Parameters. A restart or a new download keeps them. "
        + "The equipment has no setting for TrackingTime; copy it into the PLC code by hand." });
    fail = true;
    await coordinator.handle(keepMessage(), messageScope, loop(), post);
    assert.deepStrictEqual(posts[1], { status: "keep-failed", applicationAction: "keep", reason: "Not kept: the project changed while preparing this edit" });
  });

  test("writes a hand-written record's initializer and saves the file", async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-keep-written-"));
    const source = path.join(root, "main.st");
    const text = "PROGRAM Main\nVAR\n    Parameters : Control.PIDParameters := (ProportionalGain := LREAL#1.0);\nEND_VAR\nEND_PROGRAM\n";
    fs.writeFileSync(source, text);
    const uri = vscode.Uri.file(source).toString();
    const at = text.split("\n")[2].indexOf("LREAL#1.0");
    const requests: Array<[string, Record<string, unknown>]> = [];
    await withCommands((command, args) => {
      requests.push([command, args]);
      return { ok: true, kept: ["ProportionalGain"], expected: { [uri]: text },
        edit: { changes: { [uri]: [{ range: { start: { line: 2, character: at }, end: { line: 2, character: at + 9 } }, newText: "LREAL#2.5" }] } } };
    }, async () => {
      const result = await keepAppliedParameters(loop(), { "P1.Parameters.ProportionalGain": 2.5, "P1.Parameters.IntegralGain": 0.2 },
        vscode.Uri.file(root));
      assert.deepStrictEqual(result, { kept: ["ProportionalGain"], unknown: [] });
    });
    assert.strictEqual(requests[0][0], "trust-lsp.controlKeepParameters");
    assert.deepStrictEqual([requests[0][1].record, requests[0][1].values], ["P1.Parameters", { ProportionalGain: 2.5, IntegralGain: 0.2 }]);
    assert.strictEqual(fs.readFileSync(source, "utf8"), text.replace("LREAL#1.0", "LREAL#2.5"), "the edit is applied and saved");
    fs.rmSync(root, { recursive: true, force: true });
  });

  test("keeps a record Create PLC code declared through its equipment settings", async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-keep-generated-"));
    const changes: unknown[] = [];
    await withCommands((command, args) => {
      if (command === "trust-lsp.controlKeepParameters") return { ok: false, generated: { instance: "Main.TIC101",
        settings: { "{instance}_Parameters.ProportionalGain": 33.3, "{instance}_Parameters.IntegralGain": 0.0667, "{instance}_Parameters.TrackingTime": 1 } } };
      assert.strictEqual(command, "trust-lsp.hmiEquipment");
      assert.strictEqual(args.instance, "Main.TIC101");
      if (args.changes) { changes.push(args.changes); return { ok: true, edit: { changes: {} }, expected: {} }; }
      return { ok: true, owned: true, settings: [{ member: "{instance}_Parameters.ProportionalGain", value: 1 },
        { member: "{instance}_Parameters.IntegralGain", value: 0.0667 }] };
    }, async () => {
      const result = await keepAppliedParameters(loop(), { "P1.Parameters.ProportionalGain": 33.3 }, vscode.Uri.file(root));
      assert.deepStrictEqual(result, { kept: ["ProportionalGain"], unknown: ["TrackingTime"] },
        "the changed gain is kept; the project already has the integral gain; the equipment has no tracking-time setting");
    });
    assert.deepStrictEqual(changes, [{ settings: { "{instance}_Parameters.ProportionalGain": 33.3 } }],
      "one equipment edit with the values that differ, as the HMI Builder's Keep in project makes it");
    fs.rmSync(root, { recursive: true, force: true });
  });
});
