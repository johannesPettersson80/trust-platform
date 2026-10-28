// A small SVG node tree: the drawing module builds it without a DOM, so it runs the same in the
// editor, in tests and in the runtime page, where it is mounted.

export interface SvgNode {
  tag: string;
  attrs: Record<string, string>;
  children: SvgNode[];
  text?: string;
}

export function el(
  tag: string,
  attrs: Record<string, string | number | undefined> = {},
  children: SvgNode[] = [],
): SvgNode {
  const clean: Record<string, string> = {};
  for (const [name, value] of Object.entries(attrs)) {
    if (value !== undefined) clean[name] = String(value);
  }
  return { tag, attrs: clean, children };
}

export function text(content: string, attrs: Record<string, string | number | undefined> = {}): SvgNode {
  return { ...el("text", attrs), text: content };
}

/** A span of a text run. */
export function tspan(content: string, attrs: Record<string, string | number | undefined> = {}): SvgNode {
  return { ...el("tspan", attrs), text: content };
}

/** One text element made of spans laid out one after the other by the browser. */
export function run(attrs: Record<string, string | number | undefined>, spans: SvgNode[]): SvgNode {
  return el("text", attrs, spans);
}

/** Every node of the tree, depth first, that matches. */
export function findAll(node: SvgNode, match: (node: SvgNode) => boolean): SvgNode[] {
  const found: SvgNode[] = [];
  const visit = (current: SvgNode): void => {
    if (match(current)) found.push(current);
    current.children.forEach(visit);
  };
  visit(node);
  return found;
}

/** The text contents of a tree, in drawing order. */
export function textsOf(node: SvgNode): string[] {
  return findAll(node, (current) => current.text !== undefined).map((current) => current.text ?? "");
}

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };

function escape(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ESCAPES[character]);
}

/** SVG markup for a tree. */
export function renderToString(node: SvgNode): string {
  const attrs = Object.entries(node.attrs)
    .map(([name, value]) => ` ${name}="${escape(value)}"`)
    .join("");
  const inner = (node.text !== undefined ? escape(node.text) : "") + node.children.map(renderToString).join("");
  return `<${node.tag}${attrs}>${inner}</${node.tag}>`;
}
