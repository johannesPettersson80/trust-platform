import { Exploration } from "./Exploration";
import { useEffect, useRef, useState } from "react";
import type { HostToWebview } from "../messages";
import type { ProposalModel, ProposalStatus } from "../proposalModel";
import type { VsCodeApi } from "./vscodeApi";
import "./proposalReview.css";

/** Read-only review mode of the existing builder. Images come from the shared page renderer. */
export function ProposalReview({ api, fontUrl }: { api: VsCodeApi; fontUrl?: string }) {
    const [proposal, setProposal] = useState<ProposalModel>();
    const [active, setActive] = useState(false);
    const [status, setStatus] = useState<ProposalStatus>();
    const [message, setMessage] = useState("");
    const [busy, setBusy] = useState(false);
    const [explore, setExplore] = useState(false);
    const [index, setIndex] = useState(0);
    const [view, setView] = useState<"before" | "after">("after");
    const [zoom, setZoom] = useState(0.75);
    const viewport = useRef<HTMLDivElement>(null);
    useEffect(() => {
        const receive = (event: MessageEvent<HostToWebview>) => {
            if (event.data.type === "proposal") {
                setProposal(event.data.proposal);
                setActive(event.data.active);
                setStatus(event.data.active ? "ready" : undefined);
            } else if (event.data.type === "proposalStatus") {
                setStatus(event.data.status);
                setMessage(event.data.message ?? "");
                setBusy(false);
                setActive(event.data.status === "ready" || event.data.status === "error");
            }
        };
        window.addEventListener("message", receive);
        api.postMessage({ type: "ready" });
        return () => window.removeEventListener("message", receive);
    }, [api]);
    if (!proposal)
        return (
            <main className="hb-proposal-loading" role="status">
                Preparing proposal…
            </main>
        );
    const file = proposal.files[index];
    const fit = () => {
        const area = viewport.current;
        if (area)
            setZoom(
                Math.max(
                    0.1,
                    Math.min(1, (area.clientWidth - 48) / file.width, (area.clientHeight - 48) / file.height),
                ),
            );
    };
    const apply = () => {
        setBusy(true);
        api.postMessage({ type: "proposalApply" });
    };
    const discard = () => {
        setBusy(true);
        api.postMessage({ type: "proposalDiscard" });
    };
    const explanation =
        message ||
        (status === "applied"
            ? "Applied to the editor. Save when ready."
            : status === "discarded"
              ? "This proposal was discarded. Source files were not changed."
              : status === "stale"
                ? "This draft changed. Review its current revision before applying."
                : !proposal.engineeringValid
                  ? "Resolve the engineering errors before applying."
                  : "Source files remain unchanged until you apply this proposal.");
    return (
        <main className="hb-app hb-proposal" data-proposal-review>
            <header className="hb-topbar hb-proposal-heading">
                <div>
                    <span className="hb-proposal-eyebrow">HMI proposal · Revision {proposal.revision}</span>
                    <h1>{file.title}</h1>
                </div>
                <span className="hb-proposal-badge">Synthetic preview</span>
                <div className="hb-proposal-actions">
                    <button
                        type="button"
                        data-proposal-discard
                        disabled={busy || !active || status === "discarded" || status === "applied"}
                        onClick={discard}
                    >
                        Discard
                    </button>
                    <button
                        type="button"
                        className="trust-button trust-button--primary"
                        data-proposal-apply
                        disabled={busy || !active || !proposal.engineeringValid}
                        onClick={apply}
                    >
                        {busy ? "Working…" : status === "applied" ? "Applied" : "Apply to editor"}
                    </button>
                </div>
            </header>
            <div className="hb-topbar hb-proposal-toolbar">
                {proposal.files.length > 1 ? (
                    <select
                        aria-label="Proposal page"
                        value={index}
                        onChange={(event) => {
                            setIndex(Number(event.target.value));
                            setView("after");
                        }}
                    >
                        {proposal.files.map((item, position) => (
                            <option key={item.file} value={position}>
                                {item.title}
                            </option>
                        ))}
                    </select>
                ) : (
                    <span className="hb-proposal-file">{file.file}</span>
                )}
                <div role="tablist" aria-label="Compare page">
                    <button
                        type="button"
                        role="tab"
                        data-proposal-view="before"
                        aria-selected={view === "before"}
                        disabled={!file.before}
                        title={file.before ? "Captured source page" : "This is a new page"}
                        onClick={() => setView("before")}
                    >
                        Before
                    </button>
                    <button
                        type="button"
                        role="tab"
                        data-proposal-view="after"
                        aria-selected={view === "after"}
                        onClick={() => setView("after")}
                    >
                        After
                    </button>
                </div>
                <button
                    type="button"
                    aria-label="Zoom out"
                    onClick={() => setZoom((value) => Math.max(0.1, value - 0.25))}
                >
                    −
                </button>
                <span>{Math.round(zoom * 100)}%</span>
                <button
                    type="button"
                    aria-label="Zoom in"
                    onClick={() => setZoom((value) => Math.min(4, value + 0.25))}
                >
                    +
                </button>
                <button type="button" onClick={fit}>
                    Fit
                </button>
                {proposal.exploration ? (
                    <button type="button" aria-pressed={explore} onClick={() => setExplore(!explore)}>
                        {explore ? "Return to images" : "Explore navigation"}
                    </button>
                ) : null}
                {status === "applied" ? (
                    <button type="button" onClick={() => api.postMessage({ type: "proposalOpenSource", index })}>
                        Open page
                    </button>
                ) : null}
            </div>
            <div className="hb-proposal-body">
                <div
                    className="hb-proposal-viewport"
                    ref={viewport}
                    aria-label={`${view === "before" ? "Before" : "After"} preview`}
                >
                    {explore && proposal.exploration ? (
                        <Exploration
                            model={proposal.exploration}
                            fontUrl={fontUrl}
                            initial={file.file
                                .split("/")
                                .pop()!
                                .replace(/\.toml$/, "")}
                        />
                    ) : (
                        <img
                            data-proposal-image
                            src={`data:image/png;base64,${view === "before" ? file.before : file.after}`}
                            alt={`${file.title}, ${view} the proposed changes. Synthetic values.`}
                            style={{ width: file.width * zoom, height: file.height * zoom }}
                        />
                    )}
                </div>
                <aside className="hb-proposal-summary" aria-label="Proposed changes">
                    <p className="hb-proposal-explanation" role="status">
                        {explanation}
                    </p>
                    <h2>Changes</h2>
                    <ul>
                        {file.changes.added.map((id) => (
                            <li key={`add:${id}`}>
                                Add <strong>{id}</strong>
                            </li>
                        ))}
                        {file.changes.removed.map((id) => (
                            <li key={`remove:${id}`}>
                                Remove <strong>{id}</strong>
                            </li>
                        ))}
                        {file.changes.modified.map((change) => (
                            <li key={change.id}>
                                <strong>{change.id}</strong>
                                <span>{change.properties.join(", ")}</span>
                            </li>
                        ))}
                        {file.changes.pageProperties.length ? (
                            <li>Page: {file.changes.pageProperties.join(", ")}</li>
                        ) : null}
                        {file.changes.orderChanged ? <li>Layer order changed</li> : null}
                    </ul>
                    <h2>Checks</h2>
                    <p>
                        {proposal.engineeringValid ? "Engineering checks passed" : "Engineering errors need attention"}
                    </p>
                    {proposal.findings.length ? (
                        <ul>
                            {proposal.findings.map((finding, position) => (
                                <li key={position}>
                                    {finding.object ? <strong>{finding.object}</strong> : null}
                                    <span>{finding.message ?? "Check the proposed file"}</span>
                                </li>
                            ))}
                        </ul>
                    ) : (
                        <p>No reported findings. Inspect the design before applying.</p>
                    )}
                    {proposal.intent ? (
                        <section aria-label="Design intent">
                            <h2>
                                {proposal.intent.taskSource === "request" ? "Operator task" : "Suggested operator task"}
                            </h2>
                            <p>{proposal.intent.operatorTask}</p>
                            <p>{proposal.intent.rationale}</p>
                            <h2>Assumptions</h2>
                            <ul>
                                {proposal.intent.assumptions.map((assumption, position) => (
                                    <li key={position}>{assumption}</li>
                                ))}
                            </ul>
                        </section>
                    ) : null}
                </aside>
            </div>
        </main>
    );
}
