/**
 * The Identify page's Method list shows the method that the engine holds for the study (specification 35, B3.2). The page sends a chosen
 * method that the engine records (its offer says so with `records`) together with the limits as they stand, and the engine records it or
 * refuses it with the rule it breaks: a relay without its declared direction, or a missing duration, which is the Limits editor's
 * refusal. A refused change leaves the study's earlier method, and Identify runs that one, so the list shows the earlier method again
 * while the refusal's sentence stays on the page.
 *
 * This class decides no rule. It pairs each answer with the change it answers, and it puts the earlier method back in the list when
 * the engine refused the change. The engine answers the changes of the study inputs in the order the page sent them
 * (`engine/routes/inputsRoute.ts`), so an answer belongs to the oldest change that waits. A method that the engineer has replaced by
 * another choice since is not put back: the answer to the newer choice decides.
 */
export class ThreePageMethod {
  private recorded: string | undefined;
  /** What each change that waits for its answer asked the study to record, oldest first: a method, or `undefined` for a change of another input. */
  private readonly waiting: Array<string | undefined> = [];

  constructor(private readonly list: HTMLSelectElement,
    /** Shows the sections of the method that the list shows again. */
    private readonly shown: () => void,
    /** Keeps the method that the list shows as the study's page view (`persistView`). */
    private readonly kept: () => void) {}

  /** The method that the study holds, as the list names it: the open answer's `heldInputs.identifyMethod`, or the list's first method for a study that records none. */
  holds(method: string | undefined): void {
    if (method !== undefined) this.recorded = method;
  }

  /** The page sent a change of the study inputs; `asked` is the method it asks the study to record, when it asks for one. */
  posted(asked?: string): void {
    this.waiting.push(asked);
  }

  /** A loop opens: no change of an earlier open waits for its answer any more. */
  reset(): void {
    this.waiting.length = 0;
  }

  /**
   * The engine answered the oldest change that waits. `refused` says it left the study as it was, and `limits` that the refusal is the Limits
   * editor's: that editor reopens, and its Done sends the method again with the corrected limits, so the list keeps the method it shows. An
   * answer that is no refusal says the engine recorded the method the change asked for.
   */
  answered(refused: boolean, limits: boolean): void {
    const asked = this.waiting.shift();
    if (!refused) {
      if (asked !== undefined) this.recorded = asked;
      return;
    }
    if (limits || asked === undefined || this.list.value !== asked) return;
    const recorded = this.recorded;
    if (recorded === undefined || recorded === asked || !Array.from(this.list.options).some(option => option.value === recorded)) return;
    this.list.value = recorded;
    this.shown();
    this.kept();
  }
}
