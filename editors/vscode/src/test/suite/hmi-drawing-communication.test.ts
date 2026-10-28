import * as assert from "assert";

import { findAll, renderToString } from "../../hmiBuilder/drawing/svg";
import { drawObject, part, stateWord, symbolObject, valuesOf } from "./symbolTestKit";

// Specification 39, §5.2 phase 2 (communication) and §5.3: the network link's status: connected
// with its packets' age, lost, holding the last value, or fallen back to local control. Packets
// travel only while the link is connected and its status is a current observation.

const LINK = (): ReturnType<typeof symbolObject> =>
  symbolObject("network-link", ["connected", "age", "hold", "fallback"], { unit: "ms", decimals: 0, size: [160, 108] });

function sign(drawn: ReturnType<typeof drawObject>, name: string) {
  return findAll(drawn, (child) => child.attrs["data-sign"] === name)[0];
}

suite("HMI drawing: communication", () => {
  test("a connected link gives its packets' age, and its packets travel", () => {
    const drawn = drawObject(LINK(), valuesOf({ connected: true, age: 120, hold: false, fallback: false }));
    assert.strictEqual(stateWord(drawn), "Connected · 120 ms", renderToString(drawn));
    assert.match(String(part(drawn, "packets")?.attrs.class), /\bhmi-flow\b/);
    const stale = drawObject(
      LINK(),
      valuesOf({
        connected: { value: true, state: "confirmed", staleMs: 9_000 },
        age: 120,
        hold: false,
        fallback: false,
      }),
    );
    assert.doesNotMatch(
      String(part(stale, "packets")?.attrs.class ?? ""),
      /hmi-flow/,
      "nothing travels on an old status",
    );
  });

  test("a lost link is a fault, broken in the middle; one that fell back to local control says so", () => {
    const lost = drawObject(LINK(), valuesOf({ connected: false, age: 4000, hold: false, fallback: false }));
    assert.strictEqual(stateWord(lost), "Lost");
    assert.ok(sign(lost, "fault"), renderToString(lost));
    assert.ok(part(lost, "break"));
    const fallback = drawObject(LINK(), valuesOf({ connected: false, age: 4000, hold: false, fallback: true }));
    assert.strictEqual(stateWord(fallback), "Fallback");
    assert.ok(part(fallback, "fallback"), renderToString(fallback));
    assert.strictEqual(sign(fallback, "fault"), undefined, "the controller took over: no fault");
  });

  test("a link switched off on purpose says Off, out of service: no fault, the line whole", () => {
    const link = symbolObject("network-link", ["connected", "out-of-service"], { size: [160, 108] });
    const off = drawObject(link, valuesOf({ connected: false, "out-of-service": true }));
    assert.strictEqual(stateWord(off), "Off", renderToString(off));
    assert.ok(sign(off, "out-of-service"), "the out-of-service sign");
    assert.strictEqual(sign(off, "fault"), undefined, "switched off on purpose: no fault");
    assert.strictEqual(part(off, "break"), undefined, "the line is not broken");
    assert.ok(part(off, "link"), "the line stands");
    assert.strictEqual(part(off, "packets"), undefined, "nothing travels");
  });

  test("a link holding its last value says Hold and shows a pause in the line", () => {
    const hold = drawObject(LINK(), valuesOf({ connected: true, age: 900, hold: true, fallback: false }));
    assert.strictEqual(stateWord(hold), "Hold");
    assert.ok(part(hold, "hold"), renderToString(hold));
  });
});
