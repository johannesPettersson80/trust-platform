import type { SvgNode } from "../drawing/svg";
import type { ValuePaintQueue } from "./valuePaintQueue";

/** The page as last drawn into the surface, which the surface's nodes show exactly. */
export type Drawn = SvgNode;

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";

/** A style without the phase of its animations, which differs in every frame while the animation
 * itself runs on. */
function steady(style: string | undefined): string | undefined {
  return style?.replace(/animation-delay: -[0-9.]+ms;?/g, "");
}

/**
 * Draws a page into the surface. After the first time it changes the drawn page in place, only
 * where its drawing differs from the last one: a new value sets a text, and an unchanged object, a
 * turning symbol included, keeps its nodes and its animation (§19.6). The drawings are compared,
 * not the nodes, so an update reads nothing back from the page. A page whose frame or number of
 * parts changed is drawn anew.
 */
export function patchSurface(
  surface: HTMLElement,
  page: SvgNode,
  render: (node: SvgNode) => string,
  last: Drawn | undefined,
  values: ValuePaintQueue,
): Drawn {
  const element = surface.firstElementChild;
  const sameFrame =
    last !== undefined &&
    element !== null &&
    last.children.length === page.children.length &&
    element.children.length === page.children.length &&
    sameAttributes(last.attrs, page.attrs);
  if (!sameFrame) {
    values.clear();
    surface.innerHTML = render(page);
    return page;
  }
  page.children.forEach((child, index) => {
    const before = last.children[index],
      node = element.children[index];
    // A whole unchanged-state value card may wait one frame. Controls, alarm/state changes,
    // geometry changes and structural replacements retain immediate painting.
    if (
      /\bhmi-value\b/.test(child.attrs.class ?? "") &&
      child.attrs["data-state"] === "confirmed" &&
      child.attrs["data-stale"] !== "true" &&
      sameAttributes(before.attrs, child.attrs)
    ) {
      if (!sameDrawing(before, child)) values.enqueue(node, before, child);
    } else values.immediate(node, before, child);
  });
  return page;
}

function sameDrawing(before: SvgNode, after: SvgNode): boolean {
  return (
    before.tag === after.tag &&
    before.text === after.text &&
    before.children.length === after.children.length &&
    sameAttributes(before.attrs, after.attrs) &&
    before.children.every((child, index) => sameDrawing(child, after.children[index]))
  );
}

function sameAttributes(before: Record<string, string>, after: Record<string, string>): boolean {
  const names = Object.keys(after);
  return names.length === Object.keys(before).length && names.every((name) => before[name] === after[name]);
}

/** Brings a drawn node, which shows `before`, to `after` in place; a part of another shape is
 * drawn anew. */
export function updateNode(drawn: Element, before: SvgNode, after: SvgNode, render: (node: SvgNode) => string): void {
  if (
    before.tag !== after.tag ||
    before.children.length !== after.children.length ||
    (before.text === undefined) !== (after.text === undefined)
  ) {
    redrawNode(drawn, after, render);
    return;
  }
  for (const [name, value] of Object.entries(after.attrs)) {
    const was = before.attrs[name];
    if (was === value) continue;
    if (name === "style" && steady(was) === steady(value)) continue;
    drawn.setAttribute(name, value);
  }
  for (const name of Object.keys(before.attrs)) {
    if (!(name in after.attrs)) drawn.removeAttribute(name);
  }
  if (after.text !== undefined) {
    if (before.text !== after.text) {
      // Retain SVG inline text layout objects instead of removing and attaching a new text node.
      const text = drawn.firstChild;
      if (text?.nodeType === 3 && drawn.childNodes.length === 1) text.nodeValue = after.text;
      else drawn.textContent = after.text;
    }
    return;
  }
  if (drawn.children.length !== after.children.length) {
    redrawNode(drawn, after, render);
    return;
  }
  after.children.forEach((child, index) => updateNode(drawn.children[index], before.children[index], child, render));
}

function redrawNode(drawn: Element, node: SvgNode, render: (node: SvgNode) => string): void {
  const holder = drawn.ownerDocument.createElementNS(SVG_NAMESPACE, "svg");
  holder.innerHTML = render(node);
  const fresh = holder.firstElementChild;
  if (fresh) drawn.replaceWith(fresh);
}
