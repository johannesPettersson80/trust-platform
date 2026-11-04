import { alarmScenario } from "../../hmiBuilder/agent/alarmScenario";
import { operationTimeout } from "../../hmiBuilder/agent/lifetime";
import {
  connectSimulator,
  endSimulatorSessions,
  isLocalSimulator,
  runSimulator,
} from "../../hmiBuilder/agent/simulator";
import * as assert from "assert";
import * as vscode from "vscode";
import { languageServerReady } from "./hmiBuilderHost";
import { scenarioFrame } from "../../hmiBuilder/agent/scenarios";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { LiveSessionDeps } from "../../hmiBuilder/editor/liveSession";

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

const WORKSPACE = "file:///projects/simulated";
const LOCAL = {
  simulator: true,
  endpoint: "tcp://127.0.0.1:9100",
  url: "http://127.0.0.1:8080",
  token: "access-token-7",
  label: "Fixture",
};
/** The target as trust-lsp receives it. */
const TARGET = { simulator: true, url: LOCAL.url, endpoint: LOCAL.endpoint, token: LOCAL.token };
const OPENED = {
  ok: true,
  data: { sessionId: "session-1", name: "Fixture", expiresAt: new Date(Date.now() + 600_000).toISOString() },
  revisions: {},
  warnings: [],
};

/** The HMI bridge of a local simulator whose login answers `status`, with the session cookie when it is 200. */
function simulatorBridge(status: number): LiveSessionDeps {
  return {
    target: async () => LOCAL,
    request: async (_target, _method, path) =>
      path === "/api/hmi/login" && status === 200
        ? { status, body: { ok: true }, cookie: "trust_hmi_session=4f1c0ffee; Path=/; HttpOnly; SameSite=Strict" }
        : { status, body: { ok: false } },
  };
}

/** Runs `body` with trust-lsp's simulator commands answered by `answers` (the target check accepts, a session opens),
 * recording each call in order; the sessions the extension holds end afterwards. */
async function withSimulatorCommands(
  answers: Record<string, (argument: Record<string, unknown>) => unknown>,
  body: (calls: Array<[string, Record<string, unknown>]>) => Promise<void>,
): Promise<void> {
  const executeCommand = vscode.commands.executeCommand;
  const calls: Array<[string, Record<string, unknown>]> = [];
  const answer: Record<string, (argument: Record<string, unknown>) => unknown> = {
    "trust-lsp.hmiAgent.checkTarget": () => ({ local: true }),
    "trust-lsp.hmiAgent.openSimulatorSession": () => OPENED,
    "trust-lsp.hmiAgent.closeSimulatorSession": () => ({ ok: true, data: { closed: true }, revisions: {}, warnings: [] }),
    ...answers,
  };
  (vscode.commands as any).executeCommand = async (command: string, ...rest: unknown[]) => {
    const argument = rest[0] as Record<string, unknown>;
    if (!(command in answer)) return executeCommand(command, ...rest);
    if (command !== "trust-lsp.hmiAgent.checkTarget") calls.push([command, argument]);
    return answer[command](argument);
  };
  try {
    await body(calls);
  } finally {
    (vscode.commands as any).executeCommand = executeCommand;
    endSimulatorSessions();
  }
}

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
  // The journey and the session rules are trust-lsp's (`trust-lsp.hmiAgent.openSimulatorSession`, `runSimulatorJourney`
  // and the rest, specification 43 AT-08), proven by the Rust tests in
  // crates/trust-lsp/src/handlers/commands/hmi_agent/simulator_*_tests.rs. These prove the wiring: the extension logs in
  // through its HMI bridge, keeps the session's cookie and hands it to trust-lsp with each journey of that session.
  test("a simulator journey runs in trust-lsp with its named session's cookie, and its answer is trust-lsp's", async () => {
    const journey = { origin: "simulator", sessionId: "session-1", name: "Fixture", outcomes: [], passed: true };
    const steps = [{ page: "settings", object: "sp", action: "set", value: 10, path: "Main.SP", expect: 10 }];
    const models = { settings: page() };
    await withSimulatorCommands(
      { "trust-lsp.hmiAgent.runSimulatorJourney": () => ({ ok: true, data: journey, revisions: {}, warnings: [] }) },
      async (calls) => {
        const session = await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), async () => ({
          user: "operator",
          password: "secret",
        }));
        assert.deepStrictEqual(session, OPENED.data);
        assert.deepStrictEqual(calls[0], [
          "trust-lsp.hmiAgent.openSimulatorSession",
          { workspace: WORKSPACE, name: "Fixture", target: TARGET },
        ]);
        const answer = await runSimulator(WORKSPACE, "session-1", steps, models, new AbortController().signal);
        assert.deepStrictEqual(answer, journey);
        assert.deepStrictEqual(
          calls.find(([command]) => command === "trust-lsp.hmiAgent.runSimulatorJourney")?.[1],
          { workspace: WORKSPACE, sessionId: "session-1", cookie: "trust_hmi_session=4f1c0ffee", target: TARGET, steps, models },
        );
      },
    );
  });
  test("a refused or abandoned simulator login closes the session in trust-lsp, and no cookie goes out", async () => {
    await withSimulatorCommands(
      {
        "trust-lsp.hmiAgent.runSimulatorJourney": () => ({
          ok: false,
          code: "EXPIRED_DRAFT",
          message: "select an unexpired named simulator session in this workspace.",
          retryable: false,
        }),
      },
      async (calls) => {
        await assert.rejects(
          connectSimulator(WORKSPACE, "Fixture", simulatorBridge(401), async () => ({ user: "operator", password: "x" })),
          /The simulator refused the named HMI login\./,
        );
        assert.strictEqual(await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), async () => undefined), undefined);
        const closed = calls.filter(([command]) => command === "trust-lsp.hmiAgent.closeSimulatorSession");
        assert.deepStrictEqual(closed, [
          ["trust-lsp.hmiAgent.closeSimulatorSession", { sessionId: "session-1" }],
          ["trust-lsp.hmiAgent.closeSimulatorSession", { sessionId: "session-1" }],
        ]);
        await assert.rejects(
          runSimulator(WORKSPACE, "session-1", [{ page: "settings" }], {}, new AbortController().signal),
          /^Error: EXPIRED_DRAFT: select an unexpired named simulator session in this workspace\.$/,
        );
        const run = calls.find(([command]) => command === "trust-lsp.hmiAgent.runSimulatorJourney");
        assert.strictEqual(run?.[1].cookie, null);
        assert.strictEqual(run?.[1].target, null);
      },
    );
  });
  test("cancelling a simulator journey cancels it in trust-lsp and still returns its completed outcomes", async () => {
    const cancelled = {
      origin: "simulator",
      sessionId: "session-1",
      name: "Fixture",
      outcomes: [{ page: "settings", outcome: "cancelled" }],
      passed: false,
    };
    let finish: (() => void) | undefined;
    await withSimulatorCommands(
      {
        "trust-lsp.hmiAgent.runSimulatorJourney": () =>
          new Promise((resolve) => {
            finish = () => resolve({ ok: true, data: cancelled, revisions: {}, warnings: [] });
          }),
        "trust-lsp.hmiAgent.cancelSimulatorJourney": () => {
          finish?.();
          return { ok: true, data: { cancelled: true }, revisions: {}, warnings: [] };
        },
      },
      async (calls) => {
        await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), async () => ({ user: "operator", password: "x" }));
        const controller = new AbortController();
        const running = runSimulator(WORKSPACE, "session-1", [{ page: "settings" }], {}, controller.signal);
        for (let tries = 0; !finish && tries < 200; tries++) await new Promise((resolve) => setTimeout(resolve, 5));
        controller.abort();
        assert.deepStrictEqual(await running, cancelled);
        assert.deepStrictEqual(
          calls.find(([command]) => command === "trust-lsp.hmiAgent.cancelSimulatorJourney")?.[1],
          { sessionId: "session-1" },
        );
      },
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
