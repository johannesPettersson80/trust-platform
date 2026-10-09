import * as assert from "assert";
import { FakeSocket } from "./hmiDisplayHost";
import { LiveSession, LiveEndpoint, LiveReply } from "../../hmiBuilder/editor/liveSession";

suite("HMI Builder: named Live session", () => {
  test("owner login does not replace the host's local named-session cookie", async () => {
    const cookies: Array<string | undefined> = [];
    const session = new LiveSession({
      target: async () => ({ endpoint: "tcp://origin:9000", url: "https://origin:8443", label: "Origin" }),
      request: async (_target, _method, path, body, cookie) => {
        cookies.push(cookie);
        return path === "/api/hmi/login" && !(body as { peer?: string }).peer
          ? { status: 200, body: { user: "anna", role: "operator" }, cookie: "trust_hmi_session=local; HttpOnly" }
          : { status: 200, body: { ok: true } };
      },
    });
    await session.request("POST", "/api/hmi/login", { user: "anna", password: "test" });
    await session.request("POST", "/api/hmi/login", { peer: "Owner", user: "owner", password: "test" });
    await session.request("POST", "/api/hmi/command", { id: "after-owner-login" });
    assert.strictEqual(cookies[2], "trust_hmi_session=local");
  });
  test("login retains only the HMI cookie on the host and commands carry that named session", async () => {
    const target: LiveEndpoint = {
      endpoint: "tcp://plc:9000",
      url: "https://plc:8443",
      token: "transport",
      label: "Cell",
    };
    const calls: Array<{ path: string; cookie?: string; body: unknown }> = [];
    const session = new LiveSession({
      target: async () => target,
      request: async (_target, _method, path, body, cookie): Promise<LiveReply> => {
        calls.push({ path, cookie, body });
        if (path === "/api/hmi/login")
          return {
            status: 200,
            body: { user: "anna", role: "operator" },
            cookie: "trust_hmi_session=named; HttpOnly; SameSite=Strict",
          };
        return { status: 200, body: { status: "queued" } };
      },
    });
    const login = await session.request("POST", "/api/hmi/login", { user: "anna", password: "test-only" });
    assert.strictEqual(login.status, 200, "the runtime's named login succeeds");
    assert.deepStrictEqual(login.body, { user: "anna", role: "operator" });
    assert.ok(!("cookie" in login), "the cookie is never returned to the webview");
    await session.request("POST", "/api/hmi/command", { id: "one" });
    assert.strictEqual(calls[1].cookie, "trust_hmi_session=named");
    assert.deepStrictEqual(calls[1].body, { id: "one" });
  });
  test("only named HMI routes are forwarded and commands require a login", async () => {
    const calls: string[] = [];
    const session = new LiveSession({
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async (_target, _method, path) => {
        calls.push(path);
        return { status: 200, body: {} };
      },
    });
    for (const path of [
      "https://elsewhere/api/hmi/page",
      "//elsewhere/api/hmi/page",
      "/api/hmi/../control",
      "/api/hmi/deploy",
      "/api/hmi/page#fragment",
    ]) {
      const answer = await session.request("GET", path);
      assert.strictEqual(answer.status, 403, `refuse ${path}`);
    }
    assert.strictEqual((await session.request("POST", "/api/hmi/command", {})).status, 401);
    assert.strictEqual(calls.length, 0, "no request escapes the allowed named-session surface");
  });

  test("the settings route is forwarded with the named session, and routes Live mode does not bridge are still refused (§21.3 item 7)", async () => {
    const calls: Array<{ method: string; path: string; cookie?: string; body: unknown }> = [];
    const session = new LiveSession({
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async (_target, method, path, body, cookie): Promise<LiveReply> => {
        calls.push({ method, path, cookie, body });
        if (path === "/api/hmi/login")
          return {
            status: 200,
            body: { user: "erik", role: "engineer" },
            cookie: "trust_hmi_session=engineer; HttpOnly",
          };
        return { status: 200, body: { ok: true, settings: [] } };
      },
    });
    const read = { faceplate: "overview/p101", action: "read" };
    assert.strictEqual(
      (await session.request("POST", "/api/hmi/settings", read)).status,
      401,
      "settings need a named login, as commands do",
    );
    assert.strictEqual(calls.length, 0);
    await session.request("POST", "/api/hmi/login", { user: "erik", password: "test-only" });
    const answer = await session.request("POST", "/api/hmi/settings", read);
    assert.deepStrictEqual(
      answer,
      { status: 200, body: { ok: true, settings: [] } },
      "the runtime's answer reaches the dialog",
    );
    assert.deepStrictEqual(calls[1], {
      method: "POST",
      path: "/api/hmi/settings",
      cookie: "trust_hmi_session=engineer",
      body: read,
    });
    for (const path of [
      "/api/hmi/deploy",
      "/api/hmi/rollback",
      "/api/hmi/settings/export",
      "/api/hmi/settings?action=read",
    ]) {
      assert.strictEqual((await session.request("POST", path, {})).status, 403, `refuse ${path}`);
    }
    assert.strictEqual(
      (await session.request("GET", "/api/hmi/settings")).status,
      403,
      "settings are posted, not read with GET",
    );
    assert.strictEqual(calls.length, 2, "nothing else reaches the runtime");
  });

  test("changing targets or ending Live mode invalidates the old login", async () => {
    let endpoint = "tcp://first:9000";
    const calls: string[] = [];
    const session = new LiveSession({
      target: async () => ({
        endpoint,
        url: `http://${endpoint.includes("first") ? "first" : "second"}:8080`,
        label: "Cell",
      }),
      request: async (_target, _method, path) => {
        calls.push(path);
        return { status: 200, body: {}, cookie: "trust_hmi_session=named; HttpOnly" };
      },
    });
    await session.request("POST", "/api/hmi/login", {});
    endpoint = "tcp://second:9000";
    assert.strictEqual((await session.request("POST", "/api/hmi/command", {})).status, 409);
    assert.deepStrictEqual(calls, ["/api/hmi/login"], "the old cookie never reaches another runtime");
    session.stop();
    assert.strictEqual((await session.request("POST", "/api/hmi/login", {})).status, 409);
    assert.strictEqual(calls.length, 1);
  });

  test("an in-flight login cannot revive an ended connection", async () => {
    let resolve!: (reply: LiveReply) => void;
    let started!: () => void;
    const ready = new Promise<void>((done) => {
      started = done;
    });
    let calls = 0;
    const session = new LiveSession({
      target: async () => ({ endpoint: "tcp://plc:9000", url: "http://plc:8080", label: "Cell" }),
      request: async () => {
        calls++;
        started();
        return new Promise<LiveReply>((done) => {
          resolve = done;
        });
      },
    });
    const pending = session.request("POST", "/api/hmi/login", {});
    await ready;
    session.stop();
    resolve({ status: 200, body: { user: "anna" }, cookie: "trust_hmi_session=late" });
    assert.strictEqual((await pending).status, 409);
    assert.strictEqual((await session.request("POST", "/api/hmi/command", {})).status, 409);
    assert.strictEqual(calls, 1);
  });

  test("the value stream carries the named login and a disconnect ends all session streams", async () => {
    const opened: Array<{ path: string; cookie?: string; socket: FakeSocket }> = [];
    const session = new LiveSession({
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async () => ({ status: 200, body: {}, cookie: "trust_hmi_session=named; HttpOnly" }),
      openSocket: (_target, path, cookie) => {
        const socket = new FakeSocket();
        opened.push({ path, cookie, socket });
        return socket;
      },
    });
    await session.request("POST", "/api/hmi/login", {});
    const page = await session.openSocket("/ws/hmi?page=overview");
    const popup = await session.openSocket("/ws/hmi?faceplate=overview%2Fpump");
    assert.ok(page && popup, "Live mode opens its page and faceplate streams");
    assert.deepStrictEqual(
      opened.map((entry) => entry.cookie),
      ["trust_hmi_session=named", "trust_hmi_session=named"],
    );
    let closed = 0;
    page.onclose = () => {
      closed++;
    };
    popup.onclose = () => {
      closed++;
    };
    opened[0].socket.onclose?.();
    assert.ok(opened.every((entry) => entry.socket.closed));
    assert.strictEqual(closed, 2);
    assert.strictEqual((await session.request("POST", "/api/hmi/command", {})).status, 409);
    assert.strictEqual(await session.openSocket("/ws/hmi?page=overview"), undefined);
  });
});
