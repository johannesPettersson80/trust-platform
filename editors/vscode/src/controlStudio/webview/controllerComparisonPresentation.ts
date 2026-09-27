import type { SavedEngineerFamilyEvaluation } from "../engineerStudyPersistence";
import type { AutomaticControllerAttempt } from "../automaticControllerRecommendation";

/** Renders evidence only; application remains owned by the verified workflow. */
export function renderControllerComparison(host: HTMLElement, attempts: readonly AutomaticControllerAttempt[]): void {
  const doc = host.ownerDocument;
  const list = doc.createElement("dl");
  for (const attempt of attempts) {
    const name = doc.createElement("dt");
    name.textContent = `${attempt.structure} · ${attempt.status === "Rejected" ? "Not suitable" : "Candidate"}`;
    const reason = doc.createElement("dd");
    reason.textContent = attempt.reason;
    list.append(name, reason);
  }
  host.append(list);
}

export function renderHistoricalControllerComparison(host: HTMLElement, attempts: readonly SavedEngineerFamilyEvaluation[]): void {
  const doc = host.ownerDocument;
  const heading = doc.createElement("p"); heading.textContent = "Historical controller comparison — verify again before Apply.";
  const list = doc.createElement("dl");
  const labels = { "actual-pass": "Passed", "actual-fail": "Failed", "model-qualified": "Prediction only", "unavailable": "Not suitable" };
  for (const attempt of attempts) {
    const name = doc.createElement("dt"); name.textContent = `${attempt.family} · ${labels[attempt.status]}`;
    const reason = doc.createElement("dd"); reason.textContent = attempt.reason;
    list.append(name, reason);
  }
  host.append(heading, list);
}
