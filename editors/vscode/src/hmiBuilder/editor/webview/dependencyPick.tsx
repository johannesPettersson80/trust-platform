import { useEffect, useState } from "react";
import type { HmiPageModel } from "../../drawing/model";
export interface DependencyPick {
    title: string;
    choices: string[];
    select: (path: string) => void;
}
export function useDependencyPick(page: HmiPageModel | undefined) {
    const [request, setRequest] = useState<DependencyPick>();
    const [error, setError] = useState<string>();
    const cancel = (): void => {
        setRequest(undefined);
        setError(undefined);
    };
    useEffect(() => {
        if (!request) return;
        const escape = (event: KeyboardEvent): void => {
            if (event.key === "Escape") {
                event.preventDefault();
                event.stopImmediatePropagation();
                cancel();
            }
        };
        window.addEventListener("keydown", escape, true);
        return () => window.removeEventListener("keydown", escape, true);
    }, [request]);
    return {
        request,
        begin: (pick: DependencyPick) => {
            setError(undefined);
            setRequest(pick);
        },
        choose: request
            ? (id: string | undefined) => {
                  const instance = page?.object.find((object) => object.id === id)?.instance;
                  if (!instance || !request.choices.some((choice) => choice.toLowerCase() === instance.toLowerCase())) {
                      setError("Choose one of the highlighted components.");
                      return;
                  }
                  request.select(instance);
                  cancel();
              }
            : undefined,
        ids: request
            ? page?.object
                  .filter(
                      (object) =>
                          object.instance &&
                          request.choices.some((choice) => choice.toLowerCase() === object.instance!.toLowerCase()),
                  )
                  .map((object) => object.id)
            : undefined,
        notice: request ? (
            <div
                className="hb-pick-notice"
                role="status"
                style={{ padding: "8px 12px", display: "flex", gap: 12, alignItems: "center" }}
            >
                <span>{error ?? `Choose ${request.title.toLowerCase()} on the page.`}</span>
                <button type="button" onClick={cancel}>
                    Cancel
                </button>
            </div>
        ) : null,
        cancel,
    };
}
