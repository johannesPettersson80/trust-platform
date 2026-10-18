// A connection pill on its runtime: its protocol dot, its name, the role word and the marks the
// view gives (`draft`, `disabled`). Every word and tone comes from the view JSON; the pill decides
// nothing about whether a connection is a draft (design §4.5 D1).

import React from "react";

import type { Rect } from "../layout";
import {
  connectionDotStyle,
  connectionNameStyle,
  connectionPillStyle,
  connectionRoleStyle,
  markStyle,
  positionedStyle,
} from "../styles";
import type { Connection } from "../types";

/** Draws one connection pill at its layout rectangle, with the role word and marks as given. */
export function ConnectionPill({
  connection,
  rect,
}: {
  /** The connection to draw. */
  connection: Connection;
  /** Its rectangle, relative to its runtime card. */
  rect: Rect;
}) {
  return (
    <div
      data-item-id={connection.id}
      data-role="connection-pill"
      data-look={connection.look}
      data-dimmed={connection.dimmed === true ? "true" : "false"}
      tabIndex={0}
      style={{ ...connectionPillStyle(connection.look), ...positionedStyle(rect, connection.dimmed === true) }}
    >
      <span
        data-role="connection-dot"
        data-tone={connection.dot}
        data-live={connection.live ? "true" : "false"}
        style={connectionDotStyle(connection.dot, connection.live)}
      />
      <span data-role="connection-name" title={connection.name} style={connectionNameStyle}>
        {connection.name}
      </span>
      <span data-role="connection-role" style={connectionRoleStyle}>
        {connection.role_word}
      </span>
      {connection.marks.map((mark) => (
        <span key={mark} data-role="connection-mark" data-mark={mark} style={markStyle(mark)}>
          {mark}
        </span>
      ))}
    </div>
  );
}
