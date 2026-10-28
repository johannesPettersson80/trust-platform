import { withSharedPanels } from "../panelPresentation";
// The canvas (§3.2–3.4): the page drawn by the shared drawing module, with an editing layer above
// it for selection, handles, moving and resizing, and for the pipes (§6): a symbol's ports when
// the pointer is on it, a pipe drawn from port to port, a pipe selected and bent, and a valve
// dropped onto a pipe splitting it.

import { useMemo, useRef, useState } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";

import { drawPage } from "../../drawing";
import type { DrawFrame, HmiPageModel, PlacedFaceplate } from "../../drawing/model";
import { attachOrigin } from "../../drawing/attachments";
import { pipeRoutes, portPoint } from "../../drawing/pipes";
import { previewStates, previewValues } from "../../drawing/previews";
import { renderToString } from "../../drawing/svg";
import type { Catalogue } from "../catalogue";
import { designValues } from "../designSamples";
import { DESIGN_NOW_MS } from "../designHistory";
import { buildPage, buildPlacements } from "../buildPresentation";
import { drawnBox, GRID, Handle, objectAt, objectsInBox, Point, resizeObject, selectFromClick } from "../editing";
import { Gap, Guide, moveWithGuides } from "../guides";
import { pickable, withGroups } from "../layers";
import type { ScreenModel } from "../messages";
import { sameModel } from "../pageDocument";
import { withAttach } from "../attachments";
import { addPipe, bendPipe, branchPipe, pipeAt, pipeIndex, portAt, splitPipe } from "../pipes";
import { attachTargets } from "./AttachSection";
import { TranslationField } from "./TranslationField";

const HANDLES: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
/** A handle's size on screen, in pixels, whatever the zoom. */
const HANDLE_PX = 8;
/** What an alarm list says in Live mode, which reads values only: alarms reach the HMI through a
 * runtime's web server (§11, item 6). */
const LIVE_ALARMS = "Alarms show in the runtime display of a runtime with its HMI web server.";

interface Drag {
    kind: "move" | "resize" | "bend";
    handle?: Handle;
    ids: string[];
    start: Point;
    base: HmiPageModel;
    /** For a press on the selection: what a click without a drag then selects (§3.3 item 8). */
    collapse?: string[];
}

/** A pipe being drawn from a port, and where the pointer is. */
interface Connecting {
    from: string;
    at: Point;
}

/** An instrument being attached by dragging from its handle, and where the pointer is (§6 item 5). */
interface Attaching {
    id: string;
    from: Point;
    at: Point;
}

interface CanvasProps {
    composition?: HmiPageModel;
    currentPage?: string;
    sharedSelection?: string;
    onSharedSelect?: (name: string) => void;
    /** Objects hidden in the editor from the Layers panel (§3.3 item 8). */
    hidden?: string[];
    /** A right-click: where, in the window's coordinates, after selecting what it was on (§3.3 item 8). */
    onContextMenu?: (at: [number, number]) => void;
    page: HmiPageModel;
    screen: ScreenModel;
    catalogue: Catalogue | undefined;
    selection: string[];
    zoom: number;
    assetBase: string | undefined;
    paused: boolean;
    /** The binding checks' problems by object id (§7.6). */
    problems?: Record<string, string[]>;
    /** The quality checks' warnings by object id (§14.1 item 4). */
    warnings?: Record<string, string[]>;
    /** The P&ID view: each symbol as it stands, without states (§18 phase 4 item 1). */
    pid?: boolean;
    /** Live mode's values, drawn in place of design samples (§11). */
    live?: Pick<DrawFrame, "values" | "history" | "nowMs" | "missing">;
    formats?: DrawFrame["formats"];
    /** The state each object is previewed in, by id (§5.3 item 6); Design mode only. */
    previews?: Record<string, string>;
    /** The language the page is shown in, its text table, and the "longest" preview (§14.5). */
    speech?: Pick<DrawFrame, "texts" | "language" | "longest">;
    /** A click on an object that neither moved nor resized it. */
    onObjectClick?: (id: string) => void;
    dependencyPick?: (id: string | undefined) => void;
    dependencyIds?: string[];
    /** The text being translated in place (§14.5 item 7). */
    translation?: {
        id: string;
        text: string;
        translation: string | undefined;
        placeholder: string;
        languageName: string;
    };
    onTranslate?: (translation: string) => void;
    onTranslateCancel?: () => void;
    onSelect: (selection: string[]) => void;
    onCommit: (page: HmiPageModel) => void;
    canvasRef: React.RefObject<HTMLDivElement | null>;
    /** The page's placed faceplates bound to their instances, by object id (§8.9). */
    placed?: Record<string, PlacedFaceplate | null>;
}

/** The design samples of the placed faceplates' objects (§8.9). */
function placedSamples(placed: Record<string, PlacedFaceplate | null>, catalogue: Catalogue): DrawFrame["values"] {
    let values: DrawFrame["values"] = {};
    for (const placement of Object.values(placed))
        if (placement) values = { ...values, ...designValues(placement.model, catalogue) };
    return values;
}

/** The design samples, with each previewed object's values in its state (§5.3 item 6). */
function previewed(
    page: HmiPageModel,
    catalogue: Catalogue,
    previews: Record<string, string> = {},
): DrawFrame["values"] {
    let values = designValues(page, catalogue);
    for (const object of page.object) {
        const id = previews[object.id];
        if (!id) continue;
        const properties = Object.keys(
            catalogue.components.find((component) => component.type === object.type)?.properties ?? {},
        );
        const state = previewStates(object.type, properties).find((candidate) => candidate.id === id);
        if (state) values = previewValues(object, state, values);
    }
    return values;
}

function within(box: { x: number; y: number; width: number; height: number }, [x, y]: Point): boolean {
    return x >= box.x && x <= box.x + box.width && y >= box.y && y <= box.y + box.height;
}

function handlePoint(box: { x: number; y: number; width: number; height: number }, handle: Handle): Point {
    const x = handle.includes("w") ? box.x : handle.includes("e") ? box.x + box.width : box.x + box.width / 2;
    const y = handle.includes("n") ? box.y : handle.includes("s") ? box.y + box.height : box.y + box.height / 2;
    return [x, y];
}

/** How far, in screen pixels, a press may travel and still be a click. */
const CLICK_SLOP = 3;
/** How near, in screen pixels, a moving object snaps to another's edge or centre (§3.3 item 8). */
const SNAP_PX = 5;

export function Canvas({
    dependencyPick,
    dependencyIds,
    composition,
    currentPage,
    sharedSelection,
    onSharedSelect,
    page,
    screen,
    catalogue,
    selection,
    zoom,
    assetBase,
    paused,
    problems,
    warnings,
    live,
    formats,
    pid,
    previews,
    speech,
    onObjectClick,
    translation,
    onTranslate,
    onTranslateCancel,
    onSelect,
    onCommit,
    canvasRef,
    hidden = [],
    onContextMenu,
    placed = {},
}: CanvasProps) {
    const contentRegion = composition?.regions?.find((region) => region.panel === null);
    const contentAt = contentRegion?.at ?? [0, 0];
    const sharedRegions = composition?.regions?.filter((region) => region.panel !== null) ?? [];
    const layerRef = useRef<SVGSVGElement>(null);
    const [draft, setDraft] = useState<HmiPageModel>();
    const [hover, setHover] = useState<string>();
    const [connecting, setConnecting] = useState<Connecting>();
    const [attaching, setAttaching] = useState<Attaching>();
    /** The box being dragged on empty canvas to select (§3.3 item 8). */
    const [box, setBox] = useState<{ from: Point; to: Point }>();
    /** The guides and equal gaps a move snaps to, while it lasts (§3.3 item 8). */
    const [guides, setGuides] = useState<{ lines: Guide[]; gaps: Gap[] }>();
    const shown = draft ?? page;
    const displaySize = shown.panel && shown.size ? shown.size : screen.size;
    const contentSize = contentRegion?.size ?? displaySize;
    /** What a press or a box can pick: neither locked nor hidden (§3.3 item 8). */
    const picks = useMemo(() => pickable(page, hidden), [page, hidden]);
    const [width, height] = displaySize;
    // Each component type's ports, as the catalogue names them (§6, item 1).
    const ports = useMemo(
        () =>
            Object.fromEntries(
                (catalogue?.components ?? []).map((component) => [component.type, component.ports ?? []]),
            ),
        [catalogue],
    );
    const routes = useMemo(() => pipeRoutes(shown, contentSize), [shown, contentSize]);

    const drawing = useMemo(() => {
        const samples =
            catalogue && !pid ? { ...placedSamples(placed, catalogue), ...previewed(shown, catalogue, previews) } : {};
        const frame: DrawFrame = {
            size: displaySize,
            currentPage,
            formats,
            ...(live ?? {
                values: samples,
                history: {},
                nowMs: DESIGN_NOW_MS,
            }),
            placed: live ? placed : buildPlacements(placed, formats),
            // Live mode observes; the controls send nothing from the editor (§11, item 6).
            readOnly: live !== undefined,
            alarms: live ? { unavailable: LIVE_ALARMS } : undefined,
            asset: (path: string) => (assetBase ? `${assetBase}/${path}` : path),
            problems,
            warnings,
            ...speech,
        };
        // A P&ID draws each symbol as it stands, without states: as if nothing were bound.
        const visible =
            hidden.length > 0
                ? { ...shown, object: shown.object.filter((object) => !hidden.includes(object.id)) }
                : shown;
        const composed = withSharedPanels(visible, composition);
        const drawn = live ? composed : buildPage(composed, pid ? {} : previews, formats);
        return renderToString(drawPage(drawn, frame));
    }, [
        composition,
        currentPage,
        shown,
        displaySize,
        catalogue,
        assetBase,
        problems,
        warnings,
        live,
        formats,
        previews,
        speech,
        pid,
        hidden,
        placed,
    ]);

    const toDesign = (clientX: number, clientY: number): Point => {
        const rect = layerRef.current?.getBoundingClientRect();
        return [
            (clientX - (rect?.left ?? 0)) / zoom - contentAt[0],
            (clientY - (rect?.top ?? 0)) / zoom - contentAt[1],
        ];
    };

    /** Follows a drag until the button is released. The listeners are added here, in the press, so a
     * release that arrives before the next render still ends the drag. */
    const follow = (drag: Drag): void => {
        /** The page as the drag leaves it, with the guides a move snapped to. */
        const result = (event: MouseEvent): { page: HmiPageModel; guides?: { lines: Guide[]; gaps: Gap[] } } => {
            const point = toDesign(event.clientX, event.clientY);
            const delta: Point = [point[0] - drag.start[0], point[1] - drag.start[1]];
            if (drag.kind === "bend") return { page: bendPipe(drag.base, drag.ids[0], drag.start, point, screen.size) };
            if (drag.kind === "move") {
                const snapped = moveWithGuides(drag.base, drag.ids, delta, event.altKey, SNAP_PX / zoom);
                return {
                    page: snapped.page,
                    guides:
                        snapped.guides.length + snapped.gaps.length > 0
                            ? { lines: snapped.guides, gaps: snapped.gaps }
                            : undefined,
                };
            }
            return { page: resizeObject(drag.base, drag.ids[0], drag.handle ?? "se", delta, event.altKey) };
        };
        // A press that never moves is a click: it selects, and never moves or resizes (onto the grid).
        let moved = false;
        const travelled = (event: MouseEvent): boolean => {
            const point = toDesign(event.clientX, event.clientY);
            moved = moved || Math.hypot(point[0] - drag.start[0], point[1] - drag.start[1]) * zoom > CLICK_SLOP;
            return moved;
        };
        const move = (event: MouseEvent): void => {
            if (!travelled(event)) return;
            const next = result(event);
            setDraft(next.page);
            setGuides(next.guides);
        };
        const up = (event: MouseEvent): void => {
            window.removeEventListener("mousemove", move);
            window.removeEventListener("mouseup", up);
            setDraft(undefined);
            setGuides(undefined);
            if (!travelled(event)) {
                if (drag.kind === "move") onObjectClick?.(drag.ids[0]);
                if (drag.collapse) onSelect(drag.collapse);
                return;
            }
            let finished = result(event).page;
            // One object dropped onto a pipe it can sit in splits the pipe (§6, item 4).
            if (drag.kind === "move" && drag.ids.length === 1)
                finished = splitPipe(finished, drag.ids[0], ports, screen.size);
            if (!sameModel(finished, drag.base)) onCommit(finished);
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
    };

    /** Draws a pipe from a port until the button is released on another port, or on a pipe, which
     * it then branches into through a new junction (§6, item 2). */
    const connect = (from: string, point: Point): void => {
        setConnecting({ from, at: point });
        const move = (event: MouseEvent): void => setConnecting({ from, at: toDesign(event.clientX, event.clientY) });
        const up = (event: MouseEvent): void => {
            window.removeEventListener("mousemove", move);
            window.removeEventListener("mouseup", up);
            setConnecting(undefined);
            const point = toDesign(event.clientX, event.clientY);
            const to = portAt(page, point, ports);
            const joined = to ? addPipe(page, from, to) : branchPipe(page, from, point, ports, screen.size);
            if (joined === page) return;
            onCommit(joined);
            // The new pipe is the last; selected, its inspector is at hand.
            onSelect([`pipe:${joined.pipe.length - 1}`]);
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
    };

    /** Attaches an instrument to the equipment or the pipe the button is released on (§6, item 5). */
    const attach = (id: string, from: Point): void => {
        setAttaching({ id, from, at: from });
        const move = (event: MouseEvent): void =>
            setAttaching({ id, from, at: toDesign(event.clientX, event.clientY) });
        const up = (event: MouseEvent): void => {
            window.removeEventListener("mousemove", move);
            window.removeEventListener("mouseup", up);
            setAttaching(undefined);
            const point = toDesign(event.clientX, event.clientY);
            const offered = new Set(attachTargets(page, catalogue, id).map(([value]) => value));
            const target =
                [...page.object].reverse().find((object) => offered.has(object.id) && within(drawnBox(object), point))
                    ?.id ?? pipeAt(page, point, screen.size);
            if (!target || !offered.has(target)) return;
            onCommit({
                ...page,
                object: page.object.map((object) => (object.id === id ? withAttach(object, target) : object)),
            });
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
    };

    /** A box dragged on empty canvas selects the objects wholly inside it; with Shift it adds them. */
    const boxSelect = (start: Point, base: string[]): void => {
        setBox({ from: start, to: start });
        const move = (event: MouseEvent): void => setBox({ from: start, to: toDesign(event.clientX, event.clientY) });
        const up = (event: MouseEvent): void => {
            window.removeEventListener("mousemove", move);
            window.removeEventListener("mouseup", up);
            setBox(undefined);
            const inside = withGroups(picks, objectsInBox(picks, start, toDesign(event.clientX, event.clientY)));
            if (inside.length > 0) onSelect([...base, ...inside.filter((id) => !base.includes(id))]);
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
    };

    const onMouseDown = (event: ReactMouseEvent<SVGSVGElement>): void => {
        if (event.button !== 0) return;
        const point = toDesign(event.clientX, event.clientY);
        if (dependencyPick) {
            dependencyPick(
                objectAt({ ...page, object: page.object.filter((object) => !hidden.includes(object.id)) }, point),
            );
            event.preventDefault();
            return;
        }
        const handleOf = (event.target as Element).closest("[data-part='attach-handle']")?.getAttribute("data-object");
        if (handleOf && !paused) {
            attach(handleOf, point);
            event.preventDefault();
            return;
        }
        const port = (event.target as Element).closest("[data-port]")?.getAttribute("data-port");
        if (port && !paused) {
            connect(port, point);
            event.preventDefault();
            return;
        }
        const handle = (event.target as Element).closest("[data-handle]")?.getAttribute("data-handle") as Handle | null;
        if (handle && selection.length === 1 && !paused) {
            follow({ kind: "resize", handle, ids: selection, start: point, base: page });
            event.preventDefault();
            return;
        }
        const solid = {
            ...picks,
            object: picks.object.filter((item) => item.type !== "group" && item.type !== "panel"),
        };
        const hit = objectAt(solid, point) ?? pipeAt(picks, point, screen.size) ?? objectAt(picks, point);
        const pipe = hit !== undefined && pipeIndex(hit) !== undefined;
        // A press on a selected pipe bends it; a first press selects it.
        if (pipe && selection.length === 1 && selection[0] === hit && !paused) {
            follow({ kind: "bend", ids: [hit], start: point, base: page });
            event.preventDefault();
            return;
        }
        // A press on the selection keeps it, so a drag moves it all, and a click without a drag then
        // selects the pressed object; a member of a group brings its group (§3.3 item 8).
        const pressed = hit !== undefined && !pipe ? hit : undefined;
        const keep = !event.shiftKey && pressed !== undefined && selection.includes(pressed);
        const clicked = selectFromClick(selection, hit, event.shiftKey);
        const next = keep
            ? selection
            : pressed !== undefined && !selection.includes(pressed)
              ? withGroups(picks, clicked)
              : clicked;
        onSelect(next);
        if (hit === undefined && !paused) {
            boxSelect(point, next);
            event.preventDefault();
            return;
        }
        if (hit && !pipe && next.includes(hit) && !paused) {
            follow({
                kind: "move",
                ids: [
                    hit,
                    ...next.filter(
                        (id) =>
                            id !== hit &&
                            pipeIndex(id) === undefined &&
                            picks.object.some((object) => object.id === id),
                    ),
                ],
                start: point,
                base: page,
                ...(keep ? { collapse: withGroups(page, [hit]) } : {}),
            });
            event.preventDefault();
        }
    };

    const resizePanel = (event: ReactMouseEvent<SVGElement>, index: 0 | 1): void => {
        if (!page.panel || !page.size || paused) return;
        event.stopPropagation();
        event.preventDefault();
        const start = index === 0 ? event.clientX : event.clientY;
        const base = [...page.size] as [number, number];
        let last = page;
        const move = (pointer: MouseEvent): void => {
            const size = [...base] as [number, number];
            size[index] = Math.max(
                32,
                Math.min(
                    16384,
                    Math.round(
                        (base[index] + ((index === 0 ? pointer.clientX : pointer.clientY) - start) / zoom) / GRID,
                    ) * GRID,
                ),
            );
            last = { ...page, size };
            setDraft(last);
        };
        const up = (): void => {
            window.removeEventListener("mousemove", move);
            window.removeEventListener("mouseup", up);
            setDraft(undefined);
            if (!sameModel(last, page)) onCommit(last);
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
    };

    /** The symbol under the pointer shows its ports (§6, item 1). */
    const onMouseMove = (event: ReactMouseEvent<SVGSVGElement>): void => {
        if (connecting || paused) return;
        const hovered = objectAt(picks, toDesign(event.clientX, event.clientY));
        if (hovered !== hover) setHover(hovered);
    };

    // The ports shown: every port while a pipe is drawn, else those of the symbol pointed at and
    // of the selected objects.
    const portsShown = paused
        ? []
        : shown.object
              .filter((object) => connecting !== undefined || object.id === hover || selection.includes(object.id))
              .flatMap((object) =>
                  (ports[object.type] ?? []).map((name) => ({
                      id: `${object.id}.${name}`,
                      at: portPoint(object, name)?.at,
                  })),
              )
              .filter((entry): entry is { id: string; at: [number, number] } => entry.at !== undefined);
    const drawingFrom = connecting ? portsShown.find((entry) => entry.id === connecting.from)?.at : undefined;

    const single = selection.length === 1 ? shown.object.find((object) => object.id === selection[0]) : undefined;
    // A selected instrument's handle, at the foot of its bubble (§6, item 5).
    const instrument =
        single && catalogue?.components.find((component) => component.type === single.type)?.attaches && !paused
            ? attachOrigin(single)
            : undefined;
    const translated = translation ? shown.object.find((object) => object.id === translation.id) : undefined;
    const translatedBox = translated ? drawnBox(translated) : undefined;
    const handleSize = HANDLE_PX / zoom;
    return (
        <div
            className="hb-canvas"
            ref={canvasRef}
            onContextMenu={(event) => {
                // Anywhere on the canvas. The menu acts on what was right-clicked: an object outside the
                // selection becomes the selection, with its group.
                event.preventDefault();
                if (dependencyPick) return;
                const hit = objectAt(picks, toDesign(event.clientX, event.clientY));
                if (hit && !selection.includes(hit)) onSelect(withGroups(picks, [hit]));
                onContextMenu?.([event.clientX, event.clientY]);
            }}
        >
            <div
                data-part="canvas-page"
                className="hb-canvas-page"
                style={{ width: `${width * zoom}px`, height: `${height * zoom}px` }}
            >
                {!live && !pid && Object.keys(previews ?? {}).length > 0 && (
                    <span className="hb-design-sample-notice" data-part="design-sample-notice">
                        Preview · test state
                    </span>
                )}
                <div
                    className={`hb-drawing hmi-theme-${screen.theme}`}
                    lang={speech?.language}
                    dangerouslySetInnerHTML={{ __html: drawing }}
                />
                <svg
                    ref={layerRef}
                    data-part="editing-layer"
                    className="hb-layer"
                    viewBox={`0 0 ${width} ${height}`}
                    onMouseDown={onMouseDown}
                    onDoubleClick={(event) => {
                        if (dependencyPick) return;
                        // A double-click edits one member of a group (§3.3 item 8).
                        const hit = objectAt(picks, toDesign(event.clientX, event.clientY));
                        if (hit) onSelect([hit]);
                    }}
                    onMouseMove={dependencyPick ? undefined : onMouseMove}
                    onMouseLeave={() => setHover(undefined)}
                    style={{ backgroundSize: `${GRID * zoom}px ${GRID * zoom}px` }}
                >
                    {dependencyIds?.map((id) => {
                        const object = page.object.find((object) => object.id === id);
                        if (!object) return null;
                        const box = drawnBox(object);
                        return (
                            <rect
                                key={`pick-${id}`}
                                data-dependency-choice={id}
                                x={box.x + contentAt[0] - 4}
                                y={box.y + contentAt[1] - 4}
                                width={box.width + 8}
                                height={box.height + 8}
                                fill="none"
                                stroke="var(--vscode-focusBorder)"
                                strokeWidth={2 / zoom}
                                pointerEvents="none"
                            />
                        );
                    })}
                    <svg
                        x={contentAt[0]}
                        y={contentAt[1]}
                        width={contentSize[0]}
                        height={contentSize[1]}
                        viewBox={`0 0 ${contentSize[0]} ${contentSize[1]}`}
                        overflow="hidden"
                    >
                        <rect x={0} y={0} width={contentSize[0]} height={contentSize[1]} fill="transparent" />
                        {guides?.lines.map((guide, index) =>
                            guide.axis === "x" ? (
                                <line
                                    key={`g${index}`}
                                    data-part="guide"
                                    data-axis="x"
                                    data-at={guide.at}
                                    className="hb-guide"
                                    x1={guide.at}
                                    x2={guide.at}
                                    y1={guide.from}
                                    y2={guide.to}
                                    strokeWidth={1 / zoom}
                                />
                            ) : (
                                <line
                                    key={`g${index}`}
                                    data-part="guide"
                                    data-axis="y"
                                    data-at={guide.at}
                                    className="hb-guide"
                                    y1={guide.at}
                                    y2={guide.at}
                                    x1={guide.from}
                                    x2={guide.to}
                                    strokeWidth={1 / zoom}
                                />
                            ),
                        )}
                        {guides?.gaps.map((gap, index) =>
                            gap.axis === "x" ? (
                                <line
                                    key={`p${index}`}
                                    data-part="gap"
                                    className="hb-gap"
                                    x1={gap.from}
                                    x2={gap.to}
                                    y1={gap.at}
                                    y2={gap.at}
                                    strokeWidth={1 / zoom}
                                />
                            ) : (
                                <line
                                    key={`p${index}`}
                                    data-part="gap"
                                    className="hb-gap"
                                    y1={gap.from}
                                    y2={gap.to}
                                    x1={gap.at}
                                    x2={gap.at}
                                    strokeWidth={1 / zoom}
                                />
                            ),
                        )}
                        {box ? (
                            <rect
                                data-part="selection-box"
                                className="hb-selection-box"
                                x={Math.min(box.from[0], box.to[0])}
                                y={Math.min(box.from[1], box.to[1])}
                                width={Math.abs(box.to[0] - box.from[0])}
                                height={Math.abs(box.to[1] - box.from[1])}
                                strokeWidth={1 / zoom}
                            />
                        ) : null}
                        {selection.map((id) => {
                            const index = pipeIndex(id);
                            const route = index === undefined ? undefined : routes[index];
                            if (!route) return null;
                            return (
                                <g key={id}>
                                    <polyline
                                        data-selection={id}
                                        className="hb-selection-pipe"
                                        points={route.map((point) => point.join(",")).join(" ")}
                                        strokeWidth={10 / zoom}
                                    />
                                    {!paused
                                        ? route.slice(1).map((end, segment) => {
                                              const start = route[segment];
                                              return (
                                                  <rect
                                                      key={segment}
                                                      data-pipe-segment={segment}
                                                      className="hb-handle"
                                                      x={(start[0] + end[0]) / 2 - handleSize / 2}
                                                      y={(start[1] + end[1]) / 2 - handleSize / 2}
                                                      width={handleSize}
                                                      height={handleSize}
                                                      strokeWidth={1 / zoom}
                                                  >
                                                      <title>Drag to move this pipe segment</title>
                                                  </rect>
                                              );
                                          })
                                        : null}
                                </g>
                            );
                        })}
                        {selection.map((id) => {
                            const object = shown.object.find((candidate) => candidate.id === id);
                            if (!object) return null;
                            const box = drawnBox(object);
                            return (
                                <rect
                                    key={id}
                                    data-selection={id}
                                    className="hb-selection"
                                    x={box.x}
                                    y={box.y}
                                    width={box.width}
                                    height={box.height}
                                    strokeWidth={1.5 / zoom}
                                />
                            );
                        })}
                        {single && single.rotation % 180 === 0 && !paused
                            ? HANDLES.map((handle) => {
                                  const [x, y] = handlePoint(drawnBox(single), handle);
                                  return (
                                      <rect
                                          key={handle}
                                          data-handle={handle}
                                          className={`hb-handle hb-handle-${handle}`}
                                          x={x - handleSize / 2}
                                          y={y - handleSize / 2}
                                          width={handleSize}
                                          height={handleSize}
                                          strokeWidth={1 / zoom}
                                      />
                                  );
                              })
                            : null}
                        {drawingFrom && connecting ? (
                            <line
                                data-part="drawing-pipe"
                                className="hb-drawing-pipe"
                                x1={drawingFrom[0]}
                                y1={drawingFrom[1]}
                                x2={connecting.at[0]}
                                y2={connecting.at[1]}
                                strokeWidth={2 / zoom}
                            />
                        ) : null}
                        {portsShown.map((entry) => (
                            <circle
                                key={entry.id}
                                data-port={entry.id}
                                className="hb-port"
                                cx={entry.at[0]}
                                cy={entry.at[1]}
                                r={6 / zoom}
                                strokeWidth={2 / zoom}
                            />
                        ))}
                        {attaching ? (
                            <line
                                data-part="attaching"
                                className="hb-drawing-pipe"
                                x1={attaching.from[0]}
                                y1={attaching.from[1]}
                                x2={attaching.at[0]}
                                y2={attaching.at[1]}
                                strokeWidth={2 / zoom}
                            />
                        ) : null}
                        {instrument && single ? (
                            <circle
                                data-part="attach-handle"
                                data-object={single.id}
                                className="hb-port"
                                cx={instrument.centre[0]}
                                cy={instrument.centre[1] + instrument.radius}
                                r={6 / zoom}
                                strokeWidth={2 / zoom}
                            >
                                <title>Drag onto what it measures</title>
                            </circle>
                        ) : null}
                    </svg>
                    {page.panel && page.size && !paused ? (
                        <>
                            <rect
                                data-part="panel-width-handle"
                                x={width - 8 / zoom}
                                y={0}
                                width={8 / zoom}
                                height={height}
                                fill="transparent"
                                style={{ cursor: "ew-resize" }}
                                onMouseDown={(event) => resizePanel(event, 0)}
                            />
                            <rect
                                data-part="panel-height-handle"
                                x={0}
                                y={height - 8 / zoom}
                                width={width}
                                height={8 / zoom}
                                fill="transparent"
                                style={{ cursor: "ns-resize" }}
                                onMouseDown={(event) => resizePanel(event, 1)}
                            />
                        </>
                    ) : null}
                    {sharedRegions.map((region) => (
                        <rect
                            key={region.panel}
                            data-shared-panel-select={region.panel}
                            x={region.at[0]}
                            y={region.at[1]}
                            width={region.size[0]}
                            height={region.size[1]}
                            fill="transparent"
                            stroke={sharedSelection === region.panel ? "var(--vscode-focusBorder)" : "none"}
                            strokeWidth={2 / zoom}
                            onMouseDown={(event) => {
                                event.stopPropagation();
                                if (region.panel) onSharedSelect?.(region.panel);
                            }}
                            onDoubleClick={(event) => event.stopPropagation()}
                        >
                            <title>Shared panel: {region.panel}</title>
                        </rect>
                    ))}
                </svg>
                {translation && translatedBox ? (
                    <TranslationField
                        key={`${translation.id}:${translation.text}`}
                        text={translation.text}
                        translation={translation.translation}
                        placeholder={translation.placeholder}
                        languageName={translation.languageName}
                        box={{
                            x: (translatedBox.x + contentAt[0]) * zoom,
                            y: (translatedBox.y + contentAt[1]) * zoom,
                            width: translatedBox.width * zoom,
                        }}
                        onCommit={(value) => onTranslate?.(value)}
                        onCancel={() => onTranslateCancel?.()}
                    />
                ) : null}
            </div>
        </div>
    );
}
