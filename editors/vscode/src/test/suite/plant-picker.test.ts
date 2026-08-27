import * as assert from "assert";

import { groupForAddPicker } from "../../networkCanvas/webview/grouping";
import * as pickerPresentation from "../../networkCanvas/webview/grouping";

suite("Plant add picker", () => {
  test("places the simulated plant in its own Simulation group with process-model wording", () => {
    const groups = groupForAddPicker([
      { id: "simulated_plant", title: "Simulated plant", purpose: "" },
    ]);
    const simulation = groups.find((group) => group.label === "Simulation");

    assert.ok(simulation, "Simulation must be its own picker group");
    assert.deepStrictEqual(
      simulation.items.map((item) => ({ title: item.title, purpose: item.purpose })),
      [
        {
          title: "Simulated plant",
          purpose: "Models a process behind normal PLC I/O.",
        },
      ]
    );
  });

  test("keeps eligible plants actionable and explains why an ineligible runtime cannot accept one", () => {
    const api = pickerPresentation as unknown as {
      plantPickerPresentation?: (option: {
        eligible: boolean;
        disabledReason?: string;
      }) => { disabled: boolean; purpose: string };
    };

    assert.deepStrictEqual(api.plantPickerPresentation?.({ eligible: true }), {
      disabled: false,
      purpose: "Models a process behind normal PLC I/O.",
    });
    assert.deepStrictEqual(
      api.plantPickerPresentation?.({
        eligible: false,
        disabledReason: "This project is read-only.",
      }),
      {
        disabled: true,
        purpose: "Models a process behind normal PLC I/O. This project is read-only.",
      }
    );
  });

  test("creates one local unsaved plant draft for the chosen runtime", () => {
    let api:
      | {
          createPlantDraft?: (target: { id: string; name: string }) => unknown;
        }
      | undefined;
    try {
      // Dynamic loading lets the missing production owner reach an assertion
      // red instead of failing TypeScript module resolution.
      api = require("../../networkCanvas/plant/addFlow") as typeof api;
    } catch {
      api = undefined;
    }

    assert.deepStrictEqual(
      api?.createPlantDraft?.({ id: "runtime:local", name: "Simulator" }),
      {
        id: "draft:plant:runtime:local",
        plantId: "new_plant",
        runtimeId: "runtime:local",
        runtimeName: "Simulator",
        displayName: "New simulated plant",
        description: "",
        enabled: true,
        startingPoint: "temperature",
        model: "first_order_dead_time",
        parameters: {
          gain: 1,
          time_constant_s: 10,
          dead_time_s: 1,
          input_bias: 0,
          output_bias: 0,
          initial_delayed_input: 0,
        },
        initialState: [0],
        modelValuesTouched: false,
        commandSources: [],
        feedbackTargets: [],
        initialInput: [],
        inputScale: [],
        inputOffset: [],
        outputScale: [],
        outputOffset: [],
        scenarioEvents: [],
        dirty: true,
        persisted: false,
        focusSection: "plant",
      }
    );
  });

  test("accepts only the active versioned simulated-plant drag payload", () => {
    const api = require("../../networkCanvas/plant/addFlow") as {
      encodePlantDragPayload?: (dragSessionId: string) => string;
      decodePlantDragPayload?: (
        raw: string,
        activeDragSessionId: string
      ) => unknown;
    };
    const raw = api.encodePlantDragPayload?.("drag-17");

    assert.deepStrictEqual(raw && JSON.parse(raw), {
      schema_version: 1,
      kind: "simulated_plant",
      drag_session_id: "drag-17",
    });
    assert.deepStrictEqual(api.decodePlantDragPayload?.(raw ?? "", "drag-17"), {
      ok: true,
      dragSessionId: "drag-17",
    });
    assert.deepStrictEqual(api.decodePlantDragPayload?.("not-json", "drag-17"), {
      ok: false,
      code: "malformed_drag_payload",
    });
    assert.deepStrictEqual(
      api.decodePlantDragPayload?.(
        '{"schema_version":2,"kind":"simulated_plant","drag_session_id":"drag-17"}',
        "drag-17"
      ),
      { ok: false, code: "unsupported_drag_payload" }
    );
    assert.deepStrictEqual(
      api.decodePlantDragPayload?.(
        '{"schema_version":1,"kind":"simulated_plant","drag_session_id":"old"}',
        "drag-17"
      ),
      { ok: false, code: "stale_drag_payload" }
    );
    assert.deepStrictEqual(
      api.decodePlantDragPayload?.(
        '{"schema_version":1,"kind":"simulated_plant","drag_session_id":"drag-17","extra":true}',
        "drag-17"
      ),
      { ok: false, code: "malformed_drag_payload" }
    );
  });

  test("accepts drops only on eligible runtimes and returns safe explanations otherwise", () => {
    const api = require("../../networkCanvas/plant/addFlow") as {
      resolvePlantDrop?: (input: {
        raw: string;
        activeDragSessionId: string;
        targetRuntimeId?: string;
        eligibleRuntimeIds: string[];
      }) => unknown;
      plantDropPresentation?: (input: {
        runtimeId: string;
        eligible: boolean;
        activeTargetRuntimeId?: string;
        ineligibleReason?: string;
      }) => unknown;
    };
    const raw = JSON.stringify({
      schema_version: 1,
      kind: "simulated_plant",
      drag_session_id: "drag-17",
    });

    assert.deepStrictEqual(
      api.resolvePlantDrop?.({
        raw,
        activeDragSessionId: "drag-17",
        targetRuntimeId: "runtime:local",
        eligibleRuntimeIds: ["runtime:local"],
      }),
      { ok: true, runtimeId: "runtime:local" }
    );
    assert.deepStrictEqual(
      api.resolvePlantDrop?.({
        raw,
        activeDragSessionId: "drag-17",
        eligibleRuntimeIds: ["runtime:local"],
      }),
      {
        ok: false,
        code: "invalid_drop_target",
        message: "Drop the plant onto a runtime.",
      }
    );
    assert.deepStrictEqual(
      api.resolvePlantDrop?.({
        raw,
        activeDragSessionId: "drag-17",
        targetRuntimeId: "runtime:remote",
        eligibleRuntimeIds: ["runtime:local"],
      }),
      {
        ok: false,
        code: "ineligible_drop_target",
        message: "This runtime cannot accept a simulated plant.",
      }
    );
    assert.deepStrictEqual(
      api.resolvePlantDrop?.({
        raw: "not-json",
        activeDragSessionId: "drag-17",
        targetRuntimeId: "runtime:local",
        eligibleRuntimeIds: ["runtime:local"],
      }),
      {
        ok: false,
        code: "malformed_drag_payload",
        message: "Unsupported plant drag. Start again from Simulated plant.",
      }
    );
    assert.deepStrictEqual(
      api.plantDropPresentation?.({
        runtimeId: "runtime:local",
        eligible: true,
        activeTargetRuntimeId: "runtime:local",
      }),
      { state: "current", label: "Add simulated plant" }
    );
    assert.deepStrictEqual(
      api.plantDropPresentation?.({
        runtimeId: "runtime:remote",
        eligible: false,
        ineligibleReason: "Simulation is not supported by this runtime.",
      }),
      {
        state: "ineligible",
        label: "Simulation is not supported by this runtime.",
      }
    );
  });

  test("restores the prior node after cancel and falls back to the owning runtime", () => {
    const api = require("../../networkCanvas/plant/addFlow") as {
      plantDraftCancelTarget?: (input: {
        previousSelectedId?: string;
        runtimeId: string;
        availableNodeIds: string[];
      }) => string | undefined;
    };

    assert.strictEqual(
      api.plantDraftCancelTarget?.({
        previousSelectedId: "host:this-computer",
        runtimeId: "runtime:local",
        availableNodeIds: ["host:this-computer", "runtime:local"],
      }),
      "host:this-computer"
    );
    assert.strictEqual(
      api.plantDraftCancelTarget?.({
        previousSelectedId: "removed",
        runtimeId: "runtime:local",
        availableNodeIds: ["runtime:local"],
      }),
      "runtime:local"
    );
    assert.strictEqual(
      api.plantDraftCancelTarget?.({
        previousSelectedId: "removed",
        runtimeId: "removed-runtime",
        availableNodeIds: ["host:this-computer"],
      }),
      undefined
    );
  });
});
