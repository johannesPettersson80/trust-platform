// Turning parts in a layer of their own (specification 39, §19.6; §18 phase 5 item 4). The browser
// restyles and repaints a page's whole drawing for every frame of an animation inside it, so the
// turning impellers kept the main thread busy. Each turning part is drawn instead in an element of
// its own above the page, which the browser turns by itself, and the page's own part is set aside.
// A part is measured when it starts turning and again when its object moves; a change of its
// drawing is copied over, and a part that stops is given back to the page.

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
/** The parts that turn (`hmi-spin` in drawing/tokens.ts). */
const TURNING = ".hmi-spin";

/** The layer's styles, with the display's. */
export const MOTION_LAYER_CSS = `.hmi-surface { position: relative; }
[data-motion-layer] { position: absolute; left: 0; top: 0; width: 0; height: 0; overflow: visible; pointer-events: none; }
[data-motion-layer] > svg { position: absolute; overflow: visible; will-change: transform; }
[data-moved] { visibility: hidden; animation: none; }`;

/** Where a turning part stands: its box in its own drawing, and the page's transform of it. */
export interface Placement {
  box: { x: number; y: number; width: number; height: number };
  matrix: { a: number; b: number; c: number; d: number; e: number; f: number };
}

/** How the layer measures a part; none where the page is not laid out. */
export type Measure = (part: Element) => Placement | undefined;

/** The browser's measure of a part. */
export function measured(part: Element): Placement | undefined {
  const graphic = part as SVGGraphicsElement;
  if (typeof graphic.getBBox !== "function" || typeof graphic.getCTM !== "function") return undefined;
  const box = graphic.getBBox();
  // getCTM stops at the nearest SVG viewport, which can be a scaled faceplate.
  // Compose through screen space into the page viewport that owns the motion layer.
  const root = graphic.closest<SVGSVGElement>("svg.hmi-page");
  const screen = graphic.getScreenCTM?.();
  const rootScreen = root?.getScreenCTM?.();
  const rootViewport = root?.getCTM?.();
  const matrix =
    screen && rootScreen && rootViewport
      ? rootViewport.multiply(rootScreen.inverse()).multiply(screen)
      : graphic.getCTM();
  if (!matrix || !(box.width > 0) || !(box.height > 0)) return undefined;
  return {
    box: { x: box.x, y: box.y, width: box.width, height: box.height },
    matrix: { a: matrix.a, b: matrix.b, c: matrix.c, d: matrix.d, e: matrix.e, f: matrix.f },
  };
}

/** A lifted part: its element, and its drawing and its object's place when last copied. */
interface Lifted {
  element: SVGSVGElement;
  drawing: string;
  place: string;
}

export class MotionLayer {
  private layer: HTMLElement | undefined;
  private readonly lifted = new Map<Element, Lifted>();

  constructor(
    private readonly surface: HTMLElement,
    private readonly measure: Measure = measured,
  ) {}

  /** After each drawing of the page: an element for each turning part, the stopped ones given back. */
  sync(): void {
    const page = this.surface.querySelector("svg.hmi-page");
    const turning = new Set(page ? Array.from(page.querySelectorAll(TURNING)) : []);
    for (const [part, lifted] of this.lifted) {
      if (turning.has(part) && part.isConnected) continue;
      this.giveBack(part, lifted);
    }
    for (const part of turning) {
      const place = placeOf(part);
      const known = this.lifted.get(part);
      if (known && known.place === place) {
        if (known.drawing !== part.innerHTML) {
          copyDrawing(known.element, part);
          known.drawing = part.innerHTML;
        }
        continue;
      }
      const placement = this.measure(part);
      if (!placement) {
        if (known) this.giveBack(part, known);
        continue;
      }
      const element = known?.element ?? this.created(part);
      copyDrawing(element, part);
      position(element, part, placement);
      this.lifted.set(part, { element, drawing: part.innerHTML, place });
      part.setAttribute("data-moved", "");
    }
  }

  private giveBack(part: Element, lifted: Lifted): void {
    lifted.element.remove();
    part.removeAttribute("data-moved");
    this.lifted.delete(part);
  }

  private created(part: Element): SVGSVGElement {
    const document = this.surface.ownerDocument;
    if (!this.layer?.isConnected) {
      this.layer = document.createElement("div");
      this.layer.setAttribute("data-motion-layer", "");
      this.surface.append(this.layer);
    }
    const element = document.createElementNS(SVG_NAMESPACE, "svg") as SVGSVGElement;
    element.setAttribute("class", "hmi-spin");
    element.setAttribute("data-motion", part.closest("[data-object]")?.getAttribute("data-object") ?? "");
    element.append(document.createElementNS(SVG_NAMESPACE, "g"));
    this.layer.append(element);
    return element;
  }
}

/** Where a part's object stands: the transforms above the part, which an update may change. */
function placeOf(part: Element): string {
  const transforms: string[] = [];
  for (let node = part.parentElement; node; node = node.parentElement) {
    transforms.push(node.getAttribute("transform") ?? "");
    if (node.tagName.toLowerCase() === "svg") {
      transforms.push(
        ...["x", "y", "width", "height", "viewBox", "preserveAspectRatio"].map(
          (name) => node!.getAttribute(name) ?? "",
        ),
      );
    }
    if (node.matches("svg.hmi-page")) break;
  }
  return transforms.join("|");
}

/** The part's drawing, as the layer's element shows it. */
function copyDrawing(element: SVGSVGElement, part: Element): void {
  const turned = element.firstElementChild;
  if (!turned) return;
  turned.replaceChildren(...Array.from(part.childNodes, (node) => node.cloneNode(true)));
}

/** Puts the element over the part: its centre, its size and its turn on the page, its phase. */
function position(element: SVGSVGElement, part: Element, { box, matrix: m }: Placement): void {
  const centreX = box.x + box.width / 2;
  const centreY = box.y + box.height / 2;
  const x = m.a * centreX + m.c * centreY + m.e;
  const y = m.b * centreX + m.d * centreY + m.f;
  const scale = Math.hypot(m.a, m.b);
  const angle = (Math.atan2(m.b, m.a) * 180) / Math.PI;
  const width = box.width * scale;
  const height = box.height * scale;
  element.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${box.height}`);
  element.style.left = `${round(x - width / 2)}px`;
  element.style.top = `${round(y - height / 2)}px`;
  element.style.width = `${round(width)}px`;
  element.style.height = `${round(height)}px`;
  const delay = /animation-delay:\s*([^;]+)/.exec(part.getAttribute("style") ?? "")?.[1]?.trim();
  if (delay) element.style.animationDelay = delay;
  const turned = element.firstElementChild;
  if (!turned) return;
  if (Math.abs(angle) > 0.01) turned.setAttribute("transform", `rotate(${round(angle)} ${centreX} ${centreY})`);
  else turned.removeAttribute("transform");
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
