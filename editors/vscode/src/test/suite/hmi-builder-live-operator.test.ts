import * as assert from "assert";
import { LiveOperator } from "../../hmiBuilder/editor/liveOperator";
import type { HostToWebview } from "../../hmiBuilder/editor/messages";

suite("HMI Builder: deployed Live connection", () => {
  test("a saved matching bundle opens the selected runtime and routes only the active editor connection", async () => {
    const messages: HostToWebview[] = [];
    const paths: string[] = [];
    const operator = new LiveOperator(
      {
        dirty: () => false,
        bundle: async () => ({ ok: true, version: "0123456789abcdef".repeat(4) }),
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
  test("a selected-target change ends operation before any new command", async () => {
    const messages: HostToWebview[] = [];
    let endpoint = "tcp://first:9000";
    let requests = 0;
    const operator = new LiveOperator(
      {
        dirty: () => false,
        bundle: async () => ({ ok: true, version: "v1" }),
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
});
