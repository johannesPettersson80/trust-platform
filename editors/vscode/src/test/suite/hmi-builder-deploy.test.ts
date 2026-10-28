import * as assert from "assert";

import { DeployDeps, deployBundle, rollbackBundle } from "../../hmiBuilder/editor/deploy";
import { runtimeDeployDeps } from "../../hmiBuilder/editor/deployRuntime";
import { languageServerReady } from "./hmiBuilderHost";

// Specification 39, §16.2 item 8: Deploy from the editor checks the bundle, finds the selected
// runtime's web server and sends the bundle there; the simulator has no web server. Which address
// the runtime's topology gives is the language server's rule (`trust-lsp.hmiDeployTarget`, tested in
// Rust); here it is injected with the other outside calls.

const BUNDLE = { format: 1, files: { "hmi/screen.toml": "format = 1\n" } };
const WEB = { listen: "0.0.0.0:8080", tls: false };

function topology(web?: { listen: string; tls: boolean }) {
  const endpoints = web
    ? [
        {
          id: "web",
          kind: "service",
          protocol: "web",
          source: "self",
          params: { listen: web.listen, tls: web.tls, auth: "token" },
        },
      ]
    : [];
  return { schema_version: 1, hosts: [{ host_id: "h", runtimes: [{ runtime_id: "r", endpoints }], containers: [] }] };
}

function deps(overrides: Partial<DeployDeps> = {}) {
  const posts: Array<{ url: string; token?: string; body: unknown }> = [];
  const base: DeployDeps = {
    bundle: async () => ({ ok: true, bundle: BUNDLE, version: "abc123", problems: [] }),
    target: async () => ({ endpoint: "tcp://192.168.1.20:9000", token: "secret", label: "Line 1", simulator: false }),
    control: async () => topology(WEB),
    post: async (url, token, body) => {
      posts.push({ url, token, body });
      // The runtime answers with the versions now active and kept (§16.4).
      return { status: 200, body: { ok: true, active: "abc123", previous: "old999" } };
    },
    address: async (endpoint, given) =>
      endpoint === "tcp://192.168.1.20:9000" && JSON.stringify(given) === JSON.stringify(topology(WEB))
        ? "http://192.168.1.20:8080"
        : undefined,
  };
  return { posts, deps: { ...base, ...overrides } };
}

suite("HMI Builder: Deploy from the editor", () => {
  test("a checked bundle goes to the runtime's web server with its administrator access", async () => {
    const { posts, deps: injected } = deps();
    const outcome = await deployBundle("/project", injected);
    assert.deepStrictEqual(outcome, { ok: true, version: "abc123", target: "Line 1" });
    assert.deepStrictEqual(posts, [{ url: "http://192.168.1.20:8080/api/hmi/deploy", token: "secret", body: BUNDLE }]);
  });

  test("the editor check's problems stop the deployment", async () => {
    const problems = [
      { file: "hmi/pages/overview.toml", line: 12, message: '"Main.Gone" is not a variable of the program' },
    ];
    const { posts, deps: injected } = deps({
      bundle: async () => ({ ok: true, bundle: BUNDLE, version: "abc123", problems }),
    });
    const outcome = await deployBundle("/project", injected);
    assert.strictEqual(outcome.ok, false);
    assert.match(outcome.ok ? "" : outcome.reason, /1 problem/);
    assert.deepStrictEqual(outcome.ok ? undefined : outcome.problems, problems);
    assert.strictEqual(posts.length, 0);
  });

  test("the simulator, a missing runtime and a runtime without a web server are refused with what to do", async () => {
    const simulator = await deployBundle(
      "/project",
      deps({ target: async () => ({ endpoint: "tcp://127.0.0.1:9", label: "Simulator", simulator: true }) }).deps,
    );
    assert.match(simulator.ok ? "" : simulator.reason, /simulator has no web server/i);
    const none = await deployBundle("/project", deps({ target: async () => undefined }).deps);
    assert.match(none.ok ? "" : none.reason, /no runtime is running/i);
    const noWeb = await deployBundle("/project", deps({ control: async () => topology() }).deps);
    assert.match(noWeb.ok ? "" : noWeb.reason, /no web server/i);
  });

  test("the runtime's refusal comes back with its problems", async () => {
    const problems = [{ file: "hmi/pages/overview.toml", line: 3, message: "needs more than the write policy allows" }];
    const outcome = await deployBundle(
      "/project",
      deps({ post: async () => ({ status: 422, body: { ok: false, problems, active: "abc123", previous: null } }) })
        .deps,
    );
    assert.deepStrictEqual(outcome, { ok: false, reason: "Line 1 found 1 problem in the bundle.", problems });
    const refused = await deployBundle(
      "/project",
      deps({ post: async () => ({ status: 403, body: { ok: false, error: "deploying needs the deploy permission" } }) })
        .deps,
    );
    assert.deepStrictEqual(refused, { ok: false, reason: "deploying needs the deploy permission" });
  });

  test("Roll back asks the runtime to restore the previous bundle", async () => {
    const { posts, deps: injected } = deps({
      post: async (url, token, body) => {
        posts.push({ url, token, body });
        return { status: 200, body: { ok: true, active: "old999", previous: "abc123" } };
      },
    });
    const outcome = await rollbackBundle(injected);
    assert.deepStrictEqual(outcome, { ok: true, version: "old999", target: "Line 1" });
    assert.strictEqual(posts[0].url, "http://192.168.1.20:8080/api/hmi/rollback");
  });
});

// The real address comes from the language server.
suite("HMI Builder: Deploy from the editor, the web server's address", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  test("the runtime's topology and control endpoint give the address", async () => {
    assert.strictEqual(
      await runtimeDeployDeps.address("tcp://192.168.1.20:9000", topology(WEB)),
      "http://192.168.1.20:8080",
    );
    assert.strictEqual(await runtimeDeployDeps.address("tcp://192.168.1.20:9000", topology()), undefined);
  });
});
