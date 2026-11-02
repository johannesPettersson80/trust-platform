import * as assert from "assert";
import * as fs from "fs";
import * as http from "http";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { DeployDeps } from "../../hmiBuilder/editor/deploy";
import { deployAction } from "../../hmiBuilder/editor/deployRuntime";
import { fakeRuntime, type FakeRuntime } from "./fakeControlRuntime";
import { disposeFixtureProject } from "./fixtureDocuments";
import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

// Specification 39, §16.2 item 8: Deploy and Roll back from the editor. The language server carries out the
// transaction (`trust-lsp.hmiDeploy`, whose rules are tested in Rust); these tests prove the wiring: the project, the
// selected run target and its token reach it, and its outcome comes back. The runtime is a control endpoint and a web
// server on loopback ports.

const SOURCE = "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n";
const SCREEN = 'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\n';
const PAGE =
  'format = 1\ntitle = "Overview"\n\n[[object]]\nid = "level"\ntype = "value"\nat = [0, 0]\nsize = [240, 96]\n\n[object.bind]\nvalue = "Main.Level"\n';

const fixtureProjects: string[] = [];

function project(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-deploy-"));
  fixtureProjects.push(root);
  fs.mkdirSync(path.join(root, "src"), { recursive: true });
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  fs.writeFileSync(path.join(root, "src", "main.st"), SOURCE);
  fs.writeFileSync(path.join(root, "hmi", "screen.toml"), SCREEN);
  fs.writeFileSync(path.join(root, "hmi", "pages", "overview.toml"), PAGE);
  return root;
}

type Posted = { url?: string; token?: string; body: unknown };

type WebServer = { posted: Posted[]; close(): Promise<void> };

/** The runtime: a web server answering every request with `answer`, and a control endpoint whose topology names
 * that web server as its own, on every interface. */
async function runtime(answer: object): Promise<{ control: FakeRuntime; web: WebServer }> {
  const posted: Posted[] = [];
  const server = http.createServer((request, response) => {
    let text = "";
    request.on("data", (chunk) => (text += chunk));
    request.on("end", () => {
      const token = request.headers["x-trust-token"];
      posted.push({ url: request.url, token: typeof token === "string" ? token : undefined, body: JSON.parse(text) });
      response.setHeader("Content-Type", "application/json");
      response.end(JSON.stringify(answer));
    });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as { port: number };
  const web = { posted, close: () => new Promise<void>((resolve) => server.close(() => resolve())) };
  const endpoints = [{ protocol: "web", source: "self", params: { listen: `0.0.0.0:${port}`, tls: false } }];
  const control = await fakeRuntime(() => ({ ok: true, result: { hosts: [{ runtimes: [{ endpoints }] }] } }));
  return { control, web };
}

/** The run target the extension selected: the runtime `control`, with its token. */
function selected(control: FakeRuntime): DeployDeps {
  return { target: async () => ({ endpoint: control.endpoint, token: "secret", label: "Line 1", simulator: false }) };
}

suite("HMI Builder: Deploy from the editor", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  test("Deploy sends the project and the selected runtime with its token, and the outcome comes back", async () => {
    const { control, web } = await runtime({ ok: true, active: "abc123", previous: "old999" });
    try {
      const outcome = await deployAction("deploy", project(), selected(control));
      assert.deepStrictEqual(outcome, { ok: true, version: "abc123", target: "Line 1" });
      assert.deepStrictEqual(
        control.received.map((request) => [request.type, request.auth]),
        [["fleet.topology", "secret"]],
      );
      assert.deepStrictEqual(
        web.posted.map((posted) => [posted.url, posted.token]),
        [["/api/hmi/deploy", "secret"]],
      );
      const files = (web.posted[0].body as { files: Record<string, unknown> }).files;
      assert.strictEqual(files["hmi/pages/overview.toml"], PAGE, "the project's bundle");
    } finally {
      await web.close();
      await control.close();
    }
  });

  test("Roll back reaches the selected runtime, and Deploy without one says to start one", async () => {
    const { control, web } = await runtime({ ok: true, active: "old999", previous: "abc123" });
    try {
      const rolledBack = await deployAction("rollback", project(), selected(control));
      assert.deepStrictEqual(rolledBack, { ok: true, version: "old999", target: "Line 1" });
      assert.deepStrictEqual(
        web.posted.map((posted) => posted.url),
        ["/api/hmi/rollback"],
      );
      const none = await deployAction("deploy", project(), { target: async () => undefined });
      assert.deepStrictEqual(none, { ok: false, reason: "No runtime is running: start a runtime to deploy to." });
    } finally {
      await web.close();
      await control.close();
    }
  });

  test("the editor saves the page before the bundle is built and shows the outcome", async () => {
    const { control, web } = await runtime({ ok: true, active: "abc123", previous: "old999" });
    try {
      const page = path.join(project(), "hmi", "pages", "overview.toml");
      const { document, webview, panel } = await openBuilder(page, undefined, undefined, selected(control));
      const edit = new vscode.WorkspaceEdit();
      edit.insert(document.uri, new vscode.Position(0, 0), "# unsaved\n");
      assert.ok(await vscode.workspace.applyEdit(edit));
      webview.send({ type: "deploy" });
      const [deployed] = await until("the deployment's outcome", () => {
        const found = postedOfType(webview, "deployed");
        return found.length > 0 && found;
      });
      assert.strictEqual(document.isDirty, false, "the page was saved first");
      assert.deepStrictEqual(deployed.outcome, { ok: true, version: "abc123", target: "Line 1" });
      const files = (web.posted[0].body as { files: Record<string, unknown> }).files;
      assert.strictEqual(files["hmi/pages/overview.toml"], `# unsaved\n${PAGE}`, "the bundle holds the saved edit");
      panel.dispose();
    } finally {
      await web.close();
      await control.close();
    }
  });
});
