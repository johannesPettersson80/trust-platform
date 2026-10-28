// The webview's questions to the language server's HMI Builder rules (AGENTS.md: all logic is in
// Rust; specification 39, §17.3). Each goes to the editor as a `rust` message, and its answer comes
// back as `rustAnswer`; the webview shows what the answers say and keeps no rule of its own.

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { MutableRefObject } from "react";

import type { HmiObjectModel, HmiPageModel } from "../../drawing/model";
import type { CallRust, HostToWebview } from "../messages";
import type { VsCodeApi } from "./vscodeApi";

/** Questions sent through the editor, and its answers taken back. */
export interface RustChannel {
  call: CallRust;
  receive(message: Extract<HostToWebview, { type: "rustAnswer" }>): void;
}

export function rustChannel(api: VsCodeApi): RustChannel {
  let next = 0;
  const waiting = new Map<number, { resolve: (answer: unknown) => void; reject: (reason: Error) => void }>();
  return {
    call: <T>(command: string, argument: unknown) =>
      new Promise<T>((resolve, reject) => {
        next += 1;
        waiting.set(next, { resolve: resolve as (answer: unknown) => void, reject });
        api.postMessage({ type: "rust", id: next, command, argument });
      }),
    receive(message) {
      const asked = waiting.get(message.id);
      if (!asked) return;
      waiting.delete(message.id);
      const answer = message.answer as { ok?: unknown; error?: unknown } | null | undefined;
      if (answer?.ok === true) {
        asked.resolve(answer);
        return;
      }
      const reason = typeof answer?.error === "string" ? answer.error : "The language server gave no answer.";
      asked.reject(new Error(reason));
    },
  };
}

/** Why a question was not answered, in words. */
export function refusalText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** A change of the page made from an answer: the page it becomes, and what is selected then. */
export interface PageChange {
  page: HmiPageModel;
  select?: string[];
}

/** Changes the page from an answer about it: `ask` is given the page as it is and answers the page
 * it becomes, or nothing. The change is made while the page is still the one asked about; a page
 * changed meanwhile is asked about again. */
export type ChangePage = (ask: (page: HmiPageModel) => Promise<PageChange | undefined>) => void;

export interface RustRules {
  call: CallRust;
  change: ChangePage;
}

export const Rust = createContext<RustRules>({
  call: () => Promise.reject(new Error("The HMI Builder is not connected to its editor.")),
  change: () => undefined,
});

export function useRust(): RustRules {
  return useContext(Rust);
}

/** The page editor's rules: `latest` holds the page as it is and the commit that changes it. */
export function useRustRules(
  call: CallRust,
  latest: MutableRefObject<{ page?: HmiPageModel; commit: (page: HmiPageModel) => void }>,
  select: (ids: string[]) => void,
  report: (text: string) => void,
): RustRules {
  const change = useCallback<ChangePage>(
    (ask) => {
      const attempt = (tries: number): void => {
        const asked = latest.current.page;
        if (!asked) return;
        ask(asked).then(
          (made) => {
            if (latest.current.page !== asked) {
              if (tries < 3) attempt(tries + 1);
              return;
            }
            if (!made) return;
            latest.current.commit(made.page);
            if (made.select) select(made.select);
          },
          (error: unknown) => report(refusalText(error)),
        );
      };
      attempt(0);
    },
    [latest, select, report],
  );
  return useMemo(() => ({ call, change }), [call, change]);
}

/** The page with one object replaced by its changed copy. */
export function withObject(page: HmiPageModel, object: HmiObjectModel): HmiPageModel {
  return { ...page, object: page.object.map((candidate) => (candidate.id === object.id ? object : candidate)) };
}

/** Whether two keys have the same items. */
function sameKey(a: readonly unknown[], b: readonly unknown[]): boolean {
  return a.length === b.length && a.every((item, index) => Object.is(item, b[index]));
}

/** The answer to a question that depends only on `key`, compared item by item: undefined until it
 * arrives, or with `keep` the last answer meanwhile. `ask` answers nothing when there is nothing to
 * ask; a refused question answers nothing either. */
export function useRustAnswer<T>(
  call: CallRust,
  key: readonly unknown[],
  ask: (call: CallRust) => Promise<T> | undefined,
  keep = false,
): T | undefined {
  const [held, setHeld] = useState<{ key: readonly unknown[]; answer: T }>();
  // The same items keep the key asked about, so a render that builds the key anew asks nothing.
  const asked = useRef<readonly unknown[]>(key);
  if (!sameKey(asked.current, key)) asked.current = key;
  const current = asked.current;
  const question = useRef(ask);
  question.current = ask;
  useEffect(() => {
    const pending = question.current(call);
    if (!pending) return;
    let live = true;
    pending.then(
      (answer) => {
        if (live) setHeld({ key: current, answer });
      },
      () => {
        if (live) setHeld(undefined);
      },
    );
    return () => {
      live = false;
    };
  }, [call, current]);
  if (held && held.key === current) return held.answer;
  return keep ? held?.answer : undefined;
}
