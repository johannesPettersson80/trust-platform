import * as assert from "assert";
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";

import { fitWidth, textWidth } from "../../hmiBuilder/drawing/format";
import { FONT_SHA256 } from "../../hmiBuilder/drawing/fontMetrics";
import { tokensCss } from "../../hmiBuilder/drawing/tokens";

// Specification 39, §14.1 item 4 and §14.2: the drawing measures every text with the widths of the
// font it ships, so a text of any words and any length is laid out where it really ends.

const ROOT = path.resolve(__dirname, "../../../../..");

suite("HMI drawing: text measurement", () => {
  test("a text is as wide as its letters in the shipped font, not as its letter count", () => {
    assert.ok(
      textWidth("WWWW", 14) > 2 * textWidth("iiii", 14),
      `W ${textWidth("WWWW", 14)}, i ${textWidth("iiii", 14)}`,
    );
    // Inter's "%" is nearly a full em wide.
    assert.ok(textWidth("62 %", 14) > 30, String(textWidth("62 %", 14)));
  });

  test("a heavier weight is wider, as the font is", () => {
    assert.ok(
      textWidth("Running", 14, 700) > textWidth("Running", 14, 400),
      `${textWidth("Running", 14, 700)} > ${textWidth("Running", 14, 400)}`,
    );
  });

  test("letters outside the table are measured wide rather than narrow, so they never overflow unseen", () => {
    assert.ok(textWidth("運転中", 14) >= 3 * 14 * 0.95, String(textWidth("運転中", 14)));
  });

  test("a text cut to fit ends with … and fits", () => {
    const cut = fitWidth("Fuera de servicio", 80, 14);
    assert.ok(cut.endsWith("…"), cut);
    assert.ok(textWidth(cut, 14) <= 80, `${cut}: ${textWidth(cut, 14)}`);
    assert.strictEqual(fitWidth("Open", 80, 14), "Open");
  });

  test("the widths belong to the font the HMI ships, and the text keeps them at every size", () => {
    const font = fs.readFileSync(path.join(ROOT, "editors/vscode/media/fonts/InterVariable.woff2"));
    assert.strictEqual(
      crypto.createHash("sha256").update(font).digest("hex"),
      FONT_SHA256,
      "the font changed; run scripts/build-font-metrics.py",
    );
    // Optical sizing would change the widths with the size; the HMI keeps the text's shapes.
    assert.match(tokensCss({}), /\.hmi-page \{[^}]*font-optical-sizing: none/);
  });
});
