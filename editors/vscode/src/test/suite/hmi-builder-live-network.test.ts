import * as assert from "assert";
import * as http from "http";
import { openLiveSocket, requestLive } from "../../hmiBuilder/editor/liveNetwork";

suite("HMI Builder: Live HTTP transport", () => {
  test("transport access and the named cookie reach only the selected server, and redirects are refused", async () => {
    let redirected = false;
    const server = http.createServer((request, response) => {
      if (request.url === "/redirect") {
        response.writeHead(302, { Location: "/outside" });
        response.end();
        return;
      }
      if (request.url === "/outside") redirected = true;
      response.setHeader("Content-Type", "application/json");
      response.setHeader("Set-Cookie", "trust_hmi_session=named; HttpOnly; SameSite=Strict");
      response.end(
        JSON.stringify({
          token: request.headers["x-trust-token"],
          cookie: request.headers.cookie,
          method: request.method,
        }),
      );
    });
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
    try {
      const port = (server.address() as { port: number }).port;
      const target = {
        endpoint: "tcp://127.0.0.1:9000",
        url: `http://127.0.0.1:${port}`,
        token: "test-access",
        label: "test",
      };
      const answer = await requestLive(
        target,
        "POST",
        "/api/hmi/login",
        { user: "anna", password: "test-only" },
        "trust_hmi_session=old",
      );
      assert.strictEqual(answer.status, 200, "the selected runtime receives the request");
      assert.deepStrictEqual(answer.body, { token: "test-access", cookie: "trust_hmi_session=old", method: "POST" });
      assert.strictEqual(answer.cookie, "trust_hmi_session=named; HttpOnly; SameSite=Strict");
      await assert.rejects(() => requestLive(target, "GET", "/redirect", undefined), /redirect/i);
      assert.strictEqual(redirected, false);
    } finally {
      server.closeAllConnections();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    }
  });
  test("the live stream sends host access and named session headers and carries runtime messages", async () => {
    const { WebSocketServer } = require("ws");
    const server = http.createServer();
    const sockets = new WebSocketServer({ server });
    let headers: http.IncomingHttpHeaders | undefined;
    sockets.on("connection", (socket: { send(value: string): void }, request: http.IncomingMessage) => {
      headers = request.headers;
      socket.send(JSON.stringify({ session: { user: "anna", role: "operator" } }));
    });
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
    let stream: ReturnType<typeof openLiveSocket> | undefined;
    try {
      const port = (server.address() as { port: number }).port;
      stream = openLiveSocket(
        { endpoint: "tcp://127.0.0.1:9000", url: `http://127.0.0.1:${port}`, token: "test-access", label: "test" },
        "/ws/hmi?page=overview",
        "trust_hmi_session=named",
      );
      assert.ok(stream, "the host opens the runtime stream");
      const opened = stream;
      const data = await new Promise<unknown>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("test stream did not arrive")), 1500);
        opened.onmessage = (event) => {
          clearTimeout(timer);
          resolve(event.data);
        };
      });
      assert.deepStrictEqual(JSON.parse(String(data)), { session: { user: "anna", role: "operator" } });
      assert.strictEqual(headers?.["x-trust-token"], "test-access");
      assert.strictEqual(headers?.cookie, "trust_hmi_session=named");
      assert.strictEqual(
        require("../../../package.json").dependencies.ws,
        "8.21.1",
        "the shipped extension includes its socket transport",
      );
    } finally {
      stream?.close();
      sockets.clients.forEach((socket: { terminate(): void }) => socket.terminate());
      await new Promise<void>((resolve) => sockets.close(() => resolve()));
      await new Promise<void>((resolve) => server.close(() => resolve()));
    }
  });
});
