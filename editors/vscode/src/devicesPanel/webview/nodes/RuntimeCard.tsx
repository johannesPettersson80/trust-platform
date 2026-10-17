// A runtime card, and the status pill that the card, its computer frame and a device card all draw
// (design §4.5 D1). Every word and tone comes from the view JSON; the card computes nothing.

import React from "react";

import type { Rect } from "../layout";
import { cardStyle, pillStyle, platformStyle, positionedStyle, runtimeNameStyle } from "../styles";
import type { Action, Pill as PillModel, Runtime } from "../types";
import { ConnectionPill } from "./ConnectionPill";
import { Slot } from "./Slot";

/** A status word with its tone, drawn as a pill. */
export function StatusPill({ pill }: { pill: PillModel }) {
  return (
    <span data-role="pill" data-tone={pill.tone} style={pillStyle(pill.tone)}>
      {pill.word}
    </span>
  );
}

/** One runtime card: its name, its computer's platform, its state, its connection pills and slots. */
export function RuntimeCard({
  runtime,
  rect,
  connections,
  platform,
  selected,
  onSelect,
  onAction,
}: {
  /** The runtime to draw. */
  runtime: Runtime;
  /** Its rectangle, relative to its computer frame. */
  rect: Rect;
  /** The rectangles of every connection, relative to its runtime. */
  connections: Record<string, Rect>;
  /** The platform text of the computer that runs it. */
  platform?: string;
  /** Whether it is the selected card. */
  selected: boolean;
  /** Called with its identity when the card is chosen. */
  onSelect: (id: string) => void;
  /** Called with a slot's action when the slot is chosen. */
  onAction?: (action: Action) => void;
}) {
  const pills = (runtime.connections ?? []).filter((pill) => pill.hidden !== true);
  const slots = runtime.slots ?? [];
  return (
    <article
      data-item-id={runtime.id}
      data-role="runtime-card"
      data-dimmed={runtime.dimmed === true ? "true" : "false"}
      style={{ ...cardStyle(selected), ...positionedStyle(rect, runtime.dimmed === true) }}
      onClick={() => onSelect(runtime.id)}
    >
      <div data-role="runtime-name" title={runtime.name} style={runtimeNameStyle}>
        {runtime.name}
      </div>
      {platform !== undefined && (
        <div data-role="runtime-platform" title={platform} style={platformStyle}>
          {platform}
        </div>
      )}
      <StatusPill pill={runtime.state} />
      {slots.map((action) => (
        <Slot key={action.id} action={action} onAction={onAction} />
      ))}
      {pills.map((connection) => (
        <ConnectionPill key={connection.id} connection={connection} rect={connections[connection.id]} />
      ))}
    </article>
  );
}
