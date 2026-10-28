import * as assert from "assert";

import { DEFAULT_ARRANGEMENT, normalizeArrangement, shownArrangement } from "../../hmiBuilder/editor/arrangement";

// Specification 39, §3.2 items 6–7: the panel arrangement, its limits and a narrow editor.

suite("HMI Builder arrangement", () => {
  test("the first view shows Components, the canvas and the inspector, with the bottom panel closed", () => {
    assert.deepStrictEqual(normalizeArrangement(undefined), {
      left: { width: 264, collapsed: false },
      right: { width: 300, collapsed: false },
      bottom: { open: false, height: 220 },
    });
  });

  test("a remembered arrangement is kept within the limits", () => {
    const stored = {
      left: { width: 50, collapsed: true },
      right: { width: 9999, collapsed: false },
      bottom: { open: true, height: "tall" },
    };
    assert.deepStrictEqual(normalizeArrangement(stored), {
      left: { width: 200, collapsed: true },
      right: { width: 520, collapsed: false },
      bottom: { open: true, height: 220 },
    });
    assert.deepStrictEqual(normalizeArrangement("nonsense"), DEFAULT_ARRANGEMENT);
  });

  test("open panels leave the rest to the canvas, and a collapsed one is a strip", () => {
    const shown = shownArrangement(DEFAULT_ARRANGEMENT, 1400, 900);
    assert.deepStrictEqual([shown.left, shown.right, shown.narrow], [264, 300, false]);
    const collapsed = shownArrangement({ ...DEFAULT_ARRANGEMENT, right: { width: 300, collapsed: true } }, 1400, 900);
    assert.deepStrictEqual([collapsed.right, collapsed.rightCollapsed], [40, true]);
  });

  test("a narrow editor shows the side panels as strips, one opened from its strip, and wide again the arrangement comes back", () => {
    const narrow = shownArrangement(DEFAULT_ARRANGEMENT, 900, 900);
    assert.deepStrictEqual([narrow.left, narrow.right, narrow.narrow], [40, 40, true]);
    const peek = shownArrangement(DEFAULT_ARRANGEMENT, 900, 900, "left");
    assert.deepStrictEqual([peek.left, peek.leftCollapsed, peek.right, peek.rightCollapsed], [264, false, 40, true]);
    const wide = shownArrangement(DEFAULT_ARRANGEMENT, 1400, 900, "left");
    assert.deepStrictEqual([wide.left, wide.right, wide.narrow], [264, 300, false]);
  });

  test("the bottom panel is at least 120 px and at most half the editor's height", () => {
    const tall = { ...DEFAULT_ARRANGEMENT, bottom: { open: true, height: 600 } };
    assert.strictEqual(shownArrangement(tall, 1400, 800).bottomHeight, 400);
    const short = { ...DEFAULT_ARRANGEMENT, bottom: { open: true, height: 40 } };
    assert.strictEqual(shownArrangement(short, 1400, 800).bottomHeight, 120);
  });
});
