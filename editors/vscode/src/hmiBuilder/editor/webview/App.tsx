// The HMI Builder webview (§3.2–3.4): Components, canvas and inspector around one page. Edits are
// sent to the editor, which writes them into the page's text; the page that comes back is the one
// shown.

import { useCallback, useEffect, useRef, useState, useMemo } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";

import { LiveValueStore, StreamMessage } from "../../display/liveValues";
import { trendSpans } from "../../display/shared";
import { adaptPage } from "../../drawing/adaptive";
import type { HmiPageModel } from "../../drawing/model";
import { tokensCss } from "../../drawing/tokens";
import { alignObjects, distributeObjects } from "../align";
import type { Catalogue, ComponentDefinition } from "../catalogue";
import { copyText, duplicated, PASTE_STEP, pasted } from "../clipboard";
import type { DeployOutcome } from "../deploy";
import { addObject, cornerFor, deleteObjects, freeCorner, Point, updateObject, visiblePageCentre } from "../editing";
import { definitions, palette as paletteOf, placed as placedEquipment } from "../equipment";
import { languageTables } from "../languages";
import { groupObjects, reorder, ungroupObjects, withLock } from "../layers";
import { DEFAULT_SCREEN, FaceplateEntry, HostToWebview, LiveState, ScreenModel, VariableNode } from "../messages";
import type { FileError } from "../pageDocument";
import { deletePipes, pipeIndex, splitPipe } from "../pipes";
import { addPlaced, besideSymbol, panelOf, placedPaths, placedSize, symbolInstance } from "../placed";
import { dropped } from "../variables";

import { BottomPanel, BottomTab } from "./BottomPanel";
import { Canvas } from "./Canvas";
import { ComponentsPanel } from "./ComponentsPanel";
import { ContextMenu, EditAction, menuEntries } from "./ContextMenu";
import { Inspector, VariableDrag } from "./Inspector";
import { InstanceChooser } from "./InstanceChooser";
import { LayersPanel } from "./LayersPanel";
import { LiveOperating } from "./LiveOperating";
import { Banner, DeployNotice } from "./Notices";
import { PanelStrip } from "./PanelTitle";
import type { FittingInstances } from "./PlacedSection";
import { SCREEN_SIZES, TopBar } from "./TopBar";
import { canvasOwnsKey } from "./canvasKeys";
import { useDependencyPick } from "./dependencyPick";
import { boundPaths, pageArea } from "./liveFrames";
import { componentName } from "./names";
import { PickerValues } from "./pickerValues";
import { refusalText, Rust, rustChannel, useRustAnswer, useRustRules, withObject } from "./rust";
import { useCanvasShortcuts } from "./useCanvasShortcuts";
import { useCanvasZoom } from "./useCanvasZoom";
import { useEquipmentEditing } from "./useEquipmentEditing";
import { useHostReports } from "./useHostReports";
import { usePanelArrangement } from "./usePanelArrangement";
import type { VsCodeApi } from "./vscodeApi";

interface AppProps {
    api: VsCodeApi;
    fontUrl: string | undefined;
    assetBase: string | undefined;
}

/** What is being dragged onto the canvas, and where the pointer is. */
interface Ghost {
    text: string;
    at: Point;
}

/** How long a notice stays, in milliseconds. */
const NOTICE_MS = 6000;

/** The previews with one object's state set, or cleared for the sample. */
function withPreview(previews: Record<string, string>, id: string, state: string | undefined): Record<string, string> {
    const next = { ...previews };
    if (state) next[id] = state;
    else delete next[id];
    return next;
}

/** Each component type's ports, as the catalogue names them (§6, item 1). */
function portsOf(catalogue: Catalogue | undefined): Record<string, string[]> {
    return Object.fromEntries(
        (catalogue?.components ?? []).map((component) => [component.type, component.ports ?? []]),
    );
}

export function App({ api, fontUrl, assetBase }: AppProps) {
    const [screen, setScreen] = useState<ScreenModel>(DEFAULT_SCREEN);
    const [page, setPage] = useState<HmiPageModel>();
    const dependencyPick = useDependencyPick(page);
    const equipmentEditing = useEquipmentEditing(api, dependencyPick.begin);
    // Registered before the page's own listener, so nothing the host sends after "ready" is missed.
    const {
        catalogue,
        formats,
        pages,
        problems,
        points,
        warnings,
        colourWarnings,
        targets,
        faceplates,
        placed,
        projectTexts,
        composition,
        pidView,
        pickerUpdate,
    } = useHostReports();
    const [errors, setErrors] = useState<FileError[]>();
    const [refused, setRefused] = useState<FileError[]>();
    const [selection, setSelection] = useState<string[]>([]);
    useEffect(() => {
        api.postMessage({ type: "selection", ids: selection });
    }, [api, selection]);
    /** The left panel's tab, and the objects hidden in the editor from the Layers panel (§3.3 item 8). */
    const [leftTab, setLeftTab] = useState<"components" | "layers">("components");
    /** The project's pages for the Pages panel, and the one edited (§3.3 item 8). */
    const [sharedSelection, setSharedSelection] = useState<string>();
    useEffect(() => {
        if (selection.length > 0) setSharedSelection(undefined);
    }, [selection]);
    const [hiddenObjects, setHiddenObjects] = useState<string[]>([]);
    const [ghost, setGhost] = useState<Ghost>();
    const [variables, setVariables] = useState<VariableNode[]>();
    const [variablesProblem, setVariablesProblem] = useState<string>();
    const [notice, setNotice] = useState<string>();
    /** The faceplates to place, the page's placements bound by the host, the instances a faceplate
     * fits as last answered, and the faceplate being placed with where (§8.9). */
    const [fitting, setFitting] = useState<FittingInstances>();
    const [placing, setPlacing] = useState<{ faceplate: FaceplateEntry; centre: Point }>();
    // The project's languages (§14.5 item 7): the one the canvas shows, the "longest" preview, and
    // the text being translated in place.
    const [language, setLanguage] = useState<string>();
    const [longest, setLongest] = useState(false);
    const [translating, setTranslating] = useState<{ id: string; text: string }>();
    const [bottomTab, setBottomTab] = useState<BottomTab>("tags");
    const warningTexts = useMemo(
        () =>
            Object.fromEntries(
                Object.entries(warnings).map(([id, list]) => [id, list.map((warning) => warning.message)]),
            ),
        [warnings],
    );
    // Live mode (§11): the run target's state and its values, drawn in place of design samples.
    const [mode, setMode] = useState<"design" | "live">("design");
    const [operating, setOperating] = useState(false);
    const [screenSize, setScreenSize] = useState("design");
    // The state switcher's previews (§5.3 item 6), by object id; never saved.
    const [previews, setPreviews] = useState<Record<string, string>>({});
    const [live, setLive] = useState<{ state?: LiveState; target?: string; reason?: string }>({});
    const liveStore = useRef<LiveValueStore>(undefined);
    const [, setLiveTick] = useState(0);
    // Deploy (§16.2, item 8): under way, and the last outcome.
    const [deploying, setDeploying] = useState(false);
    const [deployed, setDeployed] = useState<{ action: "deploy" | "rollback"; outcome: DeployOutcome }>();
    const variableDrag = useRef<VariableDrag>(undefined);
    // The language server's rules (AGENTS.md: all logic is in Rust), asked through the editor.
    const channel = useMemo(() => rustChannel(api), [api]);
    const { arrangement, shown, adopt, remember, collapse, open, dragEdge } = usePanelArrangement(api);
    const hostPage = useRef<HmiPageModel>(undefined);
    /** The last duplicate, so the next Ctrl+D repeats the step its copy was moved by (§3.3 item 8). */
    const lastDuplicate = useRef<{ sources: string[]; copies: string[] }>(undefined);
    const canvasRef = useRef<HTMLDivElement>(null);
    // A popup is drawn at its own size, a page at the screen's (§4.9).
    // A page previewed on a smaller screen, as the runtime lays it out there, read-only (§4 item 5).
    const previewSize =
        mode === "design" && page && !page.size
            ? SCREEN_SIZES.find((entry) => entry.value === screenSize)?.size
            : undefined;
    const preview = previewSize && page ? adaptPage(page, screen.size, pageArea(previewSize, screen)) : undefined;
    const canvasScreen: ScreenModel = preview
        ? { ...screen, size: preview.size }
        : page?.size
          ? { ...screen, size: page.size }
          : screen;
    const { zoom, setZoom, fit, zoomToSelection, restore } = useCanvasZoom(
        api,
        canvasRef,
        page,
        selection,
        canvasScreen.size,
    );
    // Editing stops while the text does not load and in Live mode.
    const paused = errors !== undefined || mode === "live";
    // A placed faceplate shows its bound faceplate's values (§8.9).
    const livePaths = page ? [...new Set([...boundPaths(page), ...placedPaths(placed)])] : [];
    const livePathsKey = livePaths.join("\n");

    // In Live mode the host reads the page's bound paths; a changed page changes them.
    useEffect(() => {
        if (mode !== "live") return;
        api.postMessage({ type: "live", on: true, paths: livePathsKey ? livePathsKey.split("\n") : [] });
    }, [api, mode, livePathsKey]);
    useEffect(() => {
        if (mode !== "live") return;
        const timer = window.setInterval(() => setLiveTick((tick) => tick + 1), 1000);
        return () => window.clearInterval(timer);
    }, [mode]);
    const switchMode = (next: "design" | "live"): void => {
        if (next === mode) return;
        if (next === "live") {
            const placedModels = Object.values(placed).flatMap((entry) => (entry ? [entry.model] : []));
            liveStore.current = new LiveValueStore({ trendSpansMs: page ? trendSpans(page, placedModels) : {} });
            setLive({});
        } else {
            liveStore.current = undefined;
            api.postMessage({ type: "live", on: false });
        }
        setMode(next);
    };

    const commit = useCallback(
        (next: HmiPageModel): void => {
            if (paused || !page) return;
            setRefused(undefined);
            setPage(next);
            // A second answer arriving before the next drawing changes this page, not the one before.
            latest.current.page = next;
            api.postMessage({ type: "edit", model: next });
        },
        [api, paused, page],
    );
    /** What a host message acts on, as it is now. */
    const latest = useRef({ page, variables, commit });
    latest.current = { page, variables, commit };
    const rust = useRustRules(channel.call, latest, setSelection, setNotice);
    const changePage = rust.change;
    const showError = (error: unknown): void => setNotice(refusalText(error));
    // The Components panel's entries and the definition each equipment object shows are the language
    // server's (§21.3 item 1, §8.5), kept here so a panel opened again shows them at once.
    const palette = useRustAnswer(
        channel.call,
        [catalogue, pidView],
        (call) => (catalogue ? paletteOf(call, catalogue, pidView) : undefined),
        true,
    );
    const blocks = [...new Set((page?.object ?? []).flatMap((object) => (object.block ? [object.block] : [])))].sort();
    const shownEquipment = useRustAnswer(
        channel.call,
        [catalogue, blocks.join("\n")],
        (call) => (catalogue && blocks.length > 0 ? definitions(call, catalogue.equipment ?? [], blocks) : undefined),
        true,
    );

    useEffect(() => {
        const onMessage = (event: MessageEvent<HostToWebview>): void => {
            const message = event.data;
            switch (message?.type) {
                case "page":
                    hostPage.current = message.model;
                    setPage(message.model);
                    setErrors(undefined);
                    setRefused(undefined);
                    // The selection keeps what the page still has: its objects, and its pipes by place.
                    setSelection((current) =>
                        current.filter((id) => {
                            const pipe = pipeIndex(id);
                            return pipe === undefined
                                ? message.model.object.some((object) => object.id === id)
                                : pipe < message.model.pipe.length;
                        }),
                    );
                    return;
                case "invalid":
                    setErrors(message.errors);
                    return;
                case "editRefused":
                    setRefused(message.errors);
                    setPage(hostPage.current);
                    return;
                case "screen":
                    setScreen(message.screen);
                    return;
                case "variables":
                    setVariables(message.variables);
                    setVariablesProblem(message.problem);
                    return;
                case "zoom":
                    // The page opens at the zoom it was last left at, instead of fitted (§3.3 item 8).
                    restore(message.zoom);
                    return;
                case "rustAnswer":
                    channel.receive(message);
                    return;
                case "clipboard": {
                    // A paste: the clipboard's objects land next to where they were copied (§3.3 item 8).
                    const text = message.text;
                    changePage((current) =>
                        pasted(channel.call, current, text, PASTE_STEP, latest.current.variables ?? []).then(
                            (result) => result && { page: result.page, select: result.ids },
                        ),
                    );
                    return;
                }
                case "arrangement":
                    adopt(message.arrangement);
                    return;
                case "live":
                    setLive({ state: message.state, target: message.target, reason: message.reason });
                    if (message.state !== "connected") liveStore.current?.disconnected(Date.now());
                    setLiveTick((tick) => tick + 1);
                    return;
                case "liveValues":
                    liveStore.current?.receive(message.update as StreamMessage, Date.now());
                    setLiveTick((tick) => tick + 1);
                    return;
                case "deploying":
                    setDeploying(true);
                    setDeployed(undefined);
                    return;
                case "deployed":
                    setDeploying(false);
                    setDeployed({ action: message.action, outcome: message.outcome });
                    return;
                case "faceplateInstances":
                    setFitting({
                        faceplate: message.faceplate,
                        instances: message.instances,
                        ...(message.problem ? { problem: message.problem } : {}),
                    });
                    return;
            }
        };
        window.addEventListener("message", onMessage);
        api.postMessage({ type: "ready" });
        return () => window.removeEventListener("message", onMessage);
    }, [adopt, api, restore, channel, changePage]);

    const tables = useMemo(() => (projectTexts ? languageTables(projectTexts) : []), [projectTexts]);
    // The language shown; a language no longer in the project falls back to the first.
    const table = tables.find((candidate) => candidate.tag === language) ?? tables[0];
    const speech = useMemo(
        () => (table ? { texts: table.texts, language: table.tag, longest } : { longest }),
        [table, longest],
    );
    const translatable = mode === "design" && table !== undefined && table.tag !== tables[0]?.tag;
    /** A click on an object in a language other than the first edits its text's translation. */
    const objectClicked = (id: string): void => {
        const object = page?.object.find((candidate) => candidate.id === id);
        const text = object?.text ?? object?.label;
        if (translatable && text) setTranslating({ id, text });
    };
    const translation =
        translating && table && translatable && page?.object.some((object) => object.id === translating.id)
            ? {
                  ...translating,
                  translation: projectTexts?.project[table.tag]?.[translating.text],
                  placeholder: projectTexts?.builtin[table.tag]?.[translating.text] ?? translating.text,
                  languageName: table.name,
              }
            : undefined;
    const translate = (tag: string, text: string, value: string): void =>
        api.postMessage({ type: "translate", language: tag, text, translation: value });
    /** A bottom tab's button: opens the panel on it, or closes the panel when it is shown. */
    const showTab = (tab: BottomTab): void => {
        if (arrangement.bottom.open && bottomTab === tab) {
            remember({ ...arrangement, bottom: { ...arrangement.bottom, open: false } });
            return;
        }
        setBottomTab(tab);
        if (!arrangement.bottom.open) remember({ ...arrangement, bottom: { ...arrangement.bottom, open: true } });
    };

    const visibleCentre = (): Point => {
        const area = canvasRef.current?.getBoundingClientRect();
        const sheet = canvasRef.current?.querySelector('[data-part="canvas-page"]')?.getBoundingClientRect();
        const none = { left: 0, top: 0, width: 0, height: 0 };
        const centre = visiblePageCentre(area ?? none, sheet ?? none, zoom, canvasScreen.size);
        const content = composition?.regions?.find((region) => region.panel === null);
        return content ? [Math.max(0, centre[0] - content.at[0]), Math.max(0, centre[1] - content.at[1])] : centre;
    };

    const add = (component: ComponentDefinition, centre: Point): void => {
        if (paused || !page) return;
        const size = canvasScreen.size;
        const ports = portsOf(catalogue);
        const placeOn = (current: HmiPageModel) =>
            addObject(
                current,
                component,
                freeCorner(current, cornerFor(component.size, centre, size), component.size, size),
            );
        const definition = component.equipment;
        if (!definition) {
            const added = placeOn(page);
            // A valve added onto a pipe splits it (§6, item 4).
            commit(splitPipe(added.page, added.id, ports, size));
            setSelection([added.id]);
        } else {
            // One of the project's own blocks placed for the first time: its files are written (§21.9).
            if (definition.proposed) api.postMessage({ type: "ensureEquipment", block: definition.block });
            const known = variables ?? [];
            // The language server binds the new object to its instance (§21.3 item 3).
            changePage(async (current) => {
                const added = placeOn(current);
                const object = added.page.object.find((candidate) => candidate.id === added.id);
                if (!object) return undefined;
                const bound = await placedEquipment(channel.call, object, definition, known, current);
                return { page: splitPipe(withObject(added.page, bound), added.id, ports, size), select: [added.id] };
            });
        }
        // The new object takes the keys: the card or the search that added it gives up the focus
        // (§21.3 item 6), so the arrows move what was just placed.
        const focused = document.activeElement as HTMLElement | null;
        if (focused && !canvasOwnsKey(focused)) focused.blur();
    };

    /** Asks for the instances a faceplate fits; the answer comes as a message. */
    const askInstances = (faceplate: string): void => {
        setFitting(undefined);
        api.postMessage({ type: "faceplateInstances", faceplate });
    };

    /** A faceplate dropped or clicked in the Faceplates group: it asks for its instance (§8.9). */
    const placeFaceplate = (faceplate: FaceplateEntry, centre: Point): void => {
        if (paused || !page) return;
        if (faceplate.requiresInstance === false) {
            const size = placedSize(faceplate);
            const added = addPlaced(page, faceplate.name, "", cornerFor(size, centre, canvasScreen.size), size);
            commit(added.page);
            setSelection([added.id]);
            return;
        }
        setPlacing({ faceplate, centre });
        askInstances(faceplate.name);
    };

    /** Places the faceplate being placed for the instance chosen, at its own size. */
    const placeFor = (instance: string): void => {
        const current = placing;
        setPlacing(undefined);
        if (!current || paused || !page) return;
        const size = placedSize(current.faceplate);
        const added = addPlaced(
            page,
            current.faceplate.name,
            instance,
            cornerFor(size, current.centre, canvasScreen.size),
            size,
        );
        commit(added.page);
        setSelection([added.id]);
    };

    /** A variable dropped on the page: the component the language server says it makes, bound to it
     * and named after it (§7.1), or why it makes none. */
    const dropVariable = (node: VariableNode, centre: Point): void => {
        if (paused || !page || !catalogue) return;
        const size = canvasScreen.size;
        void dropped(channel.call, catalogue.components, node).then((made) => {
            if ("refused" in made) {
                setNotice(made.refused);
                return;
            }
            const component = catalogue.components.find((candidate) => candidate.type === made.type);
            const { page: current, commit: apply } = latest.current;
            if (!component || !current) return;
            const corner = freeCorner(current, cornerFor(component.size, centre, size), component.size, size);
            const added = addObject(current, component, corner);
            apply(updateObject(added.page, added.id, { bind: made.bind, label: made.label }));
            setSelection([added.id]);
        }, showError);
    };

    useEffect(() => {
        if (!notice) return;
        const timer = setTimeout(() => setNotice(undefined), NOTICE_MS);
        return () => clearTimeout(timer);
    }, [notice]);

    /** Where a release lands on the page, in design units, or undefined off the page. */
    const pagePoint = (event: MouseEvent): Point | undefined => {
        const sheet = canvasRef.current?.querySelector('[data-part="canvas-page"]')?.getBoundingClientRect();
        if (!sheet || sheet.width === 0) return undefined;
        const inside =
            event.clientX >= sheet.left &&
            event.clientX <= sheet.right &&
            event.clientY >= sheet.top &&
            event.clientY <= sheet.bottom;
        return inside ? [(event.clientX - sheet.left) / zoom, (event.clientY - sheet.top) / zoom] : undefined;
    };

    /** Follows a press until its release, keeping `state.moved` up to date; the listeners are added
     * in the press, so a quick release is never missed. */
    const follow = (
        event: ReactMouseEvent,
        text: string,
        onRelease: (release: MouseEvent, moved: boolean) => void,
        state = { moved: false },
    ): void => {
        const start: Point = [event.clientX, event.clientY];
        const move = (next: MouseEvent): void => {
            state.moved = state.moved || Math.hypot(next.clientX - start[0], next.clientY - start[1]) > 4;
            if (state.moved) setGhost({ text, at: [next.clientX, next.clientY] });
        };
        const up = (next: MouseEvent): void => {
            window.removeEventListener("mousemove", move);
            window.removeEventListener("mouseup", up);
            setGhost(undefined);
            onRelease(next, state.moved);
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
    };

    /** An editing action of a key or of the context menu, on the selection (§3.3 item 8). */
    const act = (action: EditAction): void => {
        if (!page || paused) return;
        const objects = selection.filter((id) => pipeIndex(id) === undefined);
        if (action === "paste") {
            api.postMessage({ type: "paste" });
            return;
        }
        if (action === "copy" || action === "cut") {
            const cut = action === "cut" ? selection : undefined;
            void copyText(channel.call, page, objects).then((text) => {
                if (!text) return;
                api.postMessage({ type: "copy", text });
                const { page: current, commit: apply } = latest.current;
                if (!cut || !current) return;
                apply(deleteObjects(deletePipes(current, cut), cut));
                setSelection([]);
            }, showError);
            return;
        }
        if (action === "duplicate") {
            // A duplicate of the last duplicate's copies repeats the step they were moved by.
            const last = lastDuplicate.current;
            const source =
                last && last.copies.length === objects.length && last.copies.every((id) => objects.includes(id))
                    ? page.object.find((object) => object.id === last.sources[0])
                    : undefined;
            const copy = source ? page.object.find((object) => object.id === last?.copies[0]) : undefined;
            const step: Point = source && copy ? [copy.at[0] - source.at[0], copy.at[1] - source.at[1]] : PASTE_STEP;
            const known = variables ?? [];
            changePage((current) =>
                duplicated(channel.call, current, objects, step, known).then((result) => {
                    if (!result) return undefined;
                    lastDuplicate.current = { sources: objects, copies: result.ids };
                    return { page: result.page, select: result.ids };
                }),
            );
            return;
        }
        if (objects.length === 0) return;
        if (action === "delete") {
            commit(deleteObjects(deletePipes(page, selection), selection));
            setSelection([]);
        } else if (action === "group" || action === "ungroup") {
            commit(action === "group" ? groupObjects(page, objects) : ungroupObjects(page, objects));
        } else if (action === "forward" || action === "backward" || action === "front" || action === "back") {
            commit(reorder(page, objects, action));
        } else if (action.startsWith("align-")) {
            commit(alignObjects(page, objects, action.slice("align-".length) as Parameters<typeof alignObjects>[2]));
        } else if (action.startsWith("distribute-")) {
            commit(
                distributeObjects(
                    page,
                    objects,
                    action.slice("distribute-".length) as Parameters<typeof distributeObjects>[2],
                ),
            );
        } else if (action === "lock" || action === "unlock") {
            commit(objects.reduce((next, id) => withLock(next, id, action === "lock"), page));
        } else if (action === "faceplate") {
            const symbol = page.object.find((object) => object.id === objects[0]);
            const faceplate = symbol ? panelOf(symbol) : undefined;
            if (faceplate) api.postMessage({ type: "openFaceplate", name: faceplate });
        } else if (action === "place-faceplate") {
            // The symbol's panel stands beside it, for the symbol's instance (§8.9 item 1).
            const symbol = page.object.find((object) => object.id === objects[0]);
            const faceplate = symbol ? panelOf(symbol) : undefined;
            const instance = symbol ? symbolInstance(symbol) : undefined;
            if (!symbol || !faceplate || !instance) return;
            const added = addPlaced(
                page,
                faceplate,
                instance,
                besideSymbol(symbol),
                placedSize(faceplates.find((entry) => entry.name === faceplate)),
            );
            commit(added.page);
            setSelection([added.id]);
        }
    };
    const actRef = useRef(act);
    actRef.current = act;
    /** The current values of the variables the pickers show, read by the host (§3.3 item 8). */
    const pickerPaths = useRef(new Map<string, string[]>());
    const watching = useRef("[]");
    const showInPicker = useCallback(
        (tree: string, paths: string[]): void => {
            if (paths.length > 0) pickerPaths.current.set(tree, paths);
            else pickerPaths.current.delete(tree);
            // At most what one `hmi.read` takes (§11, item 6).
            const all = [...new Set([...pickerPaths.current.values()].flat())].slice(0, 1000);
            const text = JSON.stringify(all);
            if (text === watching.current) return;
            watching.current = text;
            api.postMessage({ type: "watch", paths: all });
        },
        [api],
    );
    const pickerValues = useMemo(() => ({ update: pickerUpdate, show: showInPicker }), [pickerUpdate, showInPicker]);
    /** Where the context menu is open, in the window's coordinates (§3.3 item 8). */
    const [menuAt, setMenuAt] = useState<[number, number]>();
    const closeMenu = useCallback(() => setMenuAt(undefined), []);

    useCanvasShortcuts({
        page,
        paused,
        selection,
        hiddenObjects,
        commit,
        setSelection,
        setZoom,
        fit,
        zoomToSelection,
        actRef,
    });

    /** Drags a component from the panel; dropped on the page, it is added there. */
    const startComponentDrag = (component: ComponentDefinition, event: ReactMouseEvent): void => {
        if (event.button !== 0) return;
        follow(event, componentName(component.type), (release, moved) => {
            const point = moved ? pagePoint(release) : undefined;
            if (point) add(component, point);
        });
    };

    const startFaceplateDrag = (faceplate: FaceplateEntry, event: ReactMouseEvent): void => {
        if (event.button !== 0) return;
        follow(event, faceplate.title ?? faceplate.name, (release, moved) => {
            const point = moved ? pagePoint(release) : undefined;
            if (point) placeFaceplate(faceplate, point);
        });
    };

    /** Drags a variable from the Tags panel: onto the page it makes a component (§7.1); a binding in
     * the inspector takes it itself and marks the drag handled. */
    const startVariableDrag = (node: VariableNode, event: ReactMouseEvent): void => {
        if (event.button !== 0) return;
        const drag: VariableDrag = { node, moved: false, handled: false };
        variableDrag.current = drag;
        follow(
            event,
            node.path,
            (release, moved) => {
                variableDrag.current = undefined;
                const point = moved && !drag.handled ? pagePoint(release) : undefined;
                if (point) dropVariable(node, point);
            },
            drag,
        );
    };

    return (
        <Rust.Provider value={rust}>
            <PickerValues.Provider value={pickerValues}>
                <div className="hb-app">
                    <style>{tokensCss(screen.colors ?? {}, fontUrl)}</style>
                    <TopBar
                        title={page?.title}
                        zoom={zoom}
                        onZoom={setZoom}
                        onFit={fit}
                        onOpenAsText={() => api.postMessage({ type: "openAsText" })}
                        mode={mode}
                        operating={operating}
                        live={live}
                        onMode={switchMode}
                        deploying={deploying}
                        onDeploy={() => api.postMessage({ type: "deploy" })}
                        onCreatePlcCode={() => api.postMessage({ type: "createPlcCode" })}
                        onCreateHmi={() => api.postMessage({ type: "createHmi" })}
                        languages={projectTexts?.languages}
                        language={table?.tag}
                        onLanguage={(tag) => {
                            setLanguage(tag);
                            setTranslating(undefined);
                        }}
                        shipped={projectTexts?.shipped}
                        onLanguages={(languages) => api.postMessage({ type: "screenLanguages", languages })}
                        longest={longest}
                        onLongest={() => setLongest((on) => !on)}
                        screenSize={page && !page.size && !pidView ? screenSize : undefined}
                        pidView={pidView}
                        onScreenSize={setScreenSize}
                    />
                    {deployed ? (
                        <DeployNotice
                            action={deployed.action}
                            outcome={deployed.outcome}
                            onRollback={() => api.postMessage({ type: "rollback" })}
                            onClose={() => setDeployed(undefined)}
                        />
                    ) : null}
                    {menuAt && page ? (
                        <ContextMenu
                            at={menuAt}
                            entries={menuEntries(page, selection)}
                            onAction={act}
                            onClose={closeMenu}
                        />
                    ) : null}
                    {errors ? (
                        <Banner
                            heading="This page does not load. Editing is paused until the text loads again."
                            errors={errors}
                        />
                    ) : null}
                    {refused ? <Banner heading="The change could not be written." errors={refused} /> : null}
                    {dependencyPick.notice}
                    <div
                        className="hb-body"
                        data-operating={operating}
                        style={{ gridTemplateColumns: `${shown.left}px minmax(0, 1fr) ${shown.right}px` }}
                    >
                        {mode === "live" && pages?.current ? (
                            <LiveOperating
                                api={api}
                                page={pages.current}
                                fontUrl={fontUrl}
                                assetBase={assetBase}
                                onActive={setOperating}
                            />
                        ) : null}
                        <div className="hb-side hb-side-left">
                            {shown.leftCollapsed ? (
                                <PanelStrip title="Components" panel="components" onOpen={() => open("left")} />
                            ) : (
                                <>
                                    <div className="hb-left">
                                        <div className="hb-left-tabs" role="tablist">
                                            {(["components", "layers"] as const).map((tab) => (
                                                <button
                                                    key={tab}
                                                    type="button"
                                                    role="tab"
                                                    data-left-tab={tab}
                                                    aria-selected={leftTab === tab}
                                                    onClick={() => setLeftTab(tab)}
                                                >
                                                    {tab === "components" ? "Components" : "Objects"}
                                                </button>
                                            ))}
                                            <button
                                                type="button"
                                                className="hb-collapse"
                                                data-action="collapse-components"
                                                title="Hide the side panel"
                                                aria-label="Hide the side panel"
                                                onClick={() => collapse("left")}
                                            >
                                                ‹
                                            </button>
                                        </div>
                                        {leftTab === "layers" && page ? (
                                            <LayersPanel
                                                page={page}
                                                selection={selection}
                                                hidden={hiddenObjects}
                                                readOnly={paused}
                                                onSelect={setSelection}
                                                onHide={(id) =>
                                                    setHiddenObjects((current) =>
                                                        current.includes(id)
                                                            ? current.filter((hidden) => hidden !== id)
                                                            : [...current, id],
                                                    )
                                                }
                                                onCommit={commit}
                                            />
                                        ) : (
                                            <ComponentsPanel
                                                palette={palette}
                                                theme={screen.theme}
                                                onAdd={(component) => add(component, visibleCentre())}
                                                onDragStart={startComponentDrag}
                                                faceplates={pidView ? [] : faceplates}
                                                onPlaceFaceplate={(faceplate) =>
                                                    placeFaceplate(faceplate, visibleCentre())
                                                }
                                                onFaceplateDragStart={startFaceplateDrag}
                                            />
                                        )}
                                    </div>
                                    <div
                                        className="hb-resize hb-resize-x"
                                        data-resize="left"
                                        title="Drag to resize"
                                        onMouseDown={(event) => dragEdge("left", event)}
                                    />
                                </>
                            )}
                        </div>
                        {page ? (
                            <Canvas
                                page={preview?.page ?? page}
                                composition={preview ? undefined : composition}
                                currentPage={pages?.current}
                                sharedSelection={sharedSelection}
                                onSharedSelect={(name) => {
                                    setSelection([]);
                                    setSharedSelection(name);
                                }}
                                screen={canvasScreen}
                                catalogue={catalogue}
                                formats={formats}
                                selection={preview ? [] : selection}
                                zoom={zoom}
                                assetBase={assetBase}
                                paused={paused || preview !== undefined}
                                pid={pidView}
                                problems={problems}
                                warnings={warningTexts}
                                live={mode === "live" ? liveStore.current?.frame(Date.now()) : undefined}
                                previews={previews}
                                speech={speech}
                                onObjectClick={objectClicked}
                                dependencyPick={dependencyPick.choose}
                                dependencyIds={dependencyPick.ids}
                                translation={translation}
                                onTranslate={(value) => {
                                    if (translation && table) translate(table.tag, translation.text, value);
                                    setTranslating(undefined);
                                }}
                                onTranslateCancel={() => setTranslating(undefined)}
                                onSelect={(ids) => {
                                    setSharedSelection(undefined);
                                    setSelection(ids);
                                }}
                                onCommit={commit}
                                canvasRef={canvasRef}
                                hidden={hiddenObjects}
                                onContextMenu={(at) => setMenuAt(at)}
                                placed={placed}
                            />
                        ) : (
                            <div className="hb-canvas hb-loading" ref={canvasRef}>
                                {errors ? null : "Loading the page…"}
                            </div>
                        )}
                        <div className="hb-side hb-side-right">
                            {shown.rightCollapsed ? (
                                <PanelStrip title="Inspector" panel="inspector" onOpen={() => open("right")} />
                            ) : (
                                <>
                                    {page ? (
                                        sharedSelection ? (
                                            <aside className="hb-panel hb-inspector">
                                                <h2>Shared panel</h2>
                                                <p>{sharedSelection}</p>
                                                <p className="hb-hint">
                                                    Edit this panel once to update every page that uses it.
                                                </p>
                                                <button
                                                    type="button"
                                                    onClick={() =>
                                                        api.postMessage({
                                                            type: "openSharedPanel",
                                                            name: sharedSelection,
                                                        })
                                                    }
                                                >
                                                    Edit shared panel
                                                </button>
                                            </aside>
                                        ) : (
                                            <Inspector
                                                page={page}
                                                selection={selection}
                                                catalogue={catalogue}
                                                definitions={shownEquipment}
                                                paused={paused}
                                                variables={variables}
                                                variablesProblem={variablesProblem}
                                                variableDrag={variableDrag}
                                                targets={targets}
                                                warnings={selection.length === 1 ? warnings[selection[0]] : undefined}
                                                problems={selection.length === 1 ? problems[selection[0]] : undefined}
                                                points={selection.length === 1 ? points[selection[0]] : undefined}
                                                onAllowWriting={(point) =>
                                                    api.postMessage({ type: "allowWriting", point })
                                                }
                                                screen={screen}
                                                colourWarnings={colourWarnings}
                                                onColour={(theme, token, value) =>
                                                    api.postMessage({
                                                        type: "colour",
                                                        theme,
                                                        token,
                                                        ...(value !== undefined ? { value } : {}),
                                                    })
                                                }
                                                onCommit={commit}
                                                onSelect={setSelection}
                                                onNotice={setNotice}
                                                onCollapse={() => collapse("right")}
                                                writtenIn={
                                                    table && table.tag !== tables[0]?.tag ? tables[0]?.name : undefined
                                                }
                                                preview={
                                                    mode === "design"
                                                        ? {
                                                              states: previews,
                                                              onPreview: (id, state) =>
                                                                  setPreviews((current) =>
                                                                      withPreview(current, id, state),
                                                                  ),
                                                          }
                                                        : undefined
                                                }
                                                equipmentEditor={equipmentEditing}
                                                placing={{
                                                    studio: (id) => api.postMessage({ type: "openControlStudio", id }),
                                                    answer: fitting,
                                                    ask: askInstances,
                                                    open: (name) => api.postMessage({ type: "openFaceplate", name }),
                                                }}
                                            />
                                        )
                                    ) : (
                                        <aside className="hb-panel hb-inspector" />
                                    )}
                                    <div
                                        className="hb-resize hb-resize-x"
                                        data-resize="right"
                                        title="Drag to resize"
                                        onMouseDown={(event) => dragEdge("right", event)}
                                    />
                                </>
                            )}
                        </div>
                    </div>
                    {!operating ? (
                        <BottomPanel
                            variables={variables}
                            problem={variablesProblem}
                            open={arrangement.bottom.open}
                            height={shown.bottomHeight}
                            tab={bottomTab}
                            onTab={showTab}
                            onResizeStart={(event) => dragEdge("bottom", event)}
                            onDragStart={startVariableDrag}
                            texts={projectTexts}
                            onTranslate={translate}
                        />
                    ) : null}
                    {notice ? (
                        <div className="hb-notice" data-part="notice" role="status">
                            {notice}
                        </div>
                    ) : null}
                    {placing ? (
                        <InstanceChooser
                            title={placing.faceplate.title ?? placing.faceplate.name}
                            instances={fitting?.faceplate === placing.faceplate.name ? fitting.instances : undefined}
                            problem={fitting?.faceplate === placing.faceplate.name ? fitting.problem : undefined}
                            onChoose={placeFor}
                            onCancel={() => setPlacing(undefined)}
                        />
                    ) : null}
                    {ghost ? (
                        <div className="hb-ghost" style={{ left: ghost.at[0] + 12, top: ghost.at[1] + 12 }}>
                            {ghost.text}
                        </div>
                    ) : null}
                </div>
            </PickerValues.Provider>
        </Rust.Provider>
    );
}
