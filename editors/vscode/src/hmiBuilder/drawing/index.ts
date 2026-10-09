// The HMI Builder's drawing module (specification 39, §3.4, §5.3): one implementation shared by
// the editor canvas, Live mode and the runtime display.

import { withValueFormat } from "./valueFormats";
import { drawAttachments } from "./attachments";
import { drawPipes } from "./pipes";
import { drawSignals } from "./signals";
import { translatedPage } from "./translated";
import { drawingObject } from "./cuts";
import { equipmentBadge } from "./alarms";
import { drawObject } from "./components";
import type { DrawFrame, HmiObjectModel, HmiPageModel } from "./model";
import { textWidth } from "./format";
import { backdrop, definitions, token } from "./parts";
import { el, SvgNode, text } from "./svg";
import { say, withSpeech } from "./speech";
import { SYMBOLS } from "./symbols";
import { liveValueOf } from "./values";
import { drawPlaced, PLACED_FACEPLATE } from "./placed";

/** Draws a page from its model and live values. */
export function drawPage(page: HmiPageModel, frame: DrawFrame): SvgNode {
  return withSpeech(frame, () => drawSpoken(translatedPage(page), frame));
}

function drawSpoken(page: HmiPageModel, frame: DrawFrame): SvgNode {
  const [width, height] = frame.size;
  return el(
    "svg",
    { xmlns: "http://www.w3.org/2000/svg", class: "hmi-page", viewBox: `0 0 ${width} ${height}`, width, height },
    [
      definitions(),
      ...backdrop(width, height),
      ...(page.regions?.length ? drawRegions(page, frame) : drawContent(page, frame)),
    ],
  );
}

/** The native composition supplies non-overlapping, clipped regions. Coordinates stay local
 * to each authored definition in both editor and operator display. */
function drawRegions(page: HmiPageModel, frame: DrawFrame): SvgNode[] {
  return (page.regions ?? []).map((region) => {
    const ids = new Set(region.objects);
    const part: HmiPageModel = {
      ...page,
      regions: undefined,
      object: page.object
        .filter((object) => ids.has(object.id))
        .map((object) => {
          if (!object.attach?.startsWith("pipe:")) return object;
          return { ...object, attach: `pipe:${region.pipes.indexOf(Number(object.attach.slice(5)))}` };
        }),
      pipe: region.pipes.map((index) => page.pipe[index]),
    };
    return el(
      "svg",
      {
        x: region.at[0],
        y: region.at[1],
        width: region.size[0],
        height: region.size[1],
        viewBox: `0 0 ${region.size[0]} ${region.size[1]}`,
        overflow: "hidden",
        "data-shared-panel": region.panel ?? undefined,
      },
      [
        ...(region.panel
          ? [el("rect", { width: region.size[0], height: region.size[1], fill: token("surface") })]
          : []),
        ...drawContent(part, { ...frame, size: region.size }),
      ],
    );
  });
}

/** A page's or a placed faceplate's pipes, instruments' lines and objects. */
function drawContent(page: HmiPageModel, frame: DrawFrame): SvgNode[] {
  const draw = (object: HmiObjectModel): SvgNode =>
    hiddenNow(object, frame)
      ? el("g", { "data-object": object.id, "data-hidden": "true" })
      : drawingObject(object.id, () =>
          placed(
            object,
            flagged(object, equipped(page, object, marked(object, drawShown(object, frame), frame), frame), frame),
          ),
        );
  return [
    // A layout panel is a background, such as a room the process runs through: the pipes cross it.
    ...page.object.filter((object) => object.type === "panel").map(draw),
    // The pipes and the instruments' lines under the other objects, so a symbol lies over their
    // ends (§6).
    ...drawPipes(page, frame),
    ...drawAttachments(page, frame),
    ...drawSignals(page, frame),
    ...page.object.filter((object) => object.type !== "panel").map(draw),
  ];
}

/** An object as drawn: a placed faceplate draws its faceplate's objects (§8.9). */
function drawShown(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  if (object.type === PLACED_FACEPLATE) return drawPlaced(object, frame, drawContent);
  const shown =
    object.type === "trend" || object.type === "split-bar" || object.type.startsWith("my:")
      ? object
      : withValueFormat(object, frame);
  const drawn = drawObject(shown, frame);
  if (frame.currentPage && object.click?.page === frame.currentPage) {
    return {
      ...drawn,
      attrs: { ...drawn.attrs, "aria-current": "page" },
      children: [
        el("rect", {
          x: 0,
          y: 0,
          width: object.size[0],
          height: object.size[1],
          rx: 8,
          fill: token("accent"),
          "fill-opacity": 0.15,
        }),
        ...drawn.children,
        el("rect", { x: 0, y: 4, width: 3, height: Math.max(0, object.size[1] - 8), rx: 1.5, fill: token("accent") }),
      ],
    };
  }
  return drawn;
}

/** An object with a visibility binding is hidden while its BOOL is FALSE, stale or not; while it
 * is unknown the object stays shown (§18 phase 5 item 2). */
function hiddenNow(object: HmiObjectModel, frame: DrawFrame): boolean {
  if (!object.bind.visible) return false;
  const live = liveValueOf(object, "visible", frame);
  return live?.state !== "unknown" && live?.value === false;
}

/** An object bound to a path the running program does not have is outlined and says so (§16.3). */
function marked(object: HmiObjectModel, drawn: SvgNode, frame: DrawFrame): SvgNode {
  const missing = frame.missing ?? [];
  if (!Object.values(object.bind).some((binding) => missing.includes(binding.path))) return drawn;
  const [width, height] = object.size;
  // The words of §11.1 at 14 px (§3.5), 8 px either side; on a narrow object the label reaches
  // past its edges.
  const label = say("Not in running program");
  const labelWidth = Math.round(textWidth(label, 14, 600) + 16);
  const top = Math.max(4, height - 30);
  const mark = el("g", { "data-part": "missing" }, [
    el("rect", {
      x: 1,
      y: 1,
      width: width - 2,
      height: height - 2,
      rx: 12,
      fill: "none",
      stroke: token("warning"),
      "stroke-width": 1.5,
      "stroke-dasharray": "6 4",
    }),
    el("rect", {
      x: (width - labelWidth) / 2,
      y: top,
      width: labelWidth,
      height: 24,
      rx: 12,
      fill: token("raised"),
      stroke: token("warning"),
      "stroke-opacity": 0.6,
    }),
    text(label, {
      x: width / 2,
      y: top + 17,
      fill: token("warning"),
      "font-size": 14,
      "font-weight": 600,
      "text-anchor": "middle",
    }),
  ]);
  return { ...drawn, attrs: { ...drawn.attrs, "data-missing": "true" }, children: [...drawn.children, mark] };
}

/**
 * In the editor an object with a binding problem is outlined in the alarm colour with "!" on its
 * top-left corner, clear of its label, and its problems show when the pointer rests on it (§7.6).
 * An object with only quality warnings carries the sign in the warning colour (§14.1 item 4).
 */
function flagged(object: HmiObjectModel, drawn: SvgNode, frame: DrawFrame): SvgNode {
  const problems = frame.problems?.[object.id] ?? [];
  const warnings = frame.warnings?.[object.id] ?? [];
  if (problems.length === 0 && warnings.length === 0) return drawn;
  const [width, height] = object.size;
  const sign = (colour: "alarm" | "warning"): SvgNode[] => [
    el("circle", { cx: 0, cy: 0, r: 9, fill: token(colour), stroke: token("page"), "stroke-width": 2 }),
    text("!", { x: 0, y: 4.5, fill: token("page"), "font-size": 12, "font-weight": 800, "text-anchor": "middle" }),
  ];
  const mark =
    problems.length > 0
      ? el("g", { "data-part": "problem" }, [
          el("rect", {
            x: 1,
            y: 1,
            width: Math.max(0, width - 2),
            height: Math.max(0, height - 2),
            rx: 12,
            fill: "none",
            stroke: token("alarm"),
            "stroke-width": 2,
            "stroke-dasharray": "6 4",
          }),
          ...sign("alarm"),
        ])
      : el("g", { "data-part": "warning" }, sign("warning"));
  const title = { ...el("title"), text: [...problems, ...warnings].join("\n") };
  const attrs = {
    ...drawn.attrs,
    ...(problems.length > 0 ? { "data-problems": "true" } : {}),
    ...(warnings.length > 0 ? { "data-warnings": "true" } : {}),
  };
  return { ...drawn, attrs, children: [title, ...drawn.children, mark] };
}

/** An object shows the alarms of its equipment (§13.6, items 4 and 9). A faceplate is its
 * equipment's own view, with its alarms in it: there only its symbol carries the mark (§8.7). */
function equipped(page: HmiPageModel, object: HmiObjectModel, drawn: SvgNode, frame: DrawFrame): SvgNode {
  if (page.fields !== undefined && !(object.type in SYMBOLS)) return drawn;
  const badge = equipmentBadge(object, frame);
  return badge ? { ...drawn, children: [...drawn.children, badge] } : drawn;
}

/** Puts a drawn object at its position, rotated about its centre. */
function placed(object: HmiObjectModel, drawn: SvgNode): SvgNode {
  const [x, y] = object.at;
  const [width, height] = object.size;
  const transform = object.rotation
    ? `translate(${x} ${y}) rotate(${object.rotation} ${width / 2} ${height / 2})`
    : `translate(${x} ${y})`;
  // A click names its target ("page:trends"), not the faceplate's instance; the editor and the
  // runtime wire it. Equipment that names its panel opens it when it has no click of its own
  // (§21.1). An invisible box under the drawing takes the click anywhere in the object, also where
  // a symbol leaves its box empty.
  const own = object.click ? Object.entries(object.click).find(([key]) => key !== "instance") : undefined;
  const click = own ?? (object.type !== "faceplate" && object.faceplate ? ["faceplate", object.faceplate] : undefined);
  const hit = el("rect", { "data-part": "hit", x: 0, y: 0, width, height, fill: "transparent" });
  return {
    ...drawn,
    attrs: {
      ...drawn.attrs,
      "data-object": object.id,
      transform,
      ...(click ? { "data-click": `${click[0]}:${click[1]}` } : {}),
    },
    children: click ? [hit, ...drawn.children] : drawn.children,
  };
}
