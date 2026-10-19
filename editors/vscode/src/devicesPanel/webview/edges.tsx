// The edges of the canvas: a link between two items and a mesh bus's drop to an item. What the
// view says is drawn exactly as it says it — solid when proven and dashed otherwise, the arrow at
// the end it names, and a lock only when the link is secure. The route is geometry from
// `layout.ts`; the edge decides no rule (design §4.5 D1).

import React from "react";

import type { Point, Route } from "./layout";
import { toneColour } from "./styles";
import type { Link, MeshDrop } from "./types";

/** The identity of the arrowhead marker the link edges point their ends at. */
const ARROW_MARKER = "url(#devices-link-arrow)";

/** The SVG path data of a route's polyline. */
function pathData(points: Point[]): string {
  return points
    .map((point, index) => `${index === 0 ? "M" : "L"}${point.x} ${point.y}`)
    .join(" ");
}

/** The last corner of a route, where a lock is drawn. */
function end(point: Point[]): Point {
  return point[point.length - 1];
}

/** One link, drawn solid or dashed, with its arrow and lock as the view says. */
export function LinkEdge({
  link,
  route,
}: {
  /** The link to draw. */
  link: Link;
  /** Its route, in canvas coordinates. */
  route: Route;
}) {
  const colour = toneColour(link.tone);
  const last = end(route.points);
  return (
    <g
      data-role="link"
      data-link-id={link.id}
      data-proven={link.proven ? "true" : "false"}
      data-arrow={link.arrow}
      data-secure={link.secure ? "true" : "false"}
      data-dimmed={link.dimmed === true ? "true" : "false"}
      style={{ opacity: link.dimmed === true ? 0.4 : 1 }}
    >
      <path
        data-role="link-line"
        d={pathData(route.points)}
        fill="none"
        stroke={colour}
        strokeWidth={1.6}
        strokeDasharray={link.proven ? undefined : "5 4"}
        markerStart={link.arrow === "from" ? ARROW_MARKER : undefined}
        markerEnd={link.arrow === "to" ? ARROW_MARKER : undefined}
      />
      {link.secure && (
        <text data-role="link-lock" x={last.x} y={last.y} fill={colour} fontSize={12}>
          {"\u{1F512}"}
        </text>
      )}
      {link.hover_text !== undefined && <title>{link.hover_text}</title>}
    </g>
  );
}

/** One mesh drop, drawn solid or dashed as the view says. */
export function MeshDropEdge({
  drop,
  route,
}: {
  /** The drop to draw. */
  drop: MeshDrop;
  /** Its route, in canvas coordinates. */
  route: Route;
}) {
  return (
    <g data-role="mesh-drop" data-item={drop.item} data-proven={drop.proven ? "true" : "false"}>
      <path
        data-role="mesh-drop-line"
        d={pathData(route.points)}
        fill="none"
        stroke={toneColour(drop.tone)}
        strokeWidth={1.6}
        strokeDasharray={drop.proven ? undefined : "5 4"}
      />
      {drop.hover_text !== undefined && <title>{drop.hover_text}</title>}
    </g>
  );
}
