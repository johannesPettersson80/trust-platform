import * as assert from "assert";

import { adaptPage } from "../../hmiBuilder/drawing/adaptive";
import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";

// Specification 39, §4 item 5: a page on a page area smaller than its design size is shown at
// scale 1 and laid out again at the area's width: blocks that do not fit move, a block too wide
// scales or, arranged, rearranges its objects; nothing overlaps and controls stay 44 × 44 px.

const DESIGN: [number, number] = [1688, 1024];

function object(
  id: string,
  type: string,
  at: [number, number],
  size: [number, number],
  extra: Partial<HmiObjectModel> = {},
): HmiObjectModel {
  return { id, type, at, size, rotation: 0, bind: {}, ...extra };
}

/** Four buttons in a row inside a titled panel, 16 px apart. */
function buttons(x: number, y: number): HmiObjectModel[] {
  return [0, 1, 2, 3].map((index) =>
    object(`b${index}`, "button", [x + 16 + index * 216, y + 44], [200, 56], { text: `Step ${index}` }),
  );
}

const PAGE: HmiPageModel = {
  title: "Overview",
  object: [
    // The process graphic: a pump, a tank and a value, the pipe between them with a bend.
    object("p1", "pump", [40, 40], [128, 108]),
    object("t1", "tank", [220, 40], [128, 172]),
    object("v1", "value", [40, 260], [160, 48]),
    // A titled panel of four buttons, arranged in a row.
    object("steps", "panel", [760, 40], [880, 116], { label: "Steps", arrange: "row" }),
    ...buttons(760, 40),
    // A group with a trend, not arranged.
    object("trends", "group", [40, 360], [1580, 400], { label: "Trends" }),
    object("tr", "trend", [56, 400], [1548, 344]),
  ],
  pipe: [{ from: "p1.outlet", to: "t1.inlet", waypoints: [[200, 94]] }],
};

const find = (page: HmiPageModel, id: string): HmiObjectModel => {
  const found = page.object.find((item) => item.id === id);
  assert.ok(found, id);
  return found;
};

const right = (item: HmiObjectModel): number => item.at[0] + item.size[0];
const bottom = (item: HmiObjectModel): number => item.at[1] + item.size[1];

function overlaps(a: HmiObjectModel, b: HmiObjectModel): boolean {
  return a.at[0] < right(b) && b.at[0] < right(a) && a.at[1] < bottom(b) && b.at[1] < bottom(a);
}

/** The blocks of an adapted page, which must not overlap and must lie within its width: the panel,
 * the group, the value on its own and the process graphic of the pump and the tank. */
function assertLaidOut(page: HmiPageModel, width: number): void {
  const blocks = ["steps", "trends", "v1"].map((id) => find(page, id));
  const process = ["p1", "t1"].map((id) => find(page, id));
  const box = object(
    "process",
    "group",
    [Math.min(...process.map((item) => item.at[0])), Math.min(...process.map((item) => item.at[1]))],
    [0, 0],
  );
  box.size = [Math.max(...process.map(right)) - box.at[0], Math.max(...process.map(bottom)) - box.at[1]];
  const all = [box, ...blocks];
  for (const [index, a] of all.entries()) {
    assert.ok(a.at[0] >= 0 && right(a) <= width, `${a.id} lies within ${width}: ${JSON.stringify(a)}`);
    for (const b of all.slice(index + 1)) assert.ok(!overlaps(a, b), `${a.id} and ${b.id} overlap`);
  }
  for (const id of ["b0", "b1", "b2", "b3"]) {
    const button = find(page, id);
    assert.ok(button.size[0] >= 44 && button.size[1] >= 44, `${id} keeps its touch size`);
    const panel = find(page, "steps");
    assert.ok(
      button.at[0] >= panel.at[0] &&
        right(button) <= right(panel) &&
        button.at[1] >= panel.at[1] &&
        bottom(button) <= bottom(panel),
      `${id} stays in its panel`,
    );
  }
}

suite("HMI drawing: adaptive display", () => {
  test("a page area at least the design size shows the page as designed, scaled to fit", () => {
    const adapted = adaptPage(PAGE, DESIGN, [1688, 1024]);
    assert.deepStrictEqual(adapted, { page: PAGE, size: DESIGN, fit: true });
    assert.strictEqual(adaptPage(PAGE, DESIGN, [1864, 1024]).fit, true);
  });

  test("a shorter page area shows the page at scale 1 as designed, to scroll", () => {
    const adapted = adaptPage(PAGE, DESIGN, [1864, 744]);
    assert.strictEqual(adapted.fit, false);
    assert.deepStrictEqual(adapted.size, DESIGN);
    assert.deepStrictEqual(adapted.page, PAGE);
  });

  test("at 1280 px a block that does not fit moves left, and one too wide scales with its objects and pipes", () => {
    const adapted = adaptPage(PAGE, DESIGN, [1280, 744]);
    assert.strictEqual(adapted.fit, false);
    assert.strictEqual(adapted.size[0], 1280);
    const page = adapted.page;
    assert.deepStrictEqual(find(page, "p1").at, [40, 40], "the process graphic fits and stays");
    assert.deepStrictEqual(page.pipe[0].waypoints, [[200, 94]]);
    // The panel moves left until it fits: 1280 less the 40 px margin less its 880 px.
    assert.deepStrictEqual(find(page, "steps").at, [360, 40]);
    assert.deepStrictEqual(find(page, "b0").at, [376, 84], "its buttons move with it");
    // The trend group, 1580 px wide, scales to the 1200 px between the margins.
    const trends = find(page, "trends");
    assert.deepStrictEqual(trends.at, [40, 360]);
    assert.deepStrictEqual(trends.size, [1200, 304]);
    assert.deepStrictEqual(find(page, "tr").at, [52, 390]);
    assert.deepStrictEqual(find(page, "tr").size, [1176, 261]);
    assertLaidOut(page, 1280);
  });

  test("a block that would overlap once moved left starts again at the left margin, below what it overlaps", () => {
    // At 1100 px the panel would fit from 180, over the process graphic, which ends at 348 and
    // 212; the value designed below the pump then makes way for it.
    const page = adaptPage(PAGE, DESIGN, [1100, 744]).page;
    assert.deepStrictEqual(find(page, "steps").at, [40, 228]);
    assert.deepStrictEqual(find(page, "b0").at, [56, 272]);
    assert.deepStrictEqual(find(page, "v1").at, [40, 360]);
    assertLaidOut(page, 1100);
  });

  test("on a tablet in portrait the arranged panel wraps its buttons, and the blocks stack without overlapping", () => {
    const adapted = adaptPage(PAGE, DESIGN, [820, 1124]);
    assert.strictEqual(adapted.size[0], 820);
    const page = adapted.page;
    const steps = find(page, "steps");
    assert.strictEqual(steps.size[0], 740, "the panel takes the width between the margins");
    // Three 200 px buttons fit in the 708 px inside its padding; the fourth starts a new row.
    assert.deepStrictEqual(
      ["b0", "b1", "b2", "b3"].map((id) => find(page, id).size),
      [
        [200, 56],
        [200, 56],
        [200, 56],
        [200, 56],
      ],
      "the buttons keep their size",
    );
    const [b0, b1, b2, b3] = ["b0", "b1", "b2", "b3"].map((id) => find(page, id));
    assert.strictEqual(b1.at[1], b0.at[1]);
    assert.strictEqual(b2.at[1], b0.at[1]);
    assert.strictEqual(b1.at[0] - right(b0), 16, "16 px apart");
    assert.strictEqual(b3.at[0], b0.at[0], "the fourth starts the next row");
    assert.strictEqual(b3.at[1], bottom(b0) + 16);
    assert.strictEqual(bottom(steps), bottom(b3) + 16, "the panel grows to hold them");
    assertLaidOut(page, 820);
    assert.ok(
      adapted.size[1] >= Math.max(...page.object.map(bottom)),
      "the page is tall enough to scroll to everything",
    );
  });

  test("a page without panels or groups: its displays and controls flow on their own, its equipment scales as the graphic", () => {
    const loose: HmiPageModel = {
      title: "Loose",
      object: [
        object("p1", "pump", [40, 40], [128, 108]),
        object("t1", "tank", [600, 40], [128, 172]),
        object("start", "button", [800, 40], [200, 48], { text: "Start" }),
        object("sp", "setpoint", [1040, 40], [240, 96]),
        object("trend", "trend", [40, 300], [1240, 300]),
      ],
      pipe: [{ from: "p1.outlet", to: "t1.inlet" }],
    };
    const page = adaptPage(loose, [1320, 640], [820, 640]).page;
    // The graphic, 688 px wide, fits the 740 px between the margins and stays.
    assert.deepStrictEqual(
      [find(page, "p1").at, find(page, "t1").at],
      [
        [40, 40],
        [600, 40],
      ],
    );
    const [start, sp, trend] = ["start", "sp", "trend"].map((id) => find(page, id));
    assert.deepStrictEqual(
      [start.size, sp.size],
      [
        [200, 48],
        [240, 96],
      ],
      "the controls keep their size",
    );
    assert.ok(
      start.at[1] >= 212 + 16 && sp.at[1] >= 212 + 16,
      `below the graphic: ${JSON.stringify([start.at, sp.at])}`,
    );
    assert.strictEqual(trend.size[0], 740, "the trend scales to the width between the margins");
    for (const control of [start, sp])
      assert.ok(
        control.at[0] >= 40 && control.at[0] + control.size[0] <= 780,
        `within the width: ${JSON.stringify(control)}`,
      );
    assert.ok(
      !(
        start.at[0] < sp.at[0] + sp.size[0] &&
        sp.at[0] < start.at[0] + start.size[0] &&
        start.at[1] < sp.at[1] + sp.size[1] &&
        sp.at[1] < start.at[1] + start.size[1]
      ),
      "they do not overlap",
    );
  });

  test("narrower, a design's columns follow one another: the graphic with its values, then the controls beside it, then what lies below", () => {
    const columns: HmiPageModel = {
      title: "Columns",
      object: [
        object("p1", "pump", [40, 40], [128, 108]),
        object("t1", "tank", [220, 40], [128, 172]),
        object("level", "value", [40, 260], [200, 56]),
        object("panel", "panel", [820, 40], [400, 116], { label: "Pump" }),
        object("go", "button", [836, 84], [160, 48], { text: "Start" }),
        object("group", "group", [820, 180], [400, 196], { label: "Values" }),
        object("g1", "value", [836, 224], [184, 56]),
        object("trend", "trend", [40, 420], [1180, 300]),
      ],
      pipe: [],
    };
    const page = adaptPage(columns, [1260, 760], [588, 1124]).page;
    const [level, panel, group, trend] = ["level", "panel", "group", "trend"].map((id) => find(page, id));
    assert.deepStrictEqual(level.at, [40, 260], "the value stays under its equipment, as designed");
    assert.ok(panel.at[1] >= bottom(level) + 16, `the panel after the graphic's column: ${JSON.stringify(panel.at)}`);
    assert.ok(group.at[1] >= bottom(panel) + 16, "the group under the panel, as designed");
    assert.ok(trend.at[1] >= bottom(group) + 16, "the trend last, as it was below both columns");
  });

  test("a grid keeps its cells and takes as many columns as fit; a column stacks its objects", () => {
    const values = [0, 1, 2, 3, 4, 5].map((index) =>
      object(`g${index}`, "value", [56 + (index % 3) * 176, 100 + Math.floor(index / 3) * 64], [160, 48]),
    );
    const grid: HmiPageModel = {
      title: "Grid",
      object: [object("grid", "group", [40, 40], [560, 200], { label: "Values", arrange: "grid" }), ...values],
      pipe: [],
    };
    const narrow = adaptPage(grid, [640, 400], [480, 400]).page;
    const [g0, g1, g2] = ["g0", "g1", "g2"].map((id) => find(narrow, id));
    assert.strictEqual(g1.at[1], g0.at[1], "two columns fit in the 368 px inside");
    assert.strictEqual(g2.at[0], g0.at[0], "the third starts the next row");
    const column: HmiPageModel = {
      ...grid,
      object: [object("grid", "group", [40, 40], [560, 200], { label: "Values", arrange: "column" }), ...values],
    };
    const stacked = adaptPage(column, [640, 400], [480, 400]).page;
    const ys = ["g0", "g1", "g2", "g3", "g4", "g5"].map((id) => find(stacked, id).at[1]);
    assert.deepStrictEqual(
      ys,
      [...ys].sort((a, b) => a - b),
      "one under the other, in reading order",
    );
    assert.ok(
      new Set(["g0", "g1", "g2", "g3", "g4", "g5"].map((id) => find(stacked, id).at[0])).size === 1,
      "aligned at the left",
    );
  });

  test("a block holding controls scales no further than keeps them 44 px, and the page then scrolls sideways", () => {
    const panel: HmiPageModel = {
      title: "Panel",
      object: [
        object("box", "panel", [20, 20], [1200, 100]),
        object("go", "button", [36, 36], [1100, 48], { text: "Go" }),
      ],
      pipe: [],
    };
    const adapted = adaptPage(panel, [1240, 400], [400, 400]);
    assert.strictEqual(adapted.fit, false);
    // 360 px would need a scale of 0.3; the 48 px button allows 44 / 48 at the least.
    assert.deepStrictEqual(find(adapted.page, "go").size, [1008, 44], "the button stays 44 px high");
    assert.deepStrictEqual(find(adapted.page, "box").size, [1100, 92]);
    assert.strictEqual(adapted.size[0], 1140, "the page is wider than the area, to scroll sideways");
  });
});
