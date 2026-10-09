import { equipmentConnectionSummary, missingSignalHelp } from "../equipmentConnectionsModel";
import { useEffect, useRef, useState } from "react";
import type { EquipmentDetails } from "../../hmiBuilder/editor/equipmentDetails";

export interface EquipmentConnectionState {
    instance: string;
    details?: EquipmentDetails;
    error?: string;
    result?: { sequence: number; error?: string };
}
export function EquipmentConnections({
    state,
    post,
    onClose,
    onBrowse,
}: {
    state: EquipmentConnectionState;
    post: (message: unknown) => void;
    onClose: () => void;
    onBrowse: () => void;
}) {
    const data = state.details;
    const [draft, setDraft] = useState<Record<string, string>>({});
    const [busy, setBusy] = useState(false);
    const changed = useRef(new Set<string>()),
        revision = useRef(0),
        refreshing = useRef(false);
    useEffect(() => {
        setBusy(false);
        if (state.result && !state.result.error) {
            changed.current.clear();
            refreshing.current = true;
        }
    }, [state.result]);
    useEffect(() => {
        if (!data || (changed.current.size && !refreshing.current)) return;
        setDraft((current) =>
            Object.fromEntries(
                (data.terminals ?? []).map((terminal) => [
                    terminal.member,
                    changed.current.has(terminal.member) ? current[terminal.member] : (terminal.value ?? ""),
                ]),
            ),
        );
        revision.current = data.revision;
        refreshing.current = false;
    }, [data, state.result]);
    return (
        <aside
            className="trust-inspector"
            aria-label="Equipment connections"
            style={{ position: "absolute", top: 0, right: 0, bottom: 0, width: 360, zIndex: 8, overflow: "auto" }}
        >
            <div className="trust-inspector__header">
                <div className="trust-inspector__title" style={{ flex: 1 }}>
                    {state.instance.split(".").pop()}
                </div>
                <button
                    className="trust-button"
                    type="button"
                    aria-label="Close equipment connections"
                    onClick={onClose}
                >
                    ×
                </button>
            </div>
            <section className="trust-section">
                <h3 className="trust-section__title">Hardware connections</h3>
                <p className="trust-help">
                    {data ? equipmentConnectionSummary(data) : (state.error ?? "Reading equipment connections…")}
                </p>
                {data
                    ? (data.terminals ?? []).map((terminal) => {
                          const value = draft[terminal.member] ?? terminal.value ?? "";
                          return (
                              <label key={terminal.member} style={{ display: "grid", gap: 6, marginTop: 16 }}>
                                  <span>
                                      {terminal.title}
                                      {!terminal.required ? " (optional)" : ""}
                                  </span>
                                  <select
                                      aria-label={terminal.title}
                                      value={value}
                                      disabled={busy || !data.owned}
                                      onChange={(event) => {
                                          changed.current.add(terminal.member);
                                          setDraft((current) => ({
                                              ...current,
                                              [terminal.member]: event.target.value,
                                          }));
                                      }}
                                  >
                                      <option value="">{`Choose ${terminal.direction}…`}</option>
                                      {value && !terminal.choices.some((choice) => choice.path === value) ? (
                                          <option value={value}>{value} — unavailable</option>
                                      ) : null}
                                      {terminal.choices.map((choice) => (
                                          <option key={choice.path} value={choice.path}>
                                              {choice.path}
                                              {choice.address ? ` · ${choice.address}` : ""}
                                          </option>
                                      ))}
                                  </select>
                                  {terminal.error ? <small role="alert">{terminal.error}</small> : null}
                                  {!terminal.choices.length ? (
                                      <small className="trust-help">
                                          {missingSignalHelp(state.instance, terminal)}
                                      </small>
                                  ) : null}
                              </label>
                          );
                      })
                    : null}
                {data && !data.owned ? (
                    <p className="trust-help">
                        This equipment is declared in your PLC code; connect it there. Its connections are shown for
                        reference.
                    </p>
                ) : null}
                {state.result?.error ? (
                    <p role="alert" className="trust-help">
                        {state.result.error}
                    </p>
                ) : null}
                <div style={{ display: "flex", gap: 8, marginTop: 20, flexWrap: "wrap" }}>
                    {data?.owned ? (
                        <button
                            className="trust-button trust-button--primary"
                            disabled={busy}
                            onClick={() => {
                                if (!changed.current.size) return;
                                setBusy(true);
                                post({
                                    type: "equipmentHardwareApply",
                                    instance: state.instance,
                                    revision: revision.current,
                                    signals: Object.fromEntries(
                                        [...changed.current].map((key) => [key, draft[key] ?? ""]),
                                    ),
                                });
                            }}
                        >
                            {busy ? "Applying…" : "Apply connections"}
                        </button>
                    ) : null}
                    <button
                        className="trust-button"
                        disabled={busy}
                        onClick={() => {
                            refreshing.current = true;
                            post({ type: "equipmentHardwareRefresh" });
                        }}
                    >
                        Refresh
                    </button>
                </div>
                <p className="trust-help" style={{ marginTop: 16 }}>
                    Assign device channels in the device settings. These are project connections; deployment and
                    checking the real wiring are separate steps.
                </p>
                <button className="trust-button" disabled={busy} onClick={onBrowse}>
                    Show devices
                </button>
                <button className="trust-button" onClick={() => post({ type: "equipmentHardwareReturn" })}>
                    Back to HMI
                </button>
            </section>
        </aside>
    );
}

export function useEquipmentConnections(post: (message: unknown) => void) {
    const [state, setState] = useState<EquipmentConnectionState>();
    const [visible, setVisible] = useState(false);
    useEffect(() => {
        const receive = (event: MessageEvent): void => {
            const message = event.data;
            if (message?.type === "equipmentHardwareFocus" && typeof message.instance === "string") {
                setState((current) =>
                    current?.instance === message.instance ? current : { instance: message.instance },
                );
                setVisible(true);
            }
            if (message?.type === "equipmentHardwareDetails")
                setState((current) =>
                    current && current.instance === message.instance
                        ? { ...current, details: message.details, error: message.error }
                        : current,
                );
            if (message?.type === "equipmentHardwareConfigured")
                setState((current) =>
                    current && current.instance === message.instance
                        ? {
                              ...current,
                              result: { sequence: (current.result?.sequence ?? 0) + 1, error: message.error },
                          }
                        : current,
                );
        };
        window.addEventListener("message", receive);
        return () => window.removeEventListener("message", receive);
    }, []);
    return {
        state,
        visible,
        setVisible,
        close: () => {
            setState(undefined);
            setVisible(false);
            post({ type: "equipmentHardwareClose" });
        },
    };
}
