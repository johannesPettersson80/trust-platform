import { alarmScenario } from "../../hmiBuilder/agent/alarmScenario";
import { operationTimeout } from "../../hmiBuilder/agent/lifetime";
import { isLocalSimulator, executeSimulatorJourney, SimulatorJourney } from "../../hmiBuilder/agent/simulator";
import { controllerBelongsToEquipment } from "../../hmiBuilder/editor/controlStudioHandoff";
import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { composeAgentPage } from "../../hmiBuilder/agent/compose";
import { scenarioFrame } from "../../hmiBuilder/agent/scenarios";
import { reviewRecord } from "../../hmiBuilder/agent/visualReview";
import { DraftStore } from "../../hmiBuilder/agent/drafts";
import { recordSelection, forgetSelection, contextSelection } from "../../hmiBuilder/agent/queryMetadata";
import { validateToolInput } from "../../hmiBuilder/agent/schema";
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
  test("simulator journeys refuse physical mode and nonlocal endpoints without fallback", () => {
    assert.ok(
      isLocalSimulator({
        simulator: true,
        endpoint: "tcp://127.0.0.1:9100",
        url: "http://127.0.0.1:8080",
        label: "Fixture",
      }),
    );
    assert.ok(
      !isLocalSimulator({
        simulator: false,
        endpoint: "tcp://127.0.0.1:9100",
        url: "http://127.0.0.1:8080",
        label: "Forwarded PLC",
      }),
    );
    assert.ok(
      !isLocalSimulator({
        simulator: true,
        endpoint: "tcp://192.168.1.2:9100",
        url: "http://192.168.1.2:8080",
        label: "Remote",
      }),
    );
    validateToolInput("hmi_preview_states", {
      draftId: "draft",
      revision: "1",
      mode: "simulator",
      sessionId: "named",
      steps: [{ page: "overview", object: "sp", action: "write", value: 2, path: "Main.SP", expect: 2 }],
    });
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
  test("Control Studio handoff matches the actual equipment path boundary", () => {
    assert.ok(controllerBelongsToEquipment("Main.LIC101", "main.lic101"), "a loop's own Control.PID");
    assert.ok(controllerBelongsToEquipment("Main.LIC101.Controller", "main.lic101"));
    assert.ok(!controllerBelongsToEquipment("Main.LIC1010.Controller", "Main.LIC101"));
    assert.ok(!controllerBelongsToEquipment("Other.Controller", "Main.LIC101"));
  });
  test("padding and equal sizing retain readable controls within the reserved region", () => {
    const result = composeAgentPage(
      {
        title: "Settings",
        pattern: "settings",
        size: [1280, 800],
        layout: { padding: 24, equalSize: "both" },
        items: [
          { id: "a", type: "setpoint", size: [200, 64] },
          { id: "b", type: "setpoint", size: [240, 80] },
        ],
      },
      catalogue,
    );
    const controls = result.object.filter((object) => object.id !== "page-heading");
    assert.deepStrictEqual(
      controls.map((object) => object.size),
      [
        [240, 80],
        [240, 80],
      ],
    );
    assert.ok(controls.every((object) => object.at[0] >= 56 && object.at[1] >= 112));
    assert.ok(controls[1].at[1] >= controls[0].at[1] + controls[0].size[1] + 24);
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
  test("selection is scoped to a project and active panel and disappears on close", () => {
    const a = vscode.Uri.file(path.join(os.tmpdir(), "hmi-select-a")),
      b = vscode.Uri.file(path.join(os.tmpdir(), "hmi-select-b"));
    const owner = {};
    recordSelection(a, owner, { file: "hmi/pages/main.toml", ids: ["pump"], documentVersion: 3 }, true);
    assert.deepStrictEqual(contextSelection(a)?.ids, ["pump"]);
    assert.strictEqual(contextSelection(b), null);
    forgetSelection(a, owner);
    assert.strictEqual(contextSelection(a), null);
  });
  test("review issues escalate after three attempts and cannot masquerade as accepted", () => {
    const input = {
      reviewer: "operator",
      verdict: "needs-repair" as const,
      issues: [{ file: "hmi/pages/main.toml", objects: ["sp"], message: "Control overlaps the reading" }],
    };
    assert.strictEqual(reviewRecord(input, "3", 2).customerDecisionRequired, true);
    assert.throws(() => reviewRecord({ ...input, verdict: "accepted" }, "3", 2), /unresolved/);
  });
  test("editing invalidates the stored visual verdict but retains the bounded repair history", async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), "hmi-review-record-"));
    try {
      const store = new DraftStore(directory),
        draft = await store.create("workspace", [], { "hmi/pages/main.toml": "before" });
      await store.review(draft.id, draft.revision, [], {}, undefined, {
        reviewer: "operator",
        verdict: "needs-repair",
        issues: [],
      });
      const changed = await store.edit(draft.id, draft.revision, "edit", "fingerprint", async () => ({
        "hmi/pages/main.toml": "after",
      }));
      assert.strictEqual(changed.visualReview, undefined);
      assert.strictEqual(changed.repairAttempts, 1);
      assert.strictEqual((await new DraftStore(directory).get(draft.id)).repairAttempts, 1);
    } finally {
      await fs.rm(directory, { recursive: true, force: true });
    }
  });
  test("public schemas accept explicit multipage proposals and refuse incomplete draft references", () => {
    validateToolInput("hmi_compose_page", {
      key: "multi",
      pages: [
        { file: "hmi/pages/a.toml", title: "A", pattern: "settings", items: [] },
        { file: "hmi/pages/b.toml", sourceFile: "hmi/pages/main.toml" },
      ],
    });
    assert.throws(
      () => validateToolInput("hmi_inspect_page", { file: "hmi/pages/a.toml", draftId: "draft" }),
      /revision/,
    );
    validateToolInput("hmi_render_draft", { draftId: "draft", revision: "1", viewport: { width: 1280, height: 800 } });
    validateToolInput("hmi_apply_draft", { draftId: "draft", revision: "1", action: "apply", allowIncomplete: true });
  });
});
