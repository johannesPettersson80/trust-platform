import * as assert from "assert";
import { FakeSocket } from "./hmiDisplayHost";
import { LiveSession, LiveEndpoint, LiveReply } from "../../hmiBuilder/editor/liveSession";
import type { LiveBridge } from "../../hmiBuilder/editor/liveSession";
import { liveBridge } from "../../hmiBuilder/editor/liveOperatorRuntime";

suite("HMI Builder: named Live session", () => {
  test("owner login does not replace the host's local named-session cookie", async () => {
    const cookies: Array<string | undefined> = [];
    const session = new LiveSession({
      bridge: liveBridge,
      target: async () => ({ endpoint: "tcp://origin:9000", url: "https://origin:8443", label: "Origin" }),
      request: async (_target, _method, path, body, cookie) => {
        cookies.push(cookie);
        return path === "/api/hmi/login" && !(body as { peer?: string }).peer
          ? { status: 200, body: { user: "anna", role: "operator" }, cookies: ["trust_hmi_session=local; HttpOnly"] }
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
      bridge: liveBridge,
      target: async () => target,
      request: async (_target, _method, path, body, cookie): Promise<LiveReply> => {
        calls.push({ path, cookie, body });
        if (path === "/api/hmi/login")
          return {
            status: 200,
            body: { user: "anna", role: "operator" },
            cookies: ["trust_hmi_session=named; HttpOnly; SameSite=Strict"],
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
      bridge: liveBridge,
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
      bridge: liveBridge,
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async (_target, method, path, body, cookie): Promise<LiveReply> => {
        calls.push({ method, path, cookie, body });
        if (path === "/api/hmi/login")
          return {
            status: 200,
            body: { user: "erik", role: "engineer" },
            cookies: ["trust_hmi_session=engineer; HttpOnly"],
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
      bridge: liveBridge,
      target: async () => ({
        endpoint,
        url: `http://${endpoint.includes("first") ? "first" : "second"}:8080`,
        label: "Cell",
      }),
      request: async (_target, _method, path) => {
        calls.push(path);
        return { status: 200, body: {}, cookies: ["trust_hmi_session=named; HttpOnly"] };
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
      bridge: liveBridge,
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
    resolve({ status: 200, body: { user: "anna" }, cookies: ["trust_hmi_session=late"] });
    assert.strictEqual((await pending).status, 409);
    assert.strictEqual((await session.request("POST", "/api/hmi/command", {})).status, 409);
    assert.strictEqual(calls, 1);
  });

  test("the value stream carries the named login and a disconnect ends all session streams", async () => {
    const opened: Array<{ path: string; cookie?: string; socket: FakeSocket }> = [];
    const session = new LiveSession({
      bridge: liveBridge,
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async () => ({ status: 200, body: {}, cookies: ["trust_hmi_session=named; HttpOnly"] }),
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

  // §11 item 6: the language server decides from facts only. Catches a password or the session cookie handed to it,
  // where a `verbose` language client trace would log them.
  test("the language server decides from facts, and neither the password nor the cookie reaches it", async () => {
    const facts: unknown[] = [];
    const recording: LiveBridge = {
      request: (event) => (facts.push(event), liveBridge.request(event)),
      reply: (event) => (facts.push(event), liveBridge.reply(event)),
      stream: (event) => (facts.push(event), liveBridge.stream(event)),
    };
    const cookies: Array<string | undefined> = [];
    const session = new LiveSession({
      bridge: recording,
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async (_target, _method, path, _body, cookie) => {
        cookies.push(cookie);
        return path === "/api/hmi/login"
          ? { status: 200, body: { user: "anna" }, cookies: ["trust_hmi_session=s3cr3t-cookie; HttpOnly"] }
          : { status: 200, body: {} };
      },
    });
    await session.request("POST", "/api/hmi/login", { user: "anna", password: "s3cr3t-password" });
    await session.request("POST", "/api/hmi/command", { id: "one" });
    assert.strictEqual(cookies[1], "trust_hmi_session=s3cr3t-cookie", "the command carries the kept session");
    const sent = JSON.stringify(facts);
    assert.ok(!sent.includes("s3cr3t"), `no secret in the facts: ${sent}`);
    assert.deepStrictEqual(facts, [
      { method: "POST", path: "/api/hmi/login", session: false, peer: false },
      { path: "/api/hmi/login", status: 200, peer: false },
      { method: "POST", path: "/api/hmi/command", session: true, peer: false },
    ]);
  });

  // §11 item 6: without the language server's verdict within 5 seconds, the request does not go out. Catches a
  // bridge that fails open, forwarding a request nobody allowed.
  test("without a verdict nothing goes out, also when the language server never answers", async function () {
    this.timeout(15000);
    let sent = 0;
    const deps = (bridge: LiveBridge) => ({
      bridge,
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async () => {
        sent++;
        return { status: 200, body: {} };
      },
      openSocket: () => {
        sent++;
        return new FakeSocket();
      },
    });
    const failing = new LiveSession(
      deps({ ...liveBridge, request: async () => Promise.reject(new Error("the language server failed")) }),
    );
    assert.deepStrictEqual(await failing.request("GET", "/api/hmi/session"), {
      status: 503,
      body: { error: "the language server failed" },
    });
    const silent = new LiveSession(
      deps({ request: () => new Promise(() => undefined), reply: liveBridge.reply, stream: () => new Promise(() => undefined) }),
    );
    const started = Date.now();
    assert.deepStrictEqual(await silent.request("GET", "/api/hmi/session"), {
      status: 503,
      body: { error: "trust-lsp.hmiLiveBridge did not answer within 5 seconds" },
    });
    assert.ok(Date.now() - started >= 4_900, "it waits the 5 seconds");
    assert.strictEqual(await silent.openSocket("/ws/hmi?page=overview"), undefined);
    assert.strictEqual(sent, 0, "nothing reached the runtime");
  });

  // §11 item 6: a login keeps the named session only on the language server's verdict. Catches a session kept when the
  // verdict on the login's answer fails, a login that then reports success, and a stream the bridge refuses opened anyway.
  test("an answer without a verdict keeps no session, and a refused stream opens no socket", async () => {
    const opened: string[] = [];
    const session = new LiveSession({
      bridge: { ...liveBridge, reply: async () => Promise.reject(new Error("no verdict on the answer")) },
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async () => ({ status: 200, body: {}, cookies: ["trust_hmi_session=named; HttpOnly"] }),
      openSocket: (_target, path) => {
        opened.push(path);
        return new FakeSocket();
      },
    });
    assert.deepStrictEqual(await session.request("POST", "/api/hmi/login", { user: "anna", password: "x" }), {
      status: 503,
      body: { error: "no verdict on the answer" },
    });
    assert.deepStrictEqual(await session.request("POST", "/api/hmi/command", {}), {
      status: 401,
      body: { error: "Log in with a named HMI user to operate." },
    });
    assert.strictEqual(await session.openSocket("/ws/hmi?alarms=1"), undefined);
    assert.deepStrictEqual(opened, []);
  });

  // §11 item 6: a verdict holds for the session it was asked for. Catches a command approved for a named session that
  // a logout ended while the command waited for its verdict, then sent without that session.
  test("a logout that ends while a command waits makes the command ask again, and it does not go out", async () => {
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    const asked: boolean[] = [];
    const sent: string[] = [];
    const session = new LiveSession({
      bridge: {
        ...liveBridge,
        request: async (facts) => {
          if (facts.path === "/api/hmi/command") {
            asked.push(facts.session);
            if (asked.length === 1) await held;
          }
          return liveBridge.request(facts);
        },
      },
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async (_target, _method, path) => {
        sent.push(path);
        return path === "/api/hmi/login"
          ? { status: 200, body: {}, cookies: ["trust_hmi_session=named; HttpOnly"] }
          : { status: 200, body: {} };
      },
    });
    await session.request("POST", "/api/hmi/login", { user: "anna", password: "x" });
    const command = session.request("POST", "/api/hmi/command", { id: "late" });
    await session.request("POST", "/api/hmi/logout", {});
    release();
    assert.deepStrictEqual(await command, { status: 401, body: { error: "Log in with a named HMI user to operate." } });
    assert.deepStrictEqual(asked, [true, false], "asked again for the session held then");
    assert.deepStrictEqual(sent, ["/api/hmi/login", "/api/hmi/logout"], "the command never went out");
  });

  // §11 item 6: without a verdict the named session is dropped. Catches a logout whose verdict fails leaving the
  // operator's session held in the extension host.
  test("a logout without a verdict still drops the named session", async () => {
    let failing = false;
    const session = new LiveSession({
      bridge: {
        ...liveBridge,
        request: (facts) =>
          failing ? Promise.reject(new Error("the language server failed")) : liveBridge.request(facts),
      },
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async (_target, _method, path) =>
        path === "/api/hmi/login"
          ? { status: 200, body: {}, cookies: ["trust_hmi_session=named; HttpOnly"] }
          : { status: 200, body: {} },
    });
    await session.request("POST", "/api/hmi/login", { user: "anna", password: "x" });
    assert.strictEqual(session.namedCookie(), "trust_hmi_session=named", "the login keeps the session");
    failing = true;
    assert.deepStrictEqual(await session.request("POST", "/api/hmi/logout", {}), {
      status: 503,
      body: { error: "the language server failed" },
    });
    assert.strictEqual(session.namedCookie(), undefined);
  });

  // §11 item 6 and RFC 6265 §5.2: the session's cookie is taken from the answer's Set-Cookie fields by the name the
  // language server gives. Catches only the first field read (a proxy's cookie before the runtime's), and a name or
  // value kept with the white space around it.
  test("the named session's cookie is taken by its name from every Set-Cookie field", async () => {
    const session = new LiveSession({
      bridge: liveBridge,
      target: async () => ({ endpoint: "tcp://plc:9000", url: "https://plc:8443", label: "Cell" }),
      request: async (_target, _method, path) =>
        path === "/api/hmi/login"
          ? { status: 200, body: {}, cookies: ["SERVERID=1; Path=/", " trust_hmi_session = named ; HttpOnly"] }
          : { status: 200, body: {} },
    });
    await session.request("POST", "/api/hmi/login", { user: "anna", password: "x" });
    assert.strictEqual(session.namedCookie(), "trust_hmi_session=named");
  });
});
