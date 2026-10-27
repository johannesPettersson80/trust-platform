// The session on the runtime display (specification 39, §12.4, §12.8): who is logged in, what the
// runtime says the session may do, the session bar with Log in or Log out, the login dialog, and
// what happens when the session ends.

import type { DisplayHost } from "./runtimeDisplay";
import { button, element, labelledInput, Overlay } from "./overlay";
import { say } from "../drawing/speech";

/** A session as the runtime reports it, with what it may do (§12.4 item 4). */
export interface Session {
  user: string;
  role: string;
  /** The roles it may act as; a control whose needed role is not among them is locked. */
  acts_as?: string[];
  /** Whether it may change settings (§21.4 item 1). */
  may_change_settings?: boolean;
}

export class SessionControl {
  private session: Session | undefined;

  constructor(
    private readonly host: DisplayHost,
    private readonly overlay: Overlay,
    /** After a login or logout: the page redraws and its stream opens again with the session. */
    private readonly changed: () => void,
    /** Where the session bar goes: the top bar, or by default the top right of the page. */
    private readonly slot?: HTMLElement,
  ) {}

  /** Reads the session the page opened with. */
  async start(): Promise<void> {
    try {
      const answer = await this.host.fetchJson("/api/hmi/session");
      this.session = answer.status === 200 ? sessionOf(answer.body) : undefined;
    } catch {
      this.session = undefined;
    }
    this.showBar();
  }

  /** Without a session the controls cannot be used (§12.4). */
  get readOnly(): boolean {
    return this.session === undefined;
  }

  /** The session's role, if there is a session. */
  get role(): string | undefined {
    return this.session?.role;
  }

  /** The roles the session may act as, as the runtime says; none without a session. */
  get actsAs(): readonly string[] {
    return this.session?.acts_as ?? [];
  }

  /** Whether the session may change settings, as the runtime says (§21.4 item 1). */
  get mayChangeSettings(): boolean {
    return this.session?.may_change_settings === true;
  }

  /** The session the stream reports; only a session that ends asks to log in again. */
  receive(session: Session | null | undefined): void {
    const had = this.session !== undefined;
    this.session = sessionOf(session);
    this.showBar();
    if (had && !this.session) this.showLogin(say("Your session ended. Log in to operate."));
  }

  showLogin(message: string, peer?: string): void {
    const form = this.overlay.dialog("form", "login");
    form.append(element(form, "h2", peer ? `${say("Log in")} · ${peer}` : say("Log in")));
    form.append(element(form, "p", message, "hmi-hint"));
    const user = labelledInput(form, say("User"), "user", "username");
    const password = labelledInput(form, say("Password"), "password", "current-password");
    password.type = "password";
    const error = element(form, "p", "", "hmi-error");
    form.append(error);
    const actions = element(form, "div", "", "hmi-actions");
    const cancel = button(form, say("Not now"), "cancel");
    cancel.type = "button";
    cancel.addEventListener("click", () => this.overlay.closeDialog());
    const submit = button(form, say("Log in"));
    submit.type = "submit";
    actions.append(cancel, submit);
    form.append(actions);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.login(user.value, password.value, error, peer);
    });
    user.focus();
  }

  private async login(user: string, password: string, error: HTMLElement, peer?: string): Promise<void> {
    try {
      const answer = await this.host.postJson("/api/hmi/login", { user, password, ...(peer ? { peer } : {}) });
      const session = answer.status === 200 ? sessionOf(answer.body) : undefined;
      if (!session) {
        error.textContent = (answer.body as { error?: string } | null)?.error ?? say("The login was refused.");
        return;
      }
      if (!peer) this.session = session;
      this.overlay.closeDialog();
      this.showBar();
      this.changed();
    } catch {
      error.textContent = say("The runtime cannot be reached.");
    }
  }

  private async logout(): Promise<void> {
    try {
      await this.host.postJson("/api/hmi/logout", {});
    } catch {
      // The session ends here either way; the runtime's ends with its idle time.
    }
    this.session = undefined;
    this.showBar();
    this.changed();
  }

  /** The session bar: the user and role with Log out, or Log in (§12.4). */
  private showBar(): void {
    const home = this.slot ?? this.overlay.layer;
    home.querySelector('[data-part="session"]')?.remove();
    const bar = element(home, "div", "", this.slot ? "hmi-session hmi-session-inline" : "hmi-session");
    bar.setAttribute("data-part", "session");
    if (this.session) {
      bar.append(
        element(bar, "span", this.session.user, "hmi-user"),
        element(bar, "span", roleName(this.session.role), "hmi-role"),
      );
      const out = button(bar, say("Log out"), "logout");
      out.addEventListener("click", () => void this.logout());
      bar.append(out);
    } else {
      const login = button(bar, say("Log in"), "login");
      login.addEventListener("click", () => this.showLogin(say("Log in to operate.")));
      bar.append(login);
    }
    if (this.slot) home.append(bar);
    else home.prepend(bar);
  }
}

/** A session from a login, session or stream answer, with the facts the runtime gives it. */
function sessionOf(body: unknown): Session | undefined {
  const session = body as { user?: unknown; role?: unknown; acts_as?: unknown; may_change_settings?: unknown } | null;
  if (typeof session?.user !== "string" || typeof session.role !== "string") return undefined;
  const actsAs = Array.isArray(session.acts_as)
    ? session.acts_as.filter((role): role is string => typeof role === "string")
    : [];
  return {
    user: session.user,
    role: session.role,
    acts_as: actsAs,
    may_change_settings: session.may_change_settings === true,
  };
}

function roleName(role: string): string {
  return role.charAt(0).toUpperCase() + role.slice(1);
}
