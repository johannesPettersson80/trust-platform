// A faceplate placed on a page (specification 39, §8.9 item 3): the faceplate's objects bound to
// its instance, drawn exactly as its popup draws them, under a slim header with the instance's tag
// and no close button. Header and objects are one picture with the faceplate's proportions, so a
// resized placement scales as one (item 4).

import { fitWidth } from "./format";
import { textWidth } from "./measure";
import type { DrawFrame, HmiObjectModel, HmiPageModel } from "./model";
import { card, token } from "./parts";
import { say, translate } from "./speech";
import { el, SvgNode, text } from "./svg";
import { translatedPage } from "./translated";

/** The type of a placed faceplate. */
export const PLACED_FACEPLATE = "faceplate";

/** The header's height at the faceplate's design scale. */
export const PLACED_HEADER = 44;

/** Draws a page's or a faceplate's pipes, lines and objects. */
export type DrawContent = (page: HmiPageModel, frame: DrawFrame) => SvgNode[];

/** A placed faceplate, its objects drawn by `content` in its own frame: its controls' states
 * belong to its own surface; values, history and alarms are by path, so they are the page's. */
export function drawPlaced(object: HmiObjectModel, frame: DrawFrame, content: DrawContent): SvgNode {
  const [width, height] = object.size;
  const placement = frame.placed?.[object.id];
  if (placement === null) return missing(object, true);
  if (placement === undefined) return missing(object, false);
  const { model } = placement;
  const [faceplateWidth, faceplateHeight] = model.size;
  const inner: DrawFrame = {
    ...frame,
    size: model.size,
    commands: placement.commands,
    drafts: placement.drafts,
    locked: placement.locked,
    placed: undefined,
    trendViews: undefined,
    problems: undefined,
    warnings: undefined,
  };
  const total = faceplateHeight + PLACED_HEADER;
  return el("g", { class: "hmi-object hmi-placed" }, [
    el(
      "svg",
      {
        "data-placed": object.id,
        width,
        height,
        viewBox: `0 0 ${faceplateWidth} ${total}`,
        preserveAspectRatio: "xMidYMid meet",
        overflow: "visible",
      },
      [
        card(faceplateWidth, total, undefined, 16),
        header(model.title, model.description !== undefined ? translate(model.description) : undefined, faceplateWidth),
        // The panel's own labels and buttons in the display's language, as a page's are (§14.5).
        el("g", { transform: `translate(0 ${PLACED_HEADER})` }, content(translatedPage(model), inner)),
      ],
    ),
  ]);
}

/** The slim header: the instance's tag, and the faceplate's description after it where it fits. */
function header(title: string, description: string | undefined, width: number): SvgNode {
  const shownTitle = fitWidth(title, width - 36, 16, 650);
  const children: SvgNode[] = [
    text(shownTitle, { x: 18, y: 28, fill: token("text"), "font-size": 16, "font-weight": 650 }),
    el("line", { x1: 0, y1: PLACED_HEADER, x2: width, y2: PLACED_HEADER, stroke: token("border") }),
  ];
  const after = 18 + textWidth(shownTitle, 16, 650) + 12;
  const room = width - after - 18;
  if (description && room >= 48) {
    children.push(
      text(fitWidth(description, room, 14, 500), {
        x: after,
        y: 28,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
      }),
    );
  }
  return el("g", { "data-part": "placed-header" }, children);
}

/** A placement drawn without its faceplate's objects: its card and header, and where the project
 * and its libraries lack the faceplate, a notice saying so; one not known yet shows no notice. */
function missing(object: HmiObjectModel, lacking: boolean): SvgNode {
  const [width, height] = object.size;
  const tag = object.tag ?? object.instance?.split(".").pop() ?? "";
  const children = [card(width, height, undefined, 16), header(tag, undefined, width)];
  if (lacking) {
    const notice = say("No faceplate {name}", { name: object.faceplate ?? "" });
    children.push(
      text(fitWidth(notice, width - 36, 14, 500), {
        x: width / 2,
        y: PLACED_HEADER + (height - PLACED_HEADER) / 2 + 5,
        "text-anchor": "middle",
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
      }),
    );
  }
  return el("g", { class: "hmi-object hmi-placed" }, children);
}
