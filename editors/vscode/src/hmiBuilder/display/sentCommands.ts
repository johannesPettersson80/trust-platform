// What sent each of a surface's newest commands, so that a command's later outcome on the value
// stream reaches the control or the alarm action that sent it (specification 39, §12.5 item 6).

/** The most commands a surface remembers for their later outcomes. A later outcome arrives within
 * its command's confirmation and pulse times, long before a page could send this many more. */
export const REMEMBERED_COMMANDS = 1024;

/** The sender of each of the newest {@link REMEMBERED_COMMANDS} commands, by command id. A page
 * stays open for as long as the operator works, so the oldest command is forgotten first. */
export class SentCommands<T> {
  private readonly sent = new Map<string, T>();

  /** Remembers that `from` sent the command `id`. */
  add(id: string, from: T): void {
    this.sent.set(id, from);
    if (this.sent.size > REMEMBERED_COMMANDS) {
      const oldest = this.sent.keys().next();
      if (!oldest.done) this.sent.delete(oldest.value);
    }
  }

  /** What sent the command `id`, while it is remembered. */
  from(id: string): T | undefined {
    return this.sent.get(id);
  }
}
