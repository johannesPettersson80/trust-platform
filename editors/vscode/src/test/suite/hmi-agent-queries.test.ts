import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { WorkspaceDocuments } from "../../hmiBuilder/agent/documents";
import { AuthoringQueries, NativeCommand } from "../../hmiBuilder/agent/queries";

suite("HMI agent engineering queries", () => {
  let root: string;
  setup(async () => {
    root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-queries-"));
    await fs.mkdir(path.join(root, "hmi/pages"), { recursive: true });
    await fs.writeFile(path.join(root, "hmi/screen.toml"), "screen");
    await fs.writeFile(path.join(root, "hmi/pages/process.toml"), "page");
  });
  teardown(async () => fs.rm(root, { recursive: true, force: true }));
  const page = {
    title: "Process",
    object: [
      {
        id: "tank",
        type: "tank",
        at: [0, 0],
        size: [100, 100],
        bind: { level: { path: "Main.Level", direction: "read" } },
      },
      { id: "pump", type: "pump", at: [200, 0], size: [100, 100], bind: {} },
    ],
    pipe: [{ from: "tank.outlet", to: "pump.inlet" }],
  };
  const command: NativeCommand = async (name, input) => {
    if (name === "trust-lsp.hmiBindings")
      return { ok: true, variables: [{ path: "Main.Level", type: "REAL", kinds: ["number"] }], alarms: [] };
    if (name === "trust-lsp.hmiCatalogue")
      return {
        ok: true,
        catalogue: {
          format: 1,
          components: Array.from({ length: 105 }, (_, i) => ({
            type: "pump-" + i,
            category: "equipment",
            size: [100, 100],
            properties: { running: { accepts: ["boolean"], directions: ["read"] } },
          })),
        },
        symbols: [],
      };
    if (name === "trust-lsp.hmiTexts")
      return { ok: true, languages: [{ tag: "en", name: "English" }], project: {}, builtin: {}, errors: [] };
    if (name === "trust-lsp.hmiFaceplates") return { ok: true, faceplates: [{ name: "pump" }] };
    if (name === "trust-lsp.hmiFileParse")
      return { ok: true, model: input.kind === "screen" ? { size: [1280, 800], theme: "dark" } : page };
    throw new Error("Unexpected command " + name);
  };
  const service = (root: string, native = command) =>
    new AuthoringQueries(new WorkspaceDocuments(vscode.Uri.file(root)), native);
  test("context exposes typed variables, screen and file revisions without inferred topology", async () => {
    const result = await service(root).context();
    assert.deepStrictEqual(result.variables, [{ path: "Main.Level", type: "REAL", kinds: ["number"] }]);
    assert.deepStrictEqual(result.screen, { size: [1280, 800], theme: "dark" });
    assert.deepStrictEqual(result.topology, { status: "unspecified", connections: [] });
    assert.ok((result.revisions as Record<string, string>)["hmi/pages/process.toml"]);
  });
  test("compiler failure is not an empty usable variable list", async () => {
    await assert.rejects(
      service(root, async (name, input) =>
        name === "trust-lsp.hmiBindings" ? { ok: false, error: "bad ST" } : command(name, input),
      ).context(),
      /COMPILATION_FAILED.*bad ST/,
    );
  });
  test("inspection returns selected objects and connected neighbours separately", async () => {
    const result = await service(root).inspect("hmi/pages/process.toml", ["tank"]);
    assert.deepStrictEqual(
      (result.objects as Array<{ id: string }>).map((o) => o.id),
      ["tank"],
    );
    assert.deepStrictEqual(result.neighbours, ["pump"]);
    assert.strictEqual(result.title, "Process");
    assert.ok(result.revision);
  });
  test("library queries paginate instead of silently dropping components", async () => {
    const first = await service(root).library("pump");
    assert.strictEqual((first.components as unknown[]).length, 100);
    assert.strictEqual(first.nextCursor, 100);
    const second = await service(root).library("pump", 100);
    assert.strictEqual((second.components as unknown[]).length, 5);
    assert.strictEqual(second.nextCursor, null);
  });
  test("region inspection selects rotated design boxes and preserves connected neighbours", async () => {
    const query = service(root);
    const result = await Reflect.apply(query.inspect, query, [
      "hmi/pages/process.toml",
      undefined,
      { x: 0, y: 0, width: 120, height: 120 },
    ]);
    assert.deepStrictEqual(
      (result.objects as Array<{ id: string }>).map((object) => object.id),
      ["tank"],
      "region must not return the entire page",
    );
    assert.deepStrictEqual(result.neighbours, ["pump"]);
    assert.deepStrictEqual(result.zOrder, { tank: 0 });
    const empty = await Reflect.apply(query.inspect, query, [
      "hmi/pages/process.toml",
      undefined,
      { x: 500, y: 500, width: 20, height: 20 },
    ]);
    assert.deepStrictEqual(empty.objects, []);
    assert.deepStrictEqual(empty.pipes, []);
    await assert.rejects(
      Reflect.apply(query.inspect, query, ["hmi/pages/process.toml", ["tank"], { x: 0, y: 0, width: 20, height: 20 }]),
      /INVALID_ARGUMENT/,
    );
    await assert.rejects(
      Reflect.apply(query.inspect, query, ["hmi/pages/process.toml", undefined, { x: 0, y: 0, width: 0, height: 20 }]),
      /INVALID_ARGUMENT/,
    );
    const rotated = service(root, async (name, input) =>
      name === "trust-lsp.hmiFileParse"
        ? {
            ok: true,
            model: {
              title: "Rotated",
              pipe: [],
              object: [{ id: "turned", type: "value", at: [200, 200], size: [100, 20], rotation: 90, bind: {} }],
            },
          }
        : command(name, input),
    );
    const edge = await Reflect.apply(rotated.inspect, rotated, [
      "hmi/pages/process.toml",
      undefined,
      { x: 240, y: 160, width: 20, height: 20 },
    ]);
    assert.deepStrictEqual(
      (edge.objects as Array<{ id: string }>).map((object) => object.id),
      ["turned"],
    );
  });

  test("library exposes versioned composition families with operator tasks and honest review status", async () => {
    const result = await service(root).library();
    const patterns = result.compositionPatterns as Array<{
      id: string;
      version: number;
      task: string;
      requiredInputs: string[];
      visualReview: string;
    }>;
    assert.ok(Array.isArray(patterns), "agents must be able to discover composition definitions");
    assert.deepStrictEqual(
      patterns.map((pattern) => pattern.id),
      ["process-overview", "equipment-detail", "multi-unit", "investigation", "settings"],
    );
    for (const pattern of patterns) {
      assert.strictEqual(pattern.version, 1);
      assert.ok(pattern.task.length > 20);
      assert.ok(pattern.requiredInputs.length > 0);
      assert.strictEqual(pattern.visualReview, "pending");
    }
    const filtered = await service(root).library("commissioning");
    assert.deepStrictEqual(
      (filtered.compositionPatterns as Array<{ id: string }>).map((pattern) => pattern.id),
      ["settings"],
    );
    assert.deepStrictEqual(filtered.components, []);
  });

  test("partial discovery retains page metadata and compiler errors without a fake empty program", async () => {
    const query = service(root, async (name, input) =>
      name === "trust-lsp.hmiBindings" ? { ok: false, error: "unfinished ST" } : command(name, input),
    );
    const result = await query.context({ partial: true, sections: ["pages", "variables"] });
    assert.strictEqual(result.variables, null);
    assert.strictEqual((result.compilation as { status: string }).status, "failed");
    assert.strictEqual((result.pages as Array<{ title: string }>)[0].title, "Process");
    assert.strictEqual(result.catalogue, undefined);
  });
  test("library definitions carry stable identities and executable preview-state names", async () => {
    const first = await service(root).library("pump", 0, 1);
    const second = await service(root).library("pump", 0, 1);
    assert.strictEqual(first.catalogueRevision, second.catalogueRevision);
    const entry = (first.components as Array<{ definitionRevision: string; states: Array<{ id: string }> }>)[0];
    assert.match(entry.definitionRevision, /^[0-9a-f]{64}$/);
    assert.ok(entry.states.some((state) => state.id === "unknown"));
  });
});
