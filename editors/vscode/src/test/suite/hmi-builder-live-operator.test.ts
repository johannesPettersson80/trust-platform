import * as assert from "assert";
import { LiveOperator } from "../../hmiBuilder/editor/liveOperator";
import type { LiveOperationVerdict, LiveOperatorDeps } from "../../hmiBuilder/editor/liveOperator";
import type { LiveSessionDeps } from "../../hmiBuilder/editor/liveSession";
import { liveBridge, liveGate, liveOperation } from "../../hmiBuilder/editor/liveOperatorRuntime";

/** A project with no unsaved document, and one with an unsaved page of its HMI. */
const ROOT = "/work/plant";
const SAVED = { root: ROOT, unsaved: [] as string[] };
const EDITED = { root: ROOT, unsaved: ["/work/plant/hmi/pages/overview.toml"] };
/** No change of the HMI's files while a test operates. */
const NO_EDITS: LiveOperatorDeps["watchEdits"] = async () => ({ dispose: () => undefined });
import type { HostToWebview } from "../../hmiBuilder/editor/messages";

/** The first message that `find` picks, waiting up to five seconds for it. */
async function posted<T>(find: () => T | undefined): Promise<T> {
  const deadline = Date.now() + 5_000;
  for (;;) {
    const found = find();
    if (found !== undefined) return found;
    assert.ok(Date.now() < deadline, "the message did not come");
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

suite("HMI Builder: deployed Live connection", () => {
  test("a saved matching bundle opens the selected runtime and routes only the active editor connection", async () => {
    const messages: HostToWebview[] = [];
    const paths: string[] = [];
    const operator = new LiveOperator(
      {
        unsaved: () => SAVED,
        bundle: async () => ({ ok: true, version: "0123456789abcdef".repeat(4) }),
        runTarget: async () => ({ label: "Cell runtime", simulator: false }),
        liveGate,
        liveOperation,
        watchEdits: NO_EDITS,
        session: {
          bridge: liveBridge,
          target: async () => ({ endpoint: "tcp://plc:9000", url: "http://plc:8080", label: "Cell runtime" }),
          request: async (_target, _method, path) => {
            paths.push(path);
            return { status: 200, body: { bundle: "0123456789abcdef", page: { name: "overview" } } };
          },
        },
      },
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    assert.deepStrictEqual(messages[0], { type: "operateReady", id: "one", target: "Cell runtime" });
    await operator.handle({ type: "operateRequest", id: "one", request: 1, method: "GET", path: "/api/hmi/session" });
    assert.ok(messages.some((message) => message.type === "operateReply" && message.request === 1));
    await operator.handle({ type: "operateClose", id: "one" });
    await operator.handle({ type: "operateRequest", id: "one", request: 2, method: "GET", path: "/api/hmi/session" });
    assert.deepStrictEqual(paths, ["/api/hmi/page?name=overview", "/api/hmi/session"]);
  });

  test("unsaved or different project bundles stay read-only with an explanation", async () => {
    for (const dirty of [true, false]) {
      const messages: HostToWebview[] = [];
      let reads = 0;
      const operator = new LiveOperator(
        {
          unsaved: () => (dirty ? EDITED : SAVED),
          bundle: async () => ({ ok: true, version: "local" }),
          runTarget: async () => ({ label: "Cell runtime", simulator: false }),
          liveGate,
          liveOperation,
          watchEdits: NO_EDITS,
          session: {
            bridge: liveBridge,
            target: async () => ({ endpoint: "tcp://plc:9000", url: "http://plc:8080", label: "Cell runtime" }),
            request: async () => {
              reads++;
              return { status: 200, body: { bundle: "deployed", page: { name: "overview" } } };
            },
          },
        },
        (message) => messages.push(message),
      );
      await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
      const reply = messages[0];
      assert.ok(reply.type === "operateReady" && reply.reason);
      assert.match(reply.reason, dirty ? /save/i : /deploy/i);
      assert.strictEqual(reads, dirty ? 0 : 1);
    }
  });

  test("live read-only explanation names the simulator", async () => {
    // §11 item 6: a run target without an HMI web server stays read-only, and the explanation
    // names it. Caught: the simulator told to enable [runtime.web], which only a runtime has.
    const explanation = async (selected: { label: string; simulator: boolean }): Promise<string> => {
      const messages: HostToWebview[] = [];
      const operator = new LiveOperator(
        {
          unsaved: () => SAVED,
          bundle: async () => ({ ok: true, version: "v1" }),
          runTarget: async () => selected,
          liveGate,
          liveOperation,
          watchEdits: NO_EDITS,
          session: {
            bridge: liveBridge,
            target: async () => undefined,
            request: async () => assert.fail("a target without an HMI web server is never asked for the page"),
          },
        },
        (message) => messages.push(message),
      );
      await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
      const reply = messages[0];
      assert.ok(reply.type === "operateReady" && reply.reason, "Live mode stays read-only with an explanation");
      return reply.reason;
    };
    const simulator = await explanation({ label: "Simulator", simulator: true });
    assert.match(simulator, /^The simulator has no HMI web server\./);
    assert.match(simulator, /with its alarms, on a runtime/);
    assert.match(simulator, /Devices & Connections/);
    assert.doesNotMatch(simulator, /\[runtime\.web\]/);
    const runtime = await explanation({ label: "Cell runtime", simulator: false });
    assert.match(runtime, /^Cell runtime has no HMI web server\. Enable \[runtime\.web\]/);
  });

  test("a selected-target change ends operation before any new command", async () => {
    const messages: HostToWebview[] = [];
    let endpoint = "tcp://first:9000";
    let requests = 0;
    const operator = new LiveOperator(
      {
        unsaved: () => SAVED,
        bundle: async () => ({ ok: true, version: "v1" }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        watchEdits: NO_EDITS,
        session: {
          bridge: liveBridge,
          target: async () => ({ endpoint, url: "http://plc:8080", label: "Cell" }),
          request: async () => {
            requests++;
            return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
          },
        },
      },
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    endpoint = "tcp://second:9000";
    await operator.checkTarget();
    assert.ok(
      messages.some((message) => message.type === "operateEnded"),
      "a changed target disables the old operating view immediately",
    );
    assert.strictEqual(requests, 1, "checking the selected target needs no HMI write");
  });

  test("a different deployed bundle ends the editor operating view on navigation or reload", async () => {
    const messages: HostToWebview[] = [];
    let version = "v1";
    const operator = new LiveOperator(
      {
        unsaved: () => SAVED,
        bundle: async () => ({ ok: true, version: "v1" }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        watchEdits: NO_EDITS,
        session: {
          bridge: liveBridge,
          target: async () => ({ endpoint: "tcp://plc:9000", url: "http://plc:8080", label: "Cell" }),
          request: async () => ({ status: 200, body: { bundle: version, page: { name: "overview" } } }),
        },
      },
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    version = "v2";
    await operator.handle({
      type: "operateRequest",
      id: "one",
      request: 1,
      method: "GET",
      path: "/api/hmi/page?name=overview",
    });
    assert.ok(
      messages.some(
        (message) =>
          message.type === "operateEnded" &&
          message.reason === "The deployed HMI changed. Check and deploy the matching project before operating again.",
      ),
      "the editor cannot operate another bundle after reload",
    );
    assert.ok(
      !messages.some((message) => message.type === "operateReply" && message.status === 200),
      "the new page is not admitted to the operating view",
    );
  });

  test("requests cannot overtake the saved/deployed bundle check", async () => {
    let resolve!: (bundle: { ok: boolean; version: string }) => void;
    let reading!: () => void;
    const bundleRead = new Promise<void>((done) => {
      reading = done;
    });
    let requests = 0;
    const operator = new LiveOperator(
      {
        unsaved: () => SAVED,
        bundle: () =>
          new Promise((done) => {
            resolve = done;
            reading();
          }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        watchEdits: NO_EDITS,
        session: {
          bridge: liveBridge,
          target: async () => ({ endpoint: "tcp://plc:9000", url: "http://plc:8080", label: "Cell" }),
          request: async () => {
            requests++;
            return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
          },
        },
      },
      () => undefined,
    );
    const opening = operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    // The request comes while the bundle check is pending, however many steps opening takes before it.
    await bundleRead;
    await operator.handle({ type: "operateRequest", id: "one", request: 1, method: "GET", path: "/api/hmi/session" });
    const premature = requests;
    resolve({ ok: true, version: "v1" });
    await opening;
    assert.strictEqual(premature, 0, "the host does not forward requests before the bundle is admitted");
  });
  test("a 409 answer is shown, and then operation ends without resending", async () => {
    // Catches a 409 hidden from the operating view, or operation that goes on after it: the named session's target
    // changed or is gone (specification 39, section 11 item 6). The language server decides and words it.
    const messages: HostToWebview[] = [];
    const operator = new LiveOperator(
      {
        unsaved: () => SAVED,
        bundle: async () => ({ ok: true, version: "v1" }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        watchEdits: NO_EDITS,
        session: {
          bridge: liveBridge,
          target: async () => ({ endpoint: "tcp://plc:9000", url: "http://plc:8080", label: "Cell" }),
          request: async (_target, _method, path) =>
            path === "/api/hmi/session"
              ? { status: 409, body: { error: "the session's target changed" } }
              : { status: 200, body: { bundle: "v1", page: { name: "overview" } } },
        },
      },
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    await operator.handle({ type: "operateRequest", id: "one", request: 7, method: "GET", path: "/api/hmi/session" });
    const reply = messages.findIndex((message) => message.type === "operateReply" && message.request === 7);
    const ended = messages.findIndex((message) => message.type === "operateEnded");
    assert.ok(reply >= 0, "the 409 is shown");
    assert.ok(ended > reply, "and then operation ends");
    const end = messages[ended];
    assert.ok(end.type === "operateEnded");
    assert.strictEqual(
      end.reason,
      "The run target changed or disconnected. Any unfinished command has an unknown outcome; none will be resent.",
    );
  });

  /** Live operation dependencies for a deployed bundle `v1` at `endpoint()`, with `changes` over them. */
  function deps(
    changes: Partial<LiveOperatorDeps> & { request?: LiveSessionDeps["request"]; openSocket?: LiveSessionDeps["openSocket"] },
    endpoint: () => string = () => "tcp://plc:9000",
  ): LiveOperatorDeps {
    const { request, openSocket, ...rest } = changes;
    return {
      unsaved: () => SAVED,
      bundle: async () => ({ ok: true, version: "v1" }),
      runTarget: async () => ({ label: "Cell", simulator: false }),
      liveGate,
      liveOperation,
      watchEdits: NO_EDITS,
      session: {
        bridge: liveBridge,
        target: async () => ({ endpoint: endpoint(), url: "http://plc:8080", label: "Cell" }),
        request: request ?? (async () => ({ status: 200, body: { bundle: "v1", page: { name: "overview" } } })),
        openSocket,
      },
      ...rest,
    };
  }

  const ended = (messages: HostToWebview[]) => () => {
    const end = messages.find((message) => message.type === "operateEnded");
    return end?.type === "operateEnded" ? end.reason : undefined;
  };

  test("an unreachable runtime stops operation before the language server words why", async () => {
    // Catches an ending that waits for the language server's reason before it stops operation: a request handled while
    // the reason is pending must not go out (specification 39, section 11 item 6).
    const messages: HostToWebview[] = [];
    const paths: string[] = [];
    let release!: () => void;
    const held = new Promise<void>((done) => {
      release = done;
    });
    const operator = new LiveOperator(
      deps({
        liveOperation: async (event) => {
          if (event.event === "unreachable") await held;
          return liveOperation(event);
        },
        request: async (_target, _method, path) => {
          paths.push(path);
          if (path === "/api/hmi/session") throw new Error("connection refused");
          return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
        },
      }),
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    const failing = operator.handle({ type: "operateRequest", id: "one", request: 1, method: "GET", path: "/api/hmi/session" });
    await posted(() => (paths.includes("/api/hmi/session") ? true : undefined));
    await operator.handle({ type: "operateRequest", id: "one", request: 2, method: "GET", path: "/api/hmi/alarms" });
    assert.deepStrictEqual(paths, ["/api/hmi/page?name=overview", "/api/hmi/session"], "nothing goes out while the reason is pending");
    assert.strictEqual(ended(messages)(), undefined, "the reason is still pending");
    release();
    await failing;
    assert.strictEqual(
      ended(messages)(),
      "The runtime cannot be reached. Any unfinished command has an unknown outcome; none will be resent.",
    );
  });

  test("a target change that closes the value stream ends with the target's reason", async () => {
    // Catches the stream's reason shown when the selected run target changed: the session closes its streams when it
    // finds another target, so the operator must say why it stopped (specification 39, section 11 item 6).
    const messages: HostToWebview[] = [];
    let endpoint = "tcp://first:9000";
    const operator = new LiveOperator(
      deps({ openSocket: () => ({ onmessage: null, onclose: null, close: () => undefined }) }, () => endpoint),
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    await operator.handle({ type: "operateSocket", id: "one", socket: 1, path: "/ws/hmi?page=overview" });
    endpoint = "tcp://second:9000";
    await operator.checkTarget();
    assert.strictEqual(
      await posted(ended(messages)),
      "The selected run target changed. Reopen Live mode to connect.",
    );
  });

  test("unsaved changes end operation before a request goes out", async () => {
    // Catches a request sent from an HMI that is no longer the deployed one: the editor's unsaved state must reach the
    // language server's verdict (specification 39, section 11 item 6).
    const messages: HostToWebview[] = [];
    const paths: string[] = [];
    let dirty = false;
    const operator = new LiveOperator(
      deps({
        unsaved: () => (dirty ? EDITED : SAVED),
        request: async (_target, _method, path) => {
          paths.push(path);
          return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
        },
      }),
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    dirty = true;
    await operator.handle({ type: "operateRequest", id: "one", request: 1, method: "GET", path: "/api/hmi/session" });
    assert.deepStrictEqual(paths, ["/api/hmi/page?name=overview"], "the request does not go out");
    assert.strictEqual(ended(messages)(), "The HMI changed. Save and deploy it before operating again.");
  });

  test("an edit while operating ends operation at once, with the language server's reason", async () => {
    // Catches operation that goes on after an HMI file changed, and an edit that ends it with words of the extension's
    // own (specification 39, section 11 item 6).
    const messages: HostToWebview[] = [];
    const paths: string[] = [];
    let edit = (): void => assert.fail("the operator watches the HMI's files");
    const operator = new LiveOperator(
      deps({
        watchEdits: async (edited) => {
          edit = edited;
          return { dispose: () => undefined };
        },
        request: async (_target, _method, path) => {
          paths.push(path);
          return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
        },
      }),
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    edit();
    await operator.handle({ type: "operateRequest", id: "one", request: 1, method: "GET", path: "/api/hmi/session" });
    assert.deepStrictEqual(paths, ["/api/hmi/page?name=overview"], "nothing goes out after the edit");
    assert.strictEqual(await posted(ended(messages)), "The HMI changed. Save and deploy it before operating again.");
  });

  test("the HMI's files are watched from the start of opening until operation ends", async () => {
    // Catches a watch that starts after the bundle is read, which misses a change while Live mode opens, and one that
    // outlives operation (specification 39, section 11 item 6: an edit stops operation at once).
    const calls: string[] = [];
    let edit = (): void => assert.fail("the operator watches the HMI's files");
    const watchEdits: LiveOperatorDeps["watchEdits"] = async (edited) => {
      calls.push("watch");
      edit = edited;
      return { dispose: () => calls.push("dispose") };
    };
    const bundle = async (): Promise<{ ok: boolean; version: string }> => {
      calls.push("bundle");
      return { ok: true, version: "v1" };
    };
    const operator = new LiveOperator(deps({ watchEdits, bundle }), () => undefined);
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    assert.deepStrictEqual(calls, ["watch", "bundle"], "the watch starts before the bundle is read and runs on");
    await operator.handle({ type: "operateClose", id: "one" });
    assert.deepStrictEqual(calls, ["watch", "bundle", "dispose"], "closing ends the watch");

    calls.length = 0;
    const refused = new LiveOperator(deps({ watchEdits, bundle, unsaved: () => EDITED }), () => undefined);
    await refused.handle({ type: "operateOpen", id: "two", page: "overview" });
    assert.deepStrictEqual(calls, ["watch", "bundle", "dispose"], "a read-only verdict ends the watch");

    const messages: HostToWebview[] = [];
    const opening = new LiveOperator(
      deps({
        watchEdits,
        bundle: async () => {
          edit();
          return { ok: true, version: "v1" };
        },
      }),
      (message) => messages.push(message),
    );
    await opening.handle({ type: "operateOpen", id: "three", page: "overview" });
    assert.strictEqual(await posted(ended(messages)), "The HMI changed. Save and deploy it before operating again.");
    assert.ok(!messages.some((message) => message.type === "operateReady"), "a change while it opens ends it first");
  });

  test("a language server without an answer leaves Live mode read-only with its error, not the runtime's", async () => {
    // Catches the runtime blamed for a language server that failed while Live mode opened, and the language server
    // blamed for a runtime that does not answer (specification 39, section 11 item 6).
    const reason = async (changes: Parameters<typeof deps>[0]): Promise<string | undefined> => {
      const messages: HostToWebview[] = [];
      const operator = new LiveOperator(deps(changes), (message) => messages.push(message));
      await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
      const ready = messages.find((message) => message.type === "operateReady");
      return ready?.type === "operateReady" ? ready.reason : undefined;
    };
    assert.strictEqual(
      await reason({
        liveGate: async () => {
          throw new Error("the truST language server stopped");
        },
      }),
      "the truST language server stopped",
    );
    assert.strictEqual(
      await reason({
        watchEdits: async () => {
          throw new Error("trust-lsp.hmiFiles did not name the HMI's files");
        },
      }),
      "trust-lsp.hmiFiles did not name the HMI's files",
    );
    assert.strictEqual(
      await reason({
        request: async () => {
          throw new Error("connection refused");
        },
      }),
      "The runtime cannot be reached. Check its connection and HMI web server.",
    );
  });

  test("without the language server's verdict, operation ends with its error and nothing is sent", async () => {
    // Catches a request sent, or operation left running, when the language server cannot decide (specification 39,
    // section 11 item 6: operation fails closed).
    const messages: HostToWebview[] = [];
    const paths: string[] = [];
    const operator = new LiveOperator(
      deps({
        liveOperation: async () => {
          throw new Error("the truST language server stopped");
        },
        request: async (_target, _method, path) => {
          paths.push(path);
          return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
        },
      }),
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    await operator.handle({ type: "operateRequest", id: "one", request: 1, method: "GET", path: "/api/hmi/session" });
    assert.deepStrictEqual(paths, ["/api/hmi/page?name=overview"], "the request does not go out");
    assert.strictEqual(ended(messages)(), "the truST language server stopped");
  });

  test("a language server that does not answer ends operation after 5 seconds", async function () {
    // Catches operation left frozen on a hung language server: the view would show old values as live (specification
    // 39, section 11 item 6).
    this.timeout(15_000);
    const messages: HostToWebview[] = [];
    const operator = new LiveOperator(
      deps({ liveOperation: () => new Promise<LiveOperationVerdict>(() => undefined) }),
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    const started = Date.now();
    await operator.handle({ type: "operateRequest", id: "one", request: 1, method: "GET", path: "/api/hmi/session" });
    assert.ok(Date.now() - started >= 4_900, "it waited the 5 seconds");
    assert.strictEqual(ended(messages)(), "trust-lsp.hmiLiveOperation did not answer within 5 seconds");
  });
});
