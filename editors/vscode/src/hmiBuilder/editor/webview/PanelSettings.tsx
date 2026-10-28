import type { HmiPageModel, PanelPlacement } from "../../drawing/model";

export function PanelSettings({
    page,
    pages,
    readOnly,
    onCommit,
}: {
    page: HmiPageModel;
    pages: string[];
    readOnly: boolean;
    onCommit: (page: HmiPageModel) => void;
}) {
    const panel = page.panel;
    if (!panel) return null;
    const change = (values: Partial<PanelPlacement>): void => onCommit({ ...page, panel: { ...panel, ...values } });
    return (
        <section className="hb-section" data-part="panel-assignment">
            <h2>Show on pages</h2>
            <p className="hb-hint">This is one shared panel. Changes appear everywhere it is used.</p>
            <label className="hb-field">
                <span>Placement</span>
                <select
                    name="panel-scope"
                    disabled={readOnly}
                    value={panel.scope}
                    onChange={(event) =>
                        change({
                            scope: event.target.value as PanelPlacement["scope"],
                            pages: event.target.value === "selected" ? panel.pages : [],
                        })
                    }
                >
                    <option value="manual">Place or open when needed</option>
                    <option value="all">All pages</option>
                    <option value="selected">Selected pages</option>
                </select>
            </label>
            {panel.scope === "selected" ? (
                <div className="hb-panel-pages">
                    {pages.map((name) => (
                        <label className="hb-check" key={name}>
                            <input
                                type="checkbox"
                                name={`panel-page-${name}`}
                                checked={panel.pages.includes(name)}
                                disabled={readOnly}
                                onChange={(event) =>
                                    change({
                                        pages: event.target.checked
                                            ? [...panel.pages, name]
                                            : panel.pages.filter((page) => page !== name),
                                    })
                                }
                            />
                            <span>{name}</span>
                        </label>
                    ))}
                    {pages.length === 0 ? (
                        <p className="hb-hint">Create a page in Explorer to assign it here.</p>
                    ) : null}
                </div>
            ) : null}
            {panel.scope !== "manual" ? (
                <label className="hb-field">
                    <span>Position</span>
                    <select
                        name="panel-dock"
                        disabled={readOnly}
                        value={panel.dock}
                        onChange={(event) => change({ dock: event.target.value as PanelPlacement["dock"] })}
                    >
                        <option value="left">Left</option>
                        <option value="right">Right</option>
                        <option value="top">Top</option>
                        <option value="bottom">Bottom</option>
                    </select>
                </label>
            ) : null}
        </section>
    );
}
