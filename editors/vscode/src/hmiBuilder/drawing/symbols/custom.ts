// The project's own symbols (specification 39, §14.4): a drawing cleaned by the language server or
// the runtime, with its bindings, ports and the parts the bindings drive, drawn as equipment is.

import { GLOW, token } from "../parts";
import { say } from "../speech";
import { el, SvgNode } from "../svg";
import type { SymbolDefinition } from "./index";
import type { PartView, Rule } from "./rules";
import { MOVING, round } from "./style";

/** A symbol of the project, as the language server and the runtime give it. */
export interface CustomSymbolModel {
  name: string;
  title: string;
  /** A new object's size. */
  size: [number, number];
  /** The drawing's own coordinates: x, y, width and height. */
  viewBox: [number, number, number, number];
  properties: Record<string, "boolean" | "number" | "text">;
  ports: Array<{ name: string; at: [number, number]; direction: "left" | "right" | "up" | "down" }>;
  /** The parts the bindings drive, as visualization rules. */
  parts: Rule[];
  /** The cleaned drawing, its root a group; a part's element carries `data-part`. */
  svg: SvgNode;
}

/** The symbol as the drawing's symbol definition: its drawing in its own coordinates, its rules
 * the parts the bindings drive, its word Fault while its `fault` binding says so. */
export function customSymbol(model: CustomSymbolModel): SymbolDefinition {
  const [x, y, width, height] = model.viewBox;
  return {
    size: [width, height],
    ports: Object.fromEntries(
      model.ports.map((port) => [
        port.name,
        { at: [port.at[0] - x, port.at[1] - y] as [number, number], direction: port.direction },
      ]),
    ),
    properties: Object.keys(model.properties),
    deciding: [],
    states: [],
    sample: "",
    read: (_value, _frame, faulted) => ({
      state: faulted ? "fault" : undefined,
      word: faulted ? say("Fault") : "",
      fault: faulted,
      view: { unknown: false },
    }),
    rules: model.parts,
    // `currentColor` in the drawing is the line colour of the built-in symbols, so a drawing that
    // uses it follows the dark and light themes and the project's colours.
    draw: (view) => [
      el("g", { transform: x === 0 && y === 0 ? undefined : `translate(${-x} ${-y})`, color: token("muted") }, [
        drawn(model.svg, view.parts ?? {}),
      ]),
    ],
  };
}

/** The drawing with each part as its rules leave it: turning, moving to and fro, at an angle,
 * filled to its level, moved along its travel, showing a value, blinking, painted in the alarm or
 * the moving colour, or hidden while its state is another. */
function drawn(node: SvgNode, parts: Record<string, PartView>, inheritedFill?: string): SvgNode {
  // A part drawn as a line (no fill, its own or its group's) is painted on its line, a filled part
  // on its fill, so the alarm and moving colours never fill an outline.
  const fill = node.attrs.fill ?? inheritedFill;
  const view = node.attrs["data-part"] !== undefined ? parts[node.attrs["data-part"]] : undefined;
  const children = node.children.map((child) => drawn(child, parts, fill));
  if (!view) return { ...node, children };
  const attrs = { ...node.attrs };
  const classes: string[] = [];
  const styles: string[] = [];
  if (view.spin) {
    classes.push("hmi-spin");
    styles.push(`animation-delay: -${view.spin.delayMs}ms`);
  }
  if (view.stroke) {
    classes.push("hmi-stroke");
    styles.push(`animation-delay: -${view.stroke.delayMs}ms`);
  }
  if (view.moved) styles.push(`transform: translate(${round(view.moved[0])}px, ${round(view.moved[1])}px)`);
  if (view.hidden) attrs.display = "none";
  if (view.blink) classes.push("hmi-blink");
  if (view.angle !== undefined)
    styles.push(`transform-box: fill-box; transform-origin: center; transform: rotate(${round(view.angle)}deg)`);
  if (view.level !== undefined)
    styles.push(`transform-box: fill-box; transform-origin: 50% 100%; transform: scaleY(${round(view.level)})`);
  const paint = view.alarm ? token("alarm") : view.active ? MOVING : undefined;
  if (paint && fill === "none") attrs.stroke = paint;
  else if (paint) attrs.fill = paint;
  // An active part is lit: it glows softly, as a lit status does.
  if (view.active && !view.alarm) attrs.filter = GLOW;
  if (classes.length > 0) attrs.class = classes.join(" ");
  if (styles.length > 0) attrs.style = styles.join("; ");
  return view.text !== undefined ? { ...node, attrs, children: [], text: view.text } : { ...node, attrs, children };
}
