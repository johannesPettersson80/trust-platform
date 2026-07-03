import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Konva from "konva";
import { ElementPropertiesPanel } from "./ElementPropertiesPanel";
import {
  LADDER_TOOL_DRAG_MIME,
  LadderToolsPanel,
  type LadderToolId,
} from "./LadderToolsPanel";
import { getVsCodeApi } from "../../visual/runtime/webview/vscodeApi";
import { useRightPaneResize } from "../../visual/runtime/webview/useRightPaneResize";
import {
  connectorOffset,
  drawBranchMergeNode,
  drawBranchSplitNode,
  drawCoilNode,
  drawCompareNode,
  drawContactNode,
  drawCounterNode,
  drawJunctionNode,
  drawMathNode,
  drawTimerNode,
  type DrawNodeContext,
  type VariableDisplay,
} from "./nodeDrawing";
import {
  addParallelContactBranchLeg,
  autoRouteNetwork,
  autoRouteProgram,
  pasteElementIntoNetwork,
  pasteRungIntoProgram,
  reconcileContactCoilVariableDeclarations,
  replaceSymbolInProgram,
} from "./editorOps";
import type {
  Coil as CoilType,
  CompareNode,
  Contact as ContactType,
  Counter as CounterType,
  LadderElement,
  LadderProgram,
  Rung as RungType,
  Timer as TimerType,
} from "../ladderEngine.types";
import "../../visual/runtime/webview/rightPaneResize.css";
import { canvasColor, t, tint } from "../../webview/theme";

interface SelectedElement {
  rungIndex: number;
  elementIndex: number;
}

interface EdgeLinkSource {
  rungIndex: number;
  nodeId: string;
}

type LadderContextMenuTarget =
  | {
      kind: "element";
      rungIndex: number;
      elementIndex: number;
    }
  | {
      kind: "rung";
      rungIndex: number;
    };

interface LadderContextMenuState {
  x: number;
  y: number;
  target: LadderContextMenuTarget;
}

type LadderClipboard =
  | {
      kind: "element";
      element: LadderElement;
    }
  | {
      kind: "rung";
      rung: RungType;
    }
  | null;

const vscodeApi = getVsCodeApi();

const STAGE_WIDTH = 1200;
const STAGE_HEIGHT = 2000;
const RUNG_HEIGHT = 100;
const LEFT_RAIL_X = 50;
const RIGHT_RAIL_X = 1100;
const GRID_SIZE = 20;
const HISTORY_LIMIT = 100;
const k = canvasColor;
const kt = (color: string, alpha: number): string => canvasColor(tint(color, alpha));

type LadderInsertTool = LadderToolId;

interface RuntimeIoEntryPayload {
  name?: string;
  address?: string;
  value?: unknown;
}

interface RuntimeIoStatePayload {
  inputs?: RuntimeIoEntryPayload[];
  outputs?: RuntimeIoEntryPayload[];
  memory?: RuntimeIoEntryPayload[];
}

interface LadderRuntimeExecutionState {
  inputs: Record<string, boolean>;
  outputs: Record<string, boolean>;
  markers: Record<string, boolean>;
  variableBooleans: Record<string, boolean>;
}

function createEmptyProgram(): LadderProgram {
  return {
    schemaVersion: 2,
    networks: [],
    variables: [],
    metadata: {
      name: "New Ladder Program",
      description: "Ladder logic program",
    },
  };
}

function cloneProgram(program: LadderProgram): LadderProgram {
  if (typeof structuredClone === "function") {
    return structuredClone(program);
  }
  return JSON.parse(JSON.stringify(program)) as LadderProgram;
}

function hasEditableFocus(): boolean {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement)) {
    return false;
  }
  return ["INPUT", "TEXTAREA", "SELECT"].includes(active.tagName);
}

function sortByX(elements: LadderElement[]): LadderElement[] {
  return [...elements].sort((left, right) => {
    if (left.position.x !== right.position.x) {
      return left.position.x - right.position.x;
    }
    if (left.position.y !== right.position.y) {
      return left.position.y - right.position.y;
    }
    return left.id.localeCompare(right.id);
  });
}

function mergeIntervals(
  intervals: Array<{ start: number; end: number }>
): Array<{ start: number; end: number }> {
  if (intervals.length === 0) {
    return [];
  }
  const sorted = [...intervals].sort((left, right) => left.start - right.start);
  const merged: Array<{ start: number; end: number }> = [sorted[0]];
  for (let index = 1; index < sorted.length; index += 1) {
    const current = sorted[index];
    const previous = merged[merged.length - 1];
    if (current.start <= previous.end) {
      previous.end = Math.max(previous.end, current.end);
      continue;
    }
    merged.push({ ...current });
  }
  return merged;
}

function isLadderInsertTool(value: string): value is LadderInsertTool {
  return (
    value === "contact" ||
    value === "coil" ||
    value === "timer" ||
    value === "counter" ||
    value === "compare" ||
    value === "math" ||
    value === "branchSplit" ||
    value === "branchMerge" ||
    value === "junction"
  );
}

function buildOrthogonalEdgePoints(
  from: LadderElement,
  to: LadderElement
): Array<{ x: number; y: number }> {
  const fromX = from.position.x + connectorOffset(from.type, "right");
  const fromY = from.position.y;
  const toX = to.position.x + connectorOffset(to.type, "left");
  const toY = to.position.y;
  const midX = Math.round(((fromX + toX) / 2) / GRID_SIZE) * GRID_SIZE;

  return [
    { x: fromX, y: fromY },
    { x: midX, y: fromY },
    { x: midX, y: toY },
    { x: toX, y: toY },
  ];
}

function normalizeRuntimeKey(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function variableDisplayKey(value: unknown): string | null {
  const normalized = normalizeRuntimeKey(value);
  return normalized ? normalized.toUpperCase() : null;
}

function parseRuntimeBoolean(value: unknown): boolean | undefined {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "number") {
    return value !== 0;
  }
  if (typeof value !== "string") {
    return undefined;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }

  const normalized = trimmed.toUpperCase();
  const maybeWrapped =
    normalized.startsWith("BOOL(") && normalized.endsWith(")")
      ? normalized.slice(5, -1).trim()
      : normalized;

  if (maybeWrapped === "TRUE" || maybeWrapped === "1") {
    return true;
  }
  if (maybeWrapped === "FALSE" || maybeWrapped === "0") {
    return false;
  }
  return undefined;
}

function isRuntimeIoEntryPayload(value: unknown): value is RuntimeIoEntryPayload {
  return typeof value === "object" && value !== null;
}

function toRuntimeIoEntries(value: unknown): RuntimeIoEntryPayload[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter(isRuntimeIoEntryPayload);
}

function assignBooleanValue(
  record: Record<string, boolean>,
  key: unknown,
  value: boolean
): void {
  const normalized = normalizeRuntimeKey(key);
  if (!normalized) {
    return;
  }
  record[normalized] = value;
  const uppercase = normalized.toUpperCase();
  if (uppercase !== normalized) {
    record[uppercase] = value;
  }
}

function assignEntryBooleanValue(
  record: Record<string, boolean>,
  entry: RuntimeIoEntryPayload,
  value: boolean
): void {
  assignBooleanValue(record, entry.address, value);
  assignBooleanValue(record, entry.name, value);
}

function executionStateFromIoState(payload: unknown): LadderRuntimeExecutionState {
  const executionState: LadderRuntimeExecutionState = {
    inputs: {},
    outputs: {},
    markers: {},
    variableBooleans: {},
  };

  if (!payload || typeof payload !== "object") {
    return executionState;
  }

  const ioState = payload as RuntimeIoStatePayload;

  for (const entry of toRuntimeIoEntries(ioState.inputs)) {
    const value = parseRuntimeBoolean(entry.value);
    if (value === undefined) {
      continue;
    }
    assignEntryBooleanValue(executionState.inputs, entry, value);
    assignEntryBooleanValue(executionState.variableBooleans, entry, value);
  }

  for (const entry of toRuntimeIoEntries(ioState.outputs)) {
    const value = parseRuntimeBoolean(entry.value);
    if (value === undefined) {
      continue;
    }
    assignEntryBooleanValue(executionState.outputs, entry, value);
    assignEntryBooleanValue(executionState.variableBooleans, entry, value);
  }

  for (const entry of toRuntimeIoEntries(ioState.memory)) {
    const value = parseRuntimeBoolean(entry.value);
    if (value === undefined) {
      continue;
    }
    assignEntryBooleanValue(executionState.variableBooleans, entry, value);

    const address = normalizeRuntimeKey(entry.address)?.toUpperCase();
    if (address?.startsWith("%MX")) {
      assignEntryBooleanValue(executionState.markers, entry, value);
    }
  }

  return executionState;
}

export function LadderEditor() {
  const [bootError, setBootError] = useState<string | null>(null);
  const [program, setProgram] = useState<LadderProgram>(createEmptyProgram);
  const [selectedTool, setSelectedTool] = useState<LadderInsertTool | null>(
    null
  );
  const [executionState, setExecutionState] = useState<any>(null);
  const [scale, setScale] = useState(1);
  const [selectedElement, setSelectedElement] = useState<SelectedElement | null>(
    null
  );
  const [activeRungIndex, setActiveRungIndex] = useState<number | null>(null);
  const [linkModeEnabled, setLinkModeEnabled] = useState(false);
  const [edgeLinkSource, setEdgeLinkSource] = useState<EdgeLinkSource | null>(
    null
  );
  const [linkPreviewPoint, setLinkPreviewPoint] = useState<{
    x: number;
    y: number;
  } | null>(null);
  const [linkFeedback, setLinkFeedback] = useState<string | null>(null);
  const [isPanModifierActive, setIsPanModifierActive] = useState(false);
  const [isPanning, setIsPanning] = useState(false);
  const [isToolDragActive, setIsToolDragActive] = useState(false);
  const [hoverRungIndex, setHoverRungIndex] = useState<number | null>(null);
  const [undoDepth, setUndoDepth] = useState(0);
  const [redoDepth, setRedoDepth] = useState(0);
  const [hasClipboard, setHasClipboard] = useState(false);
  const [contextMenu, setContextMenu] = useState<LadderContextMenuState | null>(
    null
  );
  const {
    rightPaneStyle,
    resizeHandleClassName,
    resizeHandleProps,
  } = useRightPaneResize("ladder");

  const containerRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<Konva.Stage | null>(null);
  const layerRef = useRef<Konva.Layer | null>(null);
  const spacePressedRef = useRef(false);
  const undoStackRef = useRef<LadderProgram[]>([]);
  const redoStackRef = useRef<LadderProgram[]>([]);
  const clipboardRef = useRef<LadderClipboard>(null);
  const runtimeIsExecutingRef = useRef(false);
  const edgeLinkSourceRef = useRef<EdgeLinkSource | null>(null);

  const variableDisplayByReference = useMemo(() => {
    const lookup = new Map<string, VariableDisplay>();

    const register = (reference: unknown, display: VariableDisplay) => {
      const key = variableDisplayKey(reference);
      if (key) {
        lookup.set(key, display);
      }
    };

    for (const variable of program.variables) {
      const name = variable.name?.trim();
      const address = variable.address?.trim();
      if (!name && !address) {
        continue;
      }

      const primary = name || address || "";
      const secondary =
        address && variableDisplayKey(address) !== variableDisplayKey(primary)
          ? address
          : undefined;
      const display = { primary, secondary };

      register(name, display);
      register(address, display);
    }

    return lookup;
  }, [program.variables]);

  const resolveVariableDisplay = useCallback((reference: string): VariableDisplay => {
    const key = variableDisplayKey(reference);
    return (
      (key ? variableDisplayByReference.get(key) : undefined) ?? {
        primary: reference.trim() || "Unassigned",
      }
    );
  }, [variableDisplayByReference]);

  const syncHistoryState = () => {
    setUndoDepth(undoStackRef.current.length);
    setRedoDepth(redoStackRef.current.length);
  };

  const resetHistory = () => {
    undoStackRef.current = [];
    redoStackRef.current = [];
    syncHistoryState();
  };

  const setClipboard = (value: LadderClipboard) => {
    clipboardRef.current = value;
    setHasClipboard(value !== null);
  };

  const setEdgeLinkSourceState = (value: EdgeLinkSource | null) => {
    edgeLinkSourceRef.current = value;
    setEdgeLinkSource(value);
    if (!value) {
      setLinkPreviewPoint(null);
    }
  };

  const applyProgramChange = (
    updater: (previous: LadderProgram) => LadderProgram,
    trackHistory = true
  ) => {
    setProgram((previous) => {
      const next = updater(previous);
      if (next === previous) {
        return previous;
      }

      if (trackHistory) {
        undoStackRef.current.push(cloneProgram(previous));
        if (undoStackRef.current.length > HISTORY_LIMIT) {
          undoStackRef.current.shift();
        }
        redoStackRef.current = [];
        syncHistoryState();
      }

      return next;
    });
  };

  const undo = () => {
    const previous = undoStackRef.current.pop();
    if (!previous) {
      return;
    }

    setProgram((current) => {
      redoStackRef.current.push(cloneProgram(current));
      return previous;
    });
    setSelectedElement(null);
    syncHistoryState();
  };

  const redo = () => {
    const next = redoStackRef.current.pop();
    if (!next) {
      return;
    }

    setProgram((current) => {
      undoStackRef.current.push(cloneProgram(current));
      return next;
    });
    setSelectedElement(null);
    syncHistoryState();
  };

  useEffect(() => {
    if (!containerRef.current) {
      return;
    }

    // Size the stage to the visible pane and fit the rung width on open, so the output coils on
    // the right rail aren't clipped off the edge (the fixed 1200px stage was wider than the panel).
    const containerWidth = containerRef.current.clientWidth || STAGE_WIDTH;
    const fitScale = Math.max(
      0.3,
      Math.min(1, (containerWidth - 24) / (RIGHT_RAIL_X + 40))
    );

    let stage: Konva.Stage;
    try {
      stage = new Konva.Stage({
        container: containerRef.current,
        width: containerWidth,
        height: STAGE_HEIGHT,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setBootError(message);
      vscodeApi?.postMessage({
        type: "webviewBootError",
        message,
      });
      return;
    }

    const layer = new Konva.Layer();
    stage.add(layer);

    stage.on("wheel", (event) => {
      event.evt.preventDefault();

      const oldScale = stage.scaleX();
      const pointer = stage.getPointerPosition();
      if (!pointer) {
        return;
      }

      const mousePoint = {
        x: (pointer.x - stage.x()) / oldScale,
        y: (pointer.y - stage.y()) / oldScale,
      };

      const direction = event.evt.deltaY > 0 ? -1 : 1;
      const scaleBy = 1.1;
      const newScale = direction > 0 ? oldScale * scaleBy : oldScale / scaleBy;
      const boundedScale = Math.max(0.3, Math.min(3, newScale));

      stage.scale({ x: boundedScale, y: boundedScale });
      setScale(boundedScale);

      stage.position({
        x: pointer.x - mousePoint.x * boundedScale,
        y: pointer.y - mousePoint.y * boundedScale,
      });
    });

    const startPan = (event: Konva.KonvaEventObject<MouseEvent | TouchEvent>) => {
      const mouseEvent = event.evt;
      const middleMouse =
        mouseEvent instanceof MouseEvent && mouseEvent.button === 1;
      const shouldPan = middleMouse || spacePressedRef.current;

      if (!shouldPan) {
        stage.draggable(false);
        return;
      }

      stage.draggable(true);
      setIsPanning(true);
      event.cancelBubble = true;
    };

    const stopPan = () => {
      if (stage.draggable()) {
        stage.draggable(false);
      }
      setIsPanning(false);
    };

    stage.on("mousedown touchstart", startPan);
    stage.on("mouseup mouseleave touchend dragend", stopPan);

    stageRef.current = stage;
    layerRef.current = layer;

    // Apply the initial fit so the whole rung (contacts + coils + right rail) shows on open.
    if (fitScale < 1) {
      stage.scale({ x: fitScale, y: fitScale });
      stage.position({ x: 12, y: 0 });
      setScale(fitScale);
    }

    return () => {
      stage.off("mousedown touchstart", startPan);
      stage.off("mouseup mouseleave touchend dragend", stopPan);
      stage.destroy();
      stageRef.current = null;
      layerRef.current = null;
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.code !== "Space" || event.repeat || hasEditableFocus()) {
        return;
      }
      spacePressedRef.current = true;
      setIsPanModifierActive(true);
      event.preventDefault();
    };

    const onKeyUp = (event: KeyboardEvent) => {
      if (event.code !== "Space") {
        return;
      }
      spacePressedRef.current = false;
      setIsPanModifierActive(false);
      if (!isPanning) {
        setIsPanning(false);
      }
      event.preventDefault();
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);

    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [isPanning]);

  const addElementWithTool = (
    tool: LadderInsertTool,
    rungIndex: number,
    x: number,
    y: number
  ): number => {
    let insertedIndex = -1;

    applyProgramChange((previous) => {
      const networks = [...previous.networks];
      const network = networks[rungIndex];
      if (!network) {
        return previous;
      }

      const elementId = `${tool}_${Date.now()}`;
      let element: LadderElement | null = null;
      let nextVariables = [...previous.variables];

      const ensureVariable = (
        name: string,
        type: LadderProgram["variables"][number]["type"],
        scope: LadderProgram["variables"][number]["scope"] = "local",
        initialValue?: unknown
      ) => {
        const key = name.trim().toUpperCase();
        const exists = nextVariables.some(
          (candidate) =>
            candidate.name.trim().toUpperCase() === key &&
            (candidate.scope ?? "global") === (scope ?? "global")
        );
        if (exists) {
          return;
        }
        nextVariables = [
          ...nextVariables,
          {
            name,
            type,
            scope,
            initialValue,
          },
        ];
      };

      if (tool === "contact") {
        element = {
          id: elementId,
          type: "contact",
          contactType: "NO",
          variable: "%IX0.0",
          position: { x, y },
        } as ContactType;
      }

      if (tool === "coil") {
        element = {
          id: elementId,
          type: "coil",
          coilType: "NORMAL",
          variable: "%QX0.0",
          position: { x, y },
        } as CoilType;
      }

      if (tool === "timer") {
        const instance = `T_${Date.now()}`;
        const qOutput = `${instance}_Q`;
        const etOutput = `${instance}_ET`;
        element = {
          id: elementId,
          type: "timer",
          timerType: "TON",
          instance,
          input: "",
          presetMs: 1000,
          qOutput,
          etOutput,
          position: { x, y },
        } as TimerType;
        ensureVariable(qOutput, "BOOL", "local", false);
        ensureVariable(etOutput, "TIME", "local", 0);
      }

      if (tool === "counter") {
        const instance = `C_${Date.now()}`;
        const qOutput = `${instance}_Q`;
        const cvOutput = `${instance}_CV`;
        element = {
          id: elementId,
          type: "counter",
          counterType: "CTU",
          instance,
          input: "",
          preset: 10,
          qOutput,
          cvOutput,
          position: { x, y },
        } as CounterType;
        ensureVariable(qOutput, "BOOL", "local", false);
        ensureVariable(cvOutput, "INT", "local", 0);
      }

      if (tool === "compare") {
        const instance = `CMP_${Date.now()}`;
        const in1 = `${instance}_IN1`;
        const in2 = `${instance}_IN2`;
        element = {
          id: elementId,
          type: "compare",
          op: "GT",
          left: in1,
          right: in2,
          position: { x, y },
        } as CompareNode;
        ensureVariable(in1, "INT", "local", 0);
        ensureVariable(in2, "INT", "local", 0);
      }

      if (tool === "math") {
        const instance = `MATH_${Date.now()}`;
        const in1 = `${instance}_IN1`;
        const in2 = `${instance}_IN2`;
        const out = `${instance}_OUT`;
        element = {
          id: elementId,
          type: "math",
          op: "ADD",
          left: in1,
          right: in2,
          output: out,
          position: { x, y },
        } as MathNode;
        ensureVariable(in1, "INT", "local", 0);
        ensureVariable(in2, "INT", "local", 0);
        ensureVariable(out, "INT", "local", 0);
      }

      if (tool === "branchSplit") {
        element = {
          id: elementId,
          type: "branchSplit",
          position: { x, y },
        } as LadderElement;
      }

      if (tool === "branchMerge") {
        element = {
          id: elementId,
          type: "branchMerge",
          position: { x, y },
        } as LadderElement;
      }

      if (tool === "junction") {
        element = {
          id: elementId,
          type: "junction",
          position: { x, y },
        } as LadderElement;
      }

      if (!element) {
        return previous;
      }

      const nextNodes = [...network.nodes, element];
      insertedIndex = nextNodes.length - 1;
      const nextNetwork = {
        ...network,
        nodes: nextNodes,
      };

      networks[rungIndex] = nextNetwork;

      return {
        ...previous,
        networks,
        variables: nextVariables,
      };
    });

    return insertedIndex;
  };

  const addRungAt = (insertIndex: number) => {
    const nextIndex = Math.max(0, Math.min(insertIndex, program.networks.length));
    const rung: RungType = {
      id: `rung_${Date.now()}`,
      order: nextIndex,
      nodes: [],
      edges: [],
      layout: {
        y: nextIndex * RUNG_HEIGHT + 100,
      },
    };

    applyProgramChange((previous) => {
      const networks = [...previous.networks];
      networks.splice(nextIndex, 0, rung);
      return {
        ...previous,
        networks: networks.map((candidate, index) => ({
          ...candidate,
          order: index,
          layout: {
            ...candidate.layout,
            y: index * RUNG_HEIGHT + 100,
          },
        })),
      };
    });
    setActiveRungIndex(nextIndex);
    setSelectedElement(null);
  };

  const addRung = () => {
    addRungAt(program.networks.length);
  };

  const removeRungAt = (targetIndex: number) => {
    if (program.networks.length === 0) {
      return;
    }
    if (targetIndex < 0 || targetIndex >= program.networks.length) {
      return;
    }

    const nextRungs = program.networks
      .filter((_, index) => index !== targetIndex)
      .map((rung, index) => ({
        ...rung,
        order: index,
        layout: {
          ...rung.layout,
          y: index * RUNG_HEIGHT + 100,
        },
      }));

    applyProgramChange((previous) => ({
      ...previous,
      networks: nextRungs,
    }));

    if (selectedElement?.rungIndex === targetIndex) {
      setSelectedElement(null);
    } else if (selectedElement && selectedElement.rungIndex > targetIndex) {
      setSelectedElement({
        rungIndex: selectedElement.rungIndex - 1,
        elementIndex: selectedElement.elementIndex,
      });
    }

    if (nextRungs.length === 0) {
      setActiveRungIndex(null);
    } else if (targetIndex >= nextRungs.length) {
      setActiveRungIndex(nextRungs.length - 1);
    } else {
      setActiveRungIndex(targetIndex);
    }
  };

  const removeRung = () => {
    if (program.networks.length === 0) {
      return;
    }

    const targetIndex =
      activeRungIndex !== null && activeRungIndex < program.networks.length
        ? activeRungIndex
        : program.networks.length - 1;
    removeRungAt(targetIndex);
  };

  const clearActiveRungWiring = () => {
    if (activeRungIndex === null) {
      return;
    }
    applyProgramChange((previous) => {
      const networks = [...previous.networks];
      const rung = networks[activeRungIndex];
      if (!rung || rung.edges.length === 0) {
        return previous;
      }
      networks[activeRungIndex] = {
        ...rung,
        edges: [],
      };
      return {
        ...previous,
        networks,
      };
    });
    setEdgeLinkSourceState(null);
  };

  const addParallelContactFromSelection = (
    targetSelection: SelectedElement | null = selectedElement
  ) => {
    if (!targetSelection) {
      setLinkFeedback("Select a contact first.");
      return;
    }

    const { rungIndex, elementIndex } = targetSelection;
    const selectedNode = program.networks[rungIndex]?.nodes[elementIndex];
    if (!selectedNode || selectedNode.type !== "contact") {
      setLinkFeedback("Select a contact first.");
      return;
    }

    let failureMessage: string | null = null;
    let created = false;
    let nextSelection: SelectedElement | null = null;
    let totalParallelLegs = 0;
    let rungShiftAppliedPx = 0;
    applyProgramChange((previous) => {
      const rung = previous.networks[rungIndex];
      if (!rung) {
        failureMessage = "Selected rung no longer exists.";
        return previous;
      }

      const result = addParallelContactBranchLeg(previous, rungIndex, selectedNode.id);
      if (!result.ok) {
        if (result.error === "unsupported-topology") {
          failureMessage = "This contact is not part of a branchable path.";
        } else if (result.error === "insufficient-horizontal-space") {
          failureMessage =
            "Not enough horizontal space in this rung; move the contact right and retry.";
        } else if (result.error === "invalid-topology") {
          failureMessage =
            "Cannot add another parallel contact on this topology. Use Clear Wiring and retry.";
        } else {
          failureMessage =
            result.error === "rung-not-found"
              ? "Selected rung no longer exists."
              : "Selected contact is no longer valid.";
        }
        return previous;
      }

      created = true;
      totalParallelLegs = result.totalParallelLegs;
      rungShiftAppliedPx = result.rungShiftAppliedPx;
      const selectedIndex = result.program.networks[rungIndex]?.nodes.findIndex(
        (node) => node.id === result.selectedNodeId
      );
      if (selectedIndex !== undefined && selectedIndex >= 0) {
        nextSelection = { rungIndex, elementIndex: selectedIndex };
      }
      return {
        ...result.program,
      };
    });

    if (!created) {
      setLinkFeedback(failureMessage ?? "Unable to create parallel branch.");
      return;
    }

    setSelectedTool(null);
    setLinkModeEnabled(false);
    setEdgeLinkSourceState(null);
    setSelectedElement(nextSelection);
    setActiveRungIndex(rungIndex);
    if (rungShiftAppliedPx > 0) {
      setLinkFeedback(
        `Parallel branch now has ${totalParallelLegs} legs. Lower rungs shifted by ${rungShiftAppliedPx}px.`
      );
    } else {
      setLinkFeedback(`Parallel branch now has ${totalParallelLegs} legs.`);
    }
  };

  const removeElementAt = (rungIndex: number, elementIndex: number) => {
    applyProgramChange((previous) => {
      const networks = [...previous.networks];
      const rung = networks[rungIndex];
      if (!rung || !rung.nodes[elementIndex]) {
        return previous;
      }

      const nextNetwork = {
        ...rung,
        nodes: rung.nodes.filter((_, index) => index !== elementIndex),
      };
      networks[rungIndex] = nextNetwork;

      return {
        ...previous,
        networks,
      };
    });
  };

  const removeSelectedElement = () => {
    if (!selectedElement) {
      return;
    }

    const { rungIndex, elementIndex } = selectedElement;
    removeElementAt(rungIndex, elementIndex);
    setSelectedElement(null);
  };

  const updateElementAt = (
    rungIndex: number,
    elementIndex: number,
    updater: (element: LadderElement) => LadderElement
  ) => {
    applyProgramChange((previous) => {
      const networks = [...previous.networks];
      const rung = networks[rungIndex];
      if (!rung || !rung.nodes[elementIndex]) {
        return previous;
      }

      const nodes = [...rung.nodes];
      const updatedElement = updater(nodes[elementIndex]);
      nodes[elementIndex] = updatedElement;
      const nextNetwork = {
        ...rung,
        nodes,
      };

      networks[rungIndex] = nextNetwork;

      const nextProgram: LadderProgram = {
        ...previous,
        networks,
        variables: previous.variables,
      };

      if (
        updatedElement &&
        (updatedElement.type === "contact" || updatedElement.type === "coil")
      ) {
        return reconcileContactCoilVariableDeclarations(nextProgram);
      }

      return nextProgram;
    });
  };

  const updateSelectedElement = (
    updater: (element: LadderElement) => LadderElement
  ) => {
    if (!selectedElement) {
      return;
    }

    const { rungIndex, elementIndex } = selectedElement;
    updateElementAt(rungIndex, elementIndex, updater);
  };

  const updateElementPosition = (
    rungIndex: number,
    elementIndex: number,
    x: number,
    y: number
  ) => {
    applyProgramChange((previous) => {
      const networks = [...previous.networks];
      const rung = networks[rungIndex];
      if (!rung || !rung.nodes[elementIndex]) {
        return previous;
      }

      const nodes = [...rung.nodes];
      nodes[elementIndex] = {
        ...nodes[elementIndex],
        position: { x, y },
      } as LadderElement;

      const nextNetwork = {
        ...rung,
        nodes,
      };

      networks[rungIndex] = nextNetwork;

      return {
        ...previous,
        networks,
      };
    });
  };

  const toggleLinkMode = () => {
    setLinkModeEnabled((enabled) => !enabled);
    setLinkFeedback(null);
    setEdgeLinkSourceState(null);
    setSelectedTool(null);
  };

  const addEdgeBetweenNodes = (
    rungIndex: number,
    fromNodeId: string,
    toNodeId: string
  ): void => {
    applyProgramChange((previous) => {
      const networks = [...previous.networks];
      const network = networks[rungIndex];
      if (!network || fromNodeId === toNodeId) {
        return previous;
      }

      const baseNetwork =
        network.edges.length > 0 ? network : autoRouteNetwork(network);
      const fromNode = baseNetwork.nodes.find((node) => node.id === fromNodeId);
      const toNode = baseNetwork.nodes.find((node) => node.id === toNodeId);
      if (!fromNode || !toNode) {
        return previous;
      }

      const exists = baseNetwork.edges.some(
        (edge) => edge.fromNodeId === fromNodeId && edge.toNodeId === toNodeId
      );
      if (exists) {
        return previous;
      }

      const edgeId = `edge_${baseNetwork.id}_${Date.now()}_${Math.floor(
        Math.random() * 1000
      )}`;

      const nextNetwork: RungType = {
        ...baseNetwork,
        edges: [
          ...baseNetwork.edges,
          {
            id: edgeId,
            fromNodeId,
            toNodeId,
            points: buildOrthogonalEdgePoints(fromNode, toNode),
          },
        ],
      };

      networks[rungIndex] = nextNetwork;
      return {
        ...previous,
        networks,
      };
    });
  };

  const connectNodesIfNeeded = (
    rungIndex: number,
    fromNodeId: string,
    toNodeId: string
  ): "added" | "already-connected" | "invalid" => {
    const network = program.networks[rungIndex];
    if (!network || fromNodeId === toNodeId) {
      return "invalid";
    }

    const baseNetwork =
      network.edges.length > 0 ? network : autoRouteNetwork(network);
    const fromNode = baseNetwork.nodes.find((node) => node.id === fromNodeId);
    const toNode = baseNetwork.nodes.find((node) => node.id === toNodeId);
    if (!fromNode || !toNode) {
      return "invalid";
    }

    const exists = baseNetwork.edges.some(
      (edge) => edge.fromNodeId === fromNodeId && edge.toNodeId === toNodeId
    );
    if (exists) {
      return "already-connected";
    }

    addEdgeBetweenNodes(rungIndex, fromNodeId, toNodeId);
    return "added";
  };

  const handleStartLink = (rungIndex: number, elementIndex: number) => {
    const node = program.networks[rungIndex]?.nodes[elementIndex];
    if (!node || !linkModeEnabled) {
      return;
    }
    const source = edgeLinkSourceRef.current;
    if (source && source.rungIndex === rungIndex) {
      if (source.nodeId === node.id) {
        setLinkFeedback("Wire source cleared.");
        setEdgeLinkSourceState(null);
        return;
      }
      const result = connectNodesIfNeeded(rungIndex, source.nodeId, node.id);
      if (result === "added") {
        setLinkFeedback("Wire connected.");
      } else if (result === "already-connected") {
        setLinkFeedback("Nodes are already connected.");
      } else {
        setLinkFeedback("Invalid wire target.");
      }
      setEdgeLinkSourceState(null);
      return;
    }
    setActiveRungIndex(rungIndex);
    setLinkFeedback(null);
    setEdgeLinkSourceState({
      rungIndex,
      nodeId: node.id,
    });
  };

  const copyElementAt = (rungIndex: number, elementIndex: number): boolean => {
    const rung = program.networks[rungIndex];
    const node = rung?.nodes[elementIndex];
    if (!node) {
      return false;
    }
    setClipboard({
      kind: "element",
      element: cloneProgram({
        ...createEmptyProgram(),
        networks: [
          {
            id: "tmp",
            order: 0,
            nodes: [node],
            edges: [],
            layout: { y: node.position.y },
          },
        ],
      }).networks[0].nodes[0],
    });
    return true;
  };

  const copyRungAt = (rungIndex: number): boolean => {
    const rung = program.networks[rungIndex];
    if (!rung) {
      return false;
    }
    setClipboard({
      kind: "rung",
      rung: cloneProgram({
        ...createEmptyProgram(),
        networks: [rung],
      }).networks[0],
    });
    return true;
  };

  const copySelection = () => {
    if (selectedElement) {
      if (copyElementAt(selectedElement.rungIndex, selectedElement.elementIndex)) {
        return;
      }
    }
    if (activeRungIndex !== null) {
      copyRungAt(activeRungIndex);
    }
  };

  const pasteIntoRung = (targetRungIndex: number) => {
    const clipboard = clipboardRef.current;
    if (!clipboard) {
      return;
    }

    if (clipboard.kind === "element") {
      if (targetRungIndex < 0) {
        return;
      }

      let insertedIndex = -1;
      applyProgramChange((previous) => {
        const networks = [...previous.networks];
        const rung = networks[targetRungIndex];
        if (!rung) {
          return previous;
        }

        const pasted = pasteElementIntoNetwork(rung, clipboard.element, rung.layout.y);
        insertedIndex = pasted.insertedIndex;
        networks[targetRungIndex] = pasted.network;

        return {
          ...previous,
          networks,
        };
      });

      if (insertedIndex >= 0) {
        setSelectedElement({
          rungIndex: targetRungIndex,
          elementIndex: insertedIndex,
        });
      }
      return;
    }

    const insertAfter =
      targetRungIndex >= 0 ? targetRungIndex : program.networks.length - 1;
    let insertedRung = -1;
    applyProgramChange((previous) => {
      const result = pasteRungIntoProgram(previous, clipboard.rung, insertAfter);
      insertedRung = result.insertedIndex;
      return result.program;
    });

    if (insertedRung >= 0) {
      setActiveRungIndex(insertedRung);
      setSelectedElement(null);
    }
  };

  const pasteSelection = () => {
    const targetRungIndex =
      activeRungIndex !== null ? activeRungIndex : program.networks.length - 1;
    pasteIntoRung(targetRungIndex);
  };

  const handleSearchReplace = () => {
    const findValue = window.prompt("Find symbol/address", "");
    if (!findValue) {
      return;
    }

    const replaceValue = window.prompt("Replace with", "") ?? "";
    const result = replaceSymbolInProgram(program, findValue, replaceValue);

    if (result.replacements === 0) {
      window.alert(`No matches for '${findValue}'.`);
      return;
    }

    applyProgramChange(() => result.program);
    window.alert(`Updated ${result.replacements} occurrence(s).`);
  };

  const openElementContextMenu = (
    rungIndex: number,
    elementIndex: number,
    clientX: number,
    clientY: number
  ) => {
    setSelectedElement({ rungIndex, elementIndex });
    setActiveRungIndex(rungIndex);
    setRightPaneView("tools");
    setContextMenu({
      x: clientX,
      y: clientY,
      target: {
        kind: "element",
        rungIndex,
        elementIndex,
      },
    });
  };

  const openRungContextMenu = (
    rungIndex: number,
    clientX: number,
    clientY: number
  ) => {
    setActiveRungIndex(rungIndex);
    setSelectedElement(null);
    setRightPaneView("tools");
    setContextMenu({
      x: clientX,
      y: clientY,
      target: {
        kind: "rung",
        rungIndex,
      },
    });
  };

  const executeElementContextAction = (action: string) => {
    if (!contextMenu || contextMenu.target.kind !== "element") {
      return;
    }
    const { rungIndex, elementIndex } = contextMenu.target;
    const node = program.networks[rungIndex]?.nodes[elementIndex];
    if (!node) {
      return;
    }
    if (action === "copy") {
      copyElementAt(rungIndex, elementIndex);
      return;
    }
    if (action === "cut") {
      copyElementAt(rungIndex, elementIndex);
      removeElementAt(rungIndex, elementIndex);
      setSelectedElement(null);
      return;
    }
    if (action === "paste") {
      pasteIntoRung(rungIndex);
      return;
    }
    if (action === "delete") {
      removeElementAt(rungIndex, elementIndex);
      setSelectedElement(null);
      return;
    }
    if (action === "parallel-contact") {
      setActiveRungIndex(rungIndex);
      addParallelContactFromSelection({ rungIndex, elementIndex });
      return;
    }
    if (action === "toggle-contact-type" && node.type === "contact") {
      updateElementAt(rungIndex, elementIndex, (element) => {
        const contact = element as ContactType;
        return {
          ...contact,
          contactType: contact.contactType === "NO" ? "NC" : "NO",
        };
      });
      return;
    }
    if (action.startsWith("set-coil-type:") && node.type === "coil") {
      const coilType = action.split(":")[1] as CoilType["coilType"];
      updateElementAt(rungIndex, elementIndex, (element) => ({
        ...(element as CoilType),
        coilType,
      }));
    }
  };

  const executeRungContextAction = (action: string) => {
    if (!contextMenu || contextMenu.target.kind !== "rung") {
      return;
    }
    const { rungIndex } = contextMenu.target;
    if (action === "add-rung-above") {
      addRungAt(rungIndex);
      return;
    }
    if (action === "add-rung-below") {
      addRungAt(rungIndex + 1);
      return;
    }
    if (action === "remove-rung") {
      removeRungAt(rungIndex);
      return;
    }
    if (action === "paste") {
      pasteIntoRung(rungIndex);
    }
  };

  const stagePointFromClient = (clientX: number, clientY: number) => {
    const stage = stageRef.current;
    if (!stage) {
      return null;
    }

    const bounds = stage.container().getBoundingClientRect();
    const transform = stage.getAbsoluteTransform().copy();
    transform.invert();

    return transform.point({
      x: clientX - bounds.left,
      y: clientY - bounds.top,
    });
  };

  const findRungIndexForY = (y: number): number => {
    if (program.networks.length === 0) {
      return -1;
    }

    let closestIndex = -1;
    let closestDistance = Number.POSITIVE_INFINITY;

    program.networks.forEach((rung, index) => {
      const distance = Math.abs(rung.layout.y - y);
      if (distance < closestDistance) {
        closestDistance = distance;
        closestIndex = index;
      }
    });

    return closestDistance <= RUNG_HEIGHT / 2 ? closestIndex : -1;
  };

  const placeToolOnRung = (
    tool: LadderInsertTool,
    rungIndex: number,
    x: number
  ): number => {
    const rung = program.networks[rungIndex];
    if (!rung) {
      return -1;
    }

    const leftOffset = connectorOffset(tool, "left");
    const rightOffset = connectorOffset(tool, "right");
    const minX = LEFT_RAIL_X - leftOffset;
    const maxX = RIGHT_RAIL_X - rightOffset;

    const snappedX = Math.max(
      minX,
      Math.min(
        maxX,
        Math.round(x / GRID_SIZE) * GRID_SIZE
      )
    );
    const insertedIndex = addElementWithTool(tool, rungIndex, snappedX, rung.layout.y);
    if (insertedIndex >= 0) {
      setSelectedElement({ rungIndex, elementIndex: insertedIndex });
      setActiveRungIndex(rungIndex);
    }

    return insertedIndex;
  };

  const resetToolDragState = () => {
    setIsToolDragActive(false);
    setHoverRungIndex(null);
  };

  const handleCanvasDragOver = (event: React.DragEvent<HTMLDivElement>) => {
    const toolId = event.dataTransfer.getData(LADDER_TOOL_DRAG_MIME);
    if (!isLadderInsertTool(toolId)) {
      return;
    }

    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
    setIsToolDragActive(true);

    const point = stagePointFromClient(event.clientX, event.clientY);
    if (!point) {
      setHoverRungIndex(null);
      return;
    }

    setHoverRungIndex(findRungIndexForY(point.y));
  };

  const handleCanvasDrop = (event: React.DragEvent<HTMLDivElement>) => {
    const toolId = event.dataTransfer.getData(LADDER_TOOL_DRAG_MIME);
    if (!isLadderInsertTool(toolId)) {
      return;
    }

    event.preventDefault();

    const point = stagePointFromClient(event.clientX, event.clientY);
    if (!point) {
      resetToolDragState();
      return;
    }

    const rungIndex = findRungIndexForY(point.y);
    if (rungIndex < 0) {
      resetToolDragState();
      return;
    }

    const insertedIndex = placeToolOnRung(toolId, rungIndex, point.x);
    if (insertedIndex >= 0) {
      setSelectedTool(null);
    }
    resetToolDragState();
  };

  const handleCanvasDragLeave = (event: React.DragEvent<HTMLDivElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    const outsideBounds =
      event.clientX < bounds.left ||
      event.clientX > bounds.right ||
      event.clientY < bounds.top ||
      event.clientY > bounds.bottom;

    if (outsideBounds) {
      resetToolDragState();
    }
  };

  useEffect(() => {
    const reset = () => {
      setIsToolDragActive(false);
      setHoverRungIndex(null);
    };

    window.addEventListener("drop", reset);
    window.addEventListener("dragend", reset);

    return () => {
      window.removeEventListener("drop", reset);
      window.removeEventListener("dragend", reset);
    };
  }, []);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }

    const handleClick = () => {
      if (!selectedTool || isPanning || linkModeEnabled) {
        return;
      }

      const position = stage.getPointerPosition();
      if (!position) {
        return;
      }

      const transform = stage.getAbsoluteTransform().copy();
      transform.invert();
      const stagePoint = transform.point(position);
      const rungIndex = findRungIndexForY(stagePoint.y);

      if (rungIndex < 0) {
        return;
      }

      if (placeToolOnRung(selectedTool, rungIndex, stagePoint.x) >= 0) {
        setSelectedTool(null);
      }
    };

    stage.on("click", handleClick);

    return () => {
      stage.off("click", handleClick);
    };
  }, [
    findRungIndexForY,
    isPanning,
    linkModeEnabled,
    placeToolOnRung,
    program.networks,
    selectedTool,
  ]);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage || !linkModeEnabled) {
      return;
    }

    const updatePreviewPoint = () => {
      const source = edgeLinkSourceRef.current;
      if (!source) {
        setLinkPreviewPoint(null);
        return;
      }

      const pointer = stage.getPointerPosition();
      if (!pointer) {
        return;
      }

      const transform = stage.getAbsoluteTransform().copy();
      transform.invert();
      const stagePoint = transform.point(pointer);
      setLinkPreviewPoint({
        x: stagePoint.x,
        y: stagePoint.y,
      });
    };

    const findTargetNodeAtPoint = (
      x: number,
      y: number
    ): { rungIndex: number; nodeId: string } | null => {
      let best:
        | {
            rungIndex: number;
            nodeId: string;
            distance: number;
          }
        | null = null;

      program.networks.forEach((rung, rungIndex) => {
        rung.nodes.forEach((node) => {
          const hitPoints = [
            {
              x: node.position.x + connectorOffset(node.type, "left"),
              y: node.position.y,
              radius: 18,
            },
            {
              x: node.position.x + connectorOffset(node.type, "right"),
              y: node.position.y,
              radius: 18,
            },
            {
              x: node.position.x,
              y: node.position.y,
              radius: node.type === "junction" ? 20 : 24,
            },
          ];

          hitPoints.forEach((point) => {
            const dx = point.x - x;
            const dy = point.y - y;
            const distance = Math.sqrt(dx * dx + dy * dy);
            if (distance > point.radius) {
              return;
            }
            if (!best || distance < best.distance) {
              best = { rungIndex, nodeId: node.id, distance };
            }
          });
        });
      });

      return best ? { rungIndex: best.rungIndex, nodeId: best.nodeId } : null;
    };

    const finalizeLink = () => {
      const source = edgeLinkSourceRef.current;
      if (!source) {
        return;
      }

      const pointer = stage.getPointerPosition();
      if (!pointer) {
        setEdgeLinkSourceState(null);
        return;
      }

      const transform = stage.getAbsoluteTransform().copy();
      transform.invert();
      const stagePoint = transform.point(pointer);
      const target = findTargetNodeAtPoint(stagePoint.x, stagePoint.y);

      if (!target) {
        setLinkFeedback("Release on a target node.");
        return;
      }

      if (target.rungIndex !== source.rungIndex) {
        setLinkFeedback("Target must be in the same rung.");
        setEdgeLinkSourceState(null);
        return;
      }

      if (target.nodeId === source.nodeId) {
        setLinkFeedback("Select a different target node.");
        return;
      }

      const result = connectNodesIfNeeded(
        source.rungIndex,
        source.nodeId,
        target.nodeId
      );
      if (result === "added") {
        setLinkFeedback("Wire connected.");
      } else if (result === "already-connected") {
        setLinkFeedback("Nodes are already connected.");
      } else {
        setLinkFeedback("Invalid wire target.");
      }
      setEdgeLinkSourceState(null);
    };

    stage.on("mousemove", updatePreviewPoint);
    stage.on("touchmove", updatePreviewPoint);
    stage.on("mouseup", finalizeLink);
    stage.on("touchend", finalizeLink);

    return () => {
      stage.off("mousemove", updatePreviewPoint);
      stage.off("touchmove", updatePreviewPoint);
      stage.off("mouseup", finalizeLink);
      stage.off("touchend", finalizeLink);
    };
  }, [connectNodesIfNeeded, linkModeEnabled, program.networks]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (hasEditableFocus()) {
        return;
      }

      if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
        event.preventDefault();
        if (selectedElement) {
          setActiveRungIndex(selectedElement.rungIndex);
          setRightPaneView("tools");
          setContextMenu({
            x: window.innerWidth / 2,
            y: window.innerHeight / 2,
            target: {
              kind: "element",
              rungIndex: selectedElement.rungIndex,
              elementIndex: selectedElement.elementIndex,
            },
          });
          return;
        }
        if (activeRungIndex !== null) {
          setRightPaneView("tools");
          setContextMenu({
            x: window.innerWidth / 2,
            y: window.innerHeight / 2,
            target: {
              kind: "rung",
              rungIndex: activeRungIndex,
            },
          });
        }
        return;
      }

      const usesModifier = event.metaKey || event.ctrlKey;
      if (usesModifier) {
        const key = event.key.toLowerCase();

        if (key === "z" && !event.shiftKey) {
          event.preventDefault();
          undo();
          return;
        }

        if (key === "y" || (key === "z" && event.shiftKey)) {
          event.preventDefault();
          redo();
          return;
        }

        if (key === "c") {
          event.preventDefault();
          copySelection();
          return;
        }

        if (key === "v") {
          event.preventDefault();
          pasteSelection();
          return;
        }

        if (key === "h") {
          event.preventDefault();
          handleSearchReplace();
          return;
        }
      }

      if (
        (event.key === "Delete" || event.key === "Backspace") &&
        selectedElement
      ) {
        event.preventDefault();
        removeSelectedElement();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [activeRungIndex, program, selectedElement]);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const closeContextMenu = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && target.closest(".ladder-context-menu")) {
        return;
      }
      setContextMenu(null);
    };

    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setContextMenu(null);
      }
    };

    window.addEventListener("mousedown", closeContextMenu, true);
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("mousedown", closeContextMenu, true);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [contextMenu]);

  useEffect(() => {
    if (!layerRef.current) {
      return;
    }

    const layer = layerRef.current;
    layer.destroyChildren();

    for (let x = 0; x < STAGE_WIDTH; x += GRID_SIZE) {
      layer.add(
        new Konva.Line({
          points: [x, 0, x, STAGE_HEIGHT],
          stroke: k(t.gridLine),
          strokeWidth: 0.5,
          opacity: 0.3,
          listening: false,
        })
      );
    }

    for (let y = 0; y < STAGE_HEIGHT; y += GRID_SIZE) {
      layer.add(
        new Konva.Line({
          points: [0, y, STAGE_WIDTH, y],
          stroke: k(t.gridLine),
          strokeWidth: 0.5,
          opacity: 0.3,
          listening: false,
        })
      );
    }

    layer.add(
      new Konva.Line({
        points: [LEFT_RAIL_X, 0, LEFT_RAIL_X, STAGE_HEIGHT],
        stroke: k(t.rail),
        strokeWidth: 3,
        listening: false,
      })
    );

    layer.add(
      new Konva.Line({
        points: [RIGHT_RAIL_X, 0, RIGHT_RAIL_X, STAGE_HEIGHT],
        stroke: k(t.rail),
        strokeWidth: 3,
        listening: false,
      })
    );

    program.networks.forEach((rung, rungIndex) => {
      const isActiveRung = rungIndex === activeRungIndex;
      const isHoverRung = isToolDragActive && rungIndex === hoverRungIndex;
      const rungY = rung.layout.y;

      const hitArea = new Konva.Rect({
        x: LEFT_RAIL_X,
        y: rungY - 16,
        width: RIGHT_RAIL_X - LEFT_RAIL_X,
        height: 32,
        fill: isHoverRung ? kt(t.accent, 0.14) : "transparent",
      });
      hitArea.on("click tap", (event) => {
        event.cancelBubble = true;
        setActiveRungIndex(rungIndex);
        if (selectedTool) {
          const stage = stageRef.current;
          const position = stage?.getPointerPosition();
          if (!stage || !position) {
            return;
          }

          const transform = stage.getAbsoluteTransform().copy();
          transform.invert();
          const stagePoint = transform.point(position);
          if (placeToolOnRung(selectedTool, rungIndex, stagePoint.x) >= 0) {
            setSelectedTool(null);
          }
        } else {
          setSelectedElement(null);
        }
      });
      hitArea.on("contextmenu", (event) => {
        event.cancelBubble = true;
        event.evt.preventDefault();
        const mouseEvent = event.evt as MouseEvent;
        openRungContextMenu(rungIndex, mouseEvent.clientX ?? 0, mouseEvent.clientY ?? 0);
      });
      layer.add(hitArea);

      const rungStroke = isActiveRung
        ? k(t.accent)
        : isHoverRung
          ? kt(t.accent, 0.72)
          : k(t.textSubtle);
      const rungStrokeWidth = isActiveRung || isHoverRung ? 3 : 2;
      if (rung.edges.length === 0) {
        const occlusions = mergeIntervals(
          rung.nodes
            .filter((node) => Math.abs(node.position.y - rungY) <= GRID_SIZE)
            .map((node) => ({
              start:
                node.position.x +
                Math.min(
                  connectorOffset(node.type, "left"),
                  connectorOffset(node.type, "right")
                ),
              end:
                node.position.x +
                Math.max(
                  connectorOffset(node.type, "left"),
                  connectorOffset(node.type, "right")
                ),
            }))
        );
        let cursor = LEFT_RAIL_X;
        for (const occlusion of occlusions) {
          const segmentEnd = Math.max(
            LEFT_RAIL_X,
            Math.min(occlusion.start, RIGHT_RAIL_X)
          );
          if (segmentEnd > cursor) {
            layer.add(
              new Konva.Line({
                points: [cursor, rungY, segmentEnd, rungY],
                stroke: rungStroke,
                strokeWidth: rungStrokeWidth,
                listening: false,
              })
            );
          }
          cursor = Math.max(cursor, Math.min(occlusion.end, RIGHT_RAIL_X));
        }
        if (cursor < RIGHT_RAIL_X) {
          layer.add(
            new Konva.Line({
              points: [cursor, rungY, RIGHT_RAIL_X, rungY],
              stroke: rungStroke,
              strokeWidth: rungStrokeWidth,
              listening: false,
            })
          );
        }
      }

      layer.add(
        new Konva.Text({
          x: 10,
          y: rungY - 10,
          text: `${rungIndex + 1}`,
          fontSize: 14,
          fill: isActiveRung ? k(t.accent) : isHoverRung ? kt(t.accent, 0.72) : k(t.text),
          listening: false,
        })
      );

      const sortedElements = sortByX(rung.nodes);

      if (sortedElements.length > 0) {
        const first = sortedElements[0];
        const last = sortedElements[sortedElements.length - 1];

        layer.add(
          new Konva.Line({
            points: [
              LEFT_RAIL_X,
              rungY,
              first.position.x + connectorOffset(first.type, "left"),
              first.position.y,
            ],
            stroke: k(t.ladderWire),
            strokeWidth: 2,
            listening: false,
          })
        );

        if (rung.edges.length === 0) {
          for (let index = 0; index < sortedElements.length - 1; index += 1) {
            const current = sortedElements[index];
            const next = sortedElements[index + 1];
            layer.add(
              new Konva.Line({
                points: [
                  current.position.x + connectorOffset(current.type, "right"),
                  current.position.y,
                  next.position.x + connectorOffset(next.type, "left"),
                  next.position.y,
                ],
                stroke: k(t.ladderWire),
                strokeWidth: 2,
                listening: false,
              })
            );
          }
        } else {
          for (const edge of rung.edges) {
            const from = rung.nodes.find((node) => node.id === edge.fromNodeId);
            const to = rung.nodes.find((node) => node.id === edge.toNodeId);
            if (!from || !to) {
              continue;
            }

            const points =
              edge.points && edge.points.length > 0
                ? edge.points.flatMap((point) => [point.x, point.y])
                : [
                    from.position.x + connectorOffset(from.type, "right"),
                    from.position.y,
                    to.position.x + connectorOffset(to.type, "left"),
                    to.position.y,
                  ];

            layer.add(
              new Konva.Line({
                points,
                stroke: k(t.ladderWire),
                strokeWidth: 2,
                lineJoin: "round",
                lineCap: "round",
                listening: false,
              })
            );
          }
        }

        layer.add(
          new Konva.Line({
            points: [
              last.position.x + connectorOffset(last.type, "right"),
              last.position.y,
              RIGHT_RAIL_X,
              rungY,
            ],
            stroke: k(t.ladderWire),
            strokeWidth: 2,
            listening: false,
          })
        );
      }

      if (
        linkModeEnabled &&
        edgeLinkSource &&
        linkPreviewPoint &&
        edgeLinkSource.rungIndex === rungIndex
      ) {
        const sourceNode = rung.nodes.find(
          (node) => node.id === edgeLinkSource.nodeId
        );
        if (sourceNode) {
          const sourceX =
            sourceNode.position.x + connectorOffset(sourceNode.type, "right");
          const sourceY = sourceNode.position.y;
          const midX =
            Math.round(((sourceX + linkPreviewPoint.x) / 2) / GRID_SIZE) * GRID_SIZE;
          layer.add(
            new Konva.Line({
              points: [
                sourceX,
                sourceY,
                midX,
                sourceY,
                midX,
                linkPreviewPoint.y,
                linkPreviewPoint.x,
                linkPreviewPoint.y,
              ],
              stroke: k(t.ladderPreview),
              strokeWidth: 2,
              opacity: 0.82,
              lineJoin: "round",
              lineCap: "round",
              listening: false,
            })
          );
        }
      }

      rung.nodes.forEach((element, elementIndex) => {
        const isSelected =
          selectedElement?.rungIndex === rungIndex &&
          selectedElement.elementIndex === elementIndex;

        const drawContext: DrawNodeContext = {
          layer,
          stage: stageRef.current,
          isPanning,
          linkModeEnabled,
          gridSize: GRID_SIZE,
          rungIndex,
          elementIndex,
          isSelected,
          onUpdatePosition: updateElementPosition,
          onStartLink: handleStartLink,
          onContextMenu: (targetRungIndex, targetElementIndex, clientX, clientY) => {
            openElementContextMenu(
              targetRungIndex,
              targetElementIndex,
              clientX,
              clientY
            );
          },
          resolveVariableDisplay,
          onSelect: (nextRungIndex, nextElementIndex) => {
            setSelectedElement({
              rungIndex: nextRungIndex,
              elementIndex: nextElementIndex,
            });
            setActiveRungIndex(nextRungIndex);
          },
        };

        if (element.type === "contact") {
          drawContactNode(element, drawContext, executionState);
        }
        if (element.type === "coil") {
          drawCoilNode(element, drawContext, executionState);
        }
        if (element.type === "timer") {
          drawTimerNode(element, drawContext);
        }
        if (element.type === "counter") {
          drawCounterNode(element, drawContext);
        }
        if (element.type === "compare") {
          drawCompareNode(element, drawContext);
        }
        if (element.type === "math") {
          drawMathNode(element, drawContext);
        }
        if (element.type === "branchSplit") {
          drawBranchSplitNode(element, drawContext);
        }
        if (element.type === "branchMerge") {
          drawBranchMergeNode(element, drawContext);
        }
        if (element.type === "junction") {
          drawJunctionNode(element, drawContext);
        }
      });
    });

    layer.batchDraw();
  }, [
    activeRungIndex,
    edgeLinkSource,
    executionState,
    hoverRungIndex,
    isPanning,
    isToolDragActive,
    linkModeEnabled,
    linkPreviewPoint,
    placeToolOnRung,
    program,
    resolveVariableDisplay,
    selectedElement,
    selectedTool,
  ]);

  useEffect(() => {
    vscodeApi?.postMessage({ type: "ready" });
  }, []);

  useEffect(() => {
    vscodeApi?.postMessage({ type: "programState", program });
  }, [program]);

  useEffect(() => {
    const messageHandler = (event: MessageEvent) => {
      const message = event.data;
      switch (message.type) {
        case "loadProgram": {
          const reconciledProgram = reconcileContactCoilVariableDeclarations(
            message.program
          );
          setProgram(reconciledProgram);
          resetHistory();
          setClipboard(null);
          setLinkFeedback(null);
          setEdgeLinkSourceState(null);
          setLinkModeEnabled(false);
          setSelectedTool(null);
          if (reconciledProgram.networks.length > 0) {
            setActiveRungIndex(0);
          } else {
            setActiveRungIndex(null);
          }
          setSelectedElement(null);
          break;
        }
        case "runtime.state":
          runtimeIsExecutingRef.current = Boolean(message.state?.isExecuting);
          if (!message.state.isExecuting) {
            setExecutionState(null);
          }
          break;
        case "runtime.error":
          console.error("[Ladder webview runtime error]", message.message);
          break;
        case "ioState":
          if (!runtimeIsExecutingRef.current) {
            setExecutionState(null);
            break;
          }
          setExecutionState(executionStateFromIoState(message.payload));
          break;
        case "stateUpdate":
          setExecutionState(message.state);
          break;
        default:
          break;
      }
    };

    window.addEventListener("message", messageHandler);
    return () => {
      window.removeEventListener("message", messageHandler);
    };
  }, []);

  const selectedElementData =
    selectedElement &&
    program.networks[selectedElement.rungIndex] &&
    program.networks[selectedElement.rungIndex].nodes[selectedElement.elementIndex]
      ? program.networks[selectedElement.rungIndex].nodes[selectedElement.elementIndex]
      : null;
  const contextMenuElementData =
    contextMenu?.target.kind === "element" &&
    program.networks[contextMenu.target.rungIndex]?.nodes[
      contextMenu.target.elementIndex
    ]
      ? program.networks[contextMenu.target.rungIndex].nodes[
          contextMenu.target.elementIndex
        ]
      : null;
  const canAddParallelContact =
    Boolean(selectedElementData) && selectedElementData?.type === "contact";

  if (bootError) {
    return (
      <div className="ladder-error">
        <h2>Ladder editor failed to initialize</h2>
        <p>{bootError}</p>
      </div>
    );
  }

  const handleSave = () => {
    vscodeApi?.postMessage({ type: "save", program });
  };

  const handleValidate = () => {
    vscodeApi?.postMessage({ type: "validate", program });
  };

  const handleGenerateST = () => {
    vscodeApi?.postMessage({ type: "generateST", program });
  };

  const handleAutoRoute = () => {
    applyProgramChange((previous) => autoRouteProgram(previous));
  };

  const handleFitView = () => {
    const stage = stageRef.current;
    const container = containerRef.current;
    if (!stage || !container) {
      return;
    }
    const containerWidth = container.clientWidth || STAGE_WIDTH;
    const fitScale = Math.max(
      0.3,
      Math.min(1, (containerWidth - 24) / (RIGHT_RAIL_X + 40))
    );
    stage.scale({ x: fitScale, y: fitScale });
    stage.position({ x: 12, y: 0 });
    setScale(fitScale);
    stage.batchDraw();
  };

  return (
    <div className="ladder-editor trust-product-shell">
      <header className="trust-product-header" aria-label="Ladder editor header">
        <div className="trust-product-brand">
          tru<span className="trust-product-brand__accent">ST</span>
          <span className="trust-product-brand__separator">·</span>
          <span className="trust-product-brand__surface">Ladder editor</span>
        </div>
        <div className="trust-product-header__meta">Ladder logic diagram</div>
      </header>

      <div className="editor-body trust-product-workspace">
        <div
          className={`canvas-container trust-canvas-pane ${selectedTool ? "tool-selected" : ""} ${
            linkModeEnabled ? "link-mode" : ""
          } ${
            isPanModifierActive ? "pan-enabled" : ""
          } ${isPanning ? "panning" : ""} ${
            isToolDragActive ? "drag-tool-active" : ""
          }`}
          onDragOver={handleCanvasDragOver}
          onDrop={handleCanvasDrop}
          onDragLeave={handleCanvasDragLeave}
        >
          <div ref={containerRef} />
        </div>

        <div className={resizeHandleClassName} {...resizeHandleProps} />

        <div className="trust-inspector ladder-side-panel right-pane-resizable" style={rightPaneStyle}>
          <div className="trust-inspector__header" aria-label="Ladder editor">
            <div className="trust-inspector__title">Ladder editor</div>
          </div>

          <LadderToolsPanel
            selectedTool={selectedTool}
            onToolSelect={(tool) => {
              setSelectedTool(tool);
              if (tool) {
                setLinkModeEnabled(false);
                setLinkFeedback(null);
                setEdgeLinkSourceState(null);
              }
            }}
            onDeleteSelection={removeSelectedElement}
            onAddRung={addRung}
            onRemoveRung={removeRung}
            onAddParallelContact={() => addParallelContactFromSelection()}
            onClearWiring={clearActiveRungWiring}
            onUndo={undo}
            onRedo={redo}
            onCopy={copySelection}
            onPaste={pasteSelection}
            onSearchReplace={handleSearchReplace}
            onAutoRoute={handleAutoRoute}
            onFitView={handleFitView}
            onValidate={handleValidate}
            onGenerateST={handleGenerateST}
            onSave={handleSave}
            onToggleLinkMode={toggleLinkMode}
            linkModeEnabled={linkModeEnabled}
            linkSourceLabel={edgeLinkSource?.nodeId}
            linkFeedback={linkFeedback}
            canUndo={undoDepth > 0}
            canRedo={redoDepth > 0}
            canPaste={hasClipboard}
            canDeleteSelection={Boolean(selectedElementData)}
            canRemoveRung={program.networks.length > 0}
            canAddParallelContact={canAddParallelContact}
            canClearWiring={
              activeRungIndex !== null &&
              Boolean(program.networks[activeRungIndex]?.edges.length)
            }
          />
          <ElementPropertiesPanel
            selectedElement={selectedElement}
            selectedElementData={selectedElementData}
            activeRungIndex={activeRungIndex}
            networkCount={program.networks.length}
            gridSize={GRID_SIZE}
            onUpdateSelectedElement={updateSelectedElement}
            onRemoveSelectedElement={removeSelectedElement}
          />
        </div>
      </div>

      {contextMenu && (
        <div
          className="ladder-context-menu"
          style={{
            left: `${contextMenu.x}px`,
            top: `${contextMenu.y}px`,
          }}
          onContextMenu={(event) => event.preventDefault()}
        >
          {contextMenu.target.kind === "element" ? (
            <>
              <button
                type="button"
                onClick={() => {
                  executeElementContextAction("cut");
                  setContextMenu(null);
                }}
              >
                Cut
              </button>
              <button
                type="button"
                onClick={() => {
                  executeElementContextAction("copy");
                  setContextMenu(null);
                }}
              >
                Copy
              </button>
              <button
                type="button"
                disabled={!hasClipboard}
                onClick={() => {
                  executeElementContextAction("paste");
                  setContextMenu(null);
                }}
              >
                Paste
              </button>
              <button
                type="button"
                onClick={() => {
                  executeElementContextAction("delete");
                  setContextMenu(null);
                }}
              >
                Delete
              </button>
              {contextMenuElementData?.type === "contact" && (
                <>
                  <button
                    type="button"
                    onClick={() => {
                      executeElementContextAction("parallel-contact");
                      setContextMenu(null);
                    }}
                  >
                    Parallel Contact
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      executeElementContextAction("toggle-contact-type");
                      setContextMenu(null);
                    }}
                  >
                    Toggle NO/NC
                  </button>
                </>
              )}
              {contextMenuElementData?.type === "coil" && (
                <>
                  <button
                    type="button"
                    onClick={() => {
                      executeElementContextAction("set-coil-type:NORMAL");
                      setContextMenu(null);
                    }}
                  >
                    Coil NORMAL
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      executeElementContextAction("set-coil-type:SET");
                      setContextMenu(null);
                    }}
                  >
                    Coil SET
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      executeElementContextAction("set-coil-type:RESET");
                      setContextMenu(null);
                    }}
                  >
                    Coil RESET
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      executeElementContextAction("set-coil-type:NEGATED");
                      setContextMenu(null);
                    }}
                  >
                    Coil NEGATED
                  </button>
                </>
              )}
            </>
          ) : (
            <>
              <button
                type="button"
                onClick={() => {
                  executeRungContextAction("add-rung-above");
                  setContextMenu(null);
                }}
              >
                Add Rung Above
              </button>
              <button
                type="button"
                onClick={() => {
                  executeRungContextAction("add-rung-below");
                  setContextMenu(null);
                }}
              >
                Add Rung Below
              </button>
              <button
                type="button"
                disabled={program.networks.length === 0}
                onClick={() => {
                  executeRungContextAction("remove-rung");
                  setContextMenu(null);
                }}
              >
                Remove Rung
              </button>
              <button
                type="button"
                disabled={!hasClipboard}
                onClick={() => {
                  executeRungContextAction("paste");
                  setContextMenu(null);
                }}
              >
                Paste
              </button>
            </>
          )}
        </div>
      )}

      <div className="trust-visual-status status-bar">
        <span>Rungs: {program.networks.length}</span>
        <span>Zoom: {Math.round(scale * 100)}%</span>
        <span>Undo: {undoDepth}</span>
        <span>Redo: {redoDepth}</span>
        {selectedTool && (
          <span>
            Selected tool: {selectedTool} (click or drag to a rung to place)
          </span>
        )}
        {linkModeEnabled && (
          <span>
            Wire mode:{" "}
            {edgeLinkSource
              ? `source ${edgeLinkSource.nodeId}, click/drag to target`
              : "click source then target"}
          </span>
        )}
        {linkModeEnabled && linkFeedback && <span>{linkFeedback}</span>}
        <span>Grid: {GRID_SIZE}px</span>
        <span>Pan: Space+drag</span>
      </div>
    </div>
  );
}
