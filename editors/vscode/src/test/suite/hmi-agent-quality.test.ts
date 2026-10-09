import * as assert from "assert";
import { routingQuality } from "../../hmiBuilder/agent/routingQuality";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { AuthoringService } from "../../hmiBuilder/agent/service";
import { WorkspaceDocuments } from "../../hmiBuilder/agent/documents";
import type { HmiObjectModel } from "../../hmiBuilder/drawing/model";

suite("HMI agent visual quality", () => {
  let root: string;
  setup(async () => {
    root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-quality-"));
  });
  teardown(async () => {
    await fs.rm(root, { recursive: true, force: true });
  });
  const object = (
    id: string,
    at: [number, number],
    size: [number, number],
    extra: Partial<HmiObjectModel> = {},
  ): HmiObjectModel => ({ id, type: "text", text: id, at, size, rotation: 0, bind: {}, ...extra });
  async function validate(objects: HmiObjectModel[], options: Record<string, unknown> = {}, colors: object = {}) {
    const catalogue = JSON.parse(
      await fs.readFile(
        path.resolve(__dirname, "../../../../../crates/trust-runtime/src/hmi/catalogue/components.json"),
        "utf8",
      ),
    );
    const service = new AuthoringService(
      "fixture",
      new WorkspaceDocuments(vscode.Uri.file(root)),
      async (name, args) => {
        if (name === "trust-lsp.hmiFileParse")
          return { ok: true, model: { title: "Quality", object: objects, pipe: [] } };
        if (name === "trust-lsp.hmiFileCheck") {
          assert.strictEqual(args.authoring, true, "validation uses declaration-only checks");
          return { ok: true, problems: [] };
        }
        throw new Error(`Unexpected request ${name}`);
      },
      path.join(root, "private"),
      "unused-font",
    );
    service.queries.context = async () => ({
      screen: { size: [1280, 800], theme: "dark", colors },
      catalogue,
      symbols: [],
      variables: [],
      texts: { languages: [{ tag: "en", name: "English" }], builtin: {}, project: {} },
    });
    const draft = await service.store.create("fixture", [], { "hmi/pages/quality.toml": "fixture" });
    return (
      await service.execute(
        "hmi_validate_draft",
        { draftId: draft.id, revision: draft.revision, ...options },
        new AbortController().signal,
      )
    ).data;
  }
  test("reports rotated page bounds and unintended overlaps with object IDs and measured rectangles", async () => {
    const result = await validate([
      object("turned", [0, 20], [40, 120], { rotation: 90 }),
      object("a", [200, 100], [200, 100]),
      object("b", [300, 140], [160, 80]),
    ]);
    const findings = result.findings as Array<{ rule: string; objects?: string[]; rectangle?: object }>;
    assert.ok(
      findings.some((finding) => finding.rule === "outside-page" && finding.objects?.includes("turned")),
      "rotation must participate in bounds checking",
    );
    const overlap = findings.find((finding) => finding.rule === "object-overlap");
    assert.deepStrictEqual(overlap?.objects, ["a", "b"]);
    assert.deepStrictEqual(overlap?.rectangle, { x: 300, y: 140, width: 100, height: 60 });
    assert.strictEqual(result.engineeringValid, true, "visual findings do not become engineering blockers");
  });
  test("containment, attached instruments and touching edges do not create overlap warnings", async () => {
    const result = await validate([
      object("background", [32, 80], [600, 400], { type: "group" }),
      object("unit", [100, 150], [200, 200]),
      object("instrument", [280, 200], [80, 40], { attach: "unit" }),
      object("edge", [300, 320], [100, 80]),
    ]);
    assert.ok(!(result.findings as Array<{ rule: string }>).some((finding) => finding.rule === "object-overlap"));
  });
  test("target viewport uses readable adaptive layout and reports genuinely undersized controls", async () => {
    const result = await validate([object("control", [100, 100], [200, 64], { type: "setpoint" })], {
      viewports: [
        { id: "small-panel", width: 640, height: 400 },
        { id: "design", width: 1280, height: 800 },
      ],
    });
    assert.ok(
      !(result.findings as Array<Record<string, unknown>>).some((finding) => finding.rule === "viewport-touch-target"),
    );
    assert.deepStrictEqual(
      (result.viewports as Array<{ id: string; scale: number }>).map((viewport) => [viewport.id, viewport.scale]),
      [
        ["small-panel", 1],
        ["design", 1],
      ],
    );
    const small = await validate([object("tiny", [100, 100], [24, 24], { type: "setpoint" })], {
      viewports: [{ id: "design", width: 1280, height: 800 }],
    });
    const warning = (small.findings as Array<Record<string, unknown>>).find(
      (finding) => finding.rule === "viewport-touch-target",
    );
    assert.ok(warning);
    assert.deepStrictEqual(warning.measured, { width: 24, height: 24 });
    assert.deepStrictEqual(warning.required, { width: 44, height: 44 });
    await assert.rejects(validate([], { viewports: [{ id: "bad", width: 0, height: 400 }] }), /INVALID_ARGUMENT/);
  });
  test("routing findings identify disconnected ports and preserve intentional crossing geometry", () => {
    const junction = (id: string, at: [number, number]) => object(id, at, [24, 24], { type: "junction" });
    const objects = [
      junction("left", [40, 100]),
      junction("right", [400, 100]),
      junction("top", [220, 20]),
      junction("bottom", [220, 240]),
    ];
    const page = {
      title: "Crossing",
      object: objects,
      pipe: [
        { from: "left.right", to: "right.left" },
        { from: "top.bottom", to: "bottom.top" },
      ],
    };
    assert.deepStrictEqual(routingQuality(page, [640, 400]), [], "crossing pipes are not collisions");
    const broken = routingQuality({ ...page, pipe: [{ from: "missing.outlet", to: "right.left" }] }, [640, 400]);
    assert.strictEqual(broken[0].rule, "disconnected-port");
    assert.deepStrictEqual(broken[0].objects, ["missing", "right"]);
    // A forced waypoint inside a reading is an actual drawn collision, not a router replacement.
    const collision = routingQuality(
      {
        ...page,
        object: [...objects, object("reading", [160, 80], [80, 60])],
        pipe: [{ from: "left.right", to: "right.left", waypoints: [[200, 112] as [number, number]] }],
      },
      [640, 400],
    );
    assert.ok(collision.some((finding) => finding.rule === "pipe-object-collision" && finding.object === "reading"));
  });
  test("repeated equipment findings measure row gaps and preserve deliberately separate rows", () => {
    const cards = [
      object("a", [0, 80], [100, 80], { type: "faceplate", faceplate: "pump" }),
      object("b", [120, 88], [100, 80], { type: "faceplate", faceplate: "pump" }),
      object("c", [280, 80], [100, 80], { type: "faceplate", faceplate: "pump" }),
    ];
    const check = () => routingQuality({ title: "Units", object: cards, pipe: [] }, [640, 400]);
    assert.deepStrictEqual(
      check().map((finding) => finding.rule),
      ["repeated-equipment-alignment", "repeated-equipment-spacing"],
    );
    cards[1].at = [120, 80];
    cards[2].at = [240, 80];
    assert.deepStrictEqual(check(), []);
    cards[2].at = [240, 240];
    assert.deepStrictEqual(check(), [], "separate process rows are intentional");
    cards[2].size = [120, 80];
    assert.ok(check().some((finding) => finding.rule === "repeated-equipment-size"));
  });
  test("project contrast and state-colour warnings reach the agent without changing engineering validity", async () => {
    const result = await validate(
      [object("label", [32, 32], [200, 40])],
      {},
      { dark: { text: "#06080c", accent: "#ff4165" } },
    );
    const findings = result.findings as Array<{ rule: string; severity: string }>;
    assert.ok(
      findings.some((finding) => finding.rule === "contrast" && finding.severity === "warning"),
      "shared contrast warnings must reach draft validation",
    );
    assert.strictEqual(result.engineeringValid, true);
  });
});
