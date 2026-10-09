// The equipment on a page (specification 39, §5.3 item 4, §14.2): the symbol, centred above its
// label of tag and state word, with fault, manual, out-of-service and stale signs beside the word.

import { fitWidth, formatAge, textWidth } from "../format";
import type { DrawFrame, HmiObjectModel } from "../model";
import { stateAttributes, token, TokenName } from "../parts";
import { el, SvgNode, text } from "../svg";
import { LABEL_HEIGHT, symbolFor, symbolPlacement } from "../symbols";
import { round } from "../symbols/style";
import { EquipmentReading, readEquipment } from "./equipmentState";
import { say } from "../speech";

/** How much an out-of-service symbol is dimmed. */
const OUT_OF_SERVICE_OPACITY = 0.45;
/** The baselines of the tag and the state word under the symbol. */
const TAG_BASELINE = 20;
const STATE_BASELINE = 40;
/** The room between the state word and a sign, and between signs. */
const GAP = 8;

export function drawEquipment(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const type = object.type.replace(":", "-");
  const definition = symbolFor(object.type);
  if (!definition) return el("g", { class: "hmi-object", "data-object": object.id });
  const [width, height] = object.size;
  const [symbolWidth, symbolHeight] = definition.size;
  const beside = definition.label === "beside";
  const { left, top, scale, area } = symbolPlacement(object.size, definition.size, definition.label);
  const reading = readEquipment(definition, object, frame);
  const children: SvgNode[] = [
    el(
      "g",
      {
        "data-part": "symbol",
        transform: `translate(${left} ${top}) scale(${round(scale)})`,
        opacity: reading.outOfService ? OUT_OF_SERVICE_OPACITY : undefined,
      },
      definition.draw(reading.view),
    ),
  ];
  if (reading.live?.staleMs !== undefined) {
    // Around the symbol only, so the label stays clear.
    const box = beside
      ? {
          x: 1,
          y: Math.max(1, top - 3),
          width: round(symbolWidth * scale + 6),
          height: Math.min(height - 2, round(symbolHeight * scale + 6)),
        }
      : {
          x: Math.max(1, left - 4),
          y: Math.max(1, top - 4),
          width: Math.min(width - 2, symbolWidth * scale + 8),
          height: Math.min(area, symbolHeight * scale + 6),
        };
    children.push(
      el("rect", {
        "data-part": "stale-outline",
        ...box,
        rx: 10,
        fill: "none",
        stroke: token("stale"),
        "stroke-width": 1.5,
        "stroke-dasharray": "6 4",
      }),
    );
  }
  const tag = object.tag ?? object.label;
  if (beside) {
    // The tag and the word beside the symbol, the two lines centred on its height.
    // Clear of the stale outline around the symbol.
    const start = round(symbolWidth * scale + GAP + 6);
    const first = round((height - LABEL_HEIGHT) / 2);
    if (tag) {
      children.push(
        text(fitWidth(tag, width - start, 14, 600), {
          "data-part": "tag",
          x: start,
          y: first + TAG_BASELINE,
          fill: token("text"),
          "font-size": 14,
          "font-weight": 600,
          "text-anchor": "start",
        }),
      );
    }
    children.push(...stateLine(reading, width, first + STATE_BASELINE, start));
  } else {
    if (tag) {
      children.push(
        text(fitWidth(tag, width, 14, 600), {
          "data-part": "tag",
          x: width / 2,
          y: area + TAG_BASELINE,
          fill: token("text"),
          "font-size": 14,
          "font-weight": 600,
          "text-anchor": "middle",
        }),
      );
    }
    children.push(...stateLine(reading, width, area + STATE_BASELINE));
  }
  return el(
    "g",
    {
      class: `hmi-object hmi-equipment hmi-${type}`,
      "data-equipment": reading.state,
      "data-manual": reading.manual ? "true" : undefined,
      "data-out-of-service": reading.outOfService ? "true" : undefined,
      ...stateAttributes(reading.live),
    },
    children,
  );
}

interface Sign {
  name: string;
  label: string;
  paint: TokenName;
  ink: TokenName;
  edge?: TokenName;
}

/**
 * The state word and its signs on one line. Alone, the word is centred, or starts at `start` when
 * the label stands beside the symbol. With signs, the word ends where they begin, so whatever the
 * font's widths no sign covers it; the measure of its width only places the line.
 */
function stateLine(reading: EquipmentReading, width: number, baseline: number, start?: number): SvgNode[] {
  const signs: Sign[] = [];
  if (reading.fault) signs.push({ name: "fault", label: "!", paint: "alarm", ink: "page" });
  if (reading.manual) signs.push({ name: "manual", label: say("M"), paint: "raised", ink: "text", edge: "muted" });
  if (reading.outOfService)
    signs.push({ name: "out-of-service", label: say("OOS"), paint: "raised", ink: "text", edge: "muted" });
  if (reading.live?.staleMs !== undefined)
    signs.push({ name: "stale", label: formatAge(reading.live.staleMs), paint: "raised", ink: "stale", edge: "stale" });
  const widths = signs.map((sign) => Math.max(22, Math.round(textWidth(sign.label, 14, 700) + 14)));
  const signsWidth =
    signs.length === 0 ? 0 : widths.reduce((sum, signWidth) => sum + signWidth, 0) + GAP * (signs.length - 1);
  // The word takes the room the signs leave and is cut to fit it; signs are never cut (§14.5 item 6).
  const room = width - (start ?? 0);
  const shown = reading.word
    ? fitWidth(reading.word, Math.max(0, room - (signs.length === 0 ? 0 : signsWidth + GAP)), 14, 500)
    : "";
  const word = (x: number, anchor: "start" | "middle" | "end"): SvgNode =>
    text(shown, {
      "data-part": "state",
      x: round(x),
      y: baseline,
      fill: reading.fault ? token("alarm") : token("muted"),
      "font-size": 14,
      "font-weight": 500,
      "text-anchor": anchor,
    });
  if (signs.length === 0) return shown ? [start === undefined ? word(width / 2, "middle") : word(start, "start")] : [];
  const wordWidth = shown ? textWidth(shown, 14, 500) : 0;
  const total = wordWidth + (shown ? GAP : 0) + signsWidth;
  const wordEnd = start === undefined ? Math.max(wordWidth, (width - total) / 2 + wordWidth) : start + wordWidth;
  const nodes: SvgNode[] = shown ? [word(wordEnd, "end")] : [];
  let x = shown ? wordEnd + GAP : (start ?? Math.max(0, (width - signsWidth) / 2));
  signs.forEach((sign, index) => {
    const signWidth = widths[index];
    const top = baseline - 16;
    nodes.push(
      el("g", { "data-sign": sign.name }, [
        el("rect", {
          x: round(x),
          y: top,
          width: signWidth,
          height: 22,
          rx: 11,
          fill: token(sign.paint),
          stroke: sign.edge ? token(sign.edge) : undefined,
          "stroke-width": sign.edge ? 1 : undefined,
        }),
        text(sign.label, {
          x: round(x + signWidth / 2),
          y: top + 16,
          fill: token(sign.ink),
          "font-size": 14,
          "font-weight": 700,
          "text-anchor": "middle",
        }),
      ]),
    );
    x += signWidth + GAP;
  });
  return nodes;
}
