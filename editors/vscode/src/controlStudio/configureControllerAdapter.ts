import type { SetupDialogs } from "./setupPromptTypes";
import * as fs from "fs";
import * as vscode from "vscode";
import type { ControlLoopDefinition } from "./loopDefinition";
import { decodeParameterBinding } from "./parameterBinding";
import { controllerAdapter } from "./controllerAdapters";

/** Import an existing ST adapter, validate its scalar leaves against compilation, then let setup review/save the complete loop. */
export async function configureControllerAdapter(loop: ControlLoopDefinition,
  catalog: readonly { symbol: string; type: string }[], selectedTask?: string, ui: SetupDialogs = vscode.window): Promise<boolean> {
  const selected = await ui.showOpenDialog({ title: "Existing controller adapter mapping", canSelectMany: false, filters: { "Controller adapter JSON": ["json"] } });
  if (!selected?.[0]) return false;
  const file = await fs.promises.open(selected[0].fsPath, "r");
  let contents: string;
  try {
    const buffer = Buffer.alloc(256001);
    const { bytesRead } = await file.read(buffer, 0, buffer.length, 0);
    if (bytesRead > 256000) throw new Error("Controller adapter mappings are limited to 256 kB.");
    contents = buffer.subarray(0, bytesRead).toString("utf8");
  } finally { await file.close(); }
  const value: unknown = JSON.parse(contents);
  if (!record(value) || !record(value.controller) || !record(value.parameterBinding)) throw new Error("Provide controller and parameterBinding objects in the adapter mapping.");
  const parameterBinding = decodeParameterBinding({ ...value.parameterBinding, loop_id: loop.id }, loop.id);
  if (!parameterBinding || selectedTask && parameterBinding.task_name !== selectedTask) throw new Error("The controller adapter does not match the selected task/parameter contract.");
  const check = (symbol: string, type: string) => {
    if (!/^[\p{L}_][\p{L}\p{N}_]*(?:\.[\p{L}_][\p{L}\p{N}_]*)+$/u.test(symbol)
      || !catalog.some(candidate => candidate.symbol === symbol && candidate.type === type)) throw new Error(`Adapter field ${symbol} must be a compiled scalar member of type ${type}.`);
  };
  parameterBinding.parameters.forEach(parameter => check(parameter.symbol, parameter.iec_type));
  check(parameterBinding.mode.symbol, "BOOL");
  const fields = ["project", "stateFeedback", "onOff", "law", "pidInitialization", "manualOutputSignal", "manualOutputType", "manualOutputAddress"];
  const controller = { ...loop.controller, ...Object.fromEntries(fields.filter(field => field in value.controller!).map(field => [field, value.controller![field]])),
    design: "project" as const, modeSignal: parameterBinding.mode.symbol, modeWhenTrue: parameterBinding.mode.manual_value ? "Manual" as const : "Auto" as const };
  if (controller.manualOutputSignal && controller.manualOutputType) check(controller.manualOutputSignal, controller.manualOutputType);
  const configured: ControlLoopDefinition = { ...loop, controller, parameterBinding };
  const adapter = controllerAdapter(configured);
  try { adapter.transferBinding().states.forEach(state => check(state.symbol, state.iec_type)); }
  catch (error) {
    if (controller.project || controller.onOff || controller.stateFeedback?.initialization || controller.pidInitialization) throw error;
    // An explicitly absent Auto initialization contract still permits reviewed Manual staging.
  }
  loop.controller = configured.controller; loop.parameterBinding = parameterBinding;
  loop.signals.manipulatedVariable.direction = "write";
  loop.experiment = { execution: "control-studio", kind: "observation", amplitude: 0,
    durationSeconds: loop.experiment?.durationSeconds ?? 10, seed: loop.experiment?.seed ?? 1 };
  return true;
}
function record(value: unknown): value is Record<string, any> { return !!value && typeof value === "object" && !Array.isArray(value); }
