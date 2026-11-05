// Credentials on the language server's channel (specification 25 §6.4), through the real language client and server:
// a command that carries a credential the server names is refused, unsent, while the client's trace logs each
// message's parameters, and goes out otherwise.
import * as assert from "assert";
import * as vscode from "vscode";
import { languageServerReady } from "./hmiBuilderHost";

/** The refusal of a command that carries a credential under a trace that logs parameters. */
const REFUSAL =
  /^Error: UNSUPPORTED_CONTEXT: the language server trace logs each message's parameters, which would log a credential/;

/** The language server's answer to a deployment without a project: it reached the server, which sent nothing on. */
const NO_PROJECT = { ok: false, error: "trust-lsp.hmiDeploy needs the project to deploy." };

/** Runs `body` while `trust.languageServer.trace` reads `verbose`. The test answers through a wrapper of VS Code's
 * frozen configuration object: writing the real setting would turn the language client's trace on for the rest of the
 * suite. */
async function underVerboseTrace(body: () => Promise<void>): Promise<void> {
  const getConfiguration = vscode.workspace.getConfiguration;
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
    await body();
  } finally {
    (vscode.workspace as any).getConfiguration = getConfiguration;
  }
}

function run(command: string, argument: unknown): Promise<unknown> {
  return Promise.resolve(vscode.commands.executeCommand(command, argument));
}

suite("Credentials on the language server's channel", () => {
  suiteSetup(async function () {
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await languageServerReady();
  });

  // Catches G42: a deployment that carries the runtime's token while vscode-jsonrpc logs each message's parameters.
  test("a deployment's runtime token is refused, unsent, while the trace logs parameters", async () => {
    await underVerboseTrace(async () => {
      await assert.rejects(run("trust-lsp.hmiDeploy", { action: "deploy", authToken: "secret-7" }), REFUSAL);
    });
  });

  // Catches a guard that refuses more than it must: without that trace, the same request reaches the language server.
  test("the same deployment reaches the language server when the trace does not log parameters", async () => {
    assert.deepStrictEqual(await run("trust-lsp.hmiDeploy", { action: "deploy", authToken: "secret-7" }), NO_PROJECT);
  });

  // Catches a guard that reads a named field's presence instead of its value: a request without a token goes out.
  test("a request whose credential field holds no value goes out under a verbose trace", async () => {
    await underVerboseTrace(async () => {
      assert.deepStrictEqual(await run("trust-lsp.hmiDeploy", { action: "deploy", authToken: null }), NO_PROJECT);
    });
  });

  // Catches the review of #247: Devices & Connections sends the runtime's control token as `runtime.token`, a secret
  // configuration key, which the guard missed before; the refused request never reaches the language server.
  test("Devices & Connections' runtime token is refused, unsent, while the trace logs parameters", async () => {
    await underVerboseTrace(async () => {
      await assert.rejects(
        run("trust-lsp.devicesCanvasAct", {
          op: "identify",
          args: { name: "Line 1" },
          runtime: { endpoint: "unix:///nonexistent/trust-credentials-test.sock", token: "secret-7" },
        }),
        REFUSAL,
      );
    });
  });

  // Catches a guard that reads any field named like a credential: a field holding no text carries none, and the colour
  // command's token travels as `colourToken`.
  test("a blank token and a colour token go out under a verbose trace", async () => {
    await underVerboseTrace(async () => {
      assert.deepStrictEqual(await run("trust-lsp.hmiDeploy", { action: "deploy", authToken: "   " }), NO_PROJECT);
      const colour = (await run("trust-lsp.hmiScreenColour", {
        text: "",
        theme: "dark",
        colourToken: "accent",
        value: "#112233",
      })) as { ok?: unknown };
      assert.strictEqual(typeof colour.ok, "boolean");
    });
  });

  // Catches G47's class for an answer: the simulator control's `create` answers a new control token, its `check` none.
  test("the simulator control's new token is refused under a verbose trace, and its check is not", async () => {
    await underVerboseTrace(async () => {
      await assert.rejects(run("trust-lsp.simulatorControl", { action: "create" }), REFUSAL);
      const checked = (await run("trust-lsp.simulatorControl", {
        action: "check",
        endpoint: "unix:///nonexistent/trust-credentials-test.sock",
      })) as { ok?: unknown };
      assert.strictEqual(checked.ok, true);
    });
  });
});
