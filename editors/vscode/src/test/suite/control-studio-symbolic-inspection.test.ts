import * as assert from "assert";

const bindings = [
  { id: "pv", symbol: "Main.PV", iec_type: "LREAL" },
  { id: "mode", symbol: "Main.Manual", iec_type: "BOOL" },
] as const;

suite("Control Studio symbolic inspection", () => {
  test("requests the exact bindings and retains one coherent available scan", async () => {
    const api = symbolicInspectionApi();
    const calls: Array<{ operation: string; params: Record<string, unknown> }> = [];
    const result = await api.inspectSymbolicSignals(async (operation: string, params: Record<string, unknown>) => {
      calls.push({ operation, params });
      return {
        program_revision: "sha256:program-1", loop_revision: "sha256:loop-1", bindings,
        available: true, scan: 42, runtime_time_ns: 42000000, captured_at_ms: 1700000000000,
        values: [21.5, 1], forced: [false, false], quality: ["good", "good"],
      };
    }, { loopRevision: "sha256:loop-1", expectedProgramRevision: "sha256:program-1" }, bindings);

    assert.deepStrictEqual(calls, [{ operation: "control.signals.inspect", params: {
      loop_revision: "sha256:loop-1", expected_program_revision: "sha256:program-1", bindings,
    } }]);
    assert.strictEqual(result.kind, "available");
    assert.strictEqual(result.sample.scan, 42);
    assert.strictEqual(result.sample.values[0], 21.5);
    assert.strictEqual(result.sample.values[1], 1);
    assert.strictEqual(api.isUsableSymbolicInspectionValue(result.sample, "pv"), true);
    assert.strictEqual(api.isUsableSymbolicInspectionValue(result.sample, "mode"), true);
  });

  test("retains pending provenance without manufacturing a sample", async () => {
    const api = symbolicInspectionApi();
    const result = await api.inspectSymbolicSignals(async () => ({
      program_revision: "sha256:program-2", loop_revision: "sha256:loop-2", bindings,
      available: false, reason: "Waiting for the next coherent runtime scan.",
    }), { loopRevision: "sha256:loop-2" }, bindings);

    assert.deepStrictEqual(result, {
      kind: "pending", programRevision: "sha256:program-2",
      reason: "Waiting for the next coherent runtime scan.",
    });
  });

  test("rejects duplicate binding IDs before calling the runtime", async () => {
    const api = symbolicInspectionApi();
    let calls = 0;
    await assert.rejects(() => api.inspectSymbolicSignals(async () => {
      calls += 1;
      return {
        program_revision: "sha256:program-duplicate", loop_revision: "sha256:loop-duplicate",
        bindings: [bindings[0], { id: "pv", symbol: "Main.Manual", iec_type: "BOOL" }],
        available: true, scan: 1, runtime_time_ns: 1000000, captured_at_ms: 1700000000000,
        values: [1, 1], forced: [false, false], quality: ["good", "good"],
      };
    }, { loopRevision: "sha256:loop-duplicate" }, [bindings[0], { id: "pv", symbol: "Main.Manual", iec_type: "BOOL" }]),
    /invalid symbolic inspection bindings/i);
    assert.strictEqual(calls, 0, "invalid bindings must not start or replace the runtime inspection watch");
  });

  test("keeps the request identity when an awaited transport caller mutates its input", async () => {
    const api = symbolicInspectionApi();
    const identity: { loopRevision: string; expectedProgramRevision?: string } = {
      loopRevision: "sha256:loop-snapshot", expectedProgramRevision: "sha256:program-snapshot",
    };
    const result = await api.inspectSymbolicSignals(async () => {
      identity.loopRevision = "sha256:mutated-loop";
      identity.expectedProgramRevision = "sha256:mutated-program";
      return {
        program_revision: "sha256:program-snapshot", loop_revision: "sha256:loop-snapshot", bindings,
        available: true, scan: 13, runtime_time_ns: 13000000, captured_at_ms: 1700000000000,
        values: [3, 1], forced: [false, false], quality: ["good", "good"],
      };
    }, identity, bindings);

    assert.strictEqual(result.kind, "available");
    assert.strictEqual(result.sample.loopRevision, "sha256:loop-snapshot");
    assert.strictEqual(result.sample.programRevision, "sha256:program-snapshot");
  });

  test("rejects a mismatched, imprecise, or misaligned response", async () => {
    const api = symbolicInspectionApi();
    const identity = { loopRevision: "sha256:loop-3", expectedProgramRevision: "sha256:program-3" };
    const valid = {
      program_revision: "sha256:program-3", loop_revision: "sha256:loop-3", bindings,
      available: true, scan: 7, runtime_time_ns: 7000000, captured_at_ms: 1700000000000,
      values: [1, 0], forced: [false, false], quality: ["good", "good"],
    };
    for (const invalid of [
      { ...valid, loop_revision: "sha256:other" },
      { ...valid, scan: Number.MAX_SAFE_INTEGER + 1 },
      { ...valid, quality: ["good"] },
      { ...valid, bindings: [{ id: "pv", symbol: "Main.PV", iec_type: "LREAL" }, { id: "mode", address: "%MX0.0", iec_type: "BOOL" }] },
    ]) {
      await assert.rejects(() => api.inspectSymbolicSignals(async () => invalid, identity, bindings), /invalid symbolic inspection response/i);
    }
  });

  test("preserves bad and missing values but never calls them usable", async () => {
    const api = symbolicInspectionApi();
    const result = await api.inspectSymbolicSignals(async () => ({
      program_revision: "sha256:program-4", loop_revision: "sha256:loop-4", bindings,
      available: true, scan: 9, runtime_time_ns: 9000000, captured_at_ms: 1700000000000,
      values: [null, 0.5], forced: [false, false], quality: ["missing", "good"],
    }), { loopRevision: "sha256:loop-4" }, bindings);

    assert.strictEqual(result.kind, "available");
    assert.deepStrictEqual(result.sample.values, [null, 0.5]);
    assert.strictEqual(api.isUsableSymbolicInspectionValue(result.sample, "pv"), false);
    assert.strictEqual(api.isUsableSymbolicInspectionValue(result.sample, "mode"), false);
  });
});

function symbolicInspectionApi(): any {
  let api: any;
  try { api = require("../../controlStudio/symbolicInspection"); } catch { api = {}; }
  assert.strictEqual(typeof api.inspectSymbolicSignals, "function",
    "symbolic inspection must use the runtime's coherent viewer protocol");
  return api;
}
