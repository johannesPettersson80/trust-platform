// A device card: a system at the other end of a connection. Its name, what it presents and its
// status come from the view JSON; the card recomputes nothing (design §4.5 D1). It can be dragged:
// the pointer is handed to the canvas, which owns the view geometry (design §4.5 D2, spec 25 §5.13).

import React from "react";

import type { Rect } from "../layout";
import { deviceCardStyle, deviceNameStyle, devicePresentsStyle, positionedStyle } from "../styles";
import type { Device } from "../types";
import { StatusPill } from "./RuntimeCard";

/** Draws one device at its layout rectangle. */
export function DeviceCard({
  device,
  rect,
  onDragStart,
}: {
  /** The device to draw. */
  device: Device;
  /** Its rectangle, in canvas coordinates. */
  rect: Rect;
  /** Called with the device's identity and the pointer event when a drag starts. */
  onDragStart?: (id: string, event: React.MouseEvent) => void;
}) {
  return (
    <article
      data-item-id={device.id}
      data-role="device-card"
      data-kind={device.kind}
      data-dimmed={device.dimmed === true ? "true" : "false"}
      tabIndex={0}
      onMouseDown={(event) => onDragStart?.(device.id, event)}
      style={{ ...deviceCardStyle, ...positionedStyle(rect, device.dimmed === true) }}
    >
      <div data-role="device-name" title={device.name} style={deviceNameStyle}>
        {device.name}
      </div>
      <div data-role="device-presents" title={device.presents} style={devicePresentsStyle}>
        {device.presents}
      </div>
      {device.status !== undefined && <StatusPill pill={device.status} />}
    </article>
  );
}
