// Shared helpers for the symbol tests: one object drawn on a page, bound to given values, and its
// parts, signs and words.

import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import type { DrawFrame, HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";
import { LABEL_HEIGHT, SYMBOLS, SymbolType } from "../../hmiBuilder/drawing/symbols";

/** The object drawn on its own, as the page draws it. */
export function drawObject(object: HmiObjectModel, values: Record<string, LiveValue>, frame: Partial<DrawFrame> = {}): SvgNode {
  const svg = drawPage({ title: "Symbols", object: [object], pipe: [] }, { size: [1280, 720], values, nowMs: 0, ...frame });
  const found = findAll(svg, (node) => node.attrs["data-object"] === object.id);
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

/** A symbol at its design size with room for its label, its properties bound to `<tag>.<name>`. */
export function symbolObject(type: SymbolType, properties: string[], extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  const [width, height] = SYMBOLS[type].size;
  const bind = Object.fromEntries(properties.map((name) => [name, { path: `X.${name}`, direction: "read" as const }]));
  return { id: type, type, tag: "X-1", at: [0, 0], size: [width, height + LABEL_HEIGHT], rotation: 0, bind, ...extra };
}

/** Values for `symbolObject`'s bindings, by property name. */
export function valuesOf(values: Record<string, LiveValue["value"] | LiveValue>): Record<string, LiveValue> {
  return Object.fromEntries(
    Object.entries(values).map(([name, value]) => [`X.${name}`, typeof value === "object" && value !== null && "state" in value ? (value as LiveValue) : { value: value as LiveValue["value"], state: "confirmed" }]),
  );
}

export function part(node: SvgNode, name: string): SvgNode | undefined {
  return findAll(node, (child) => child.attrs["data-part"] === name)[0];
}

/** The text drawn in a part, as one string. */
export function words(node: SvgNode | undefined): string {
  return node ? textsOf(node).join(" ") : "";
}

/** The state word under the tag. */
export function stateWord(drawn: SvgNode): string {
  return words(part(drawn, "state"));
}
