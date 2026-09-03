import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

const ACTIVATION_SYMBOL = "P1.ExperimentEnabled";
const ACTIVATION_AT_SOURCE = "P1\\.ExperimentEnabled\\s+AT\\s+(%[IQM]X\\d+\\.\\d+)";

/** The runnable project owns every output; Control Studio's only write is the lesson authority Boolean. */
suite("Control curriculum experiment authority", () => {
  test("gives Control Studio no write authority beyond the lesson activation Boolean", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const controlRoot = path.join(repositoryRoot, "examples", "control");
    const loopApi = require("../../controlStudio/loopDefinition") as {
      decodeControlLoopDefinition(value: unknown): { kind: string; reason?: string; signal?: string; signals?: unknown };
      validateControlLoopDefinition(value: unknown, scope: { projectId: string; runtimeId: string }):
        { kind: string; reason?: string; signal?: string; signals?: unknown };
    };

    const loopPaths: string[] = [];
    const visit = (directory: string): void => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const entryPath = path.join(directory, entry.name);
        if (entry.isDirectory()) visit(entryPath);
        else if (entry.name === "control-loop.json") loopPaths.push(entryPath);
      }
    };
    visit(controlRoot);
    assert.strictEqual(loopPaths.length, 84);

    for (const loopPath of loopPaths.sort()) {
      const lessonDir = path.dirname(loopPath);
      const label = path.relative(controlRoot, lessonDir);
      const raw = JSON.parse(fs.readFileSync(loopPath, "utf8"));
      const scope = { projectId: raw.projectId, runtimeId: raw.runtimeId };

      const decoded = loopApi.decodeControlLoopDefinition(raw);
      assert.strictEqual(decoded.kind, "accepted", `${label} rule 1: decode rejected: ${describeResult(decoded)}`);
      const validated = loopApi.validateControlLoopDefinition(raw, scope);
      assert.strictEqual(validated.kind, "accepted", `${label} rule 1: validate rejected: ${describeResult(validated)}`);

      assert.strictEqual(raw.experiment?.execution, "project-scripted", `${label} rule 2: experiment.execution must be project-scripted`);
      assert.strictEqual(raw.experiment?.kind, "observation", `${label} rule 2: experiment.kind must be observation`);
      assert.strictEqual(raw.experiment?.amplitude, 0, `${label} rule 2: experiment.amplitude must be 0`);

      const source = readLessonSource(lessonDir);
      const address = activationAddress(source, label);
      assert.deepStrictEqual(raw.experiment.activation,
        { symbol: ACTIVATION_SYMBOL, address, iecType: "BOOL", direction: "write" },
        `${label} rule 3: experiment.activation must be the sole write binding of ${ACTIVATION_SYMBOL} at ${address}`);

      const writers: Array<Record<string, unknown>> = [];
      collectWriteDirectionEntries(raw, writers);
      assert.deepStrictEqual(writers.map((entry) => entry.symbol), [ACTIVATION_SYMBOL],
        `${label} rule 4: only ${ACTIVATION_SYMBOL} may carry direction "write"`);
      assert.strictEqual(writers[0], raw.experiment.activation, `${label} rule 4: the write entry must be experiment.activation itself`);

      const additional: Array<Record<string, unknown>> = raw.signals.additional ?? [];
      const activationSignals = additional.filter((entry) => entry.symbol === ACTIVATION_SYMBOL);
      assert.strictEqual(activationSignals.length, 1, `${label} rule 5: signals.additional must contain exactly one ${ACTIVATION_SYMBOL} entry`);
      assert.deepStrictEqual(
        { role: activationSignals[0].role, direction: activationSignals[0].direction,
          iecType: activationSignals[0].iecType, address: activationSignals[0].address },
        { role: "state", direction: "read", iecType: "BOOL", address },
        `${label} rule 5: ${ACTIVATION_SYMBOL} must be readable as a state signal at ${address}`);

      const modeSignal = raw.controller.modeSignal;
      assert.ok(modeSignal === undefined || nonEmptyString(modeSignal),
        `${label} rule 6: controller.modeSignal must be undefined or a non-empty string`);

      const lesson = JSON.parse(fs.readFileSync(path.join(lessonDir, "lesson.json"), "utf8"));
      assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"], `${label} rule 7: lesson.json workflowStages must be ["Monitor", "Test"]`);
      assert.strictEqual(lesson.experiment?.kind, "observation", `${label} rule 7: lesson.json experiment.kind must be observation`);
      assert.strictEqual(lesson.experiment?.amplitude, 0, `${label} rule 7: lesson.json experiment.amplitude must be 0`);
    }
  });
});

function readLessonSource(lessonDir: string): string {
  const sourceRoot = path.join(lessonDir, "src");
  return fs.readdirSync(sourceRoot)
    .filter((name) => name.endsWith(".st"))
    .sort()
    .map((name) => fs.readFileSync(path.join(sourceRoot, name), "utf8"))
    .join("\n");
}

function activationAddress(source: string, label: string): string {
  const matches = source.match(new RegExp(ACTIVATION_AT_SOURCE, "g")) ?? [];
  assert.strictEqual(matches.length, 1, `${label} rule 3: expected exactly one ${ACTIVATION_SYMBOL} AT binding, found ${matches.length}`);
  return new RegExp(ACTIVATION_AT_SOURCE).exec(source)![1];
}

function collectWriteDirectionEntries(value: unknown, found: Array<Record<string, unknown>>): void {
  if (Array.isArray(value)) {
    for (const item of value) collectWriteDirectionEntries(item, found);
  } else if (value && typeof value === "object") {
    const entry = value as Record<string, unknown>;
    if (entry.direction === "write") found.push(entry);
    for (const key of Object.keys(entry)) collectWriteDirectionEntries(entry[key], found);
  }
}

function nonEmptyString(value: unknown): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function describeResult(result: { reason?: string; signal?: string; signals?: unknown }): string {
  return [result.reason, result.signal, result.signals !== undefined ? JSON.stringify(result.signals) : undefined]
    .filter((part) => part !== undefined)
    .join(" ");
}
