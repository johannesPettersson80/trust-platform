import type { AnalysisTool } from "../generated/AnalyzeRequest";
import type { PageAnalysisInputs } from "./threePageAnalysis";

/**
 * The entries of an analysis editor as the engine's inputs (specification 35, B9.9): each number, list and duration as the text the
 * engineer typed, and each choice as the option it names. The engine reads every entry: a blank entry is absent, so that the engine's
 * default applies, and an entry it cannot read is refused under its field. The page converts and drops nothing, so no typed entry is lost
 * on the way to the engine.
 */
export function readEditorInputs(editor: ParentNode, tool: AnalysisTool): PageAnalysisInputs {
  const text = (name: string): string => editor.querySelector<HTMLInputElement | HTMLSelectElement>(`[data-analysis-${name}]`)?.value ?? "";
  switch (tool) {
    case "rootLocus":
    case "routh":
      return { selectedValue: text("parameter-value") };
    case "frequency":
    case "feedback":
      return { grid: { minimumFrequency: text("minimum-frequency"), maximumFrequency: text("maximum-frequency"), points: text("frequency-points") } };
    case "measuredFrequency":
      // No recording is named here: the engine takes the study's last kept recording that meets the periodic rule (B9.9).
      return {
        frequencies: text("periodic-frequencies"),
        segmentCount: text("periodic-segments"),
        minimumCoherence: text("periodic-coherence"),
        startupSamples: text("periodic-startup"),
      };
    case "leadLag": {
      const mode = choiceOf(text("lead-lag-mode"), ["lead", "lag"] as const);
      return {
        mode,
        crossoverRadPerSecond: text("lead-lag-crossover"),
        value: text(mode === "lag" ? "lead-lag-ratio" : "lead-lag-phase"),
      };
    }
    case "initialState":
      return {
        stepSeconds: text("response-step"),
        durationSeconds: text("response-duration"),
        initialState: text("response-initial"),
        input: text("response-input"),
        connection: choiceOf(text("response-connection"), ["open-loop", "feedback"] as const),
      };
    case "poleZero":
      return { view: choiceOf(text("pole-view"), ["unreduced", "minimal"] as const) };
    default:
      return {};
  }
}

/** The one of `choices` that `text` names, or none. */
export function choiceOf<T extends string>(text: string, choices: readonly T[]): T | undefined {
  return choices.find(choice => choice === text);
}
