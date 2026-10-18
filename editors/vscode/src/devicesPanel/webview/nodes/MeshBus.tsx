// A mesh bus: the fabric the peer connections share. Its label is drawn only when the view gives
// one; the bus never counts its drops to decide that (design §4.5 D1).

import React from "react";

import type { Rect } from "../layout";
import { meshBusStyle, meshLabelStyle, positionedStyle } from "../styles";
import type { MeshBus as MeshBusModel } from "../types";

/** Draws one mesh bus at its layout rectangle, labelled only when the view gives a label. */
export function MeshBus({
  bus,
  rect,
}: {
  /** The bus to draw. */
  bus: MeshBusModel;
  /** Its rectangle, in canvas coordinates. */
  rect: Rect;
}) {
  return (
    <div
      data-item-id={bus.id}
      data-role="mesh-bus"
      data-draft={bus.draft ? "true" : "false"}
      style={{ ...meshBusStyle, ...positionedStyle(rect, false) }}
    >
      {bus.label !== undefined && (
        <span data-role="mesh-label" title={bus.label} style={meshLabelStyle}>
          {bus.label}
        </span>
      )}
    </div>
  );
}
