import { controllerAdapter } from "./controllerAdapters";
import { decodePidLaw } from "./controllerLaw";
import { pinnedControllerGains } from "./controllerSettingsPins";
import { proportionalParameter } from "./engineerVerificationPlan";
import { stateControllerPoles } from "./stateControllerDesign";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { ControllerFamily, ControllerGains } from "./controllerAdapters";
import type { ControllerDesign, ControllerTuningResult } from "./controllerTuning";

/** Materializes entered settings through the selected ST adapter; no tuning rule or runtime write. */
export function manualControllerCandidate(loop: ControlLoopDefinition, model: GeneralModel, selectedFamily: string,
  entered: Readonly<Record<string, number>>, pins?: Readonly<Record<string, number>>): ControllerTuningResult {
  const adapter = controllerAdapter(loop);
  const family = selectedFamily as ControllerFamily;
  if (!adapter.families.includes(family)) throw new Error("Select a compatible controller configuration before entering its settings.");
  const values = { ...entered };
  for (const [symbol, value] of Object.entries(pins ?? {})) {
    if (values[symbol] !== undefined && values[symbol] !== value) throw new Error(`Unpin ${symbol} before changing its setting.`);
    values[symbol] = value;
  }
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  let gains: ControllerGains;
  if (pid || !loop.controller.project && !loop.controller.stateFeedback && !loop.controller.onOff) {
    const required = pid ? [pid.proportionalGainSymbol,
      ...(["PI", "PID"].includes(family) ? [pid.integralGainSymbol] : []),
      ...(["PD", "PID"].includes(family) ? [pid.derivativeGainSymbol] : []),
      ...(pid.antiWindup && ["PI", "PID"].includes(family) ? [pid.antiWindup.trackingGainSymbol] : [])]
      : [proportionalParameter(loop).symbol];
    if (required.some(symbol => !Number.isFinite(values[symbol]))) throw new Error("Enter every required controller setting before tuning.");
    gains = pinnedControllerGains(loop, family, { proportionalGain: 0, integralGain: 0, derivativeGain: 0 },
      { family, loopRevision: loop.revision, values });
  } else {
    gains = { parameters: values };
  }
  const realized = adapter.materialize(gains, {});
  const design = {
    status: loop.controller.stateFeedback ? "exploratory" : "provisional",
    candidate: { ...gains, applied: false, structure: family },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` },
    actualSTVerify: true,
    scope: "Engineer-entered controller settings. Actual-ST validation determines whether the requirements pass.",
    ...(loop.controller.stateFeedback ? { closedLoopPoles: stateControllerPoles(loop, model, realized) } : {}),
  } as ControllerDesign;
  return { family, design, values: realized, reason: "Entered settings retained. Validate the actual controller before applying." };
}
