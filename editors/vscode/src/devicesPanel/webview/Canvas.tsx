// The canvas: the summary and banner above a graph of computer frames, device cards, mesh buses
// and the links between them (design §4.5 D1). Every position comes from `layout.ts` and every
// word, tone and flag from the view JSON; the canvas decides nothing (design §3.4).

import React from "react";

import { t } from "../../webview/theme";
import { LinkEdge, MeshDropEdge } from "./edges";
import { dropRouteId, layoutCanvas, type LayoutScene } from "./layout";
import { ComputerFrame } from "./nodes/ComputerFrame";
import { DeviceCard } from "./nodes/DeviceCard";
import { MeshBus } from "./nodes/MeshBus";
import {
  bannerButtonStyle,
  bannerStyle,
  bannerTextStyle,
  canvasStyle,
  graphStyle,
  summaryBarStyle,
  summaryTextStyle,
  svgStyle,
} from "./styles";
import type {
  Action,
  Banner,
  Computer,
  Device,
  Link,
  MeshBus as MeshBusModel,
  Summary,
} from "./types";

/**
 * Draws the summary, the banner and the graph. The items the view marks hidden are not drawn; the
 * ones it marks dimmed are drawn dimmed.
 */
export function Canvas({
  computers,
  selectedRuntime,
  onSelectRuntime,
  summary,
  banner,
  devices,
  links,
  mesh,
  onAction,
}: {
  /** The computers and chips to draw. */
  computers: Computer[];
  /** The identity of the selected runtime, if any. */
  selectedRuntime: string | undefined;
  /** Called with a runtime's identity when its card is chosen. */
  onSelectRuntime: (id: string) => void;
  /** The summary strip, when the view gives one. */
  summary?: Summary;
  /** The banner, when the view shows one. */
  banner?: Banner;
  /** The devices at the other end of the connections. */
  devices?: Device[];
  /** The links between items. */
  links?: Link[];
  /** The mesh buses. */
  mesh?: MeshBusModel[];
  /** Called with the action a banner button or a slot carries, exactly as the view gave it. */
  onAction?: (action: Action) => void;
}) {
  const visibleComputers = computers.filter((computer) => computer.hidden !== true);
  const visibleDevices = (devices ?? []).filter((device) => device.hidden !== true);
  const visibleLinks = (links ?? []).filter((link) => link.hidden !== true);
  const visibleMesh = mesh ?? [];

  const scene: LayoutScene = {
    frames: visibleComputers.map((computer) => ({
      id: computer.id,
      slots: (computer.slots ?? []).length,
      runtimes: computer.runtimes
        .filter((runtime) => runtime.hidden !== true)
        .map((runtime) => ({
          id: runtime.id,
          slots: (runtime.slots ?? []).length,
          connections: (runtime.connections ?? [])
            .filter((connection) => connection.hidden !== true)
            .map((connection) => connection.id),
        })),
    })),
    devices: visibleDevices.map((device) => device.id),
    mesh: visibleMesh.map((bus) => ({ id: bus.id, drops: bus.drops.map((drop) => drop.item) })),
    links: visibleLinks.map((link) => ({ id: link.id, from: link.from, to: link.to })),
  };
  const layout = layoutCanvas(scene);

  return (
    <div style={canvasStyle}>
      {summary !== undefined && (
        <div data-role="summary" style={summaryBarStyle}>
          <span data-role="summary-text" style={summaryTextStyle}>
            {summary.text}
          </span>
        </div>
      )}
      {banner !== undefined && (
        <div data-role="banner" data-tone={banner.tone} style={bannerStyle(banner.tone)}>
          <span data-role="banner-text" style={bannerTextStyle}>
            {banner.text}
          </span>
          {banner.actions.map((action) => (
            <button
              key={action.id}
              type="button"
              data-banner-item={action.id}
              disabled={!action.enabled}
              title={action.label}
              onClick={() => {
                if (action.enabled) {
                  onAction?.(action);
                }
              }}
              style={bannerButtonStyle(action.emphasis, action.enabled)}
            >
              {action.label}
            </button>
          ))}
        </div>
      )}
      <div data-role="graph" style={{ ...graphStyle, width: layout.width, height: layout.height }}>
        <svg data-role="edges" width={layout.width} height={layout.height} style={svgStyle}>
          <defs>
            <marker
              id="devices-link-arrow"
              markerWidth={8}
              markerHeight={8}
              refX={6}
              refY={3}
              orient="auto"
              markerUnits="strokeWidth"
            >
              <path d="M0,0 L6,3 L0,6 z" fill={t.textMuted} />
            </marker>
          </defs>
          {visibleLinks.map((link) => {
            const route = layout.links.find((candidate) => candidate.id === link.id);
            return route ? <LinkEdge key={link.id} link={link} route={route} /> : null;
          })}
          {visibleMesh.map((bus) =>
            bus.drops.map((drop) => {
              const route = layout.drops.find(
                (candidate) => candidate.id === dropRouteId(bus.id, drop.item),
              );
              return route ? (
                <MeshDropEdge key={dropRouteId(bus.id, drop.item)} drop={drop} route={route} />
              ) : null;
            }),
          )}
        </svg>
        {visibleComputers.map((computer) => (
          <ComputerFrame
            key={computer.id}
            computer={computer}
            frame={layout.frames[computer.id]}
            runtimes={layout.runtimes}
            connections={layout.connections}
            selectedRuntime={selectedRuntime}
            onSelectRuntime={onSelectRuntime}
            onAction={onAction}
          />
        ))}
        {visibleDevices.map((device) => (
          <DeviceCard key={device.id} device={device} rect={layout.devices[device.id]} />
        ))}
        {visibleMesh.map((bus) => (
          <MeshBus key={bus.id} bus={bus} rect={layout.mesh[bus.id]} />
        ))}
      </div>
    </div>
  );
}
