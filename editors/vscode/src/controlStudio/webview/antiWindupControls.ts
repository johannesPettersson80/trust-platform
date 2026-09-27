import { initialAntiWindupPresentation, reduceAntiWindupPresentation, type AntiWindupPresentation } from "../antiWindupPresentation";

interface AntiWindupControlsDependencies {
  post(kind: string, payload?: Record<string, unknown>): void;
  setText(selector: string, value: unknown): void;
}

/** Owns the visible Review → Stage → Restore actions following a Test record. */
export function initializeAntiWindupControls(dependencies: AntiWindupControlsDependencies): {
  receive(phase: string, payload: Record<string, unknown> | undefined): void;
  clear(): void;
} {
  let presentation = initialAntiWindupPresentation();
  const actions = document.querySelector<HTMLElement>("[data-anti-windup-actions]");
  const review = document.querySelector<HTMLButtonElement>("[data-anti-windup-review]");
  const stage = document.querySelector<HTMLButtonElement>("[data-anti-windup-verify]");
  const rollback = document.querySelector<HTMLButtonElement>("[data-anti-windup-rollback]");
  const render = (): void => {
    if (actions) actions.hidden = !presentation.visible;
    if (review) review.hidden = !presentation.reviewVisible;
    if (stage) stage.hidden = !presentation.stageVisible;
    if (rollback) rollback.hidden = !presentation.rollbackVisible;
    dependencies.setText("[data-anti-windup-status]", presentation.status);
  };
  review?.addEventListener("click", () => {
    const trackingTimeSeconds = document.querySelector<HTMLInputElement>("[data-anti-windup-tt]")?.valueAsNumber;
    if (!Number.isFinite(trackingTimeSeconds) || trackingTimeSeconds! <= 0) {
      dependencies.setText("[data-anti-windup-status]", "Enter a positive tracking time.");
      return;
    }
    dependencies.post("control-studio.anti-windup.review", { trackingTimeSeconds: trackingTimeSeconds! });
    dependencies.setText("[data-anti-windup-status]", "Reviewing the complete tracked candidate…");
  });
  stage?.addEventListener("click", () => dependencies.post("control-studio.anti-windup.verify"));
  rollback?.addEventListener("click", () => dependencies.post("control-studio.anti-windup.rollback"));
  return {
    receive(phase, payload): void {
      presentation = reduceAntiWindupPresentation(presentation, phase, payload);
      render();
    },
    clear(): void {
      presentation = initialAntiWindupPresentation();
      render();
    },
  };
}
