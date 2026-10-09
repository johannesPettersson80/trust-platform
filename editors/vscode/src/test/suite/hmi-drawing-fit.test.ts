import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { drawPage } from "../../hmiBuilder/drawing";
import { textWidth } from "../../hmiBuilder/drawing/format";
import type { HmiObjectModel } from "../../hmiBuilder/drawing/model";
import { findAll, SvgNode } from "../../hmiBuilder/drawing/svg";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import { designValues } from "../../hmiBuilder/editor/designSamples";

// Specification 39, §14.5 item 6: text that does not fit its object is cut with "…", in every
// component, so a longer word in another language never runs out of its object.

const ROOT = path.resolve(__dirname, "../../../../..");
const CATALOGUE: Catalogue = JSON.parse(
  fs.readFileSync(path.join(ROOT, "crates/trust-runtime/src/hmi/catalogue/components.json"), "utf8"),
);
const LONG =
  "A label far too long for any object of its size to hold on one line, written twice to be sure: a label far too long for any object";

interface Extent {
  text: string;
  left: number;
  right: number;
}

/** Every text an object draws and where it starts and ends across, from the translations of its
 * groups, its anchor and the widths of the font. A scaled group is a symbol's drawing, never text. */
function extents(node: SvgNode, offset = 0): Extent[] {
  const transform = node.attrs.transform ?? "";
  if (/scale\(|rotate\(/.test(transform)) return [];
  const moved = /translate\(\s*(-?[\d.]+)/.exec(transform);
  const here = offset + (moved ? Number(moved[1]) : 0);
  if (node.tag === "text") {
    const spans = node.text !== undefined ? [node] : node.children.filter((child) => child.text !== undefined);
    const size = (span: SvgNode): number => Number(span.attrs["font-size"] ?? node.attrs["font-size"] ?? 14);
    const weight = (span: SvgNode): number => Number(span.attrs["font-weight"] ?? node.attrs["font-weight"] ?? 400);
    const width = spans.reduce((sum, span) => sum + textWidth(span.text ?? "", size(span), weight(span)), 0);
    const x = here + Number(node.attrs.x ?? 0);
    const anchor = node.attrs["text-anchor"] ?? "start";
    const left = anchor === "middle" ? x - width / 2 : anchor === "end" ? x - width : x;
    return [{ text: spans.map((span) => span.text).join(""), left, right: left + width }];
  }
  return node.children.flatMap((child) => extents(child, here));
}

suite("HMI drawing: text fits its object", () => {
  test("every component cuts a label, text or tag too long for it with …; no text runs out of its object (§14.5 item 6)", () => {
    const failures: string[] = [];
    for (const component of CATALOGUE.components) {
      const bind = Object.fromEntries(
        Object.keys(component.properties ?? {}).map((property) => [
          property,
          { path: `P.${property}`, direction: "read" as const },
        ]),
      );
      const [width, height] = component.size as [number, number];
      // The default size, and wider and taller ones, so every layout that draws a label is drawn.
      const sizes: Array<[number, number]> = [
        [width, height],
        [Math.max(width, 240), height],
        [Math.max(width, 240), Math.max(height, 120)],
      ];
      for (const size of sizes) {
        const object: HmiObjectModel = {
          id: "o",
          type: component.type,
          at: [0, 0],
          size,
          rotation: 0,
          bind,
          label: LONG,
          text: LONG,
          tag: LONG,
        };
        const page = { title: "Fit", object: [object], pipe: [] };
        const svg = drawPage(page, { size: [2400, 1200], values: designValues(page, CATALOGUE) });
        const drawn = findAll(svg, (node) => node.attrs["data-object"] === "o")[0];
        for (const extent of drawn ? extents(drawn) : []) {
          if (extent.left < -1 || extent.right > size[0] + 1) {
            failures.push(
              `${component.type} ${size.join("×")}: "${extent.text.slice(0, 24)}" spans ${Math.round(extent.left)}..${Math.round(extent.right)}`,
            );
          }
        }
      }
    }
    assert.deepStrictEqual(failures, []);
  });
});
