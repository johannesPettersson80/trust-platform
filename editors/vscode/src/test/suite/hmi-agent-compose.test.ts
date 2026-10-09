import * as assert from "assert";
import { composeAgentPage, CompositionInput } from "../../hmiBuilder/agent/compose";
const catalogue = {
  format: 1,
  components: [
    { type: "tank", category: "equipment", phase: 2, size: [200, 260] as [number, number], ports: ["inlet", "outlet"] },
    { type: "pump", category: "equipment", phase: 2, size: [160, 120] as [number, number], ports: ["inlet", "outlet"] },
    { type: "value", category: "display", phase: 1, size: [240, 64] as [number, number] },
    { type: "text", category: "display", phase: 1, size: [240, 40] as [number, number] },
  ],
};
const input = (): CompositionInput => ({
  title: "Tank process",
  pattern: "process-overview",
  size: [1280, 800],
  items: [
    {
      id: "tank",
      type: "tank",
      tag: "TK101",
      region: "process",
      bind: { level: { path: "Main.Level", direction: "read" } },
    },
    { id: "pump", type: "pump", tag: "P101", region: "process" },
    {
      id: "level",
      type: "value",
      label: "Tank level",
      region: "measurements",
      unit: "m",
      bind: { value: { path: "Main.Level", direction: "read" } },
    },
  ],
  connections: [{ from: "tank.outlet", to: "pump.inlet" }],
});
suite("HMI agent composition", () => {
  test("places real catalogue components on grid and keeps provided bindings and connections", () => {
    const page = composeAgentPage(input(), catalogue);
    assert.strictEqual(page.object.length, 4, "heading and three objects");
    assert.deepStrictEqual(page.pipe, input().connections);
    assert.strictEqual(page.object.find((o) => o.id === "tank")!.bind.level.path, "Main.Level");
    for (const object of page.object) {
      assert.ok(object.at.every((n) => n % 8 === 0));
      assert.ok(object.at[0] >= 0 && object.at[1] >= 0);
      assert.ok(object.at[0] + object.size[0] <= 1280 && object.at[1] + object.size[1] <= 800);
    }
    assert.deepStrictEqual(composeAgentPage(input(), catalogue), page, "repeatable layout");
  });
  test("readings are separated from the dominant process region", () => {
    const page = composeAgentPage(input(), catalogue);
    const tank = page.object.find((o) => o.id === "tank"),
      reading = page.object.find((o) => o.id === "level");
    assert.ok(tank && reading);
    assert.ok(reading!.at[0] > tank!.at[0] + tank!.size[0]);
  });
  test("does not invent connections or bindings", () => {
    const proposal = input();
    proposal.connections = [];
    proposal.items.forEach((item) => delete item.bind);
    const page = composeAgentPage(proposal, catalogue);
    assert.strictEqual(page.object.filter((o) => o.id !== "page-heading").length, 3);
    assert.deepStrictEqual(page.pipe, []);
    assert.ok(page.object.every((o) => Object.keys(o.bind).length === 0));
  });
  test("overcrowded screens fail instead of shrinking text or equipment", () => {
    const proposal = input();
    proposal.size = [320, 240];
    assert.throws(() => composeAgentPage(proposal, catalogue), /fit|constraint/i);
  });
  test("unknown components and duplicate ids are refused", () => {
    const proposal = input();
    proposal.items[1].id = "tank";
    assert.throws(() => composeAgentPage(proposal, catalogue), /id/i);
    proposal.items[1].id = "pump";
    proposal.items[1].type = "made-up";
    assert.throws(() => composeAgentPage(proposal, catalogue), /component/i);
  });
  test("a sparse overview gives equipment visual prominence and readings a consistent side column", () => {
    const proposal = input();
    proposal.items.push({ id: "temperature", type: "value", region: "measurements", label: "Temperature" });
    const page = composeAgentPage(proposal, catalogue);
    const tank = page.object.find((object) => object.id === "tank")!;
    const readings = page.object.filter((object) => ["level", "temperature"].includes(object.id));
    assert.ok(
      tank.size[0] >= 300 && tank.size[1] >= 360,
      "primary equipment should occupy the available process region",
    );
    assert.strictEqual(readings[0].at[0], readings[1].at[0]);
    assert.ok(readings.every((object) => object.size[0] >= 300));
    assert.ok(readings[1].at[1] >= readings[0].at[1] + readings[0].size[1] + 24);
  });

  test("explicit regions and vertical flow control deterministic placement", () => {
    const proposal = {
      ...input(),
      connections: [],
      layout: {
        flow: "vertical",
        gap: 16,
        regions: {
          process: { x: 32, y: 88, width: 704, height: 672 },
          measurements: { x: 768, y: 88, width: 480, height: 672 },
        },
      },
    };
    proposal.items = [
      { id: "first", type: "pump", size: [160, 120] },
      { id: "second", type: "pump", size: [160, 120] },
      { id: "level", type: "value", region: "measurements" },
    ];
    const page = composeAgentPage(proposal as unknown as CompositionInput, catalogue);
    const first = page.object.find((o) => o.id === "first")!,
      second = page.object.find((o) => o.id === "second")!,
      level = page.object.find((o) => o.id === "level")!;
    assert.deepStrictEqual(first.at, [32, 88]);
    assert.deepStrictEqual(second.at, [32, 224]);
    assert.strictEqual(level.at[0], 768);
    assert.strictEqual(level.size[0], 480);
    assert.deepStrictEqual(composeAgentPage(proposal as unknown as CompositionInput, catalogue), page);
  });
  test("pinned objects preserve their geometry and auto placement leaves their boxes free", () => {
    const proposal = {
      ...input(),
      connections: [],
      items: [
        { id: "auto", type: "pump", size: [160, 120] },
        { id: "fixed", type: "pump", size: [160, 120], pinnedAt: [32, 88] },
      ],
    };
    const page = composeAgentPage(proposal as unknown as CompositionInput, catalogue);
    const fixed = page.object.find((o) => o.id === "fixed")!,
      auto = page.object.find((o) => o.id === "auto")!;
    assert.deepStrictEqual(fixed.at, [32, 88]);
    assert.deepStrictEqual(fixed.size, [160, 120]);
    assert.deepStrictEqual(auto.at, [216, 88]);
    assert.ok(!("pinnedAt" in fixed), "layout-only fields must not enter the stored page model");
  });
  test("conflicting regions and pinned boxes fail with actionable object or region identity", () => {
    const base = { ...input(), connections: [] };
    assert.throws(
      () =>
        composeAgentPage(
          {
            ...base,
            layout: { regions: { process: { x: 32, y: 88, width: 1000, height: 672 } } },
          } as unknown as CompositionInput,
          catalogue,
        ),
      /process.*measurements|measurements.*process/i,
    );
    assert.throws(
      () =>
        composeAgentPage(
          {
            ...base,
            layout: { regions: { process: { x: 31, y: 88, width: 704, height: 672 } } },
          } as unknown as CompositionInput,
          catalogue,
        ),
      /grid/i,
    );
    assert.throws(
      () =>
        composeAgentPage(
          { ...base, items: [{ id: "fixed", type: "pump", pinnedAt: [1200, 88] }] } as unknown as CompositionInput,
          catalogue,
        ),
      /fixed.*fit/i,
    );
    assert.throws(
      () =>
        composeAgentPage(
          {
            ...base,
            items: [
              { id: "a", type: "pump", pinnedAt: [32, 88] },
              { id: "b", type: "pump", pinnedAt: [32, 88] },
            ],
          } as unknown as CompositionInput,
          catalogue,
        ),
      /a.*b|b.*a/,
    );
  });
});
