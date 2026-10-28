import type { SetupQuestion } from "./generated/SetupStepResult";

/**
 * What the engineer did at one question of the step-by-step setup: the answer to send back to the engine, Back to the question before, or
 * Cancel of the whole setup (which writes nothing). The engine reads every answer; the presenter only collects it.
 */
export type SetupQuestionResult =
  | { readonly kind: "answered"; readonly answer: unknown }
  | { readonly kind: "back"; readonly text?: unknown }
  | { readonly kind: "cancelled" };

/** What the page shows for one question: the answer the engineer gave it before, the engine's refusal of the last answer, and whether Back is offered. */
export interface SetupQuestionContext {
  readonly previous?: unknown;
  readonly refusal?: string;
  readonly canGoBack: boolean;
}

/** The native presentation of the engine's questions. It shows each question as the engine names it and returns the answer as typed or chosen. */
export interface SetupPromptPresenter {
  ask(question: SetupQuestion, context: SetupQuestionContext): Promise<SetupQuestionResult>;
}
