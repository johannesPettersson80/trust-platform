/** Only the owned editor transaction can certify that applyEdit was never invoked. Ordinary
 * exceptions, including an editor refusal after invocation, must retain the uncertain intent. */
export class NoSourceChangeError extends Error {
  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause));
    this.name = "NoSourceChangeError";
  }
}
