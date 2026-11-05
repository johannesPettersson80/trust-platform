import { alarmScenario } from "../../hmiBuilder/agent/alarmScenario";
import { operationTimeout } from "../../hmiBuilder/agent/lifetime";
import {
  connectSimulator,
  endSimulatorSessions,
  registerSimulatorSessions,
  runSimulator,
  setSimulatorLanguageClient,
} from "../../hmiBuilder/agent/simulator";
import * as assert from "assert";
import * as vscode from "vscode";
import { State } from "vscode-languageclient/node";
import type { LanguageClient } from "vscode-languageclient/node";
import { getSelectedRuntimeId, setSelectedRuntimeId } from "../../selectedRuntime";
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
const TARGET = { simulator: true, url: LOCAL.url, endpoint: LOCAL.endpoint, authToken: LOCAL.token };
const OPENED = {
  ok: true,
  data: { sessionId: "session-1", name: "Fixture", expiresAt: new Date(Date.now() + 600_000).toISOString() },
  revisions: {},
  warnings: [],
};
const STARTED = { ok: true, data: { started: true, sessionId: "session-1" }, revisions: {}, warnings: [] };
const success = (data: unknown) => ({ ok: true, data, revisions: {}, warnings: [] });

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

/** A language client that only delivers the notifications and state changes a test sends through it. */
class FakeClient {
  handlers = new Map<string, (params: unknown) => void>();
  stateListeners: Array<(change: { oldState: State; newState: State }) => void> = [];
  onNotification(method: string, handler: (params: unknown) => void): vscode.Disposable {
    this.handlers.set(method, handler);
    return new vscode.Disposable(() => this.handlers.delete(method));
  }
  onDidChangeState(listener: (change: { oldState: State; newState: State }) => void): vscode.Disposable {
    this.stateListeners.push(listener);
    return new vscode.Disposable(() => undefined);
  }
  /** Sends the notification a journey's end sends. */
  journeyEnded(answer: unknown): void {
    this.handlers.get("trust-lsp/simulatorJourneyEnded")?.(answer);
  }
  stop(): void {
    for (const listener of this.stateListeners) listener({ oldState: State.Running, newState: State.Stopped });
  }
}

/** Runs `body` with trust-lsp's simulator commands answered by `answers` (a session opens, its login is admitted, a
 * journey starts), recording each call in order, and with `client` as the language client; the sessions the extension
 * holds end afterwards. */
async function withSimulatorCommands(
  answers: Record<string, (argument: Record<string, unknown>) => unknown>,
  body: (calls: Array<[string, Record<string, unknown>]>, client: FakeClient) => Promise<void>,
): Promise<void> {
  const executeCommand = vscode.commands.executeCommand;
  const calls: Array<[string, Record<string, unknown>]> = [];
  const answer: Record<string, (argument: Record<string, unknown>) => unknown> = {
    "trust-lsp.hmiAgent.openSimulatorSession": () => OPENED,
    "trust-lsp.hmiAgent.admitSimulatorLogin": () => success({ admitted: true }),
    "trust-lsp.hmiAgent.closeSimulatorSession": () => success({ closed: true }),
    "trust-lsp.hmiAgent.runSimulatorJourney": () => STARTED,
    "trust-lsp.hmiAgent.cancelSimulatorJourney": () => success({ cancelled: true }),
    ...answers,
  };
  const client = new FakeClient();
  const restore = setSimulatorLanguageClient(() => client as unknown as LanguageClient);
  (vscode.commands as any).executeCommand = async (command: string, ...rest: unknown[]) => {
    const argument = rest[0] as Record<string, unknown>;
    if (!(command in answer)) return executeCommand(command, ...rest);
    calls.push([command, argument]);
    return answer[command](argument);
  };
  try {
    await body(calls, client);
  } finally {
    (vscode.commands as any).executeCommand = executeCommand;
    restore();
    endSimulatorSessions();
  }
}

/** Waits until `ready` holds, for at most a second. */
async function until(ready: () => boolean): Promise<void> {
  for (let tries = 0; !ready() && tries < 200; tries++) await new Promise((resolve) => setTimeout(resolve, 5));
}

/** The journey ID the extension gave the journey it started, once it started one. */
async function startedJourney(calls: Array<[string, Record<string, unknown>]>): Promise<string> {
  await until(() => calls.some(([command]) => command === "trust-lsp.hmiAgent.runSimulatorJourney"));
  const started = calls.find(([command]) => command === "trust-lsp.hmiAgent.runSimulatorJourney")?.[1];
  assert.strictEqual(typeof started?.journeyId, "string", "the start names its journey");
  return started?.journeyId as string;
}

const login = async () => ({ user: "operator", password: "secret" });

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
  // The journey and the session rules are trust-lsp's (`trust-lsp.hmiAgent.openSimulatorSession`, `runSimulatorJourney`
  // and the rest, specification 43 AT-08), proven by the Rust tests in
  // crates/trust-lsp/src/handlers/commands/hmi_agent/simulator_*_tests.rs and crates/trust-lsp/tests/
  // simulator_journey_transport.rs. These prove the wiring: the extension logs in through its HMI bridge to a target
  // trust-lsp admits, keeps the session's cookie, hands it to trust-lsp with each journey of that session, and waits for
  // the journey's end as the notification trust-lsp/simulatorJourneyEnded.
  test("a simulator journey starts in trust-lsp with its named session's cookie, and its answer is the end's notification", async () => {
    const steps = [{ page: "settings", object: "sp", action: "write", value: 10, path: "Main.SP", expect: 10 }];
    const models = { settings: page() };
    await withSimulatorCommands({}, async (calls, client) => {
      const session = await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), login);
      assert.deepStrictEqual(session, OPENED.data);
      assert.deepStrictEqual(calls.slice(0, 2), [
        ["trust-lsp.hmiAgent.openSimulatorSession", { workspace: WORKSPACE, name: "Fixture", target: TARGET }],
        ["trust-lsp.hmiAgent.admitSimulatorLogin", { sessionId: "session-1", target: TARGET }],
      ]);
      const running = runSimulator(WORKSPACE, "session-1", steps, models, new AbortController().signal);
      const journeyId = await startedJourney(calls);
      const journey = { origin: "simulator", sessionId: "session-1", journeyId, name: "Fixture", outcomes: [], passed: true };
      client.journeyEnded({ ...journey, journeyId: "an-earlier-journey-of-the-session", passed: false });
      client.journeyEnded(journey);
      assert.deepStrictEqual(await running, journey, "the answer of its own journey, not another of the session");
      assert.deepStrictEqual(
        calls.find(([command]) => command === "trust-lsp.hmiAgent.runSimulatorJourney")?.[1],
        {
          workspace: WORKSPACE,
          sessionId: "session-1",
          journeyId,
          cookie: "trust_hmi_session=4f1c0ffee",
          target: TARGET,
          steps,
          models,
        },
      );
    });
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
          connectSimulator(WORKSPACE, "Fixture", simulatorBridge(401), login),
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
  test("a login trust-lsp does not admit is never sent to the target", async () => {
    let logins = 0;
    const bridge: LiveSessionDeps = {
      target: async () => LOCAL,
      request: async () => {
        logins += 1;
        return { status: 200, body: { ok: true }, cookie: "trust_hmi_session=4f1c0ffee" };
      },
    };
    await withSimulatorCommands(
      { "trust-lsp.hmiAgent.admitSimulatorLogin": () => success({ admitted: false }) },
      async (calls) => {
        await assert.rejects(connectSimulator(WORKSPACE, "Fixture", bridge, login));
        assert.strictEqual(logins, 0, "the credentials never left");
        assert.ok(calls.some(([command]) => command === "trust-lsp.hmiAgent.closeSimulatorSession"));
      },
    );
  });
  test("cancelling a simulator journey cancels it in trust-lsp and still returns its completed outcomes", async () => {
    let client: FakeClient | undefined;
    let cancelled: Record<string, unknown> | undefined;
    await withSimulatorCommands(
      {
        "trust-lsp.hmiAgent.cancelSimulatorJourney": (argument) => {
          cancelled = {
            origin: "simulator",
            sessionId: "session-1",
            journeyId: argument.journeyId,
            name: "Fixture",
            outcomes: [{ page: "settings", outcome: "cancelled" }],
            passed: false,
          };
          setTimeout(() => client?.journeyEnded(cancelled), 0);
          return success({ cancelled: true });
        },
      },
      async (calls, fake) => {
        client = fake;
        await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), login);
        const controller = new AbortController();
        const running = runSimulator(WORKSPACE, "session-1", [{ page: "settings" }], {}, controller.signal);
        const journeyId = await startedJourney(calls);
        controller.abort();
        assert.deepStrictEqual(await running, cancelled);
        assert.deepStrictEqual(
          calls.find(([command]) => command === "trust-lsp.hmiAgent.cancelSimulatorJourney")?.[1],
          { sessionId: "session-1", journeyId },
          "the cancel names the journey",
        );
      },
    );
  });
  test("a cancelled journey whose answer does not come ends its wait 2 seconds after the cancel", async function () {
    this.timeout(10000);
    await withSimulatorCommands({}, async (calls) => {
      await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), login);
      const controller = new AbortController();
      const running = runSimulator(WORKSPACE, "session-1", [{ page: "settings" }], {}, controller.signal);
      await startedJourney(calls);
      const aborted = Date.now();
      controller.abort();
      await assert.rejects(running, /^Error: CANCELLED: the simulator journey was cancelled; its answer did not arrive in time\.$/);
      const waited = Date.now() - aborted;
      assert.ok(waited >= 1900 && waited < 4000, `ended ${waited} ms after the cancel`);
    });
  });
  test("a journey cancelled before its start goes out starts nothing and cancels nothing", async () => {
    let abortOnTarget: AbortController | undefined;
    const bridge: LiveSessionDeps = {
      ...simulatorBridge(200),
      target: async () => {
        abortOnTarget?.abort();
        return LOCAL;
      },
    };
    await withSimulatorCommands({}, async (calls) => {
      await connectSimulator(WORKSPACE, "Fixture", bridge, login);
      const controller = new AbortController();
      abortOnTarget = controller;
      await assert.rejects(
        runSimulator(WORKSPACE, "session-1", [{ page: "settings" }], {}, controller.signal),
        vscode.CancellationError,
      );
      const sent = calls.map(([command]) => command);
      assert.ok(!sent.includes("trust-lsp.hmiAgent.runSimulatorJourney"), "no journey started");
      assert.ok(!sent.includes("trust-lsp.hmiAgent.cancelSimulatorJourney"), "nothing to cancel");
    });
  });
  test("a journey whose language server stops ends with NOT_CONNECTED instead of waiting forever", async () => {
    await withSimulatorCommands({}, async (calls, client) => {
      await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), login);
      const running = runSimulator(WORKSPACE, "session-1", [{ page: "settings" }], {}, new AbortController().signal);
      await until(() => calls.some(([command]) => command === "trust-lsp.hmiAgent.runSimulatorJourney"));
      client.stop();
      await assert.rejects(running, /^Error: NOT_CONNECTED: the truST language server stopped\.$/);
    });
  });
  test("a verbose language server trace refuses the journey before its cookie leaves", async () => {
    const getConfiguration = vscode.workspace.getConfiguration;
    // `trust.languageServer.trace` at `verbose`, answered through a wrapper of VS Code's frozen configuration object.
    // Writing the real setting would turn the language client's trace on for the rest of the suite (CI, 4 November
    // 2026: this test then exceeded its 2 seconds while vscode-jsonrpc traced every message).
    (vscode.workspace as any).getConfiguration = (section?: string, scope?: vscode.ConfigurationScope) => {
      const real = getConfiguration(section, scope);
      if (section !== "trust") return real;
      const verbose = (key: string): boolean => key === "languageServer.trace";
      const wrapper = {
        has: (key: string) => verbose(key) || real.has(key),
        get: (key: string, fallback?: unknown) => (verbose(key) ? "verbose" : real.get(key, fallback)),
        inspect: (key: string) => (verbose(key) ? { key, globalValue: "verbose" } : real.inspect(key)),
        update: (key: string, value: unknown, target?: vscode.ConfigurationTarget | boolean | null) =>
          real.update(key, value, target),
      };
      return wrapper as unknown as vscode.WorkspaceConfiguration;
    };
    try {
      await withSimulatorCommands({}, async (calls) => {
        await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), login);
        await assert.rejects(
          runSimulator(WORKSPACE, "session-1", [{ page: "settings" }], {}, new AbortController().signal),
          /^Error: UNSUPPORTED_CONTEXT: the language server trace logs each message's parameters, which would log the named session's cookie/,
        );
        assert.ok(!calls.some(([command]) => command === "trust-lsp.hmiAgent.runSimulatorJourney"));
      });
    } finally {
      (vscode.workspace as any).getConfiguration = getConfiguration;
    }
  });
  test("a JSON language server trace refuses the journey before its cookie leaves", async () => {
    const getConfiguration = vscode.workspace.getConfiguration;
    // The client's own setting, `trust-lsp.trace.server`, which VS Code does not let a test write; VS Code's
    // configuration object is frozen, so the test answers through a wrapper of it.
    (vscode.workspace as any).getConfiguration = (section?: string, scope?: vscode.ConfigurationScope) => {
      const real = getConfiguration(section, scope);
      if (section !== "trust-lsp") return real;
      const wrapper = {
        has: (key: string) => real.has(key),
        get: (key: string, fallback?: unknown) =>
          key === "trace.server" ? { verbosity: "off", format: "json" } : real.get(key, fallback),
        inspect: (key: string) => real.inspect(key),
        update: (key: string, value: unknown, target?: vscode.ConfigurationTarget | boolean | null) =>
          real.update(key, value, target),
      };
      return wrapper as unknown as vscode.WorkspaceConfiguration;
    };
    try {
      await withSimulatorCommands({}, async (calls) => {
        await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), login);
        await assert.rejects(
          runSimulator(WORKSPACE, "session-1", [{ page: "settings" }], {}, new AbortController().signal),
          /^Error: UNSUPPORTED_CONTEXT: the language server trace logs each message's parameters/,
        );
        assert.ok(!calls.some(([command]) => command === "trust-lsp.hmiAgent.runSimulatorJourney"));
      });
    } finally {
      (vscode.workspace as any).getConfiguration = getConfiguration;
    }
  });
  test("a change of the selected run target closes the simulator sessions in trust-lsp", async function () {
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const selected = getSelectedRuntimeId();
    const registerCommand = vscode.commands.registerCommand;
    const subscriptions: vscode.Disposable[] = [];
    (vscode.commands as any).registerCommand = () => new vscode.Disposable(() => undefined);
    try {
      await withSimulatorCommands({}, async (calls, client) => {
        registerSimulatorSessions({ subscriptions } as unknown as vscode.ExtensionContext, () => client as never);
        await connectSimulator(WORKSPACE, "Fixture", simulatorBridge(200), login);
        await setSelectedRuntimeId(`${selected}-changed`);
        await until(() => calls.some(([command]) => command === "trust-lsp.hmiAgent.closeSimulatorSession"));
        assert.ok(
          calls.some(
            ([command, argument]) =>
              command === "trust-lsp.hmiAgent.closeSimulatorSession" && argument.sessionId === "session-1",
          ),
          "the session closes in trust-lsp",
        );
      });
    } finally {
      (vscode.commands as any).registerCommand = registerCommand;
      subscriptions.forEach((subscription) => subscription.dispose());
      await setSelectedRuntimeId(selected);
    }
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
