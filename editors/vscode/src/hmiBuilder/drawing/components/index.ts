// One drawer per component type; an unknown type draws a placeholder.

import type { DrawFrame, HmiObjectModel } from "../model";
import type { SvgNode } from "../svg";
import { drawAlarmList } from "./alarmList";
import { drawBar } from "./bar";
import { drawModeSelector, drawSelector, drawSwitch } from "./choices";
import { drawButton, drawSetpoint } from "./controls";
import { SYMBOLS, symbolFor } from "../symbols";
import { drawEquipment } from "./equipment";
import { drawEventLog } from "./eventLog";
import { drawGauge } from "./gauge";
import { drawImage } from "./image";
import { drawJunction } from "./junction";
import { drawGroup, drawLine, drawPageButton, drawPanel } from "./layout";
import { drawPlaceholder } from "./placeholder";
import { drawSlider } from "./slider";
import { drawSplitBar } from "./splitBar";
import { drawStatus } from "./status";
import { drawTable } from "./table";
import { drawText } from "./text";
import { drawTrend } from "./trend";
import { drawValue } from "./value";

type Drawer = (object: HmiObjectModel, frame: DrawFrame) => SvgNode;

const DRAWERS: Record<string, Drawer> = {
  "alarm-list": drawAlarmList,
  bar: drawBar,
  button: drawButton,
  "event-log": drawEventLog,
  gauge: drawGauge,
  group: drawGroup,
  image: drawImage,
  junction: drawJunction,
  line: drawLine,
  "mode-selector": drawModeSelector,
  "numeric-input": drawSetpoint,
  "page-button": drawPageButton,
  panel: drawPanel,
  "pulse-button": drawButton,
  selector: drawSelector,
  setpoint: drawSetpoint,
  slider: drawSlider,
  "split-bar": drawSplitBar,
  status: drawStatus,
  switch: drawSwitch,
  table: drawTable,
  text: drawText,
  trend: drawTrend,
  value: drawValue,
  // Every symbol is drawn as equipment (§14.3).
  ...Object.fromEntries(Object.keys(SYMBOLS).map((type) => [type, drawEquipment])),
};

/** Whether the drawing draws this type; the editor offers only those (§5.6). */
export function drawable(type: string): boolean {
  return type in DRAWERS || symbolFor(type) !== undefined;
}

export function drawObject(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  // The project's own symbols are drawn as equipment (§14.4).
  const draw = DRAWERS[object.type] ?? (symbolFor(object.type) ? drawEquipment : undefined);
  return draw ? draw(object, frame) : drawPlaceholder(object);
}
