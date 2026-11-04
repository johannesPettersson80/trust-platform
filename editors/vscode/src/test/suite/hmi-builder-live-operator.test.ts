import * as assert from "assert";
import { LiveOperator } from "../../hmiBuilder/editor/liveOperator";
import { liveGate, liveOperation } from "../../hmiBuilder/editor/liveOperatorRuntime";
import type { SocketLike } from "../../hmiBuilder/display/runtimeDisplay";
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
        dirty: () => false,
        bundle: async () => ({ ok: true, version: "0123456789abcdef".repeat(4) }),
        runTarget: async () => ({ label: "Cell runtime", simulator: false }),
        liveGate,
        liveOperation,
        session: {
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
          dirty: () => dirty,
          bundle: async () => ({ ok: true, version: "local" }),
          runTarget: async () => ({ label: "Cell runtime", simulator: false }),
          liveGate,
          liveOperation,
          session: {
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
          dirty: () => false,
          bundle: async () => ({ ok: true, version: "v1" }),
          runTarget: async () => selected,
          liveGate,
          liveOperation,
          session: {
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
        dirty: () => false,
        bundle: async () => ({ ok: true, version: "v1" }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        session: {
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
        dirty: () => false,
        bundle: async () => ({ ok: true, version: "v1" }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        session: {
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
      messages.some((message) => message.type === "operateEnded" && /deploy/i.test(message.reason)),
      "the editor cannot operate another bundle after reload",
    );
    assert.ok(
      !messages.some((message) => message.type === "operateReply" && message.status === 200),
      "the new page is not admitted to the operating view",
    );
  });

  test("requests cannot overtake the saved/deployed bundle check", async () => {
    let resolve!: (bundle: { ok: boolean; version: string }) => void;
    let requests = 0;
    const operator = new LiveOperator(
      {
        dirty: () => false,
        bundle: () =>
          new Promise((done) => {
            resolve = done;
          }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        session: {
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
        dirty: () => false,
        bundle: async () => ({ ok: true, version: "v1" }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        session: {
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

  test("a closed stream ends operation at once, and the language server says why", async () => {
    // Catches a request that goes out after the runtime's stream closed, while the language server words the reason, and
    // an ending that does not say that unfinished commands are not resent (specification 39, section 11 item 6).
    const messages: HostToWebview[] = [];
    const paths: string[] = [];
    let stream: SocketLike | undefined;
    const operator = new LiveOperator(
      {
        dirty: () => false,
        bundle: async () => ({ ok: true, version: "v1" }),
        runTarget: async () => ({ label: "Cell", simulator: false }),
        liveGate,
        liveOperation,
        session: {
          target: async () => ({ endpoint: "tcp://plc:9000", url: "http://plc:8080", label: "Cell" }),
          request: async (_target, _method, path) => {
            paths.push(path);
            return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
          },
          openSocket: () => {
            stream = { onmessage: null, onclose: null, close: () => undefined };
            return stream;
          },
        },
      },
      (message) => messages.push(message),
    );
    await operator.handle({ type: "operateOpen", id: "one", page: "overview" });
    await operator.handle({ type: "operateSocket", id: "one", socket: 1, path: "/ws/hmi?page=overview" });
    // The runtime closes its stream: the session's own handler runs.
    const closed = stream?.onclose;
    assert.ok(closed, "the stream opened");
    closed();
    await operator.handle({ type: "operateRequest", id: "one", request: 2, method: "GET", path: "/api/hmi/session" });
    assert.deepStrictEqual(paths, ["/api/hmi/page?name=overview"], "no request goes out after the stream closed");
    const end = await posted(() => messages.find((message) => message.type === "operateEnded"));
    assert.ok(end.type === "operateEnded");
    assert.strictEqual(
      end.reason,
      "The runtime disconnected. Any unfinished command has an unknown outcome; none will be resent.",
    );
  });
});
