import { formatEngineeringNumber } from "../chartContract";
import type { ControlLoopDefinition } from "../loopDefinition";
import type { RelayCycleEvidence } from "../relayCycleEvidence";
import { displayEngineeringUnit } from "../unitDisplay";

/** The identified model's relay-cycle results, and the relay rule they make available in Tune.
 * They change only when a model is identified, selected or restored; Validate keeps them. */
export function renderRelayCycle(root: HTMLElement, loop: ControlLoopDefinition | null | undefined, payload: Record<string, unknown>): void {
  if (!payload.model || typeof payload.model !== "object" || (payload.status !== "model-ready" && payload.status !== "restored")) return;
  const relay = payload.relayCycle as RelayCycleEvidence | undefined;
  const line = root.querySelector<HTMLElement>("[data-studio-relay-cycle]");
  if (line) {
    const output = displayEngineeringUnit(loop?.signals.manipulatedVariable.unit ?? ""), measured = displayEngineeringUnit(loop?.signals.processVariable.unit ?? "");
    const number = (value: number) => formatEngineeringNumber(value);
    line.hidden = !relay;
    line.textContent = !relay ? "" : relay.status === "qualified"
      ? `Relay cycle (${relay.cycles} cycles): d = ${number(relay.outputAmplitude)} ${output}, a = ${number(relay.responseAmplitude)} ${measured}, `
        + `Pu = ${number(relay.periodSeconds)} s, Ku = 4d/(πa) = ${number(relay.ultimateGain)} ${output}/${measured}`
      : `Relay cycles not usable: ${relay.reason}`;
  }
  const method = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]");
  const option = method?.querySelector<HTMLOptionElement>('option[value="relay"]');
  if (!method || !option) return;
  const available = relay?.status === "qualified" && loop?.controller.law?.kind === "pid";
  option.hidden = option.disabled = !available;
  if (!available && method.value === "relay") {
    method.value = "automatic";
    method.dispatchEvent(new (root.ownerDocument.defaultView ?? window).Event("change"));
  }
}
