import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Konva from "konva";
import {
  ElementPropertiesPanel,
  type LadderNumberField,
  type NumberFieldError,
} from "./ElementPropertiesPanel";
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
import type {
  Coil as CoilType,
  CompareNode,
  Contact as ContactType,
  Counter as CounterType,
  LadderElement,
  LadderProgram,
  Rung as RungType,
  Timer as TimerType,
} from "../ladderProgram.types";
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

/**
 * The answer of a Ladder edit (`trust-lsp.visualEdit`, forwarded by the editor's host): the
 * program after the edit, as file text, and what the edit reports.
 */
interface LadderEditAnswer {
  ok: boolean;
  programText?: string;
  errors?: Array<{ message: string }>;
  insertedIndex?: number;
  selectedNodeId?: string;
  rungShiftPx?: number;
  totalParallelLegs?: number;
  replacements?: number;
  wire?: "added" | "already-connected" | "invalid";
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

/** The text of a reference as a display lookup key: trimmed and upper case, or `null` when blank. */
function variableDisplayKey(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed.toUpperCase() : null;
}

export function LadderEditor() {
  const [bootError, setBootError] = useState<string | null>(null);
  const [program, setProgram] = useState<LadderProgram>(createEmptyProgram);
  const [selectedTool, setSelectedTool] = useState<LadderInsertTool | null>(
    null
  );
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
  // Why the language server did not store the text last typed into a number field.
  const [numberFieldError, setNumberFieldError] = useState<NumberFieldError | null>(null);
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
  // The program the next edit request starts from, and a count of its changes: an edit answer
  // for an older program is requested again on the current one.
  const programRef = useRef<LadderProgram>(program);
  const revisionRef = useRef(0);
  const pendingEditsRef = useRef(new Map<number, (answer: LadderEditAnswer) => void>());
  const nextRequestIdRef = useRef(1);
  // The latest text typed into a number field: only its answer is stored or shown.
  const numberEditRef = useRef(0);
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
      programRef.current = next;
      revisionRef.current += 1;

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
      programRef.current = previous;
      revisionRef.current += 1;
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
      programRef.current = next;
      revisionRef.current += 1;
      return next;
    });
    setSelectedElement(null);
    syncHistoryState();
  };

  /** Sends a Ladder edit to the host, which forwards it to the language server. */
  const requestEdit = (
    op: string,
    args: Record<string, unknown>,
    base: LadderProgram
  ): Promise<LadderEditAnswer> =>
    new Promise((resolve) => {
      const requestId = nextRequestIdRef.current;
      nextRequestIdRef.current += 1;
      pendingEditsRef.current.set(requestId, resolve);
      vscodeApi?.postMessage({
        type: "edit",
        requestId,
        op,
        programText: JSON.stringify(base),
        ...args,
      });
    });

  /**
   * Runs a Ladder edit (`trust-lsp.visualEdit`) on the current program and shows its result, when
   * `apply` accepts the answer. An answer for a program that changed meanwhile is requested again
   * on the current program, so no change is lost.
   */
  const runEdit = async (
    op: string,
    args: Record<string, unknown>,
    options: { trackHistory?: boolean; apply?: (answer: LadderEditAnswer) => boolean } = {}
  ): Promise<{ answer: LadderEditAnswer; program?: LadderProgram }> => {
    for (let attempt = 0; ; attempt += 1) {
      const revision = revisionRef.current;
      const answer = await requestEdit(op, args, programRef.current);
      if (!answer.ok || typeof answer.programText !== "string") {
        return { answer };
      }
      if (revisionRef.current !== revision && attempt < 3) {
        continue;
      }
      if (options.apply && !options.apply(answer)) {
        return { answer };
      }
      const next = JSON.parse(answer.programText) as LadderProgram;
      applyProgramChange(() => next, options.trackHistory ?? true);
      return { answer, program: next };
    }
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

  const addElementWithTool = async (
    tool: LadderInsertTool,
    rungIndex: number,
    x: number,
    y: number
  ): Promise<number> => {
    const { answer } = await runEdit("insertElement", { rungIndex, tool, x, y });
    return answer.ok && typeof answer.insertedIndex === "number" ? answer.insertedIndex : -1;
  };

  const addRungAt = (insertIndex: number) => {
    void runEdit("addRung", { index: Math.max(0, insertIndex) }).then(({ answer }) => {
      if (answer.ok && typeof answer.insertedIndex === "number") {
        setActiveRungIndex(answer.insertedIndex);
        setSelectedElement(null);
      }
    });
  };

  const addRung = () => {
    addRungAt(program.networks.length);
  };

  const removeRungAt = (targetIndex: number) => {
    if (targetIndex < 0 || targetIndex >= program.networks.length) {
      return;
    }
    const selection = selectedElement;
    void runEdit("removeRung", { index: targetIndex }).then(({ answer, program: next }) => {
      if (!answer.ok || !next) {
        return;
      }
      if (selection?.rungIndex === targetIndex) {
        setSelectedElement(null);
      } else if (selection && selection.rungIndex > targetIndex) {
        setSelectedElement({
          rungIndex: selection.rungIndex - 1,
          elementIndex: selection.elementIndex,
        });
      }
      const remaining = next.networks.length;
      if (remaining === 0) {
        setActiveRungIndex(null);
      } else {
        setActiveRungIndex(Math.min(targetIndex, remaining - 1));
      }
    });
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

    void runEdit("addParallelContact", { rungIndex, contactId: selectedNode.id }).then(
      ({ answer, program: next }) => {
        if (!answer.ok || !next) {
          setLinkFeedback(answer.errors?.[0]?.message ?? "Unable to create parallel branch.");
          return;
        }
        const selectedIndex =
          next.networks[rungIndex]?.nodes.findIndex((node) => node.id === answer.selectedNodeId) ?? -1;
        setSelectedTool(null);
        setLinkModeEnabled(false);
        setEdgeLinkSourceState(null);
        setSelectedElement(selectedIndex >= 0 ? { rungIndex, elementIndex: selectedIndex } : null);
        setActiveRungIndex(rungIndex);
        const legs = answer.totalParallelLegs ?? 0;
        const shift = answer.rungShiftPx ?? 0;
        setLinkFeedback(
          shift > 0
            ? `Parallel branch now has ${legs} legs. Lower rungs shifted by ${shift}px.`
            : `Parallel branch now has ${legs} legs.`
        );
      }
    );
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

      return {
        ...previous,
        networks,
        variables: previous.variables,
      };
    });
    // A contact or coil operand declares its symbol; the language server reconciles the
    // declarations as part of the same change.
    const element = program.networks[rungIndex]?.nodes[elementIndex];
    if (element && (element.type === "contact" || element.type === "coil")) {
      void runEdit("reconcileDeclarations", {}, { trackHistory: false });
    }
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

  /**
   * Sends the text typed into a number field of the selected element to the language server
   * (`trust-lsp.visualEdit` `setNumber`), which stores the number it reads or answers why not.
   * Only the answer to the latest text is stored or shown.
   */
  const setSelectedNumber = (field: LadderNumberField, text: string) => {
    if (!selectedElement) {
      return;
    }
    const { rungIndex, elementIndex } = selectedElement;
    numberEditRef.current += 1;
    const request = numberEditRef.current;
    void runEdit(
      "setNumber",
      { rungIndex, elementIndex, field, text },
      { apply: () => request === numberEditRef.current }
    ).then(({ answer }) => {
      if (request !== numberEditRef.current) {
        return;
      }
      setNumberFieldError(
        answer.ok
          ? null
          : { field, message: answer.errors?.[0]?.message ?? "The value was not stored." }
      );
    });
  };

  useEffect(() => {
    setNumberFieldError(null);
  }, [selectedElement?.rungIndex, selectedElement?.elementIndex]);

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

  /** Wires two nodes of a rung (`trust-lsp.visualEdit` `connect`) and says what happened. */
  const connectNodes = (rungIndex: number, fromNodeId: string, toNodeId: string): void => {
    void runEdit(
      "connect",
      { rungIndex, fromNodeId, toNodeId },
      { apply: (answer) => answer.wire === "added" }
    ).then(({ answer }) => {
      if (answer.wire === "added") {
        setLinkFeedback("Wire connected.");
      } else if (answer.wire === "already-connected") {
        setLinkFeedback("Nodes are already connected.");
      } else {
        setLinkFeedback("Invalid wire target.");
      }
    });
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
      connectNodes(rungIndex, source.nodeId, node.id);
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
      void runEdit("pasteElement", {
        rungIndex: targetRungIndex,
        element: clipboard.element,
      }).then(({ answer }) => {
        if (answer.ok && typeof answer.insertedIndex === "number") {
          setSelectedElement({ rungIndex: targetRungIndex, elementIndex: answer.insertedIndex });
        }
      });
      return;
    }

    const insertAfter =
      targetRungIndex >= 0 ? targetRungIndex : program.networks.length - 1;
    void runEdit("pasteRung", { rung: clipboard.rung, afterIndex: insertAfter }).then(
      ({ answer }) => {
        if (answer.ok && typeof answer.insertedIndex === "number") {
          setActiveRungIndex(answer.insertedIndex);
          setSelectedElement(null);
        }
      }
    );
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
    void runEdit(
      "replaceSymbol",
      { find: findValue, replace: replaceValue },
      { apply: (answer) => (answer.replacements ?? 0) > 0 }
    ).then(({ answer }) => {
      if (!answer.ok) {
        window.alert(answer.errors?.[0]?.message ?? "Find and replace failed.");
      } else if ((answer.replacements ?? 0) === 0) {
        window.alert(`No matches for '${findValue}'.`);
      } else {
        window.alert(`Updated ${answer.replacements} occurrence(s).`);
      }
    });
  };

  const openElementContextMenu = (
    rungIndex: number,
    elementIndex: number,
    clientX: number,
    clientY: number
  ) => {
    setSelectedElement({ rungIndex, elementIndex });
    setActiveRungIndex(rungIndex);
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

  /**
   * Drops a tool on a rung at the grid point nearest `x`, kept between the rails, and selects the
   * new element. Resolves to the element's index, or -1 when it was not placed.
   */
  const placeToolOnRung = async (
    tool: LadderInsertTool,
    rungIndex: number,
    x: number
  ): Promise<number> => {
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
    const insertedIndex = await addElementWithTool(tool, rungIndex, snappedX, rung.layout.y);
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

    void placeToolOnRung(toolId, rungIndex, point.x).then((insertedIndex) => {
      if (insertedIndex >= 0) {
        setSelectedTool(null);
      }
    });
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

      void placeToolOnRung(selectedTool, rungIndex, stagePoint.x).then((insertedIndex) => {
        if (insertedIndex >= 0) {
          setSelectedTool(null);
        }
      });
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

      // `best` is assigned inside the callbacks, which TypeScript's narrowing does not follow.
      const found = best as { rungIndex: number; nodeId: string; distance: number } | null;
      return found ? { rungIndex: found.rungIndex, nodeId: found.nodeId } : null;
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

      connectNodes(source.rungIndex, source.nodeId, target.nodeId);
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
  }, [connectNodes, linkModeEnabled, program.networks]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (hasEditableFocus()) {
        return;
      }

      if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
        event.preventDefault();
        if (selectedElement) {
          setActiveRungIndex(selectedElement.rungIndex);
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
          const tool = selectedTool;
          void placeToolOnRung(tool, rungIndex, stagePoint.x).then((insertedIndex) => {
            if (insertedIndex >= 0) {
              setSelectedTool(null);
            }
          });
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
          drawContactNode(element, drawContext, null);
        }
        if (element.type === "coil") {
          drawCoilNode(element, drawContext, null);
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
    const messageHandler = (event: MessageEvent) => {
      const message = event.data;
      switch (message.type) {
        case "loadProgram": {
          // The host sends the program as the language server opened it, declarations
          // reconciled.
          const loaded = message.program as LadderProgram;
          programRef.current = loaded;
          revisionRef.current += 1;
          setProgram(loaded);
          resetHistory();
          setClipboard(null);
          setLinkFeedback(null);
          setEdgeLinkSourceState(null);
          setLinkModeEnabled(false);
          setSelectedTool(null);
          setActiveRungIndex(loaded.networks.length > 0 ? 0 : null);
          setSelectedElement(null);
          break;
        }
        case "editResult": {
          const resolve = pendingEditsRef.current.get(message.requestId);
          if (resolve) {
            pendingEditsRef.current.delete(message.requestId);
            resolve(message as LadderEditAnswer);
          }
          break;
        }
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
    void runEdit("autoRoute", {});
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
            onSetNumber={setSelectedNumber}
            numberFieldError={numberFieldError}
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
