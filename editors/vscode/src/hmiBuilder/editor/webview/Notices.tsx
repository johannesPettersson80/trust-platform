import type { FileError } from "../pageDocument";
import type { DeployOutcome } from "../deploy";

export function Banner({ heading, errors }: { heading: string; errors: FileError[] }) {
    return (
        <div data-part="errors" className="hb-banner" role="alert">
            <strong>{heading}</strong>
            {errors.map((error, index) => (
                <div key={index} className="hb-banner-line">
                    {error.file ? `${error.file}${error.line ? `:${error.line}` : ""}: ` : ""}
                    {error.message}
                </div>
            ))}
        </div>
    );
}

/** Deploy's outcome (§16.2, item 8): the version deployed with Roll back, or why not. */
export function DeployNotice({
    action,
    outcome,
    onRollback,
    onClose,
}: {
    action: "deploy" | "rollback";
    outcome: DeployOutcome;
    onRollback: () => void;
    onClose: () => void;
}) {
    const heading = outcome.ok
        ? action === "deploy"
            ? `Deployed version ${outcome.version} to ${outcome.target}.`
            : `Rolled back to version ${outcome.version} on ${outcome.target}.`
        : `${action === "deploy" ? "Not deployed" : "Not rolled back"}: ${outcome.reason}`;
    // A rollback changes the runtime's bundle; the files in the editor stay as they are.
    const detail = outcome.ok && action === "rollback" ? "The HMI files of this project are unchanged." : undefined;
    return (
        <div
            data-part="deploy-notice"
            data-ok={outcome.ok ? "true" : "false"}
            className={`hb-outcome ${outcome.ok ? "hb-outcome-ok" : "hb-outcome-failed"}`}
            role="status"
        >
            <span className="hb-outcome-mark" aria-hidden="true">
                {outcome.ok ? "\u2713" : "!"}
            </span>
            <div className="hb-outcome-text">
                <strong>{heading}</strong>
                {detail ? <div className="hb-outcome-detail">{detail}</div> : null}
                {!outcome.ok
                    ? (outcome.problems ?? []).map((problem, index) => (
                          <div key={index} className="hb-banner-line">
                              {problem.file ? `${problem.file}${problem.line ? `:${problem.line}` : ""}: ` : ""}
                              {problem.message}
                          </div>
                      ))
                    : null}
            </div>
            <div className="hb-outcome-actions">
                {outcome.ok && action === "deploy" ? (
                    <button type="button" data-action="rollback" onClick={onRollback}>
                        Roll back
                    </button>
                ) : null}
                <button type="button" className="hb-secondary" data-action="close-notice" onClick={onClose}>
                    Close
                </button>
            </div>
        </div>
    );
}
