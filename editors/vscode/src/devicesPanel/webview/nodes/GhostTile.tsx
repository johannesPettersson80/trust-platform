// The draft tile: the ghost that a form draws on the canvas while it is open, so the item being
// configured is visible as a draft until it is saved (design §4.5 D3; specification 25 §5.3 and
// §5.5). Its name and detail come from Rust's own `Form.ghost`; the tile recomputes nothing and is
// never draggable or selectable, because it is not a device yet.

import React from "react";

import type { GhostTile as GhostTileModel } from "../formTypes";
import type { Rect } from "../layout";
import {
  deviceNameStyle,
  devicePresentsStyle,
  ghostTileStyle,
  positionedStyle,
} from "../styles";

/** Draws the draft tile at its layout rectangle, after the device cards. */
export function GhostTile({
  ghost,
  rect,
}: {
  /** The draft, as the form gives it. */
  ghost: GhostTileModel;
  /** Its rectangle, in canvas coordinates. */
  rect: Rect;
}) {
  return (
    <article
      data-role="ghost"
      data-draft="true"
      style={{ ...ghostTileStyle, ...positionedStyle(rect, false) }}
    >
      <div data-role="ghost-name" title={ghost.name} style={deviceNameStyle}>
        {ghost.name}
      </div>
      <div data-role="ghost-detail" title={ghost.detail} style={devicePresentsStyle}>
        {ghost.detail}
      </div>
    </article>
  );
}
