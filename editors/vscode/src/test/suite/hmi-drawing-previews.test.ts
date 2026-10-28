import * as assert from "assert";

import type { HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { missingBindings, previewStates, previewValues } from "../../hmiBuilder/drawing/previews";

// Specification 39, §5.3 item 6: in Design mode a state switcher shows each state, including
// stale and unknown, without a connected source. Each symbol names its states; every other
// component has the data states.

function pump(bind: string[]): HmiObjectModel {
  return {
    id: "p-101",
    type: "pump",
    at: [0, 0],
    size: [128, 108],
    rotation: 0,
    bind: Object.fromEntries(bind.map((name) => [name, { path: `P101.${name}`, direction: "read" as const }])),
  };
}

const SAMPLES: Record<string, LiveValue> = {
  "P101.running": { value: true, state: "confirmed" },
  "P101.fault": { value: false, state: "confirmed" },
};

suite("HMI drawing: preview states", () => {
  test("each symbol offers its own states, then stale and unknown; other components the data states", () => {
    assert.deepStrictEqual(
      previewStates("pump").map((state) => state.id),
      ["sample", "running", "stopped", "fault", "manual", "out-of-service", "stale", "unknown"],
    );
    assert.deepStrictEqual(
      previewStates("valve").map((state) => state.id),
      ["sample", "open", "closed", "moving", "fault", "manual", "out-of-service", "stale", "unknown"],
    );
    assert.deepStrictEqual(
      previewStates("control-valve").map((state) => state.id),
      ["sample", "closed", "partly-open", "moving", "open", "fault", "manual", "out-of-service", "stale", "unknown"],
    );
    assert.deepStrictEqual(
      previewStates("value").map((state) => state.id),
      ["sample", "commanded", "estimated", "stale", "unknown"],
    );
  });

  test("a symbol's state sets the object's bindings to the values that show it", () => {
    const object = pump(["running", "fault"]);
    const fault = previewStates("pump").find((state) => state.id === "fault")!;
    assert.deepStrictEqual(previewValues(object, fault, SAMPLES), {
      "P101.running": { value: false, state: "confirmed" },
      "P101.fault": { value: true, state: "confirmed" },
    });
    assert.strictEqual(previewValues(object, previewStates("pump")[0], SAMPLES), SAMPLES, "the sample changes nothing");
  });

  test("stale ages, and unknown blanks, every bound value of the object", () => {
    const object = pump(["running", "fault"]);
    const state = (id: string) => previewStates("pump").find((candidate) => candidate.id === id)!;
    const stale = previewValues(object, state("stale"), SAMPLES);
    assert.deepStrictEqual(stale["P101.running"], { value: true, state: "confirmed", staleMs: 12_000 });
    const unknown = previewValues(object, state("unknown"), SAMPLES);
    assert.deepStrictEqual(unknown["P101.fault"], { value: null, state: "unknown" });
  });

  test("a state that needs a binding the object lacks names it", () => {
    const object = pump(["running"]);
    const state = (id: string) => previewStates("pump").find((candidate) => candidate.id === id)!;
    assert.deepStrictEqual(missingBindings(object, state("fault")), ["fault"]);
    assert.deepStrictEqual(missingBindings(object, state("running")), []);
    assert.deepStrictEqual(
      missingBindings(pump([]), state("stale")),
      ["running"],
      "stale needs a value: the first property of the symbol",
    );
  });
});
