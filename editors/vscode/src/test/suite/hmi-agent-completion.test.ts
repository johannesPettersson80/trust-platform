import { alarmScenario } from "../../hmiBuilder/agent/alarmScenario";
import { operationTimeout } from "../../hmiBuilder/agent/lifetime";
import { isLocalSimulator, executeSimulatorJourney, SimulatorJourney } from "../../hmiBuilder/agent/simulator";
import * as assert from "assert";
import * as vscode from "vscode";
import { languageServerReady } from "./hmiBuilderHost";
import { scenarioFrame } from "../../hmiBuilder/agent/scenarios";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import type { HmiPageModel } from "../../hmiBuilder/drawing/model";

const catalogue: Catalogue = {
  format: 1,
  components: [
    { type: "value", category: "display", phase: 1, size: [160, 64] },
    { type: "text", category: "display", phase: 1, size: [200, 40] },
    {
      type: "setpoint",
      category: "control",
      phase: 1,
      size: [200, 64],
      properties: { value: { accepts: ["number"], directions: ["read-write"] } },
    },
  ],
};
const page = (): HmiPageModel => ({
  title: "Settings",
  pipe: [],
  object: [
    {
      id: "sp",
      type: "setpoint",
      at: [32, 88],
      size: [200, 64],
      rotation: 0,
      min: 5,
      max: 95,
      bind: { value: { path: "Main.SP", direction: "read-write" } },
    },
  ],
});

suite("HMI completed authoring contracts", () => {
  test("synthetic alarm exploration uses labelled samples without executing PLC alarm sources", () => {
    const source = page();
    source.object[0].instance = "Main.P101";
    const sample = alarmScenario([], [source, source], true).summary!;
    assert.strictEqual(sample.active.length, 1, "repeated views share one preview row");
    assert.strictEqual(sample.active[0].path, "Main.P101.__preview_alarm");
    assert.strictEqual(sample.active[0].message, "Preview alarm — Main.P101");
    assert.strictEqual(sample.active[0].equipment, "Main.P101");
    assert.strictEqual(alarmScenario([], [source], false).summary!.active.length, 0);
    assert.strictEqual(alarmScenario([], [], true).summary!.active.length, 0);
    assert.strictEqual(
      alarmScenario([{ path: "Main.P101.Fault", message: "Declared alarm" }], [source], true).summary!.active[0]
        .message,
      "Declared alarm",
    );
  });
  test("cold composition and validation have the same discovery budget as context", () => {
    for (const name of ["hmi_compose_page", "hmi_edit_draft", "hmi_validate_draft"])
      assert.strictEqual(operationTimeout(name), operationTimeout("hmi_get_context"));
  });
  // The target rule is trust-lsp's (`trust-lsp.hmiAgent.checkTarget`, specification 43 AT-08), proven by the Rust tests
  // in crates/trust-lsp/src/handlers/commands/hmi_agent/target_tests.rs. This proves that the extension asks it.
  test("simulator journeys ask trust-lsp whether the target is a local simulator", async function () {
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await languageServerReady();
    const local = { simulator: true, endpoint: "tcp://127.0.0.1:9100", url: "http://127.0.0.1:8080", label: "Fixture" };
    assert.strictEqual(await isLocalSimulator(local), true);
    assert.strictEqual(await isLocalSimulator({ ...local, simulator: false, label: "Forwarded PLC" }), false);
    assert.strictEqual(await isLocalSimulator(undefined), false);
  });
  test("simulator journeys retain denied and expired-login outcomes and never replay commands", async () => {
    for (const status of [401, 403]) {
      let writes = 0;
      const owner: SimulatorJourney = {
        id: "session",
        name: "fixture",
        expires: Date.now() + 10000,
        busy: false,
        session: {
          isCurrentTarget: async () => true,
          request: async (method) =>
            method === "GET"
              ? { status: 200, body: { page: { model: page() }, bundle: "b" } }
              : (writes++, { status, body: { outcome: "refused", reason: "named login or policy refused" } }),
          openSocket: async () => ({ onmessage: null, onclose: null, close() {} }),
        },
      };
      const result = await executeSimulatorJourney(
        owner,
        [
          { page: "settings", object: "sp", action: "set", value: 10 },
          { page: "settings", object: "sp", action: "set", value: 20 },
        ],
        { settings: page() },
        new AbortController().signal,
      );
      assert.strictEqual(result.passed, false);
      assert.strictEqual(writes, 1);
      assert.strictEqual(owner.busy, false);
      assert.strictEqual((result.outcomes as Array<{ status: number }>)[0].status, status);
    }
  });
  test("simulator expiry and changed target stop before any read or command", async () => {
    for (const expired of [true, false]) {
      let requests = 0;
      const owner: SimulatorJourney = {
        id: "session",
        name: "fixture",
        expires: expired ? 0 : Date.now() + 10000,
        busy: false,
        session: {
          isCurrentTarget: async () => false,
          request: async () => {
            requests++;
            return { status: 200, body: {} };
          },
          openSocket: async () => undefined,
        },
      };
      const result = await executeSimulatorJourney(
        owner,
        [{ page: "settings", object: "sp" }],
        { settings: page() },
        new AbortController().signal,
      );
      assert.strictEqual(result.passed, false);
      assert.strictEqual(requests, 0);
      assert.strictEqual(owner.busy, false);
    }
  });
  test("simulator disconnect retains the pending command instead of reporting success", async () => {
    const stream: {
      onmessage: ((event: { data: unknown }) => void) | null;
      onclose: (() => void) | null;
      close(): void;
    } = { onmessage: null, onclose: null, close() {} };
    const owner: SimulatorJourney = {
      id: "session",
      name: "fixture",
      expires: Date.now() + 10000,
      busy: false,
      session: {
        isCurrentTarget: async () => true,
        openSocket: async () => stream,
        request: async (method) => {
          if (method === "GET") return { status: 200, body: { page: { model: page() }, bundle: "b" } };
          stream.onclose?.();
          return { status: 200, body: { outcome: "pending" } };
        },
      },
    };
    const result = await executeSimulatorJourney(
      owner,
      [{ page: "settings", object: "sp", action: "set", value: 10 }],
      { settings: page() },
      new AbortController().signal,
    );
    assert.strictEqual(result.passed, false);
    assert.deepStrictEqual(
      (result.outcomes as Array<{ outcome: string }>).map((item) => item.outcome),
      ["pending", "disconnected"],
    );
  });
  test("synthetic extremes and command outcomes preserve source values and have no runtime transport", () => {
    const source = page(),
      values = { "Main.SP": { value: 50, state: "confirmed" as const } };
    assert.strictEqual(scenarioFrame([source], values, "minimum", catalogue).values["Main.SP"].value, 5);
    assert.strictEqual(scenarioFrame([source], values, "maximum", catalogue).values["Main.SP"].value, 95);
    assert.strictEqual(scenarioFrame([source], values, "pending", catalogue).frame.commands?.sp.outcome, "pending");
    assert.strictEqual(scenarioFrame([source], values, "denied", catalogue).frame.commands?.sp.outcome, "refused");
    assert.strictEqual(scenarioFrame([source], values, "longest", catalogue).frame.longest, true);
    assert.strictEqual(values["Main.SP"].value, 50);
  });
});
