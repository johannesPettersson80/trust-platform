// The canvas: the summary and banner above a graph of computer frames, device cards, mesh buses
// and the links between them (design §4.5 D1). Every position comes from `layout.ts` and every
// word, tone and flag from the view JSON; the canvas decides nothing (design §3.4).

import React from "react";

import { t } from "../../webview/theme";
import { LinkEdge, MeshDropEdge } from "./edges";
import { dropRouteId, layoutCanvas, type LayoutScene, type Point } from "./layout";
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
  summaryIssuesButtonStyle,
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

/** Where a card's drag began: the item, the pointer at that moment and the item's position then. */
interface DragStart {
  /** The dragged item's identity. */
  id: string;
  /** The pointer's horizontal position when the drag began. */
  pointerX: number;
  /** The pointer's vertical position when the drag began. */
  pointerY: number;
  /** The card's horizontal position when the drag began. */
  itemX: number;
  /** The card's vertical position when the drag began. */
  itemY: number;
}

/**
 * Draws the summary, the banner and the graph. The items the view marks hidden are not drawn; the
 * ones it marks dimmed are drawn dimmed. Tab reaches every item, Enter opens its inspector and a
 * device card can be dragged (specification 25 §5.11 and §5.13): the keyboard and the drag are
 * input and view geometry, so they live here and decide no rule.
 */
export function Canvas({
  computers,
  hideSlots,
  selectedRuntime,
  onSelectRuntime,
  onSelect,
  onContextMenu,
  summary,
  banner,
  devices,
  links,
  mesh,
  savedPositions,
  onPositions,
  onOpenIssues,
  onAction,
}: {
  /** The computers and chips to draw. */
  computers: Computer[];
  /** Whether an open right-hand pane hides the Edit slots (specification 25 §5.11). */
  hideSlots?: boolean;
  /** The identity of the selected runtime, if any. */
  selectedRuntime: string | undefined;
  /** Called with a runtime's identity when its card is chosen. */
  onSelectRuntime: (id: string) => void;
  /** Called with the identity of the clicked item, or `undefined` for the empty canvas. */
  onSelect?: (id: string | undefined) => void;
  /** Called with the identity of a right-clicked item and the pointer's position. */
  onContextMenu?: (id: string, x: number, y: number) => void;
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
  /** The item positions the host kept for the project, drawn over the layout (design §4.5 D2). */
  savedPositions?: Record<string, Point>;
  /** Called with the item positions when a drag ends, for the host to keep for the project. */
  onPositions?: (positions: Record<string, Point>) => void;
  /** Called when the summary strip's issues action is chosen (specification 25 §5.14). */
  onOpenIssues?: () => void;
  /** Called with the action a banner button or a slot carries, exactly as the view gave it. */
  onAction?: (action: Action) => void;
}) {
  const visibleComputers = computers.filter((computer) => computer.hidden !== true);
  const visibleDevices = (devices ?? []).filter((device) => device.hidden !== true);
  const visibleLinks = (links ?? []).filter((link) => link.hidden !== true);
  const visibleMesh = mesh ?? [];

  // Dragging a device card is view geometry: the card is drawn where the user left it and the
  // routes follow it (design §4.5 D2). The webview stores nothing itself — it posts the positions
  // when a drag ends, and the host keeps them for the project and sends them back.
  const [draggedPositions, setDraggedPositions] = React.useState<Record<string, Point>>({});
  const draggedRef = React.useRef<Record<string, Point>>({});
  const dragRef = React.useRef<DragStart | undefined>(undefined);
  React.useEffect(() => {
    const onMouseMove = (event: MouseEvent) => {
      const drag = dragRef.current;
      if (drag === undefined) {
        return;
      }
      const moved = {
        ...draggedRef.current,
        [drag.id]: {
          x: drag.itemX + (event.clientX - drag.pointerX),
          y: drag.itemY + (event.clientY - drag.pointerY),
        },
      };
      draggedRef.current = moved;
      setDraggedPositions(moved);
    };
    const onMouseUp = () => {
      const drag = dragRef.current;
      if (drag === undefined) {
        return;
      }
      dragRef.current = undefined;
      // A press without a move is not a drag, so it posts nothing.
      if (draggedRef.current[drag.id] === undefined) {
        return;
      }
      onPositions?.({ ...(savedPositions ?? {}), ...draggedRef.current });
    };
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [onPositions, savedPositions]);

  // While a right-hand pane is open the Edit slots are not drawn, and the layout gives their space
  // back to the items (specification 25 §5.11). They return when the pane closes.
  const drawnComputers: Computer[] =
    hideSlots === true
      ? visibleComputers.map((computer) => ({
          ...computer,
          slots: [],
          runtimes: computer.runtimes.map((runtime) => ({ ...runtime, slots: [] })),
        }))
      : visibleComputers;

  const scene: LayoutScene = {
    frames: drawnComputers.map((computer) => ({
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
  const layout = layoutCanvas(scene, { ...(savedPositions ?? {}), ...draggedPositions });

  /** The identity of the item a click landed on, or `undefined` for the empty canvas. */
  const itemIdAt = (target: EventTarget | null): string | undefined => {
    const element = target as Element | null;
    const item = element?.closest?.("[data-item-id]");
    return item?.getAttribute("data-item-id") ?? undefined;
  };
  const handleGraphClick = (event: React.MouseEvent<HTMLDivElement>) => {
    onSelect?.(itemIdAt(event.target));
  };
  const handleGraphContextMenu = (event: React.MouseEvent<HTMLDivElement>) => {
    const id = itemIdAt(event.target);
    if (id === undefined) {
      return;
    }
    event.preventDefault();
    onContextMenu?.(id, event.clientX, event.clientY);
  };
  // Enter on a focused item opens its inspector, exactly as a click does (specification 25 §5.11).
  // Only the item itself is handled, so a slot or copy button keeps its own Enter.
  const handleGraphKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Enter") {
      return;
    }
    const element = event.target as Element | null;
    const id = element?.getAttribute("data-item-id") ?? undefined;
    if (id === undefined) {
      return;
    }
    event.preventDefault();
    onSelect?.(id);
  };
  /** Starts dragging a device card from the position it is drawn at. */
  const startDrag = (id: string, event: React.MouseEvent) => {
    const rect = layout.devices[id];
    if (rect === undefined) {
      return;
    }
    event.preventDefault();
    dragRef.current = {
      id,
      pointerX: event.clientX,
      pointerY: event.clientY,
      itemX: rect.x,
      itemY: rect.y,
    };
  };

  return (
    <div style={canvasStyle}>
      {summary !== undefined && (
        <div data-role="summary" style={summaryBarStyle}>
          <span data-role="summary-text" style={summaryTextStyle}>
            {summary.text}
          </span>
          {summary.open_issues !== undefined && (
            <button
              type="button"
              data-role="summary-issues"
              data-action={summary.open_issues.id}
              disabled={!summary.open_issues.enabled}
              title={summary.open_issues.label}
              onClick={() => onOpenIssues?.()}
              style={summaryIssuesButtonStyle(summary.open_issues.enabled)}
            >
              {summary.open_issues.label}
            </button>
          )}
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
      <div
        data-role="graph"
        style={{ ...graphStyle, width: layout.width, height: layout.height }}
        onClick={handleGraphClick}
        onContextMenu={handleGraphContextMenu}
        onKeyDown={handleGraphKeyDown}
      >
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
        {drawnComputers.map((computer) => (
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
          <DeviceCard
            key={device.id}
            device={device}
            rect={layout.devices[device.id]}
            onDragStart={startDrag}
          />
        ))}
        {visibleMesh.map((bus) => (
          <MeshBus key={bus.id} bus={bus} rect={layout.mesh[bus.id]} />
        ))}
      </div>
    </div>
  );
}
