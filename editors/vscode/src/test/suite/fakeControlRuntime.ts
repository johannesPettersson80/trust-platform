// A runtime on a loopback port for the tests of the extension's wiring to the language server's runtime requests
// (`trust-lsp.runtimeRequest`, `trust-lsp.runTargets`): it answers each request line as the test says and records the
// connections and requests it got (runtime protocol annex E0.1).

import * as assert from "assert";
import * as net from "net";
import * as vscode from "vscode";

export type RuntimeRequest = {
  readonly id: number;
  readonly type: string;
  readonly auth?: string;
  readonly params?: unknown;
};

export type FakeRuntime = {
  readonly endpoint: string;
  /** The requests the runtime got, in order. */
  readonly received: RuntimeRequest[];
  /** The number of connections the runtime accepted. */
  connections(): number;
  close(): Promise<void>;
};

/** A runtime that answers each request with `answer(request)`, to which the request's `id` is added. */
export async function fakeRuntime(answer: (request: RuntimeRequest) => object): Promise<FakeRuntime> {
  const received: RuntimeRequest[] = [];
  const sockets = new Set<net.Socket>();
  let connections = 0;
  const server = net.createServer((socket) => {
    connections += 1;
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
    socket.on("error", () => undefined);
    let buffer = "";
    socket.on("data", (chunk) => {
      buffer += chunk.toString();
      for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
        const request = JSON.parse(buffer.slice(0, end)) as RuntimeRequest;
        buffer = buffer.slice(end + 1);
        received.push(request);
        socket.write(`${JSON.stringify({ id: request.id, ...answer(request) })}\n`);
      }
    });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as net.AddressInfo;
  return {
    endpoint: `tcp://127.0.0.1:${port}`,
    received,
    connections: () => connections,
    close: async () => {
      for (const socket of sockets) {
        socket.destroy();
      }
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };
}

/** A loopback endpoint on which nothing listens. */
export async function closedEndpoint(): Promise<string> {
  const server = net.createServer();
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as net.AddressInfo;
  await new Promise<void>((resolve) => server.close(() => resolve()));
  return `tcp://127.0.0.1:${port}`;
}

/** Waits until the language server offers its runtime commands. */
export async function runtimeCommandsReady(): Promise<void> {
  await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
  const deadline = Date.now() + 30000;
  for (;;) {
    const commands = await vscode.commands.getCommands(true);
    if (commands.includes("trust-lsp.runtimeRequest") && commands.includes("trust-lsp.runTargets")) {
      return;
    }
    if (Date.now() > deadline) {
      assert.fail("the language server does not offer trust-lsp.runtimeRequest and trust-lsp.runTargets");
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}
