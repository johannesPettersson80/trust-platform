import type { PageData } from "../display/runtimeDisplay";
import type { DrawFrame, HmiPageModel } from "../drawing/model";
export interface ExplorationModel {
  pages: Record<string, PageData>;
  popups: Record<string, { name: string; model: HmiPageModel & { size: [number, number] } }>;
  values: DrawFrame["values"];
  assets: Record<string, string>;
  nowMs: number;
  alarms?: import("../drawing/model").AlarmFrame;
}
