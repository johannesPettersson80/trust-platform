import * as assert from "assert";
import type { DeclaredControlAnalysis } from "../../controlStudio/declaredAnalysis";
import { isSavedRouthAnalysis } from "../../controlStudio/routhPersistence";
import { calculateDeclaredRouth } from "../../controlStudio/routhHurwitz";

function definition(): DeclaredControlAnalysis {
  return {
    plant: { kind: "transfer-function", numerator: [1], denominator: [1, 2, 0], delaySeconds: 0, inputUnit: "%", outputUnit: "°C", revision: "routh-session", source: "provided", validation: "validated" },
    controller: { numerator: [{ parameter: "Kp" }], denominator: [1], parameters: { Kp: { value: 1, minimum: 0, maximum: 10, unit: "%/°C" } } },
    rootLocus: { parameter: "Kp", selectedValue: 1, values: [0, 1, 2] },
  };
}

suite("Control Studio Routh session", () => {
  test("rejects structurally incomplete or mismatched saved Routh evidence", () => {
    const base = { selectedValue: 2, definition: definition(), result: {} };
    assert.strictEqual(isSavedRouthAnalysis(base), false);
    const complete = { ...base, result: { parameter: "Kp", parameterUnit: "%/°C", selectedValue: 3,
      allowedDomain: { minimum: 0, maximum: 10 }, fixedParameters: {},
      polynomial: { status: "qualified", coefficients: [1, 2], rows: [], firstColumn: [], signChanges: 0, stable: true, interpretation: "ok", issues: [] },
      table: [], interval: { status: "qualified", kind: "open", lower: 0, upper: 3, inequalities: [], derivation: "ok" }, provenance: "provided", scope: "declared" } };
    assert.strictEqual(isSavedRouthAnalysis(complete), false, "outer selectedValue must match the retained result");
    assert.strictEqual(isSavedRouthAnalysis({ ...complete, selectedValue: 3 }), true);
  });

  test("accepts the complete union-region result shape produced by Routh", () => {
    const unionDefinition: DeclaredControlAnalysis = { ...definition(), plant: { ...definition().plant, denominator: [1, 3, 3, 10] },
      controller: { numerator: [{ parameter: "Kp" }, { parameter: "Kp" }, { parameter: "Kp", scale: 4 }], denominator: [1], parameters: { Kp: { value: 2, minimum: -20, maximum: 20, unit: "%/°C" } } } };
    const result = calculateDeclaredRouth(unionDefinition, 2);
    const saved = { selectedValue: 2, result, definition: unionDefinition };
    assert.strictEqual(isSavedRouthAnalysis(saved), true);
    assert.ok(result.interval.kind === "union" || result.interval.intervals || result.interval.boundaries);
    assert.ok(result.polynomial.symbolicRows === undefined || result.polynomial.symbolicRows.every(row => Number.isInteger(row.power) && row.entries.every(entry => typeof entry === "string")));
  });
  test("rejects malformed nested saved Routh rows and union regions", () => {
    const valid = calculateDeclaredRouth(definition(), 2);
    const base: any = { selectedValue: 2, result: valid, definition: definition() };
    base.result.fixedParameters = { Kp: { value: "bad", unit: "%/°C" } };
    assert.strictEqual(isSavedRouthAnalysis(base), false);
    const nested: any = { selectedValue: 2, result: calculateDeclaredRouth(definition(), 2), definition: definition() };
    nested.result.polynomial.rows = [{ power: 1, entries: [1], calculations: [{ column: "bad" }] }];
    assert.strictEqual(isSavedRouthAnalysis(nested), false);
    const union: any = { selectedValue: 2, result: calculateDeclaredRouth(definition(), 2), definition: definition() };
    union.result.interval = { status: "qualified", kind: "union", inequalities: [], derivation: "missing regions" };
    assert.strictEqual(isSavedRouthAnalysis(union), false);
  });
  test("retains a read-only result snapshot and restores it without granting runtime authority", async () => {
    const { RouthSession } = require("../../controlStudio/routhSession") as typeof import("../../controlStudio/routhSession");
    const session = new RouthSession(definition());
    const messages: Array<Record<string, unknown>> = [];
    await session.handle({ schemaVersion: 1, kind: "control-studio.routh.run", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", messageId: "m", payload: { selectedValue: 2 } },
      { projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r" }, async payload => { messages.push(payload); });
    assert.strictEqual(messages[0].status, "complete");
    assert.strictEqual(typeof (session as unknown as { snapshot?: unknown }).snapshot, "function");
    const saved = (session as unknown as { snapshot(): unknown }).snapshot();
    session.invalidate();
    assert.strictEqual(session.selectedValues({ Kp: "P1.Kp" }), undefined);
    assert.ok(saved && typeof saved === "object", "the saved result remains inspectable");
    (session as unknown as { restore(value: unknown): void }).restore(saved);
    assert.strictEqual(session.selectedValues({ Kp: "P1.Kp" }), undefined, "restoring read-only evidence must not authorize a fresh runtime parameter selection");
    const other = new RouthSession({ ...definition(), plant: { ...definition().plant, revision: "other" } });
    (other as unknown as { restore(value: unknown): void }).restore(saved);
    assert.strictEqual(other.snapshot(), undefined, "a saved result from another definition must be rejected");
  });

  test("invalidate cancels pending work and clears the retained result", async () => {
    const { RouthSession } = require("../../controlStudio/routhSession") as typeof import("../../controlStudio/routhSession");
    const session = new RouthSession(definition()); let release!: () => void; const blocked = new Promise<void>(resolve => { release = resolve; });
    const running = session.handle({ schemaVersion: 1, kind: "control-studio.routh.run", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", messageId: "m", payload: { selectedValue: 1 } }, { projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r" }, async () => { await blocked; });
    session.invalidate(); release(); await running;
    assert.strictEqual(session.snapshot(), undefined);
    assert.strictEqual(session.selectedValues({ Kp: "P1.Kp" }), undefined);
  });

  test("accepts scoped cancellation and keeps the prior result unchanged", async () => {
    const { RouthSession } = require("../../controlStudio/routhSession") as typeof import("../../controlStudio/routhSession");
    const session = new RouthSession(definition());
    const messages: Array<Record<string, unknown>> = [];
    const scope = { projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r" };
    let release!: () => void; const blocked = new Promise<void>(resolve => { release = resolve; });
    const running = session.handle({ schemaVersion: 1, kind: "control-studio.routh.run", ...scope, expectedLoopRevision: "r", messageId: "m", payload: { selectedValue: 1 } }, scope, async payload => { messages.push(payload); await blocked; });
    const handled = await session.handle({ schemaVersion: 1, kind: "control-studio.routh.cancel", ...scope, expectedLoopRevision: "r", messageId: "c", payload: {} }, scope, async payload => { messages.push(payload); });
    assert.strictEqual(handled, true);
    release(); await running;
    assert.ok(messages.every(message => message.status !== "complete"), "a cancelled pending job must not publish a complete result");
  });
});
