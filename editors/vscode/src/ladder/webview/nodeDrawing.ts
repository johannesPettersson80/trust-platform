import Konva from "konva";
import type {
  BranchMergeNode,
  BranchSplitNode,
  Coil as CoilType,
  CompareNode,
  Contact as ContactType,
  Counter as CounterType,
  JunctionNode,
  LadderElement,
  MathNode,
  Timer as TimerType,
} from "../ladderEngine.types";
import { canvasColor, t, tint } from "../../webview/theme";

export interface DrawNodeContext {
  layer: Konva.Layer;
  stage: Konva.Stage | null;
  isPanning: boolean;
  linkModeEnabled: boolean;
  gridSize: number;
  rungIndex: number;
  elementIndex: number;
  isSelected: boolean;
  onUpdatePosition: (
    rungIndex: number,
    elementIndex: number,
    x: number,
    y: number
  ) => void;
  onSelect: (rungIndex: number, elementIndex: number) => void;
  onStartLink?: (rungIndex: number, elementIndex: number) => void;
  onContextMenu?: (
    rungIndex: number,
    elementIndex: number,
    clientX: number,
    clientY: number
  ) => void;
  resolveVariableDisplay?: (reference: string) => VariableDisplay;
}

export interface VariableDisplay {
  primary: string;
  secondary?: string;
}

const FB_BODY_X = 0;
const FB_BODY_Y = -34;
const FB_BODY_WIDTH = 124;
const FB_BODY_HEIGHT = 68;
const FB_CONNECTOR_RIGHT = 140;

const k = canvasColor;
const kt = (color: string, alpha: number): string => canvasColor(tint(color, alpha));

export function connectorOffset(
  elementType: LadderElement["type"],
  side: "left" | "right"
): number {
  switch (elementType) {
    case "contact":
    case "coil":
      return side === "left" ? -20 : 60;
    case "timer":
    case "counter":
    case "compare":
    case "math":
      return side === "left" ? -20 : FB_CONNECTOR_RIGHT;
    case "branchSplit":
    case "branchMerge":
    case "junction":
      return 0;
    default:
      return side === "left" ? -20 : 60;
  }
}

function wireGroupInteraction(
  group: Konva.Group,
  context: DrawNodeContext
): void {
  group.draggable(!context.linkModeEnabled);

  group.on("dragmove", function () {
    if (context.linkModeEnabled) {
      return;
    }
    this.x(Math.round(this.x() / context.gridSize) * context.gridSize);
    this.y(Math.round(this.y() / context.gridSize) * context.gridSize);
  });

  group.on("dragend", function () {
    if (context.linkModeEnabled) {
      return;
    }
    context.onUpdatePosition(
      context.rungIndex,
      context.elementIndex,
      this.x(),
      this.y()
    );
  });

  group.on("click tap", (event) => {
    event.cancelBubble = true;
    context.onSelect(context.rungIndex, context.elementIndex);
  });

  group.on("mousedown touchstart", (event) => {
    if (!context.linkModeEnabled) {
      return;
    }
    event.cancelBubble = true;
    context.onStartLink?.(context.rungIndex, context.elementIndex);
  });

  group.on("contextmenu", (event) => {
    event.cancelBubble = true;
    event.evt.preventDefault();
    const nativeEvent = event.evt as MouseEvent;
    context.onContextMenu?.(
      context.rungIndex,
      context.elementIndex,
      nativeEvent.clientX ?? 0,
      nativeEvent.clientY ?? 0
    );
  });

  group.on("mouseenter", () => {
    if (context.stage && !context.isPanning) {
      context.stage.container().style.cursor = context.linkModeEnabled
        ? "cell"
        : "move";
    }
  });

  group.on("mouseleave", () => {
    if (context.stage && !context.isPanning) {
      context.stage.container().style.cursor = "default";
    }
  });
}

function truncateLabel(value: string, maxLength: number): string {
  const text = value.trim();
  if (text.length <= maxLength) {
    return text;
  }
  if (maxLength <= 3) {
    return text.slice(0, maxLength);
  }
  const keep = maxLength - 3;
  const head = Math.ceil(keep / 2);
  const tail = Math.floor(keep / 2);
  return `${text.slice(0, head)}...${text.slice(text.length - tail)}`;
}

function variableDisplayFor(reference: string, context: DrawNodeContext): VariableDisplay {
  const resolved = context.resolveVariableDisplay?.(reference);
  if (resolved?.primary.trim()) {
    return resolved;
  }
  return { primary: reference.trim() || "Unassigned" };
}

function drawVariableLabel(
  group: Konva.Group,
  display: VariableDisplay,
  color: string
): void {
  const primary = display.primary.trim();
  const secondary = display.secondary?.trim();
  group.add(
    new Konva.Text({
      x: -18,
      y: 20,
      width: 96,
      text: truncateLabel(primary, 16),
      fontSize: 11,
      fill: color,
      fontStyle: "bold",
      align: "center",
    })
  );

  if (!secondary) {
    return;
  }

  group.add(
    new Konva.Text({
      x: -18,
      y: 34,
      width: 96,
      text: truncateLabel(secondary, 18),
      fontSize: 9,
      fill: k(t.textMuted),
      align: "center",
    })
  );
}

export function drawContactNode(
  element: ContactType,
  context: DrawNodeContext,
  executionState: any
): void {
  const { position, contactType, variable } = element;
  const variableState = Boolean(
    executionState &&
      (executionState.inputs?.[variable] ||
        executionState.outputs?.[variable] ||
        executionState.markers?.[variable] ||
        executionState.variableBooleans?.[variable])
  );
  const hasExecutionState = Boolean(executionState);
  const isActive =
    hasExecutionState && (contactType === "NC" ? !variableState : variableState);
  const color = k(isActive ? t.ladderWireLive : t.ladderWire);
  const display = variableDisplayFor(variable, context);

  const group = new Konva.Group({
    x: position.x,
    y: position.y,
    draggable: true,
  });

  wireGroupInteraction(group, context);

  group.add(
    new Konva.Rect({
      x: -8,
      y: -24,
      width: 74,
      height: 54,
      fill: context.isSelected ? k(t.selectedBg) : "transparent",
      stroke: context.isSelected ? k(t.accent) : undefined,
      strokeWidth: context.isSelected ? 2 : 0,
    })
  );

  group.add(
    new Konva.Line({
      points: [-20, 0, 0, 0],
      stroke: color,
      strokeWidth: 3,
    })
  );

  group.add(
    new Konva.Line({
      points: [40, 0, 60, 0],
      stroke: color,
      strokeWidth: 3,
    })
  );

  group.add(
    new Konva.Line({
      points: [15, -15, 15, 15],
      stroke: color,
      strokeWidth: 3,
    })
  );

  group.add(
    new Konva.Line({
      points: [25, -15, 25, 15],
      stroke: color,
      strokeWidth: 3,
    })
  );

  if (contactType === "NC") {
    group.add(
      new Konva.Line({
        points: [13, 12, 27, -12],
        stroke: color,
        strokeWidth: 2,
      })
    );
  }

  drawVariableLabel(group, display, color);

  context.layer.add(group);
}

export function drawCoilNode(
  element: CoilType,
  context: DrawNodeContext,
  executionState: any
): void {
  const { position, coilType, variable } = element;
  const isActive = Boolean(
    executionState &&
      (executionState.outputs?.[variable] ||
        executionState.markers?.[variable] ||
        executionState.variableBooleans?.[variable])
  );
  const color = k(isActive ? t.ladderWireLive : t.ladderWire);
  const display = variableDisplayFor(variable, context);

  const group = new Konva.Group({
    x: position.x,
    y: position.y,
    draggable: true,
  });

  wireGroupInteraction(group, context);

  group.add(
    new Konva.Rect({
      x: -8,
      y: -24,
      width: 74,
      height: 54,
      fill: context.isSelected ? k(t.selectedBg) : "transparent",
      stroke: context.isSelected ? k(t.accent) : undefined,
      strokeWidth: context.isSelected ? 2 : 0,
    })
  );

  group.add(
    new Konva.Line({
      points: [-20, 0, 8, 0],
      stroke: color,
      strokeWidth: 3,
    })
  );

  group.add(
    new Konva.Line({
      points: [32, 0, 60, 0],
      stroke: color,
      strokeWidth: 3,
    })
  );

  group.add(
    new Konva.Line({
      points: [14, -15, 8, -10, 5, 0, 8, 10, 14, 15],
      stroke: color,
      strokeWidth: 3,
      lineCap: "round",
      lineJoin: "round",
    })
  );

  group.add(
    new Konva.Line({
      points: [26, -15, 32, -10, 35, 0, 32, 10, 26, 15],
      stroke: color,
      strokeWidth: 3,
      lineCap: "round",
      lineJoin: "round",
    })
  );

  if (coilType === "SET") {
    group.add(
      new Konva.Text({
        x: 17,
        y: -7,
        text: "S",
        fontSize: 14,
        fill: color,
        fontStyle: "bold",
      })
    );
  }

  if (coilType === "RESET") {
    group.add(
      new Konva.Text({
        x: 17,
        y: -7,
        text: "R",
        fontSize: 14,
        fill: color,
        fontStyle: "bold",
      })
    );
  }

  if (coilType === "NEGATED") {
    group.add(
      new Konva.Line({
        points: [8, -12, 32, 12],
        stroke: color,
        strokeWidth: 2,
      })
    );
  }

  drawVariableLabel(group, display, color);

  context.layer.add(group);
}

interface FunctionBlockPin {
  label: string;
  value?: string;
}

function drawFunctionBlockNode(
  context: DrawNodeContext,
  position: { x: number; y: number },
  title: string,
  instance: string,
  leftPins: FunctionBlockPin[],
  rightPins: FunctionBlockPin[],
  accentColor: string
): void {
  const resolvedAccent = k(accentColor);
  const group = new Konva.Group({
    x: position.x,
    y: position.y,
    draggable: true,
  });

  wireGroupInteraction(group, context);

  group.add(
    new Konva.Rect({
      x: -24,
      y: -42,
      width: FB_CONNECTOR_RIGHT + 32,
      height: FB_BODY_HEIGHT + 16,
      fill: context.isSelected ? kt(t.accent, 0.12) : "transparent",
      stroke: context.isSelected ? k(t.accent) : undefined,
      strokeWidth: context.isSelected ? 2 : 0,
      cornerRadius: 6,
    })
  );

  group.add(
    new Konva.Rect({
      x: FB_BODY_X,
      y: FB_BODY_Y,
      width: FB_BODY_WIDTH,
      height: FB_BODY_HEIGHT,
      fill: k(t.ladderBody),
      stroke: context.isSelected ? k(t.accent) : k(t.textSubtle),
      strokeWidth: context.isSelected ? 2 : 1.2,
      cornerRadius: 4,
    })
  );

  group.add(
    new Konva.Rect({
      x: FB_BODY_X,
      y: FB_BODY_Y,
      width: FB_BODY_WIDTH,
      height: 18,
      fill: context.isSelected ? k(t.selectedStrongBg) : kt(t.accent, 0.16),
      cornerRadius: 4,
    })
  );

  group.add(
    new Konva.Line({
      points: [-20, 0, 0, 0],
      stroke: resolvedAccent,
      strokeWidth: 3,
    })
  );

  group.add(
    new Konva.Line({
      points: [FB_BODY_WIDTH, 0, FB_CONNECTOR_RIGHT, 0],
      stroke: resolvedAccent,
      strokeWidth: 3,
    })
  );

  group.add(
    new Konva.Text({
      x: 6,
      y: -31,
      text: title,
      fontSize: 11,
      fill: k(t.text),
      fontStyle: "bold",
    })
  );

  group.add(
    new Konva.Text({
      x: 6,
      y: -16,
      text: truncateLabel(instance, 20),
      fontSize: 10,
      fill: k(t.textMuted),
    })
  );

  leftPins.slice(0, 2).forEach((pin, index) => {
    const y = -1 + index * 15;
    const value = pin.value?.trim() ? truncateLabel(pin.value, 10) : "";
    group.add(
      new Konva.Text({
        x: 6,
        y,
        text: value ? `${pin.label} ${value}` : pin.label,
        fontSize: 9,
        fill: k(t.ladderText),
      })
    );
  });

  rightPins.slice(0, 2).forEach((pin, index) => {
    const y = -1 + index * 15;
    const value = pin.value?.trim() ? truncateLabel(pin.value, 10) : "";
    group.add(
      new Konva.Text({
        x: 58,
        y,
        width: 60,
        align: "right",
        text: value ? `${pin.label} ${value}` : pin.label,
        fontSize: 9,
        fill: k(t.ladderText),
      })
    );
  });

  context.layer.add(group);
}

export function drawTimerNode(element: TimerType, context: DrawNodeContext): void {
  drawFunctionBlockNode(
    context,
    element.position,
    element.timerType,
    element.instance,
    [
      { label: "IN", value: element.input?.trim() || "RUNG" },
      { label: "PT", value: `${element.presetMs}ms` },
    ],
    [
      { label: "Q", value: element.qOutput },
      { label: "ET", value: element.etOutput },
    ],
    t.accent
  );
}

export function drawCounterNode(
  element: CounterType,
  context: DrawNodeContext
): void {
  drawFunctionBlockNode(
    context,
    element.position,
    element.counterType,
    element.instance,
    [
      { label: "IN", value: element.input?.trim() || "RUNG" },
      { label: "PV", value: String(element.preset) },
    ],
    [
      { label: "Q", value: element.qOutput },
      { label: "CV", value: element.cvOutput },
    ],
    t.ladderPreview
  );
}

export function drawCompareNode(
  element: CompareNode,
  context: DrawNodeContext
): void {
  drawFunctionBlockNode(
    context,
    element.position,
    `CMP ${element.op}`,
    "Compare",
    [
      { label: "IN1", value: element.left },
      { label: "IN2", value: element.right },
    ],
    [{ label: "Q", value: "RUNG" }],
    t.ok
  );
}

export function drawMathNode(element: MathNode, context: DrawNodeContext): void {
  drawFunctionBlockNode(
    context,
    element.position,
    element.op,
    "Math",
    [
      { label: "IN1", value: element.left },
      { label: "IN2", value: element.right },
    ],
    [{ label: "OUT", value: element.output }],
    t.blockVariables
  );
}

function drawBranchTerminalBase(
  context: DrawNodeContext,
  position: { x: number; y: number },
  color: string
): Konva.Group {
  const resolvedColor = k(color);
  const group = new Konva.Group({
    x: position.x,
    y: position.y,
    draggable: true,
  });

  wireGroupInteraction(group, context);

  group.add(
    new Konva.Rect({
      x: -9,
      y: -9,
      width: 18,
      height: 18,
      fill: "transparent",
    })
  );

  if (context.isSelected) {
    group.add(
      new Konva.Circle({
        x: 0,
        y: 0,
        radius: 8,
        fill: kt(t.accent, 0.14),
        stroke: k(t.accent),
        strokeWidth: 1.8,
      })
    );
  }

  group.add(
    new Konva.Circle({
      x: 0,
      y: 0,
      radius: 3.5,
      fill: resolvedColor,
      stroke: k(t.canvas),
      strokeWidth: 1,
    })
  );

  return group;
}

function drawBranchStem(group: Konva.Group, color: string, direction: "up" | "down"): void {
  const stemLength = 12;
  const y2 = direction === "down" ? stemLength : -stemLength;
  const resolvedColor = k(color);
  group.add(
    new Konva.Line({
      points: [0, 0, 0, y2],
      stroke: resolvedColor,
      strokeWidth: 2.1,
    })
  );
}

export function drawBranchSplitNode(
  element: BranchSplitNode,
  context: DrawNodeContext
): void {
  const group = drawBranchTerminalBase(context, element.position, t.ladderWire);
  drawBranchStem(group, t.ladderWire, "down");
  context.layer.add(group);
}

export function drawBranchMergeNode(
  element: BranchMergeNode,
  context: DrawNodeContext
): void {
  const group = drawBranchTerminalBase(context, element.position, t.ladderPreview);
  drawBranchStem(group, t.ladderPreview, "down");
  context.layer.add(group);
}

export function drawJunctionNode(
  element: JunctionNode,
  context: DrawNodeContext
): void {
  const group = new Konva.Group({
    x: element.position.x,
    y: element.position.y,
    draggable: true,
  });

  wireGroupInteraction(group, context);

  group.add(
    new Konva.Circle({
      x: 0,
      y: 0,
      radius: 7,
      fill: context.isSelected ? k(t.accent) : k(t.textMuted),
      stroke: k(t.border),
      strokeWidth: 1.5,
    })
  );

  group.add(
    new Konva.Line({
      points: [-10, 0, 10, 0],
      stroke: k(t.textMuted),
      strokeWidth: 2,
    })
  );

  group.add(
    new Konva.Line({
      points: [0, -10, 0, 10],
      stroke: k(t.textMuted),
      strokeWidth: 2,
    })
  );

  context.layer.add(group);
}
