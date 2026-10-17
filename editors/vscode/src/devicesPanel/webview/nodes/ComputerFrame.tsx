// One computer frame: its heading, its reachability and a runtime card for every runtime it holds
// (design §2.1 and §4.5 D1). Every word and tone comes from the view JSON, and every position from
// `layout.ts`; the frame computes neither.

import React from "react";

import type { Rect } from "../layout";
import { frameHeaderStyle, frameStyle, headingStyle, positionedStyle, runtimeRowStyle } from "../styles";
import type { Action, Computer } from "../types";
import { RuntimeCard, StatusPill } from "./RuntimeCard";
import { Slot } from "./Slot";

/** Draws one computer or chip, with all of its runtimes side by side. */
export function ComputerFrame({
  computer,
  frame,
  runtimes,
  connections,
  selectedRuntime,
  onSelectRuntime,
  onAction,
}: {
  /** The computer or chip to draw. */
  computer: Computer;
  /** Its rectangle, in canvas coordinates. */
  frame: Rect;
  /** The rectangles of every runtime, relative to its frame. */
  runtimes: Record<string, Rect>;
  /** The rectangles of every connection, relative to its runtime. */
  connections: Record<string, Rect>;
  /** The identity of the selected runtime, if any. */
  selectedRuntime: string | undefined;
  /** Called with a runtime's identity when its card is chosen. */
  onSelectRuntime: (id: string) => void;
  /** Called with a slot's action when the slot is chosen. */
  onAction?: (action: Action) => void;
}) {
  const slots = computer.slots ?? [];
  return (
    <section
      data-item-id={computer.id}
      data-role="computer-frame"
      data-dimmed={computer.dimmed === true ? "true" : "false"}
      style={{ ...frameStyle, ...positionedStyle(frame, computer.dimmed === true) }}
    >
      <header style={frameHeaderStyle}>
        <h2 data-role="computer-heading" title={computer.heading} style={headingStyle}>
          {computer.heading}
        </h2>
        <StatusPill pill={computer.reachability} />
        {slots.map((action) => (
          <Slot key={action.id} action={action} onAction={onAction} />
        ))}
      </header>
      <div style={runtimeRowStyle}>
        {computer.runtimes
          .filter((runtime) => runtime.hidden !== true)
          .map((runtime) => (
            <RuntimeCard
              key={runtime.id}
              runtime={runtime}
              rect={runtimes[runtime.id]}
              connections={connections}
              platform={computer.platform}
              selected={selectedRuntime === runtime.id}
              onSelect={onSelectRuntime}
              onAction={onAction}
            />
          ))}
      </div>
    </section>
  );
}
