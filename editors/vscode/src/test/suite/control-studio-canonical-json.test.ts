import * as assert from "assert";
import { canonicalJson } from "../../controlStudio/canonicalJson";

/** The form the Studio used before: stringify, parse and re-stringify every member. */
function reference(value: unknown): string {
  const record = (item: unknown): item is Record<string, unknown> => !!item && typeof item === "object" && !Array.isArray(item);
  return JSON.stringify(Array.isArray(value) ? value.map(item => JSON.parse(reference(item))) : record(value)
    ? Object.fromEntries(Object.keys(value).sort().map(key => [key, JSON.parse(reference(value[key]))])) : value);
}

/** A retained verification run: samples of several channels, as a Validate campaign stores them. */
function recordedRun(samples: number): unknown {
  return { conditions: { durationSeconds: samples / 50, seed: 7, sampleTimesSeconds: Array.from({ length: samples }, (_, i) => i / 50) },
    channels: ["P1.AngleRad", "P1.TorqueNm", "P1.VelocityRadS"].map(id => ({ unit: "x", id })),
    samples: Array.from({ length: samples }, (_, i) => ({ timeSeconds: i / 50, values: {
      "P1.AngleRad": { value: Math.sin(i / 40), quality: "good" }, "P1.TorqueNm": { value: Math.cos(i / 40), quality: "good" },
      "P1.VelocityRadS": { value: i / 1000, quality: "good" } } })) };
}

suite("Control Studio canonical evidence", () => {
  test("keeps the exact canonical text of retained evidence", () => {
    const values: unknown[] = [null, true, 0, -0, 1.5e21, Number.NaN, "é\n\"", [], {}, [1, [2, { b: 1, a: 2 }]],
      { z: 1, a: { d: [3, { y: 1, x: 2 }], c: null }, "10": 1, "2": 2, "01": 3, b: "x" }, recordedRun(20)];
    for (const value of values) assert.strictEqual(canonicalJson(value), reference(value), JSON.stringify(value)?.slice(0, 80));
    for (const value of [{ a: undefined }, [undefined], { a: [() => 1] }]) {
      assert.throws(() => canonicalJson(value), SyntaxError, "a member that is not JSON is rejected, as before");
    }
    assert.strictEqual(canonicalJson(undefined), undefined, "an absent top-level value stays absent");
  });

  test("canonicalizes a long retained run in one pass", () => {
    // Every retained Studio draft canonicalizes each stored run several times, so the cost must stay close to
    // one serialization instead of re-serializing every subtree at every level.
    const run = recordedRun(12000);
    const time = (convert: (value: unknown) => string | undefined) => {
      convert(run);
      const started = process.hrtime.bigint();
      for (let repeat = 0; repeat < 3; repeat++) convert(run);
      return Number(process.hrtime.bigint() - started);
    };
    const ratio = time(canonicalJson) / time(JSON.stringify);
    assert.ok(canonicalJson(run).length > 1_000_000);
    assert.ok(ratio < 10, `canonical text of a 12,000-sample run costs ${ratio.toFixed(1)} serializations`);
  });
});
